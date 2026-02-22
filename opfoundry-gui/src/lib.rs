//! Bevy/egui front-end for the opFoundry cross-development tooling.
//!
//! This crate hosts the “desktop” viewer: it embeds the 6502 core, connects to
//! the cross465 [bus] crate, renders the screen/console, and exposes file &
//! service APIs for loading programs at runtime.  The same crate also backs the
//! wasm build (via [`web::start_web_app`]), so as much logic as possible lives
//! in platform-neutral modules.

#![allow(clippy::items_after_test_module)]

#[cfg(feature = "native-service")]
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bevy::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use bevy::window::WindowResolution;
use bevy_egui::EguiPlugin;
use bus::console_mmio::ConsoleSnapshot;
use bus::display_mmio::DisplaySnapshot;
use bus::input_mmio::InputSnapshot;
use bus::interrupts::InterruptController;
use bus::sprite_mmio::SpriteSnapshot;
use bus::{adapters::input::InputBackend, RasterIrqState, VideoState};
use core6502::RunOutcome;
use video_backend::VideoOverlaySignals;

mod console_ui;
mod cpu_worker;
mod display;
mod input;
mod interrupts;
#[cfg(feature = "native-service")]
mod personality_cli;
mod program_runner;
mod service;
#[cfg(feature = "native-service")]
mod service_listener;
mod ui;
mod video_backend;
mod web_url;

pub(crate) use program_runner::{run_program_with_config, write_console_line};
pub use service::*;

use self::console_ui::console_layout_job;
#[cfg(test)]
use self::display::{compute_viewport_geometry, sprite_virtual_size};
use self::display::{
    drive_raster_counter, setup_scene, sprite_world_transform, update_sprite_viewport,
    update_video_overlay_line, DisplayPalette, RasterDriver, SpriteCatalog, SpriteSlot,
    SpriteViewport, SpriteVirtualResolution, VideoOverlayConfig, SPRITE_DEFAULT_MARGIN_X,
    SPRITE_DEFAULT_MARGIN_Y, SPRITE_VIRTUAL_HEIGHT, SPRITE_VIRTUAL_WIDTH,
};
use self::input::{
    controller_input_system, sync_controller_backend, update_keyboard_tracker, ControllerState,
    KeyboardTracker,
};
use self::interrupts::{
    emit_frame_end_interrupt, emit_frame_start_interrupt, gamepad_interrupt_system,
    keyboard_interrupt_system, timer_interrupt_system, InterruptBindings, TimerInterruptState,
};
use self::ui::{ui_system, UiState};
#[cfg(target_arch = "wasm32")]
use self::web::WebSocketBridgeManager;
#[cfg(test)]
use bus::sprite_mmio::SpriteState;
use cpu_worker::{
    CpuRunReply, CpuRunStatus, CpuWorker, CpuWorkerInit, CpuWorkerOutputs, PersonalitySelection,
};

#[cfg(feature = "native-service")]
use self::personality_cli::{
    dump_personality_maps, dump_personality_registers, print_module_list, print_personality_list,
    resolve_personality_selection,
};
#[cfg(feature = "native-service")]
use self::service_listener::{start_service_listener, ServiceListener};
#[cfg(feature = "native-service")]
use clap::{ArgAction, Parser};

pub(crate) const WELCOME_MESSAGE: &str = "Welcome to the opFoundry console viewer!";

#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(target_arch = "wasm32")]
pub use web::start_web_app;

#[cfg(feature = "native-service")]
#[derive(Parser, Debug)]
#[command(author, version, about = "opFoundry console viewer", long_about = None)]
pub struct Args {
    /// Optional 6502 PRG to execute before the window opens.
    #[arg(long)]
    pub prg: Option<PathBuf>,

    /// Maximum number of CPU cycles to run the startup program for.
    #[arg(long, default_value_t = 5_000_000u64)]
    pub max_cycles: u64,

    /// Optional address to jump to instead of the PRG's load address.
    #[arg(long)]
    pub start: Option<u16>,

    /// Optional TCP port to expose the JSON service API on.
    #[arg(long)]
    pub service_port: Option<u16>,

    /// Host/interface to bind the JSON service API on.
    #[arg(long, default_value = "127.0.0.1")]
    pub service_host: String,

    /// Virtual sprite canvas width used for MMIO coordinate scaling.
    #[arg(long, default_value_t = 320u32)]
    pub virtual_width: u32,

    /// Virtual sprite canvas height used for MMIO coordinate scaling.
    #[arg(long, default_value_t = 240u32)]
    pub virtual_height: u32,

