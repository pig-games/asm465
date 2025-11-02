//! Dedicated CPU runner for the asm465 viewer.
//!
//! Native builds keep the 6502 core on a background thread so the Bevy/egui UI
//! can drive rendering and host events without blocking instruction execution.
//! The worker exposes a simple command channel for program loads and exposes
//! shared MMIO snapshots back to the UI. On wasm we fall back to a synchronous
//! runner (threads are not available) but keep the same API surface so the rest
//! of the app does not need conditional code.

use crate::video_backend::{ModernVideoBackend, VideoOverlaySignals};
use crate::{
    run_program_with_config, write_console_line, ProgramRunReport, StartupConfig, WELCOME_MESSAGE,
};
use bus::console_mmio::ConsoleOutput;
use bus::display_mmio::DisplayOutput;
use bus::input_mmio::InputOutput;
use bus::interrupts::InterruptController;
use bus::mmio::ModuleKind;
use bus::personality::{self, Personality};
use bus::personality_v2;
use bus::sprite_mmio::SpriteOutput;
use bus::{
    AdapterError, Bus, DisplayAdapter, DisplayBackend, DisplayOutputBackend, InputAdapter,
    InputBackend, InputBackendHandle, SpriteAdapter, SpriteBackend, SpriteOutputBackend,
    VideoAdapter, VideoBackend, VideoState,
};
use core6502::RunOutcome;
use log::warn;
#[cfg(not(target_arch = "wasm32"))]
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone)]
struct AdapterHandles {
    video_state: Arc<Mutex<VideoState>>,
    video_overlay: Arc<VideoOverlaySignals>,
    input_backend: Option<Arc<dyn InputBackend>>,
}

fn attach_default_adapters(bus: &mut Bus) -> AdapterHandles {
    if let Some(display_handle) = bus.display_output_handle() {
        let backend: Arc<dyn DisplayBackend> =
            Arc::new(DisplayOutputBackend::new(display_handle.clone()));
        if let Err(err) =
            bus.attach_adapter(ModuleKind::Display, Box::new(DisplayAdapter::new(backend)))
        {
            match err {
                AdapterError::ModuleNotMapped(_) | AdapterError::LegacyPersonality => {}
                _ => warn!("failed to attach display adapter: {err}"),
            }
        }
    }

    if let Some(sprite_handle) = bus.sprite_output_handle() {
        let backend: Arc<dyn SpriteBackend> =
            Arc::new(SpriteOutputBackend::new(sprite_handle.clone()));
        if let Err(err) =
            bus.attach_adapter(ModuleKind::Sprite, Box::new(SpriteAdapter::new(backend)))
        {
            match err {
                AdapterError::ModuleNotMapped(_) | AdapterError::LegacyPersonality => {}
                _ => warn!("failed to attach sprite adapter: {err}"),
            }
        }
    }

    let input_backend: Option<Arc<dyn InputBackend>> = match bus.input_output_handle() {
        Some(handle) => {
            let backend: Arc<dyn InputBackend> = Arc::new(InputBackendHandle::new(handle.clone()));
            if let Err(err) = bus.attach_adapter(
                ModuleKind::Input,
                Box::new(InputAdapter::new(backend.clone())),
            ) {
                match err {
                    AdapterError::ModuleNotMapped(_) | AdapterError::LegacyPersonality => {}
                    _ => warn!("failed to attach input adapter: {err}"),
                }
            }
            Some(backend)
        }
        None => None,
    };

    let video_state = Arc::new(Mutex::new(VideoState::default()));
    let video_overlay = Arc::new(VideoOverlaySignals::new());
    let video_backend: Arc<dyn VideoBackend> = Arc::new(ModernVideoBackend::new(
        video_state.clone(),
        video_overlay.clone(),
    ));
    if let Err(err) = bus.attach_adapter(
        ModuleKind::System,
        Box::new(VideoAdapter::new(video_backend)),
    ) {
        match err {
            AdapterError::ModuleNotMapped(_) | AdapterError::LegacyPersonality => {}
            _ => warn!("failed to attach video adapter: {err}"),
        }
    }

    AdapterHandles {
        video_state,
        video_overlay,
        input_backend,
    }
}

/// Handles to the shared MMIO output buffers that the viewer reads from.
#[derive(Clone)]
pub struct CpuWorkerOutputs {
    pub console: Arc<Mutex<ConsoleOutput>>,
    pub display: Arc<Mutex<DisplayOutput>>,
    pub sprite: Arc<Mutex<SpriteOutput>>,
    pub input: Option<Arc<Mutex<InputOutput>>>,
    pub input_backend: Option<Arc<dyn InputBackend>>,
    pub interrupts: Arc<InterruptController>,
    pub video: Arc<Mutex<VideoState>>,
    pub video_overlay: Arc<VideoOverlaySignals>,
}

