#![cfg_attr(
    not(any(
        feature = "native-service",
        feature = "native-file-dialog",
        target_arch = "wasm32"
    )),
    allow(dead_code)
)]

use std::sync::{Arc, Mutex};

use bus::console_mmio::ConsoleSnapshot;
use bus::display_mmio::DisplaySnapshot;
use bus::interrupts::InterruptController;
use bus::sprite_mmio::SpriteSnapshot;
use bus::{adapters::input::InputBackend, RasterIrqState, VideoState};
use core6502::RunOutcome;

use crate::cpu_worker::{
    CpuRunReply, CpuRunStatus, CpuWorker, CpuWorkerInit, CpuWorkerOutputs, PersonalitySelection,
};
use crate::video_backend::VideoOverlaySignals;
use crate::{
    InputSnapshot, ProgramSource, RtstMonitorConfig, ServiceCommand, ServiceResponseMessage,
    StartupConfig,
};

pub(crate) struct EmulatorState {
    cpu: CpuWorker,
    outputs: CpuWorkerOutputs,
    default_max_cycles: u64,
    status_message: Option<String>,
    last_outcome: Option<RunOutcome>,
    interrupts: Arc<InterruptController>,
    raster_irq: Arc<RasterIrqState>,
}

impl EmulatorState {
    pub(crate) fn new(
        startup: Option<StartupConfig>,
        default_max_cycles: u64,
        personality: PersonalitySelection,
    ) -> Result<Self, String> {
        let (
            cpu,
            CpuWorkerInit {
                outputs,
                status,
                outcome,
            },
        ) = CpuWorker::spawn(personality, startup)?;

        let interrupts = outputs.interrupts.clone();
        let raster_irq = outputs.raster.clone();

        Ok(Self {
            cpu,
            outputs,
            default_max_cycles,
            status_message: status,
            last_outcome: outcome,
            interrupts,
            raster_irq,
        })
    }

    pub(crate) fn set_status_message(&mut self, message: String) {
        self.status_message = Some(message);
    }

    pub(crate) fn log_console(&self, line: &str) {
        if let Some(handle) = self.outputs.console.as_ref() {
            if let Ok(mut console) = handle.lock() {
                console.write_str(line, 1, 0);
                console.newline();
            }
        }
    }

    pub(crate) fn status_message(&self) -> Option<String> {
        self.status_message.clone()
    }

    #[allow(dead_code)]
    pub(crate) fn last_outcome(&self) -> Option<RunOutcome> {
        self.last_outcome
    }

    pub(crate) fn interrupts(&self) -> Arc<InterruptController> {
        self.interrupts.clone()
    }

    pub(crate) fn raster_state(&self) -> Arc<RasterIrqState> {
        self.raster_irq.clone()
    }

    pub(crate) fn run_program(
        &mut self,
        source: ProgramSource,
        max_cycles: Option<u64>,
        start: Option<u16>,
        rtst: Option<RtstMonitorConfig>,
        progress_timeout_ms: Option<u64>,
    ) -> Result<String, String> {
        let configured_cycles = max_cycles.unwrap_or(self.default_max_cycles);
        let config = StartupConfig {
            source,
            max_cycles: configured_cycles,
            start,
            rtst,
            progress_timeout_ms,
        };
        match self.cpu.run_program(config) {
            Ok(CpuRunReply {
                summary,
                status,
                outputs,
                outcome,
            }) => {
                self.outputs = outputs;
                self.interrupts = self.outputs.interrupts.clone();
                self.raster_irq = self.outputs.raster.clone();
                self.status_message = Some(summary.clone());
                self.last_outcome = outcome;
                match status {
                    CpuRunStatus::Success => Ok(summary),
                    CpuRunStatus::Failure => Err(summary),
                }
            }
            Err(err) => {
                self.log_console(&err);
                self.status_message = Some(err.clone());
                self.last_outcome = None;
                Err(err)
            }
        }
    }

    pub(crate) fn input_backend(&self) -> Option<Arc<dyn InputBackend>> {
        self.outputs.input_backend.clone()
    }

    pub(crate) fn input_snapshot(&self) -> Option<InputSnapshot> {
        self.outputs
            .input
            .as_ref()
            .and_then(|handle| handle.lock().ok().map(|output| output.snapshot()))
    }

    pub(crate) fn snapshot(&self) -> Option<ConsoleSnapshot> {
        self.outputs
            .console
            .as_ref()
            .and_then(|handle| handle.lock().ok().map(|output| output.snapshot()))
    }

    pub(crate) fn display_snapshot(&self) -> Option<DisplaySnapshot> {
        self.outputs
            .display
            .as_ref()
            .and_then(|handle| handle.lock().ok().map(|output| output.snapshot()))
    }

    pub(crate) fn sprite_snapshot(&self) -> Option<SpriteSnapshot> {
        self.outputs
            .sprite
            .as_ref()
            .and_then(|handle| handle.lock().ok().map(|output| output.snapshot()))
    }

    pub(crate) fn video_state(&self) -> Arc<Mutex<VideoState>> {
        self.outputs.video.clone()
    }

    pub(crate) fn video_overlay(&self) -> Arc<VideoOverlaySignals> {
        self.outputs.video_overlay.clone()
    }

    pub(crate) fn handle_service_command(
        &mut self,
        command: ServiceCommand,
    ) -> ServiceResponseMessage {
        match command {
            ServiceCommand::RunProgram {
                source,
                max_cycles,
                start,
                rtst,
                progress_timeout_ms,
            } => {
                let timeout = progress_timeout_ms.filter(|ms| *ms > 0);
                match self.run_program(source, max_cycles, start, rtst, timeout) {
                    Ok(msg) => {
                        let mut resp = ServiceResponseMessage::ok(msg);
                        resp.cycles = self.last_outcome.map(|outcome| outcome.cycles);
                        resp
                    }
                    Err(err) => ServiceResponseMessage::error(err),
                }
            }
            ServiceCommand::ReadMemory { address, length } => {
                match self.cpu.read_memory(address, length as usize) {
                    Ok(bytes) => ServiceResponseMessage::ok_with_data(
                        format!("read {length} bytes from {address:#06X}"),
                        &bytes,
                    ),
                    Err(err) => ServiceResponseMessage::error(err),
                }
            }
        }
    }
}