    /// Enforce the virtual aspect ratio onto the output window.
    #[arg(long, default_value_t = true)]
    pub force_aspect_ratio: bool,

    /// Minimum horizontal border in pixels when enforcing aspect ratio.
    #[arg(long, default_value_t = 50.0)]
    pub min_border_x: f32,

    /// Minimum vertical border in pixels when enforcing aspect ratio.
    #[arg(long, default_value_t = 50.0)]
    pub min_border_y: f32,

    /// Border colour (hex RGB, e.g. FF0000).
    #[arg(long, default_value = "404040", value_parser = parse_color)]
    pub border_color: Color,

    /// Background colour inside the content area (hex RGB).
    #[arg(long, default_value = "000000", value_parser = parse_color)]
    pub background_color: Color,

    /// Left margin (virtual units) applied to the sprite coordinate mapping.
    #[arg(long, default_value_t = SPRITE_DEFAULT_MARGIN_X)]
    pub sprite_margin_left: f32,

    /// Right margin (virtual units) applied to the sprite coordinate mapping.
    #[arg(long, default_value_t = SPRITE_DEFAULT_MARGIN_X)]
    pub sprite_margin_right: f32,

    /// Top margin (virtual units) applied to the sprite coordinate mapping.
    #[arg(long, default_value_t = SPRITE_DEFAULT_MARGIN_Y)]
    pub sprite_margin_top: f32,

    /// Bottom margin (virtual units) applied to the sprite coordinate mapping.
    #[arg(long, default_value_t = SPRITE_DEFAULT_MARGIN_Y)]
    pub sprite_margin_bottom: f32,

    /// Maximum MMIO value expected for sprite X coordinates.
    #[arg(long, default_value_t = 0.0_f32)]
    pub sprite_mmio_max_x: f32,

    /// Maximum MMIO value expected for sprite Y coordinates.
    #[arg(long, default_value_t = 0.0_f32)]
    pub sprite_mmio_max_y: f32,

    /// Maximum sprite width (virtual units) tolerated while off-screen.
    #[arg(long, default_value_t = SPRITE_VIRTUAL_WIDTH)]
    pub sprite_max_offscreen_width: f32,

    /// Maximum sprite height (virtual units) tolerated while off-screen.
    #[arg(long, default_value_t = SPRITE_VIRTUAL_HEIGHT)]
    pub sprite_max_offscreen_height: f32,

    /// Resize the window to match the enforced aspect ratio (native only).
    #[arg(long, default_value_t = true)]
    pub resize_window: bool,

    /// Enable the raster/collision overlay instrumentation.
    #[arg(long, action = ArgAction::SetTrue)]
    pub enable_video_overlay: bool,

    /// Optional positional PRG path (shorthand for `--prg`).
    #[arg(conflicts_with = "prg")]
    pub program: Option<PathBuf>,

    /// Personality to load (see `--list-personalities`).
    #[arg(long, default_value = "modern-retro")]
    pub personality: String,

    /// List available personalities and exit.
    #[arg(long, default_value_t = false)]
    pub list_personalities: bool,
    /// List available module implementations and exit.
    #[arg(long, default_value_t = false)]
    pub list_modules: bool,
    /// Dump map layout for a personality and exit.
    #[arg(
        long,
        value_name = "PERSONALITY",
        conflicts_with = "dump_map_registers"
    )]
    pub dump_maps: Option<String>,
    /// Dump resolved register mappings for a personality and exit.
    #[arg(long, value_name = "PERSONALITY", conflicts_with = "dump_maps")]
    pub dump_map_registers: Option<String>,
}

/// Fully-specified viewer configuration used when bootstrapping the Bevy app.
pub struct AppConfig {
    pub startup: Option<StartupConfig>,
    pub default_max_cycles: u64,
    pub virtual_resolution: VirtualResolution,
    pub display: DisplaySettings,
    pub personality: PersonalitySelection,
    /// When `true`, spawn the raster/collision overlay helpers in addition to the
    /// core emulator pipelines.
    pub video_overlay: bool,
    #[cfg(feature = "native-service")]
    pub service: Option<ServiceConfig>,
}

#[cfg(feature = "native-service")]
pub struct ServiceConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Clone, Copy)]
pub struct VirtualResolution {
    pub width: u32,
    pub height: u32,
}