impl CpuWorkerOutputs {
    /// Snapshot the console/display/sprite handles from the supplied bus.
    fn new(bus: &Bus, adapters: &AdapterHandles) -> Self {
        let console = bus
            .console_output_handle()
            .expect("console MMIO output handle");
        let display = bus
            .display_output_handle()
            .expect("display MMIO output handle");
        let sprite = bus
            .sprite_output_handle()
            .expect("sprite MMIO output handle");
        let input = bus.input_output_handle();
        let interrupts = bus.interrupt_controller();
        Self {
            console,
            display,
            sprite,
            input,
            input_backend: adapters.input_backend.clone(),
            interrupts,
            video: adapters.video_state.clone(),
            video_overlay: adapters.video_overlay.clone(),
        }
    }
}

/// Outcome of a [`CpuWorker`] command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CpuRunStatus {
    Success,
    Failure,
}

/// Reply returned after the worker loads and executes a program batch.
pub struct CpuRunReply {
    pub summary: String,
    pub status: CpuRunStatus,
    pub outputs: CpuWorkerOutputs,
    pub outcome: Option<RunOutcome>,
}

/// Initialisation payload returned when the worker spins up.
pub struct CpuWorkerInit {
    pub outputs: CpuWorkerOutputs,
    pub status: Option<String>,
    pub outcome: Option<RunOutcome>,
}

#[derive(Clone, Copy)]
pub struct CpuThrottle {
    pub cycles_per_batch: u64,
    pub sleep: Duration,
}

impl CpuThrottle {
    /// Helper used by tests/debug tools that need an unthrottled worker.
    #[allow(dead_code)]
    pub const fn unlimited() -> Self {
        Self {
            cycles_per_batch: u64::MAX,
            sleep: Duration::from_micros(0),
        }
    }
}