impl VirtualResolution {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

impl Default for VirtualResolution {
    fn default() -> Self {
        Self {
            width: 320,
            height: 240,
        }
    }
}

#[derive(Clone, Resource)]
pub struct DisplaySettings {
    pub enforce_aspect_ratio: bool,
    pub min_border_x: f32,
    pub min_border_y: f32,
    pub border_color: Color,
    pub background_color: Color,
    pub resize_window_to_aspect: bool,
    /// Amount of virtual-space padding kept off-screen on the left edge.
    pub sprite_margin_left: f32,
    /// Amount of virtual-space padding kept off-screen on the right edge.
    pub sprite_margin_right: f32,
    /// Amount of virtual-space padding kept off-screen above the content.
    pub sprite_margin_top: f32,
    /// Amount of virtual-space padding kept off-screen below the content.
    pub sprite_margin_bottom: f32,
    /// Maximum raw MMIO value expected on the X axis (`0` defers to virtual size + margins).
    pub sprite_mmio_max_x: f32,
    /// Maximum raw MMIO value expected on the Y axis (`0` defers to virtual size + margins).
    pub sprite_mmio_max_y: f32,
    pub sprite_max_offscreen_width: f32,
    pub sprite_max_offscreen_height: f32,
}

impl Default for DisplaySettings {
    fn default() -> Self {
        Self {
            enforce_aspect_ratio: true,
            min_border_x: 50.0,
            min_border_y: 50.0,
            border_color: Color::rgb_u8(0x40, 0x40, 0x40),
            background_color: Color::BLACK,
            resize_window_to_aspect: true,
            sprite_margin_left: SPRITE_DEFAULT_MARGIN_X,
            sprite_margin_right: SPRITE_DEFAULT_MARGIN_X,
            sprite_margin_top: SPRITE_DEFAULT_MARGIN_Y,
            sprite_margin_bottom: SPRITE_DEFAULT_MARGIN_Y,
            sprite_mmio_max_x: 0.0,
            sprite_mmio_max_y: 0.0,
            sprite_max_offscreen_width: SPRITE_VIRTUAL_WIDTH,
            sprite_max_offscreen_height: SPRITE_VIRTUAL_HEIGHT,
        }
    }
}

#[cfg_attr(not(feature = "native-service"), allow(dead_code))]
fn parse_color(value: &str) -> Result<Color, String> {
    let value = value.trim();
    let value = value.trim_start_matches('#').trim_start_matches("0x");
    if value.len() != 6 {
        return Err("expected 6 hex digits (e.g. FFCC00)".into());
    }
    let r = u8::from_str_radix(&value[0..2], 16).map_err(|e| e.to_string())?;
    let g = u8::from_str_radix(&value[2..4], 16).map_err(|e| e.to_string())?;
    let b = u8::from_str_radix(&value[4..6], 16).map_err(|e| e.to_string())?;
    Ok(Color::rgb_u8(r, g, b))
}

#[cfg(feature = "native-service")]
pub fn run_native() -> Result<(), String> {
    let args = Args::parse();

    if args.list_modules {
        print_module_list();
        return Ok(());
    }

    if let Some(name) = args.dump_map_registers.as_deref() {
        dump_personality_registers(name)?;
        return Ok(());
    }

    if let Some(name) = args.dump_maps.as_deref() {
        dump_personality_maps(name)?;
        return Ok(());
    }

    if args.list_personalities {
        print_personality_list();
        return Ok(());
    }

    let personality_selection = resolve_personality_selection(&args.personality)?;
    let startup_path = args.prg.clone().or_else(|| args.program.clone());
    let startup = startup_path.map(|path| StartupConfig {
        source: ProgramSource::File(path),
        max_cycles: args.max_cycles,
        start: args.start,
        rtst: None,
        progress_timeout_ms: None,
    });

    let service = args.service_port.map(|port| ServiceConfig {
        host: args.service_host.clone(),
        port,
    });

    let display = DisplaySettings {
        enforce_aspect_ratio: args.force_aspect_ratio,
        min_border_x: args.min_border_x.max(0.0),
        min_border_y: args.min_border_y.max(0.0),
        border_color: args.border_color,
        background_color: args.background_color,
        resize_window_to_aspect: args.resize_window,
        sprite_margin_left: args.sprite_margin_left.max(0.0),
        sprite_margin_right: args.sprite_margin_right.max(0.0),
        sprite_margin_top: args.sprite_margin_top.max(0.0),
        sprite_margin_bottom: args.sprite_margin_bottom.max(0.0),
        sprite_mmio_max_x: if args.sprite_mmio_max_x <= 0.0 {
            0.0
        } else {
            args.sprite_mmio_max_x
        },
        sprite_mmio_max_y: if args.sprite_mmio_max_y <= 0.0 {
            0.0
        } else {
            args.sprite_mmio_max_y
        },
        sprite_max_offscreen_width: args.sprite_max_offscreen_width.max(0.0),
        sprite_max_offscreen_height: args.sprite_max_offscreen_height.max(0.0),
    };

    run_app(AppConfig {
        startup,
        default_max_cycles: args.max_cycles,
        virtual_resolution: VirtualResolution::new(args.virtual_width, args.virtual_height),
        display,
        personality: personality_selection,
        video_overlay: args.enable_video_overlay,
        #[cfg(feature = "native-service")]
        service,
    })?;

    Ok(())
}

/// Launches the Bevy runtime using the supplied configuration.
pub fn run_app(config: AppConfig) -> Result<(), String> {
    let AppConfig {
        startup,
        default_max_cycles,
        virtual_resolution,
        display,
        personality,
        video_overlay,
        #[cfg(feature = "native-service")]
        service,
    } = config;

    let legacy_persona = personality.legacy_personality();
    #[allow(unused_mut)]
    let mut emulator = EmulatorState::new(startup, default_max_cycles, personality.clone())?;
    let interrupt_bindings = legacy_persona
        .and_then(|legacy| InterruptBindings::from_personality(emulator.interrupts(), legacy));

    #[cfg(feature = "native-service")]
    let mut service_listener: Option<ServiceListener> = None;

    #[cfg(feature = "native-service")]
    if let Some(service_cfg) = service {
        match start_service_listener(&service_cfg.host, service_cfg.port) {
            Ok(receiver) => {
                service_listener = Some(ServiceListener { receiver });
            }
            Err(err) => {
                let msg = format!(
                    "Failed to start service listener on {}:{}: {err}",
                    service_cfg.host, service_cfg.port
                );
                emulator.status_message = Some(msg.clone());
                emulator.log_console(&msg);
            }
        }
    }

    let controller_state =
        ControllerState::new(emulator.input_backend(), emulator.input_snapshot());
    // Only mirror the raster into the UI overlay when the flag is enabled.
    let overlay_handle = if video_overlay {
        Some(emulator.video_overlay())
    } else {
        None
    };
    let raster_driver = RasterDriver::new(
        emulator.raster_state(),
        overlay_handle,
        emulator.interrupts(),
    );
    let keyboard_tracker = KeyboardTracker::default();

    let mut app = App::new();
    app.insert_resource(controller_state);
    app.insert_resource(raster_driver);
    app.insert_resource(keyboard_tracker);
    app.insert_non_send_resource(emulator);
    app.insert_resource(VideoOverlayConfig::new(video_overlay));
    if let Some(bindings) = interrupt_bindings {
        let has_timer = bindings.has_timer();
        if has_timer {
            app.insert_resource(TimerInterruptState::new(Duration::from_secs_f32(
                1.0 / 60.0,
            )));
        }
        app.insert_resource(bindings);
    }
    app.insert_resource(SpriteVirtualResolution::new(virtual_resolution));
    app.insert_resource(display.clone());
    let initial_palette = DisplayPalette::from_settings(&display);
    app.insert_resource(initial_palette.clone());
    app.insert_resource(ClearColor(initial_palette.border));

    #[cfg(feature = "native-service")]
    if let Some(listener) = service_listener {
        app.insert_resource(listener);
    }

    #[cfg(target_arch = "wasm32")]
    let web_status = web::configure_app(&mut app);

    let initial_status = app
        .world
        .get_non_send_resource::<EmulatorState>()
        .and_then(EmulatorState::status_message);

    #[allow(unused_mut)]
    let mut ui_state = UiState::with_status(initial_status);

    #[cfg(target_arch = "wasm32")]
    if let Some(status) = web_status {
        ui_state.status = Some(match ui_state.status.take() {
            Some(existing) => format!("{existing} | {status}"),
            None => status,
        });
    }

    app.insert_resource(ui_state);

    #[allow(unused_mut)]
    let mut window = Window {
        title: "opFoundry Console".to_string(),
        ..Default::default()
    };

    #[cfg(not(target_arch = "wasm32"))]
    if display.enforce_aspect_ratio && display.resize_window_to_aspect {
        let min_border_x = display.min_border_x.max(0.0);
        let min_border_y = display.min_border_y.max(0.0);
        let base_height = window
            .resolution
            .height()
            .max(virtual_resolution.height as f32 + 2.0 * min_border_y + f32::EPSILON);
        let scale = (base_height - 2.0 * min_border_y) / virtual_resolution.height as f32;
        let content_width = virtual_resolution.width as f32 * scale;
        let width = content_width + 2.0 * min_border_x;
        let height = virtual_resolution.height as f32 * scale + 2.0 * min_border_y;
        window.resolution = WindowResolution::new(width.max(1.0), height.max(1.0));
    }

    #[cfg(target_arch = "wasm32")]
    {
        window.canvas = Some(web::canvas_id().to_string());
        window.fit_canvas_to_parent = true;
    }

    app.add_plugins((
        DefaultPlugins.set(WindowPlugin {
            primary_window: Some(window),
            ..Default::default()
        }),
        EguiPlugin,
    ))
    .add_systems(Startup, setup_scene)
    .add_systems(
        Update,
        (
            update_keyboard_tracker,
            sync_controller_backend,
            controller_input_system,
            emit_frame_start_interrupt,
            timer_interrupt_system,
            drive_raster_counter,
            keyboard_interrupt_system,
            gamepad_interrupt_system,
            update_sprite_viewport,
            update_video_overlay_line,
            ui_system,
        ),
    )
    .add_systems(PostUpdate, emit_frame_end_interrupt)
    .run();

    Ok(())
}

/// Viewer state that proxies CPU execution to the background worker.
struct EmulatorState {
    #[cfg_attr(
        not(any(
            feature = "native-service",
            feature = "native-file-dialog",
            target_arch = "wasm32"
        )),
        allow(dead_code)
    )]
    cpu: CpuWorker,
    outputs: CpuWorkerOutputs,
    #[cfg_attr(
        not(any(
            feature = "native-service",
            feature = "native-file-dialog",
            target_arch = "wasm32"
        )),
        allow(dead_code)
    )]
    default_max_cycles: u64,
    status_message: Option<String>,
    last_outcome: Option<RunOutcome>,
    interrupts: Arc<InterruptController>,
    raster_irq: Arc<RasterIrqState>,
}

impl EmulatorState {
    fn new(
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

    /// Append a host message to the shared console surface (best-effort).
    #[cfg_attr(
        not(any(
            feature = "native-service",
            feature = "native-file-dialog",
            target_arch = "wasm32"
        )),
        allow(dead_code)
    )]
    fn log_console(&self, line: &str) {
        if let Some(handle) = self.outputs.console.as_ref() {
            if let Ok(mut console) = handle.lock() {
                console.write_str(line, 1, 0);
                console.newline();
            }
        }
    }

    fn status_message(&self) -> Option<String> {
        self.status_message.clone()
    }

    #[allow(dead_code)]
    fn last_outcome(&self) -> Option<RunOutcome> {
        self.last_outcome
    }

    fn interrupts(&self) -> Arc<InterruptController> {
        self.interrupts.clone()
    }

    fn raster_state(&self) -> Arc<RasterIrqState> {
        self.raster_irq.clone()
    }

    /// Ask the worker to load and execute a program, returning the status text.
    #[cfg_attr(
        not(any(
            feature = "native-service",
            feature = "native-file-dialog",
            target_arch = "wasm32"
        )),
        allow(dead_code)
    )]
    fn run_program(
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

    fn input_backend(&self) -> Option<Arc<dyn InputBackend>> {
        self.outputs.input_backend.clone()
    }

    fn input_snapshot(&self) -> Option<InputSnapshot> {
        self.outputs
            .input
            .as_ref()
            .and_then(|handle| handle.lock().ok().map(|output| output.snapshot()))
    }

    fn snapshot(&self) -> Option<ConsoleSnapshot> {
        self.outputs
            .console
            .as_ref()
            .and_then(|handle| handle.lock().ok().map(|output| output.snapshot()))
    }

    fn display_snapshot(&self) -> Option<DisplaySnapshot> {
        self.outputs
            .display
            .as_ref()
            .and_then(|handle| handle.lock().ok().map(|output| output.snapshot()))
    }

    fn sprite_snapshot(&self) -> Option<SpriteSnapshot> {
        self.outputs
            .sprite
            .as_ref()
            .and_then(|handle| handle.lock().ok().map(|output| output.snapshot()))
    }

    fn video_state(&self) -> Arc<Mutex<VideoState>> {
        self.outputs.video.clone()
    }

    fn video_overlay(&self) -> Arc<VideoOverlaySignals> {
        self.outputs.video_overlay.clone()
    }

    #[cfg_attr(
        not(any(
            feature = "native-service",
            feature = "native-file-dialog",
            target_arch = "wasm32"
        )),
        allow(dead_code)
    )]
    fn handle_service_command(&mut self, command: ServiceCommand) -> ServiceResponseMessage {
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

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;

    fn assert_color_eq(a: Color, b: Color) {
        let a = a.as_linear_rgba_f32();
        let b = b.as_linear_rgba_f32();
        for i in 0..4 {
            assert!(
                (a[i] - b[i]).abs() <= 1e-3,
                "component {i} differ: {a:?} vs {b:?}"
            );
        }
    }

    fn sprite(x: f32, y: f32) -> SpriteState {
        let max_value = u16::MAX as f32;
        let clamped_x = x.max(0.0).min(max_value);
        let clamped_y = y.max(0.0).min(max_value);
        SpriteState {
            number: 1,
            anim: 0,
            x: clamped_x.round() as u16,
            y: clamped_y.round() as u16,
            scale_x: 0,
            scale_y: 0,
            enabled: true,
        }
    }

    fn sprite_with_scale(x: f32, y: f32, shift_x: u8, shift_y: u8) -> SpriteState {
        let max_value = u16::MAX as f32;
        let clamped_x = x.max(0.0).min(max_value);
        let clamped_y = y.max(0.0).min(max_value);
        SpriteState {
            number: 1,
            anim: 0,
            x: clamped_x.round() as u16,
            y: clamped_y.round() as u16,
            scale_x: shift_x & 0x0F,
            scale_y: shift_y & 0x0F,
            enabled: true,
        }
    }

    fn virtual_res(width: u32, height: u32) -> SpriteVirtualResolution {
        SpriteVirtualResolution::new(VirtualResolution::new(width, height))
    }

    fn display_no_margins(mmio_max_x: f32, mmio_max_y: f32) -> DisplaySettings {
        DisplaySettings {
            enforce_aspect_ratio: false,
            min_border_x: 0.0,
            min_border_y: 0.0,
            border_color: Color::BLACK,
            background_color: Color::BLACK,
            resize_window_to_aspect: false,
            sprite_margin_left: 0.0,
            sprite_margin_right: 0.0,
            sprite_margin_top: 0.0,
            sprite_margin_bottom: 0.0,
            sprite_mmio_max_x: mmio_max_x,
            sprite_mmio_max_y: mmio_max_y,
            sprite_max_offscreen_width: SPRITE_VIRTUAL_WIDTH,
            sprite_max_offscreen_height: SPRITE_VIRTUAL_HEIGHT,
        }
    }

    fn display_with_margins(
        mmio_max_x: f32,
        mmio_max_y: f32,
        margin: Vec4,
        max_offscreen: Vec2,
    ) -> DisplaySettings {
        DisplaySettings {
            enforce_aspect_ratio: false,
            min_border_x: 0.0,
            min_border_y: 0.0,
            border_color: Color::BLACK,
            background_color: Color::BLACK,
            resize_window_to_aspect: false,
            sprite_margin_left: margin.x.max(0.0),
            sprite_margin_right: margin.y.max(0.0),
            sprite_margin_top: margin.z.max(0.0),
            sprite_margin_bottom: margin.w.max(0.0),
            sprite_mmio_max_x: mmio_max_x,
            sprite_mmio_max_y: mmio_max_y,
            sprite_max_offscreen_width: max_offscreen.x.max(0.0),
            sprite_max_offscreen_height: max_offscreen.y.max(0.0),
        }
    }

    fn approx_equal(a: f32, b: f32, eps: f32) {
        assert!(
            (a - b).abs() <= eps,
            "expected {b}, got {a} (|Δ| = {})",
            (a - b).abs()
        );
    }

    #[test]
    fn sprite_position_aligns_top_left_at_origin() {
        let sprite_virtual = virtual_res(128, 96);
        let mut viewport = SpriteViewport::new(800.0, 600.0);
        let scale_x = viewport.window_width() / sprite_virtual.width();
        let scale_y = viewport.window_height() / sprite_virtual.height();
        viewport.set_content(scale_x, scale_y, 0.0, 0.0);
        let sprite = sprite(0.0, 0.0);
        let display = display_no_margins(128.0, 96.0);

        let (world_pos, world_size) =
            sprite_world_transform(&sprite, &viewport, &sprite_virtual, &display)
                .expect("sprite should be visible");

        let expected_size_x = sprite_virtual_size().x * viewport.scale_x();
        let expected_size_y = sprite_virtual_size().y * viewport.scale_y();
        let expected_x = -viewport.window_width() * 0.5 + expected_size_x * 0.5;
        let expected_y = viewport.window_height() * 0.5 - expected_size_y * 0.5;

        approx_equal(world_pos.x, expected_x, 3.0);
        approx_equal(world_pos.y, expected_y, 3.0);
        approx_equal(world_size.x, expected_size_x, 1e-3);
        approx_equal(world_size.y, expected_size_y, 1e-3);
    }

    #[test]
    fn sprite_position_aligns_bottom_right_at_max() {
        let sprite_virtual = virtual_res(128, 96);
        let mut viewport = SpriteViewport::new(800.0, 600.0);
        let scale_x = viewport.window_width() / sprite_virtual.width();
        let scale_y = viewport.window_height() / sprite_virtual.height();
        viewport.set_content(scale_x, scale_y, 0.0, 0.0);
        let dims = sprite_virtual_size();
        let sprite = sprite(128.0 - dims.x, 96.0 - dims.y);
        let display = display_no_margins(128.0, 96.0);

        let (world_pos, world_size) =
            sprite_world_transform(&sprite, &viewport, &sprite_virtual, &display)
                .expect("sprite should be visible");

        let expected_size_x = sprite_virtual_size().x * viewport.scale_x();
        let expected_size_y = sprite_virtual_size().y * viewport.scale_y();
        let expected_x = viewport.window_width() * 0.5 - expected_size_x * 0.5;
        let expected_y = -viewport.window_height() * 0.5 + expected_size_y * 0.5;

        approx_equal(world_pos.x, expected_x, 3.0);
        approx_equal(world_pos.y, expected_y, 3.0);
        approx_equal(world_size.x, expected_size_x, 1e-3);
        approx_equal(world_size.y, expected_size_y, 1e-3);
    }

    #[test]
    fn sprite_position_centres_at_midpoint() {
        let sprite_virtual = virtual_res(128, 96);
        let mut viewport = SpriteViewport::new(800.0, 600.0);
        let scale_x = viewport.window_width() / sprite_virtual.width();
        let scale_y = viewport.window_height() / sprite_virtual.height();
        viewport.set_content(scale_x, scale_y, 0.0, 0.0);
        let dims = sprite_virtual_size();
        let sprite = sprite((128.0 - dims.x) * 0.5, (96.0 - dims.y) * 0.5);
        let display = display_no_margins(128.0, 96.0);

        let (world_pos, world_size) =
            sprite_world_transform(&sprite, &viewport, &sprite_virtual, &display)
                .expect("sprite should be visible");

        let expected_size_x = sprite_virtual_size().x * viewport.scale_x();
        let expected_size_y = sprite_virtual_size().y * viewport.scale_y();

        approx_equal(world_pos.x, 0.0, 3.0);
        approx_equal(world_pos.y, 0.0, 3.0);
        approx_equal(world_size.x, expected_size_x, 1e-3);
        approx_equal(world_size.y, expected_size_y, 1e-3);
    }

    #[test]
    fn sprite_respects_margins_and_culling() {
        let sprite_virtual = virtual_res(160, 120);
        let mut viewport = SpriteViewport::new(640.0, 480.0);
        let scale_x = viewport.window_width() / sprite_virtual.width();
        let scale_y = viewport.window_height() / sprite_virtual.height();
        viewport.set_content(scale_x, scale_y, 0.0, 0.0);

        // Allow 40 units margin on each side, cull after sprite width/height.
        let display = display_with_margins(
            65535.0,
            65535.0,
            Vec4::new(40.0, 40.0, 40.0, 40.0),
            Vec2::new(SPRITE_VIRTUAL_WIDTH, SPRITE_VIRTUAL_HEIGHT),
        );

        // A sprite exactly at the virtual origin should be visible.
        let on_screen =
            sprite_world_transform(&sprite(0.0, 0.0), &viewport, &sprite_virtual, &display);
        assert!(
            on_screen.is_some(),
            "expected sprite at origin to be visible"
        );

        // A sprite far outside the left/top bounds should be culled.
        let culled_display = display_with_margins(
            65535.0,
            65535.0,
            Vec4::new(80.0, 0.0, 80.0, 0.0),
            Vec2::ZERO,
        );
        let culled = sprite_world_transform(
            &sprite(0.0, 0.0),
            &viewport,
            &sprite_virtual,
            &culled_display,
        );
        assert!(
            culled.is_none(),
            "expected sprite outside margins to be culled"
        );
    }

    #[test]
    fn sprite_per_axis_scale_divides_mmio_values() {
        let sprite_virtual = virtual_res(128, 96);
        let mut viewport = SpriteViewport::new(800.0, 600.0);
        let scale_x = viewport.window_width() / sprite_virtual.width();
        let scale_y = viewport.window_height() / sprite_virtual.height();
        viewport.set_content(scale_x, scale_y, 0.0, 0.0);
        let display = display_no_margins(128.0, 96.0);

        // Write coordinates doubled using a scale shift of 1 (factor 2).
        let dims = sprite_virtual_size();
        let desired_x = (128.0 - dims.x) * 0.5;
        let desired_y = (96.0 - dims.y) * 0.5;
        let sprite = sprite_with_scale(desired_x * 2.0, desired_y * 2.0, 1, 1);
        let placement =
            sprite_world_transform(&sprite, &viewport, &sprite_virtual, &display).unwrap();

        // Expect the sprite to land at content origin despite doubled MMIO writes.
        approx_equal(placement.0.x, 0.0, 1.5);
        approx_equal(placement.0.y, 0.0, 1.5);
    }

    #[test]
    fn viewport_geometry_without_aspect_enforcement() {
        let settings = DisplaySettings {
            enforce_aspect_ratio: false,
            ..DisplaySettings::default()
        };
        let virtual_res = SpriteVirtualResolution::new(VirtualResolution::new(128, 96));
        let (scale_x, scale_y, border_x, border_y) =
            compute_viewport_geometry(800.0, 600.0, &virtual_res, &settings);

        approx_equal(scale_x, 800.0 / 128.0, 1e-6);
        approx_equal(scale_y, 600.0 / 96.0, 1e-6);
        approx_equal(border_x, 0.0, 1e-6);
        approx_equal(border_y, 0.0, 1e-6);
    }

    #[test]
    fn viewport_geometry_with_aspect_enforcement_and_min_border() {
        let settings = DisplaySettings {
            enforce_aspect_ratio: true,
            min_border_x: 10.0,
            min_border_y: 20.0,
            ..DisplaySettings::default()
        };
        let virtual_res = SpriteVirtualResolution::new(VirtualResolution::new(160, 120));
        let (scale_x, scale_y, border_x, border_y) =
            compute_viewport_geometry(800.0, 600.0, &virtual_res, &settings);

        // Aspect ratio 4:3 should be preserved; min borders padded equally.
        approx_equal(scale_x, scale_y, 1e-6);
        assert!(border_x >= settings.min_border_x - 1e-6);
        assert!(border_y >= settings.min_border_y - 1e-6);
    }

    #[test]
    fn mmio_palette_translation_uses_c64_colours() {
        let defaults = DisplaySettings::default();
        let mut palette = DisplayPalette::from_settings(&defaults);
        let snapshot = DisplaySnapshot {
            border_color: 0x06,
            background_color: 0x0A,
        };
        palette.apply_snapshot(&snapshot, &defaults);
        assert_color_eq(palette.border, Color::rgb_u8(0x00, 0x00, 0xAA));
        assert_color_eq(palette.background, Color::rgb_u8(0xFF, 0x77, 0x77));
    }

    #[test]
    fn modern_retro_interrupts_fire_from_host_events() {
        use bus::personality::MODERN_RETRO;

        let controller = Arc::new(InterruptController::new());
        controller.set_irq_enable((1 << 1) | (1 << 2) | (1 << 3) | (1 << 4));

        let bindings = InterruptBindings::from_personality(controller.clone(), &MODERN_RETRO)
            .expect("modern-retro bindings");

        // frame_start -> NMI edge
        bindings.raise_frame_start();
        let snapshot = controller.snapshot();
        assert_eq!(snapshot.nmi_pending & (1 << 0), 1 << 0);
        assert!(snapshot.nmi_line);
        assert!(controller.take_nmi_edge());
        controller.clear_nmi(1 << 0);

        // frame_end -> IRQ level
        bindings.raise_frame_end();
        let snapshot = controller.snapshot();
        assert_eq!(snapshot.irq_pending & (1 << 1), 1 << 1);
        assert!(snapshot.irq_line);
        controller.clear_irq(1 << 1);

        // timer0
        bindings.raise_timer0();
        let snapshot = controller.snapshot();
        assert_eq!(snapshot.irq_pending & (1 << 2), 1 << 2);
        controller.clear_irq(1 << 2);

        // keyboard
        bindings.raise_keyboard();
        let snapshot = controller.snapshot();
        assert_eq!(snapshot.irq_pending & (1 << 3), 1 << 3);
        controller.clear_irq(1 << 3);

        // gamepad
        bindings.raise_gamepad();
        let snapshot = controller.snapshot();
        assert_eq!(snapshot.irq_pending & (1 << 4), 1 << 4);
        controller.clear_irq(1 << 4);

        let snapshot = controller.snapshot();
        assert_eq!(snapshot.irq_pending, 0);
        assert_eq!(snapshot.nmi_pending, 0);
        assert!(!snapshot.irq_line);
        assert!(!snapshot.nmi_line);
    }
}