impl Default for CpuThrottle {
    fn default() -> Self {
        Self {
            cycles_per_batch: 50_000,
            sleep: Duration::from_millis(1),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use super::*;
    use core6502::Cpu;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
    use std::thread::{self, JoinHandle};

    /// Native worker that keeps the CPU on a background thread.
    pub struct CpuWorker {
        command_tx: Sender<CpuCommand>,
        #[allow(dead_code)]
        status: Arc<CpuWorkerStatus>,
        handle: Option<JoinHandle<()>>,
    }

    #[derive(Default)]
    struct CpuWorkerStatus {
        running: AtomicBool,
        paused: AtomicBool,
    }

    /// Control messages sent to the worker thread.
    #[allow(dead_code)]
    enum CpuCommand {
        RunProgram {
            config: StartupConfig,
            respond_to: Sender<Result<CpuRunReply, String>>,
        },
        Pause,
        Resume,
        SetThrottle(CpuThrottle),
        Shutdown,
    }

    /// Owned state that lives on the worker thread.
    struct WorkerInner {
        cpu: Cpu,
        throttle: CpuThrottle,
        running: bool,
        paused: bool,
        personality: PersonalitySelection,
        video_state: Arc<Mutex<VideoState>>,
        video_overlay: Arc<VideoOverlaySignals>,
        status: Arc<CpuWorkerStatus>,
    }

    impl WorkerInner {
        /// Build the worker state and gather initial MMIO handles.
        fn new(
            personality: PersonalitySelection,
            startup: Option<StartupConfig>,
            status: Arc<CpuWorkerStatus>,
        ) -> Result<(Self, CpuWorkerInit), String> {
            let (cpu, outputs, initial_status, initial_outcome) =
                initialize_cpu(&personality, startup)?;
            status.running.store(true, Ordering::SeqCst);
            status.paused.store(false, Ordering::SeqCst);
            let inner = Self {
                cpu,
                throttle: CpuThrottle::default(),
                running: true,
                paused: false,
                personality,
                video_state: outputs.video.clone(),
                video_overlay: outputs.video_overlay.clone(),
                status: status.clone(),
            };
            let init = CpuWorkerInit {
                outputs,
                status: initial_status,
                outcome: initial_outcome,
            };
            Ok((inner, init))
        }

        /// Main worker loop: polls commands, runs the CPU, and respects throttle settings.
        fn run(mut self, command_rx: Receiver<CpuCommand>) {
            while self.running {
                if self.paused {
                    match command_rx.recv() {
                        Ok(cmd) => self.handle_command(cmd),
                        Err(_) => break,
                    }
                    continue;
                }

                match command_rx.try_recv() {
                    Ok(cmd) => {
                        self.handle_command(cmd);
                        continue;
                    }
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Disconnected) => break,
                }

                let mut spent = 0u64;
                while spent < self.throttle.cycles_per_batch && self.running && !self.paused {
                    let cycles = self.cpu.step() as u64;
                    spent += cycles;
                    match command_rx.try_recv() {
                        Ok(cmd) => {
                            self.handle_command(cmd);
                        }
                        Err(TryRecvError::Empty) => {}
                        Err(TryRecvError::Disconnected) => {
                            self.running = false;
                        }
                    }
                }

                if self.throttle.sleep > Duration::ZERO {
                    thread::sleep(self.throttle.sleep);
                }
            }
            self.status.running.store(false, Ordering::SeqCst);
            self.status.paused.store(true, Ordering::SeqCst);
        }

        /// Dispatch a command received from the control channel.
        fn handle_command(&mut self, command: CpuCommand) {
            match command {
                CpuCommand::RunProgram { config, respond_to } => {
                    self.paused = true;
                    self.status.paused.store(true, Ordering::SeqCst);
                    let reply = self.perform_run_program(config);
                    let _ = respond_to.send(reply);
                    if self.running {
                        self.paused = false;
                        self.status.paused.store(false, Ordering::SeqCst);
                    }
                }
                CpuCommand::Pause => {
                    self.paused = true;
                    self.status.paused.store(true, Ordering::SeqCst);
                }
                CpuCommand::Resume => {
                    if self.running {
                        self.paused = false;
                        self.status.paused.store(false, Ordering::SeqCst);
                    }
                }
                CpuCommand::SetThrottle(throttle) => {
                    self.throttle = throttle;
                }
                CpuCommand::Shutdown => {
                    self.running = false;
                    self.paused = true;
                    self.status.running.store(false, Ordering::SeqCst);
                    self.status.paused.store(true, Ordering::SeqCst);
                }
            }
        }

        /// Execute a program request and keep the worker state coherent.
        fn perform_run_program(&mut self, config: StartupConfig) -> Result<CpuRunReply, String> {
            let (bus, adapters) = self.personality.build_bus()?;
            match run_program_with_config(bus, &config) {
                Ok((bus, report)) => {
                    Ok(self.finish_program(bus, report, CpuRunStatus::Success, adapters.clone()))
                }
                Err((bus, report)) => {
                    Ok(self.finish_program(bus, report, CpuRunStatus::Failure, adapters))
                }
            }
        }

        /// Reset the CPU core back to the worker loop and provide refreshed outputs.
        fn finish_program(
            &mut self,
            bus: Bus,
            report: ProgramRunReport,
            status: CpuRunStatus,
            adapters: AdapterHandles,
        ) -> CpuRunReply {
            let ProgramRunReport { outcome, message } = report;
            let mut bus = bus;
            if status == CpuRunStatus::Failure {
                write_console_line(&mut bus, &message);
            }
            let outputs = CpuWorkerOutputs::new(&bus, &adapters);
            self.cpu = Cpu::new(bus);
            self.cpu.reset();
            self.video_state = adapters.video_state.clone();
            self.video_overlay = adapters.video_overlay.clone();
            CpuRunReply {
                summary: message,
                status,
                outputs,
                outcome,
            }
        }
    }

    /// Helper that prepares the initial CPU/bus state for the worker thread.
    fn initialize_cpu(
        personality: &PersonalitySelection,
        startup: Option<StartupConfig>,
    ) -> Result<(Cpu, CpuWorkerOutputs, Option<String>, Option<RunOutcome>), String> {
        match startup {
            Some(config) => {
                let (bus, adapters) = personality.build_bus()?;
                match run_program_with_config(bus, &config) {
                    Ok((bus, report)) => {
                        let ProgramRunReport { outcome, message } = report;
                        let outputs = CpuWorkerOutputs::new(&bus, &adapters);
                        let mut cpu = Cpu::new(bus);
                        cpu.reset();
                        Ok((cpu, outputs, Some(message), outcome))
                    }
                    Err((mut bus, report)) => {
                        let ProgramRunReport { outcome, message } = report;
                        write_console_line(&mut bus, &message);
                        let outputs = CpuWorkerOutputs::new(&bus, &adapters);
                        let mut cpu = Cpu::new(bus);
                        cpu.reset();
                        Ok((cpu, outputs, Some(message), outcome))
                    }
                }
            }
            None => {
                let (mut bus, adapters) = personality.build_bus()?;
                write_console_line(&mut bus, WELCOME_MESSAGE);
                let outputs = CpuWorkerOutputs::new(&bus, &adapters);
                let mut cpu = Cpu::new(bus);
                cpu.reset();
                Ok((cpu, outputs, Some(WELCOME_MESSAGE.to_string()), None))
            }
        }
    }

    impl CpuWorker {
        /// Spawn the worker thread and return the handles the UI needs for rendering.
        pub fn spawn(
            personality: PersonalitySelection,
            startup: Option<StartupConfig>,
        ) -> Result<(Self, CpuWorkerInit), String> {
            let (command_tx, command_rx) = mpsc::channel();
            let status = Arc::new(CpuWorkerStatus::default());
            let (inner, init) = WorkerInner::new(personality, startup, status.clone())?;
            let handle = thread::Builder::new()
                .name("cpu-worker".into())
                .spawn(move || inner.run(command_rx))
                .map_err(|err| err.to_string())?;

            Ok((
                Self {
                    command_tx,
                    status,
                    handle: Some(handle),
                },
                init,
            ))
        }

        /// Request the worker to load a program and return once it finishes.
        pub fn run_program(&mut self, config: StartupConfig) -> Result<CpuRunReply, String> {
            let (tx, rx) = mpsc::channel();
            self.command_tx
                .send(CpuCommand::RunProgram {
                    config,
                    respond_to: tx,
                })
                .map_err(|err| err.to_string())?;
            rx.recv().map_err(|err| err.to_string())?
        }

        /// Pause the worker loop (no-op if it is already paused).
        #[allow(dead_code)]
        pub fn pause(&mut self) {
            let _ = self.command_tx.send(CpuCommand::Pause);
        }

        /// Resume the worker loop if it was paused.
        #[allow(dead_code)]
        pub fn resume(&mut self) {
            let _ = self.command_tx.send(CpuCommand::Resume);
        }

        /// Adjust the worker throttle (cycles per batch + host sleep).
        #[allow(dead_code)]
        pub fn set_throttle(&mut self, throttle: CpuThrottle) {
            let _ = self.command_tx.send(CpuCommand::SetThrottle(throttle));
        }

        /// Report whether the worker thread is still alive.
        #[allow(dead_code)]
        pub fn is_running(&self) -> bool {
            self.status.running.load(Ordering::SeqCst)
        }

        /// Report whether the worker loop is currently paused.
        #[allow(dead_code)]
        pub fn is_paused(&self) -> bool {
            self.status.paused.load(Ordering::SeqCst)
        }

        pub fn shutdown(&mut self) {
            let _ = self.command_tx.send(CpuCommand::Shutdown);
            if let Some(handle) = self.handle.take() {
                let _ = handle.join();
            }
        }
    }

    impl Drop for CpuWorker {
        fn drop(&mut self) {
            self.shutdown();
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod wasm {
    use super::*;
    use std::thread;

    /// Synchronous worker used in wasm builds (threads are unavailable).
    pub struct CpuWorker {
        bus: Bus,
        personality: PersonalitySelection,
        #[allow(dead_code)]
        video_state: Arc<Mutex<VideoState>>,
        video_overlay: Arc<VideoOverlaySignals>,
    }

    impl CpuWorker {
        /// Create the worker and return the initial MMIO handles.
        pub fn spawn(
            personality: PersonalitySelection,
            startup: Option<StartupConfig>,
        ) -> Result<(Self, CpuWorkerInit), String> {
            let (bus, outputs, status, outcome) = initialize_bus(&personality, startup)?;
            let video_state = outputs.video.clone();
            let video_overlay = outputs.video_overlay.clone();
            Ok((
                Self {
                    bus,
                    personality,
                    video_state,
                    video_overlay,
                },
                CpuWorkerInit {
                    outputs,
                    status,
                    outcome,
                },
            ))
        }

        /// Run the supplied program immediately on the single-threaded executor.
        pub fn run_program(&mut self, config: StartupConfig) -> Result<CpuRunReply, String> {
            let (bus, adapters) = self.personality.build_bus()?;
            match run_program_with_config(bus, &config) {
                Ok((bus, report)) => {
                    let ProgramRunReport { outcome, message } = report;
                    let outputs = CpuWorkerOutputs::new(&bus, &adapters);
                    self.bus = bus;
                    self.video_state = adapters.video_state.clone();
                    self.video_overlay = adapters.video_overlay.clone();
                    Ok(CpuRunReply {
                        summary: message,
                        status: CpuRunStatus::Success,
                        outputs,
                        outcome,
                    })
                }
                Err((mut bus, report)) => {
                    let ProgramRunReport { outcome, message } = report;
                    write_console_line(&mut bus, &message);
                    let outputs = CpuWorkerOutputs::new(&bus, &adapters);
                    self.bus = bus;
                    self.video_state = adapters.video_state.clone();
                    self.video_overlay = adapters.video_overlay.clone();
                    Ok(CpuRunReply {
                        summary: message,
                        status: CpuRunStatus::Failure,
                        outputs,
                        outcome,
                    })
                }
            }
        }

        /// Hint to the scheduler (no-op placeholder for API parity).
        pub fn pause(&mut self) {
            let _ = thread::yield_now();
        }

        /// Resume execution (no-op in wasm).
        pub fn resume(&mut self) {}

        /// Update throttle settings (ignored in wasm).
        pub fn set_throttle(&mut self, _throttle: CpuThrottle) {}

        /// Report that the interpreter is always running (wasm single thread).
        pub fn is_running(&self) -> bool {
            true
        }

        /// Wasm runner never pauses (no real worker loop).
        pub fn is_paused(&self) -> bool {
            false
        }

        /// Drop hook kept for API symmetry with the native worker.
        pub fn shutdown(&mut self) {}
    }

    impl Drop for CpuWorker {
        fn drop(&mut self) {}
    }

    /// Helper mirroring [`initialize_cpu`] for the single-threaded wasm runner.
    fn initialize_bus(
        personality: &PersonalitySelection,
        startup: Option<StartupConfig>,
    ) -> Result<(Bus, CpuWorkerOutputs, Option<String>, Option<RunOutcome>), String> {
        match startup {
            Some(config) => {
                let (bus, adapters) = personality.build_bus()?;
                match run_program_with_config(bus, &config) {
                    Ok((bus, report)) => {
                        let ProgramRunReport { outcome, message } = report;
                        let outputs = CpuWorkerOutputs::new(&bus, &adapters);
                        Ok((bus, outputs, Some(message), outcome))
                    }
                    Err((mut bus, report)) => {
                        let ProgramRunReport { outcome, message } = report;
                        write_console_line(&mut bus, &message);
                        let outputs = CpuWorkerOutputs::new(&bus, &adapters);
                        Ok((bus, outputs, Some(message), outcome))
                    }
                }
            }
            None => {
                let (mut bus, adapters) = personality.build_bus()?;
                write_console_line(&mut bus, WELCOME_MESSAGE);
                let outputs = CpuWorkerOutputs::new(&bus, &adapters);
                Ok((bus, outputs, Some(WELCOME_MESSAGE.to_string()), None))
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use native::CpuWorker;
#[cfg(target_arch = "wasm32")]
pub use wasm::CpuWorker;
#[derive(Clone)]
pub enum PersonalitySelection {
    Legacy(&'static Personality),
    Toml {
        path: PathBuf,
        legacy: Option<&'static Personality>,
    },
}

impl PersonalitySelection {
    pub fn legacy_default() -> Self {
        PersonalitySelection::Legacy(personality::default())
    }

    pub fn from_path(path: PathBuf) -> Self {
        PersonalitySelection::Toml { path, legacy: None }
    }

    pub fn with_legacy(path: PathBuf, legacy: &'static Personality) -> Self {
        PersonalitySelection::Toml {
            path,
            legacy: Some(legacy),
        }
    }

    pub fn legacy_personality(&self) -> Option<&'static Personality> {
        match self {
            PersonalitySelection::Legacy(p) => Some(*p),
            PersonalitySelection::Toml { legacy, .. } => *legacy,
        }
    }

    fn build_bus(&self) -> Result<(Bus, AdapterHandles), String> {
        match self {
            PersonalitySelection::Legacy(p) => {
                let mut bus = Bus::with_personality(p);
                let adapters = attach_default_adapters(&mut bus);
                Ok((bus, adapters))
            }
            PersonalitySelection::Toml { path, .. } => {
                #[cfg(target_arch = "wasm32")]
                {
                    return Err("TOML personalities are not supported on wasm builds".into());
                }
                #[cfg(not(target_arch = "wasm32"))]
                {
                    let toml = fs::read_to_string(path)
                        .map_err(|err| format!("Failed to read {}: {err}", path.display()))?;
                    let registry = bus::builtin_module_registry();
                    let def = personality_v2::PersonalityDef::from_toml_str(&toml, &registry)
                        .map_err(|err| err.to_string())?;
                    let mut bus = Bus::from_personality_def(def).map_err(|err| err.to_string())?;
                    let adapters = attach_default_adapters(&mut bus);
                    Ok((bus, adapters))
                }
            }
        }
    }
}
