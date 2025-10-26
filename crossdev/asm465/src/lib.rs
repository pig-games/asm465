//! Bevy/egui front-end for the asm465 cross-development tooling.
//!
//! This crate hosts the “desktop” viewer: it embeds the 6502 core, connects to
//! the cross465 [bus] crate, renders the screen/console, and exposes file &
//! service APIs for loading programs at runtime.  The same crate also backs the
//! wasm build (via [`web::start_web_app`]), so as much logic as possible lives
//! in platform-neutral modules.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use bevy::input::gamepad::GamepadEvent;
use bevy::input::keyboard::KeyboardInput;
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::render::camera::ScalingMode;
use bevy::render::mesh::shape::Quad;
use bevy::render::mesh::Mesh;
use bevy::sprite::{ColorMaterial, MaterialMesh2dBundle, Mesh2dHandle};
use bevy::time::{Timer, TimerMode};
use bevy::window::PrimaryWindow;
#[cfg(not(target_arch = "wasm32"))]
use bevy::window::WindowResolution;
use bevy_egui::{egui, EguiContexts, EguiPlugin};
use bus::console_mmio::ConsoleSnapshot;
use bus::display_mmio::DisplaySnapshot;
use bus::interrupts::{InterruptController, InterruptSnapshot};
use bus::personality::{self, Personality};
use bus::sprite_mmio::{SpriteSnapshot, SpriteState, SPRITE_SLOTS};
use bus::{unicode_to_screen, Bus};
use core6502::{Cpu, RunLimit, RunOutcome};

mod cpu_worker;
use cpu_worker::{
    CpuRunReply, CpuRunStatus, CpuWorker, CpuWorkerInit, CpuWorkerOutputs, PersonalitySelection,
};

#[cfg(all(feature = "native-file-dialog", not(target_arch = "wasm32")))]
use rfd::FileDialog;

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
#[cfg(feature = "native-service")]
use clap::Parser;
#[cfg(feature = "native-service")]
use crossbeam_channel::{Receiver, Sender};
use serde::{Deserialize, Serialize};
#[cfg(feature = "native-service")]
use std::io::{BufRead, BufReader, BufWriter, Write};
#[cfg(feature = "native-service")]
use std::net::{TcpListener, TcpStream};
#[cfg(feature = "native-service")]
use std::thread;

pub(crate) const WELCOME_MESSAGE: &str = "Welcome to the asm465 console viewer!";
const CONSOLE_FONT_SIZE: f32 = 16.0;
const BUILTIN_TOML_PERSONALITIES: &[(&str, &str)] =
    &[("modern-retro-range", "Modern Retro (Range)")];

#[cfg(feature = "native-service")]
fn builtin_personality_path(id: &str) -> Option<PathBuf> {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../cross465/personality_defs");
    match id {
        "modern-retro-range" => Some(base.join("modern-retro-range.toml")),
        _ => None,
    }
}

#[cfg(feature = "native-service")]
fn print_personality_list() {
    println!("Legacy personalities:");
    for persona in personality::all() {
        println!("  {:<20} {}", persona.name, persona.description);
    }
    println!("\nTOML personalities:");
    for (id, desc) in BUILTIN_TOML_PERSONALITIES {
        println!("  {:<20} {}", id, desc);
    }
    println!("  <path>               Load personality from TOML file");
}

#[cfg(feature = "native-service")]
fn resolve_personality_selection(name: &str) -> Result<PersonalitySelection, String> {
    if let Some(persona) = personality::find(name) {
        return Ok(PersonalitySelection::Legacy(persona));
    }

    if let Some(path) = builtin_personality_path(name) {
        return Ok(PersonalitySelection::with_legacy(path, personality::default()));
    }

    let path = PathBuf::from(name);
    if path.exists() {
        return Ok(PersonalitySelection::from_path(path));
    }

    Err(format!(
        "unknown personality '{name}'. Use --list-personalities to inspect the available options.",
    ))
}
const SPRITE_TEXTURE_WIDTH: f32 = 96.0;
const SPRITE_TEXTURE_HEIGHT: f32 = 128.0;
const SPRITE_VIRTUAL_WIDTH: f32 = 40.0;
const SPRITE_VIRTUAL_HEIGHT: f32 =
    SPRITE_VIRTUAL_WIDTH * (SPRITE_TEXTURE_HEIGHT / SPRITE_TEXTURE_WIDTH);
const SPRITE_DEFAULT_MARGIN_X: f32 = SPRITE_VIRTUAL_WIDTH;
const SPRITE_DEFAULT_MARGIN_Y: f32 = SPRITE_VIRTUAL_HEIGHT;
const SPRITE_TEXTURE_PATHS: &[&str] = &[
    "sprites/knight.png",
    "sprites/knight_crimson.png",
    "sprites/knight_glacial.png",
];

#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(target_arch = "wasm32")]
pub use web::start_web_app;

#[cfg(feature = "native-service")]
#[derive(Parser, Debug)]
#[command(author, version, about = "Asm465 console viewer", long_about = None)]
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

    /// Optional positional PRG path (shorthand for `--prg`).
    #[arg(conflicts_with = "prg")]
    pub program: Option<PathBuf>,

    /// Personality to load (see `--list-personalities`).
    #[arg(long, default_value = "modern-retro")]
    pub personality: String,

    /// List available personalities and exit.
    #[arg(long, default_value_t = false)]
    pub list_personalities: bool,
}

/// Source for a PRG payload that should be executed by the emulator.
#[derive(Debug, Clone)]
pub enum ProgramSource {
    File(PathBuf),
    Inline { name: Option<String>, data: Vec<u8> },
}

impl ProgramSource {
    fn load_bytes(&self) -> Result<Vec<u8>, String> {
        match self {
            #[cfg(any(feature = "native-file-dialog", feature = "native-service"))]
            ProgramSource::File(path) => std::fs::read(path)
                .map_err(|err| format!("Failed to read {}: {err}", path.display())),
            #[cfg(not(any(feature = "native-file-dialog", feature = "native-service")))]
            ProgramSource::File(_) => Err("File sources are not supported on this platform".into()),
            ProgramSource::Inline { data, .. } => Ok(data.clone()),
        }
    }

    fn label(&self) -> String {
        match self {
            ProgramSource::File(path) => path.display().to_string(),
            ProgramSource::Inline { name, data } => name
                .clone()
                .unwrap_or_else(|| format!("inline program ({} bytes)", data.len())),
        }
    }
}

/// Configuration used when pre-loading a PRG before the app renders.
#[derive(Clone)]
pub struct StartupConfig {
    pub source: ProgramSource,
    pub max_cycles: u64,
    pub start: Option<u16>,
}

/// Details about a bounded CPU run triggered by the host.
pub(crate) struct ProgramRunReport {
    pub outcome: Option<RunOutcome>,
    pub message: String,
}

/// Command variants exchanged with the external service API.
#[derive(Debug)]
pub enum ServiceCommand {
    RunProgram {
        source: ProgramSource,
        max_cycles: Option<u64>,
        start: Option<u16>,
    },
}

#[derive(Debug, Serialize)]
pub struct ServiceResponseMessage {
    pub status: ServiceStatus,
    pub message: String,
}

impl ServiceResponseMessage {
    pub fn ok(message: impl Into<String>) -> Self {
        Self {
            status: ServiceStatus::Ok,
            message: message.into(),
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            status: ServiceStatus::Error,
            message: message.into(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ServiceStatus {
    Ok,
    Error,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum ServiceRequestPayload {
    RunPrg {
        path: String,
        #[serde(default)]
        max_cycles: Option<u64>,
        #[serde(default)]
        start: Option<u16>,
    },
    RunPrgData {
        data: String,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        max_cycles: Option<u64>,
        #[serde(default)]
        start: Option<u16>,
    },
}

impl ServiceRequestPayload {
    pub fn into_command(self) -> Result<ServiceCommand, String> {
        match self {
            ServiceRequestPayload::RunPrg {
                path,
                max_cycles,
                start,
            } => {
                if path.is_empty() {
                    return Err("run_prg requires a non-empty path".into());
                }
                Ok(ServiceCommand::RunProgram {
                    source: ProgramSource::File(PathBuf::from(path)),
                    max_cycles,
                    start,
                })
            }
            ServiceRequestPayload::RunPrgData {
                data,
                name,
                max_cycles,
                start,
            } => {
                if data.trim().is_empty() {
                    return Err("run_prg_data requires a non-empty base64 payload".into());
                }
                let decoded = BASE64_STANDARD
                    .decode(data.as_bytes())
                    .map_err(|err| format!("invalid base64 payload for run_prg_data: {err}"))?;
                Ok(ServiceCommand::RunProgram {
                    source: ProgramSource::Inline {
                        name,
                        data: decoded,
                    },
                    max_cycles,
                    start,
                })
            }
        }
    }
}

pub struct AppConfig {
    pub startup: Option<StartupConfig>,
    pub default_max_cycles: u64,
    pub virtual_resolution: VirtualResolution,
    pub display: DisplaySettings,
    pub personality: PersonalitySelection,
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

#[derive(Resource, Clone)]
struct DisplayPalette {
    border: Color,
    background: Color,
}

impl DisplayPalette {
    fn from_settings(settings: &DisplaySettings) -> Self {
        Self {
            border: settings.border_color,
            background: settings.background_color,
        }
    }

    fn apply_snapshot(&mut self, snapshot: &DisplaySnapshot, defaults: &DisplaySettings) {
        self.border = mmio_color(snapshot.border_color, defaults.border_color);
        self.background = mmio_color(snapshot.background_color, defaults.background_color);
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

fn mmio_color(value: u8, fallback: Color) -> Color {
    match value & 0x0F {
        0x00 => Color::rgb_u8(0x00, 0x00, 0x00), // Black
        0x01 => Color::rgb_u8(0xFF, 0xFF, 0xFF), // White
        0x02 => Color::rgb_u8(0x88, 0x00, 0x00), // Red
        0x03 => Color::rgb_u8(0xAA, 0xFF, 0xEE), // Cyan
        0x04 => Color::rgb_u8(0xCC, 0x44, 0xCC), // Magenta
        0x05 => Color::rgb_u8(0x00, 0xCC, 0x55), // Green
        0x06 => Color::rgb_u8(0x00, 0x00, 0xAA), // Blue
        0x07 => Color::rgb_u8(0xEE, 0xEE, 0x77), // Yellow
        0x08 => Color::rgb_u8(0xDD, 0x88, 0x55), // Orange
        0x09 => Color::rgb_u8(0x66, 0x44, 0x00), // Brown
        0x0A => Color::rgb_u8(0xFF, 0x77, 0x77), // Light red
        0x0B => Color::rgb_u8(0xAA, 0xFF, 0xEE), // Light cyan
        0x0C => Color::rgb_u8(0xFF, 0xAA, 0xFF), // Light magenta
        0x0D => Color::rgb_u8(0xAA, 0xFF, 0xAA), // Light green
        0x0E => Color::rgb_u8(0xAA, 0xCC, 0xFF), // Light blue
        0x0F => Color::rgb_u8(0xCC, 0xCC, 0xCC), // Light gray
        _ => fallback,
    }
}

#[cfg(feature = "native-service")]
pub fn run_native() -> Result<(), String> {
    let args = Args::parse();
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
        #[cfg(feature = "native-service")]
        service,
    });

    Ok(())
}

pub fn run_app(config: AppConfig) {
    let AppConfig {
        startup,
        default_max_cycles,
        virtual_resolution,
        display,
        personality,
        #[cfg(feature = "native-service")]
        service,
    } = config;

    let legacy_persona = personality.legacy_personality();
    #[allow(unused_mut)]
    let mut emulator = EmulatorState::new(startup, default_max_cycles, personality.clone());
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

    let mut app = App::new();
    app.insert_non_send_resource(emulator);
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
        title: "asm465 Console".to_string(),
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
            emit_frame_start_interrupt,
            timer_interrupt_system,
            keyboard_interrupt_system,
            gamepad_interrupt_system,
            update_sprite_viewport,
            ui_system,
        ),
    )
    .add_systems(PostUpdate, emit_frame_end_interrupt)
    .run();
}

/// Viewer state that proxies CPU execution to the background worker.
struct EmulatorState {
    cpu: CpuWorker,
    outputs: CpuWorkerOutputs,
    default_max_cycles: u64,
    status_message: Option<String>,
    last_outcome: Option<RunOutcome>,
    interrupts: Arc<InterruptController>,
}

impl EmulatorState {
    fn new(
        startup: Option<StartupConfig>,
        default_max_cycles: u64,
        personality: PersonalitySelection,
    ) -> Self {
        let (
            cpu,
            CpuWorkerInit {
                outputs,
                status,
                outcome,
            },
        ) = CpuWorker::spawn(personality, startup)
            .unwrap_or_else(|err| panic!("Failed to start CPU worker: {err}"));

        let interrupts = outputs.interrupts.clone();

        Self {
            cpu,
            outputs,
            default_max_cycles,
            status_message: status,
            last_outcome: outcome,
            interrupts,
        }
    }

    /// Append a host message to the shared console surface (best-effort).
    fn log_console(&self, line: &str) {
        if let Ok(mut console) = self.outputs.console.lock() {
            console.write_str(line, 1, 0);
            console.newline();
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

    /// Ask the worker to load and execute a program, returning the status text.
    fn run_program(
        &mut self,
        source: ProgramSource,
        max_cycles: Option<u64>,
        start: Option<u16>,
    ) -> Result<String, String> {
        let configured_cycles = max_cycles.unwrap_or(self.default_max_cycles);
        let config = StartupConfig {
            source,
            max_cycles: configured_cycles,
            start,
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

    fn snapshot(&self) -> Option<ConsoleSnapshot> {
        self.outputs
            .console
            .lock()
            .map(|output| output.snapshot())
            .ok()
    }

    fn display_snapshot(&self) -> Option<DisplaySnapshot> {
        self.outputs
            .display
            .lock()
            .map(|output| output.snapshot())
            .ok()
    }

    fn sprite_snapshot(&self) -> Option<SpriteSnapshot> {
        self.outputs
            .sprite
            .lock()
            .map(|output| output.snapshot())
            .ok()
    }

    fn handle_service_command(&mut self, command: ServiceCommand) -> ServiceResponseMessage {
        match command {
            ServiceCommand::RunProgram {
                source,
                max_cycles,
                start,
            } => match self.run_program(source, max_cycles, start) {
                Ok(msg) => ServiceResponseMessage::ok(msg),
                Err(err) => ServiceResponseMessage::error(err),
            },
        }
    }
}

#[derive(Resource)]
struct InterruptBindings {
    controller: Arc<InterruptController>,
    frame_start: Option<u32>,
    frame_end: Option<u32>,
    timer0: Option<u32>,
    keyboard: Option<u32>,
    gamepad: Option<u32>,
}

impl InterruptBindings {
    fn from_personality(
        controller: Arc<InterruptController>,
        personality: &'static Personality,
    ) -> Option<Self> {
        let mut bindings = Self {
            controller,
            frame_start: None,
            frame_end: None,
            timer0: None,
            keyboard: None,
            gamepad: None,
        };

        for interrupt in personality.interrupts {
            let mask = 1u32 << interrupt.id;
            match interrupt.name {
                "frame_start" => bindings.frame_start = Some(mask),
                "frame_end" => bindings.frame_end = Some(mask),
                "timer0" => bindings.timer0 = Some(mask),
                "keyboard_event" => bindings.keyboard = Some(mask),
                "gamepad_event" => bindings.gamepad = Some(mask),
                _ => {}
            }
        }

        if bindings.has_any() {
            Some(bindings)
        } else {
            None
        }
    }

    fn has_any(&self) -> bool {
        self.frame_start.is_some()
            || self.frame_end.is_some()
            || self.timer0.is_some()
            || self.keyboard.is_some()
            || self.gamepad.is_some()
    }

    fn has_timer(&self) -> bool {
        self.timer0.is_some()
    }

    fn has_keyboard(&self) -> bool {
        self.keyboard.is_some()
    }

    fn has_gamepad(&self) -> bool {
        self.gamepad.is_some()
    }

    fn snapshot(&self) -> InterruptSnapshot {
        self.controller.snapshot()
    }

    fn raise_frame_start(&self) {
        if let Some(mask) = self.frame_start {
            self.raise_nmi(mask);
        }
    }

    fn raise_frame_end(&self) {
        if let Some(mask) = self.frame_end {
            self.raise_irq(mask);
        }
    }

    fn raise_timer0(&self) {
        if let Some(mask) = self.timer0 {
            self.raise_irq(mask);
        }
    }

    fn raise_keyboard(&self) {
        if let Some(mask) = self.keyboard {
            self.raise_irq(mask);
        }
    }

    fn raise_gamepad(&self) {
        if let Some(mask) = self.gamepad {
            self.raise_irq(mask);
        }
    }

    fn raise_irq(&self, mask: u32) {
        if mask != 0 && (self.controller.irq_pending() & mask) == 0 {
            self.controller.raise_irq(mask);
        }
    }

    fn raise_nmi(&self, mask: u32) {
        if mask != 0 && (self.controller.nmi_pending() & mask) == 0 {
            self.controller.raise_nmi(mask);
        }
    }
}

#[derive(Resource)]
struct TimerInterruptState {
    timer: Timer,
}

impl TimerInterruptState {
    fn new(period: Duration) -> Self {
        Self {
            timer: Timer::new(period, TimerMode::Repeating),
        }
    }
}

fn emit_frame_start_interrupt(bindings: Option<Res<InterruptBindings>>) {
    if let Some(bindings) = bindings {
        bindings.raise_frame_start();
    }
}

fn emit_frame_end_interrupt(bindings: Option<Res<InterruptBindings>>) {
    if let Some(bindings) = bindings {
        bindings.raise_frame_end();
    }
}

fn timer_interrupt_system(
    time: Res<Time>,
    bindings: Option<Res<InterruptBindings>>,
    state: Option<ResMut<TimerInterruptState>>,
) {
    let Some(bindings) = bindings else { return };
    if !bindings.has_timer() {
        return;
    }
    let Some(mut state) = state else { return };
    if state.timer.tick(time.delta()).just_finished() {
        bindings.raise_timer0();
    }
}

fn keyboard_interrupt_system(
    bindings: Option<Res<InterruptBindings>>,
    mut events: EventReader<KeyboardInput>,
) {
    let bindings = match bindings {
        Some(bindings) if bindings.has_keyboard() => bindings,
        _ => return,
    };

    for event in events.iter() {
        if matches!(event.state, ButtonState::Pressed | ButtonState::Released) {
            bindings.raise_keyboard();
            break;
        }
    }
}

fn gamepad_interrupt_system(
    bindings: Option<Res<InterruptBindings>>,
    mut events: EventReader<GamepadEvent>,
) {
    let bindings = match bindings {
        Some(bindings) if bindings.has_gamepad() => bindings,
        _ => return,
    };

    if events.iter().next().is_some() {
        bindings.raise_gamepad();
    }
}

fn interrupt_row(
    ui: &mut egui::Ui,
    name: &str,
    mask: Option<u32>,
    snapshot: &InterruptSnapshot,
    is_nmi: bool,
) {
    if let Some(mask) = mask {
        let pending = if is_nmi {
            snapshot.nmi_pending
        } else {
            snapshot.irq_pending
        };
        let enabled = if is_nmi {
            snapshot.nmi_enabled
        } else {
            snapshot.irq_enabled
        };
        let line = if is_nmi {
            snapshot.nmi_line
        } else {
            snapshot.irq_line
        };
        let pending_set = (pending & mask) != 0;
        let enabled_set = (enabled & mask) != 0;
        ui.horizontal(|ui| {
            ui.label(name);
            ui.label(format!(
                "mask=0x{mask:08X} pending={} enabled={} line={}",
                pending_set, enabled_set, line
            ));
        });
    }
}

#[derive(Resource)]
struct UiState {
    status: Option<String>,
    console_open: bool,
    bridge_connected: Option<bool>,
    active_tab: ConsoleTab,
}

impl UiState {
    fn with_status(status: Option<String>) -> Self {
        Self {
            status,
            console_open: true,
            bridge_connected: None,
            active_tab: ConsoleTab::Console,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ConsoleTab {
    Console,
    Interrupts,
}

#[derive(Component)]
struct SpriteSlot {
    index: usize,
}

#[derive(Component)]
struct ContentBackground;

#[derive(Resource)]
struct SpriteCatalog {
    handles: Vec<Handle<Image>>,
}

#[derive(Component)]
struct BorderOverlay {
    side: BorderSide,
}

#[derive(Clone, Copy)]
enum BorderSide {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Resource, Clone, Copy)]
struct SpriteVirtualResolution {
    width: f32,
    height: f32,
}

impl SpriteVirtualResolution {
    /// Construct from the user-supplied virtual resolution. Values are
    /// clamped to at least 1 so later divisions are well-defined.
    fn new(resolution: VirtualResolution) -> Self {
        Self {
            width: resolution.width.max(1) as f32,
            height: resolution.height.max(1) as f32,
        }
    }

    /// Virtual canvas width (pixels in guest space).
    fn width(&self) -> f32 {
        self.width
    }

    /// Virtual canvas height (pixels in guest space).
    fn height(&self) -> f32 {
        self.height
    }
}

#[derive(Resource, Clone, Copy)]
struct SpriteViewport {
    width: f32,
    height: f32,
    scale_x: f32,
    scale_y: f32,
    border_x: f32,
    border_y: f32,
    content_width: f32,
    content_height: f32,
}

impl SpriteViewport {
    /// Snapshot the current Bevy window dimensions (host space).
    fn new(window_width: f32, window_height: f32) -> Self {
        Self {
            width: window_width,
            height: window_height,
            scale_x: 1.0,
            scale_y: 1.0,
            border_x: 0.0,
            border_y: 0.0,
            content_width: window_width,
            content_height: window_height,
        }
    }

    /// Refresh the cached window dimensions whenever the window resizes.
    fn update_window(&mut self, width: f32, height: f32) {
        self.width = width;
        self.height = height;
    }

    /// Host-space width in logical pixels (never < 1).
    fn window_width(&self) -> f32 {
        self.width.max(1.0)
    }

    /// Host-space height in logical pixels (never < 1).
    fn window_height(&self) -> f32 {
        self.height.max(1.0)
    }

    fn set_content(&mut self, scale_x: f32, scale_y: f32, border_x: f32, border_y: f32) {
        self.scale_x = scale_x;
        self.scale_y = scale_y;
        self.border_x = border_x;
        self.border_y = border_y;
        self.content_width = (self.window_width() - 2.0 * border_x).max(0.0);
        self.content_height = (self.window_height() - 2.0 * border_y).max(0.0);
    }

    fn scale_x(&self) -> f32 {
        self.scale_x
    }

    fn scale_y(&self) -> f32 {
        self.scale_y
    }

    fn border_x(&self) -> f32 {
        self.border_x
    }

    fn border_y(&self) -> f32 {
        self.border_y
    }

    fn content_width(&self) -> f32 {
        self.content_width.max(0.0)
    }

    fn content_height(&self) -> f32 {
        self.content_height.max(0.0)
    }
}

fn setup_scene(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    palette: Res<DisplayPalette>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    window_query: Query<&Window, With<PrimaryWindow>>,
) {
    let mut camera = Camera2dBundle::default();
    camera.projection.scaling_mode = ScalingMode::WindowSize(1.0);
    commands.spawn(camera);

    let window = window_query
        .get_single()
        .expect("primary window not available during setup");
    commands.insert_resource(SpriteViewport::new(window.width(), window.height()));

    let mesh = meshes.add(Mesh::from(Quad::default()));
    let material = materials.add(ColorMaterial::from(palette.background));
    let border_material = materials.add(ColorMaterial::from(palette.border));
    let mut background_transform = Transform::from_xyz(0.0, 0.0, -0.5);
    background_transform.scale = Vec3::new(window.width(), window.height(), 1.0);
    commands.spawn((
        MaterialMesh2dBundle {
            mesh: Mesh2dHandle(mesh),
            material,
            transform: background_transform,
            visibility: Visibility::Visible,
            ..Default::default()
        },
        ContentBackground,
    ));

    let handles: Vec<Handle<Image>> = SPRITE_TEXTURE_PATHS
        .iter()
        .map(|path| asset_server.load(*path))
        .collect();
    let default_texture = handles
        .first()
        .cloned()
        .expect("sprite texture list must not be empty");

    commands.insert_resource(SpriteCatalog {
        handles: handles.clone(),
    });

    for index in 0..SPRITE_SLOTS {
        commands.spawn((
            SpriteBundle {
                texture: default_texture.clone(),
                sprite: Sprite {
                    color: Color::WHITE,
                    custom_size: None,
                    ..Default::default()
                },
                transform: Transform::from_xyz(0.0, 0.0, 1.0 + index as f32 * 0.01),
                visibility: Visibility::Hidden,
                ..Default::default()
            },
            SpriteSlot { index },
        ));
    }

    let overlay_z = 5.0;
    let overlay_mesh = Mesh2dHandle(meshes.add(Mesh::from(Quad::default())));
    for side in [
        BorderSide::Left,
        BorderSide::Right,
        BorderSide::Top,
        BorderSide::Bottom,
    ] {
        commands.spawn((
            MaterialMesh2dBundle {
                mesh: overlay_mesh.clone(),
                material: border_material.clone(),
                transform: Transform::from_xyz(0.0, 0.0, overlay_z),
                visibility: Visibility::Visible,
                ..Default::default()
            },
            BorderOverlay { side },
        ));
    }
}

fn ui_system(
    mut contexts: EguiContexts,
    #[allow(unused_mut)] mut emulator: NonSendMut<EmulatorState>,
    mut ui_state: ResMut<UiState>,
    sprite_catalog: Res<SpriteCatalog>,
    sprite_viewport: Res<SpriteViewport>,
    sprite_virtual: Res<SpriteVirtualResolution>,
    display: Res<DisplaySettings>,
    mut palette: ResMut<DisplayPalette>,
    mut sprite_query: Query<(
        &SpriteSlot,
        &mut Transform,
        &mut Visibility,
        &mut Sprite,
        &mut Handle<Image>,
    )>,
    bindings: Option<Res<InterruptBindings>>,
    #[cfg(feature = "native-service")] service_listener: Option<Res<ServiceListener>>,
    #[cfg(target_arch = "wasm32")] web_service: Option<NonSend<web::WebSocketBridge>>,
) {
    #[cfg(feature = "native-service")]
    if let Some(listener) = service_listener {
        while let Ok(envelope) = listener.receiver.try_recv() {
            let response = emulator.handle_service_command(envelope.command);
            ui_state.status = Some(response.message.clone());
            let _ = envelope.respond_to.send(response);
        }
    }

    #[cfg(target_arch = "wasm32")]
    let mut wasm_bridge_connected = false;

    #[cfg(target_arch = "wasm32")]
    if let Some(service) = web_service {
        wasm_bridge_connected = true;
        for command in service.drain_commands() {
            let response = emulator.handle_service_command(command);
            ui_state.status = Some(response.message.clone());
            service.send_response(response);
        }
    }

    #[cfg(target_arch = "wasm32")]
    for (data, name) in web::drain_pending_files() {
        let source = ProgramSource::Inline { name, data };
        let result = emulator.run_program(source, None, None);
        ui_state.status = Some(match result {
            Ok(msg) => msg,
            Err(err) => err,
        });
    }

    #[cfg(target_arch = "wasm32")]
    {
        ui_state.bridge_connected = Some(wasm_bridge_connected);
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        ui_state.bridge_connected = None;
    }

    let ctx = contexts.ctx_mut();

    egui::TopBottomPanel::top("top_panel")
        .resizable(false)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                #[cfg(target_arch = "wasm32")]
                {
                    if ui.button("Load PRG...").clicked() {
                        web::request_file_dialog();
                    }
                }

                #[cfg(all(feature = "native-file-dialog", not(target_arch = "wasm32")))]
                {
                    if ui.button("Load PRG...").clicked() {
                        if let Some(path) = FileDialog::new()
                            .add_filter("PRG/BIN", &["prg", "bin"])
                            .pick_file()
                        {
                            let result =
                                emulator.run_program(ProgramSource::File(path.clone()), None, None);
                            ui_state.status = Some(match result {
                                Ok(msg) => msg,
                                Err(err) => err,
                            });
                        }
                    }
                }

                #[cfg(all(not(target_arch = "wasm32"), not(feature = "native-file-dialog")))]
                {
                    ui.add_enabled(false, egui::Button::new("Load PRG..."));
                }

                if let Some(status) = &ui_state.status {
                    ui.label(status);
                }

                #[cfg(target_arch = "wasm32")]
                if let Some(connected) = ui_state.bridge_connected {
                    let text = if connected {
                        "Bridge: connected"
                    } else {
                        "Bridge: offline"
                    };
                    ui.label(text);
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let toggle_label = if ui_state.console_open {
                        "Hide Developer Tools"
                    } else {
                        "Show Developer Tools"
                    };
                    if ui.button(toggle_label).clicked() {
                        ui_state.console_open = !ui_state.console_open;
                    }
                });
            });
        });

    let console_snapshot = emulator.snapshot();
    let display_snapshot = emulator.display_snapshot();
    let sprite_snapshot = emulator.sprite_snapshot();

    if let Some(snapshot) = display_snapshot.as_ref() {
        palette.apply_snapshot(snapshot, &display);
    } else {
        *palette = DisplayPalette::from_settings(&display);
    }
    for (slot, mut transform, mut visibility, mut sprite, mut texture) in sprite_query.iter_mut() {
        let state = sprite_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.sprite(slot.index));
        if let Some(state) = state {
            if state.number == 0 {
                *visibility = Visibility::Hidden;
                continue;
            }
            if let Some((position, size)) =
                sprite_world_transform(state, &sprite_viewport, &sprite_virtual, &display)
            {
                *visibility = Visibility::Visible;
                transform.translation.x = position.x;
                transform.translation.y = position.y;
                sprite.custom_size = Some(size);
                let texture_index = (state.number.saturating_sub(1)) as usize;
                let desired_texture = sprite_catalog
                    .handles
                    .get(texture_index)
                    .cloned()
                    .or_else(|| sprite_catalog.handles.first().cloned());
                if let Some(handle) = desired_texture {
                    if *texture != handle {
                        *texture = handle;
                    }
                }
                sprite.color = Color::WHITE;
            } else {
                *visibility = Visibility::Hidden;
                continue;
            }
        } else {
            *visibility = Visibility::Hidden;
        }
    }

    egui::TopBottomPanel::bottom("console_panel")
        .resizable(true)
        .default_height(260.0)
        .min_height(160.0)
        .show_animated(ctx, ui_state.console_open, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut ui_state.active_tab, ConsoleTab::Console, "Console");
                ui.selectable_value(
                    &mut ui_state.active_tab,
                    ConsoleTab::Interrupts,
                    "Interrupts",
                );
            });

            ui.separator();

            match ui_state.active_tab {
                ConsoleTab::Console => {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false; 2])
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            if let Some(snapshot) = &console_snapshot {
                                let job = console_layout_job(snapshot);
                                ui.label(job);
                            } else {
                                ui.label("Console unavailable");
                            }
                        });
                }
                ConsoleTab::Interrupts => {
                    if let Some(bindings) = bindings.as_ref() {
                        let snapshot = bindings.snapshot();
                        ui.label(format!(
                            "IRQ: pending=0x{pending:08X} enabled=0x{enabled:08X} line={}",
                            snapshot.irq_line,
                            pending = snapshot.irq_pending,
                            enabled = snapshot.irq_enabled
                        ));
                        ui.label(format!(
                            "NMI: pending=0x{pending:08X} enabled=0x{enabled:08X} edge={} line={}",
                            snapshot.nmi_line,
                            snapshot.nmi_edge_latched,
                            pending = snapshot.nmi_pending,
                            enabled = snapshot.nmi_enabled
                        ));

                        ui.separator();
                        interrupt_row(ui, "frame_start", bindings.frame_start, &snapshot, true);
                        interrupt_row(ui, "frame_end", bindings.frame_end, &snapshot, false);
                        interrupt_row(ui, "timer0", bindings.timer0, &snapshot, false);
                        interrupt_row(ui, "keyboard_event", bindings.keyboard, &snapshot, false);
                        interrupt_row(ui, "gamepad_event", bindings.gamepad, &snapshot, false);
                    } else {
                        ui.label("Interrupt bindings unavailable.");
                    }
                }
            }
        });
}

pub(crate) fn run_program_with_config(
    bus: Bus,
    config: &StartupConfig,
) -> Result<(Bus, ProgramRunReport), (Bus, ProgramRunReport)> {
    let label = config.source.label();
    let data = match config.source.load_bytes() {
        Ok(bytes) => bytes,
        Err(err) => {
            return Err((
                bus,
                ProgramRunReport {
                    outcome: None,
                    message: err,
                },
            ))
        }
    };

    if data.len() < 2 {
        return Err((
            bus,
            ProgramRunReport {
                outcome: None,
                message: format!("Program {label} is too small to contain a load address"),
            },
        ));
    }

    let load_addr = u16::from_le_bytes([data[0], data[1]]);
    let body = &data[2..];
    let mut bus = bus;
    bus.load(load_addr, body);
    let start = config.start.unwrap_or(load_addr);
    bus.set_reset_vector(start);

    let mut cpu = Cpu::new(bus);
    cpu.reset();
    let outcome = cpu.run_for(config.max_cycles);
    let bus = cpu.bus;

    let limit_desc = match outcome.limit {
        RunLimit::CycleBudget => "cycle budget",
        RunLimit::Brk => "BRK",
    };

    let summary = format!(
        "Loaded {label} at ${:04X} and ran for {} cycles (reason: {limit_desc}, start=${:04X})",
        load_addr, outcome.cycles, start
    );

    Ok((
        bus,
        ProgramRunReport {
            outcome: Some(outcome),
            message: summary,
        },
    ))
}

pub(crate) fn write_console_line(bus: &mut Bus, line: &str) {
    for ch in line.chars() {
        let screen_code = unicode_to_screen(ch);
        if screen_code == b'\n' {
            bus.write(0xDF01, 0);
        } else {
            bus.write(0xDF00, screen_code);
        }
    }
    bus.write(0xDF01, 0);
}

fn sprite_virtual_size() -> Vec2 {
    Vec2::new(SPRITE_VIRTUAL_WIDTH, SPRITE_VIRTUAL_HEIGHT)
}

fn sprite_mmio_position(sprite: &SpriteState) -> Vec2 {
    let factor_x = 2f32.powi((sprite.scale_x & 0x0F) as i32).max(1.0);
    let factor_y = 2f32.powi((sprite.scale_y & 0x0F) as i32).max(1.0);
    Vec2::new(sprite.x as f32 / factor_x, sprite.y as f32 / factor_y)
}

fn sprite_world_transform(
    sprite: &SpriteState,
    viewport: &SpriteViewport,
    virtual_resolution: &SpriteVirtualResolution,
    display: &DisplaySettings,
) -> Option<(Vec2, Vec2)> {
    let window_width = viewport.window_width();
    let window_height = viewport.window_height();
    let half_width = window_width * 0.5;
    let half_height = window_height * 0.5;

    let virtual_width = virtual_resolution.width().max(1.0);
    let virtual_height = virtual_resolution.height().max(1.0);
    let margin_left = display.sprite_margin_left.max(0.0);
    let margin_right = display.sprite_margin_right.max(0.0);
    let margin_top = display.sprite_margin_top.max(0.0);
    let margin_bottom = display.sprite_margin_bottom.max(0.0);

    let map_width = virtual_width + margin_left + margin_right;
    let map_height = virtual_height + margin_top + margin_bottom;
    let mmio_max_x = if display.sprite_mmio_max_x > 0.0 {
        display.sprite_mmio_max_x
    } else {
        map_width.max(1.0)
    };
    let mmio_max_y = if display.sprite_mmio_max_y > 0.0 {
        display.sprite_mmio_max_y
    } else {
        map_height.max(1.0)
    };

    let mmio_position = sprite_mmio_position(sprite);
    let clamped_x = mmio_position.x.clamp(0.0, mmio_max_x);
    let clamped_y = mmio_position.y.clamp(0.0, mmio_max_y);

    let normalized_x = (clamped_x / mmio_max_x).clamp(0.0, 1.0);
    let normalized_y = (clamped_y / mmio_max_y).clamp(0.0, 1.0);

    let virtual_x = normalized_x * map_width - margin_left;
    let virtual_y = normalized_y * map_height - margin_top;

    let sprite_virtual = sprite_virtual_size();
    let sprite_virtual_width = sprite_virtual.x;
    let sprite_virtual_height = sprite_virtual.y;

    let max_offscreen_width = display.sprite_max_offscreen_width.max(0.0);
    let max_offscreen_height = display.sprite_max_offscreen_height.max(0.0);

    let left_limit = -max_offscreen_width;
    let right_limit = virtual_width + max_offscreen_width;
    let top_limit = -max_offscreen_height;
    let bottom_limit = virtual_height + max_offscreen_height;

    let sprite_left = virtual_x;
    let sprite_right = virtual_x + sprite_virtual_width;
    let sprite_top = virtual_y;
    let sprite_bottom = virtual_y + sprite_virtual_height;

    if sprite_right < left_limit
        || sprite_left > right_limit
        || sprite_bottom < top_limit
        || sprite_top > bottom_limit
    {
        return None;
    }

    let scale_x = viewport.scale_x();
    let scale_y = viewport.scale_y();

    let sprite_world_width = sprite_virtual_width * scale_x;
    let sprite_world_height = sprite_virtual_height * scale_y;
    let sprite_half_width = sprite_world_width * 0.5;
    let sprite_half_height = sprite_world_height * 0.5;

    let content_left = -half_width + viewport.border_x();
    let content_top = half_height - viewport.border_y();

    let host_left = content_left + virtual_x * scale_x;
    let host_top = content_top - virtual_y * scale_y;

    Some((
        Vec2::new(host_left + sprite_half_width, host_top - sprite_half_height),
        Vec2::new(sprite_world_width, sprite_world_height),
    ))
}

#[cfg(test)]
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

fn update_sprite_viewport(
    window_query: Query<&Window, With<PrimaryWindow>>,
    mut viewport: ResMut<SpriteViewport>,
    virtual_resolution: Res<SpriteVirtualResolution>,
    display: Res<DisplaySettings>,
    palette: Res<DisplayPalette>,
    mut clear_color: ResMut<ClearColor>,
    mut background: Query<
        (&Handle<ColorMaterial>, &mut Transform),
        (With<ContentBackground>, Without<BorderOverlay>),
    >,
    mut overlays: Query<
        (&BorderOverlay, &Handle<ColorMaterial>, &mut Transform),
        Without<ContentBackground>,
    >,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    if let Ok(window) = window_query.get_single() {
        viewport.update_window(window.width(), window.height());

        let (scale_x, scale_y, computed_border_x, computed_border_y) = compute_viewport_geometry(
            viewport.window_width(),
            viewport.window_height(),
            &virtual_resolution,
            &display,
        );

        viewport.set_content(scale_x, scale_y, computed_border_x, computed_border_y);

        clear_color.0 = palette.border;
        if let Ok((material_handle, mut transform)) = background.get_single_mut() {
            transform.scale = Vec3::new(
                viewport.content_width().max(1.0),
                viewport.content_height().max(1.0),
                1.0,
            );
            if let Some(material) = materials.get_mut(material_handle) {
                material.color = palette.background;
            }
        }

        let half_width = viewport.window_width() * 0.5;
        let half_height = viewport.window_height() * 0.5;
        let border_x = viewport.border_x().max(0.0);
        let border_y = viewport.border_y().max(0.0);

        for (overlay, material_handle, mut transform) in overlays.iter_mut() {
            if let Some(material) = materials.get_mut(material_handle) {
                material.color = palette.border;
            }

            match overlay.side {
                BorderSide::Left => {
                    transform.translation.x = -half_width + border_x * 0.5;
                    transform.translation.y = 0.0;
                    transform.scale = Vec3::new(border_x.max(0.0), viewport.window_height(), 1.0);
                }
                BorderSide::Right => {
                    transform.translation.x = half_width - border_x * 0.5;
                    transform.translation.y = 0.0;
                    transform.scale = Vec3::new(border_x.max(0.0), viewport.window_height(), 1.0);
                }
                BorderSide::Top => {
                    transform.translation.x = 0.0;
                    transform.translation.y = half_height - border_y * 0.5;
                    transform.scale = Vec3::new(viewport.window_width(), border_y.max(0.0), 1.0);
                }
                BorderSide::Bottom => {
                    transform.translation.x = 0.0;
                    transform.translation.y = -half_height + border_y * 0.5;
                    transform.scale = Vec3::new(viewport.window_width(), border_y.max(0.0), 1.0);
                }
            }
        }
    }
}

fn compute_viewport_geometry(
    window_width: f32,
    window_height: f32,
    virtual_resolution: &SpriteVirtualResolution,
    display: &DisplaySettings,
) -> (f32, f32, f32, f32) {
    let min_border_x = display.min_border_x.max(0.0);
    let min_border_y = display.min_border_y.max(0.0);

    if display.enforce_aspect_ratio {
        let inner_width = (window_width - 2.0 * min_border_x).max(1.0);
        let inner_height = (window_height - 2.0 * min_border_y).max(1.0);
        let uniform_scale = (inner_width / virtual_resolution.width())
            .min(inner_height / virtual_resolution.height());
        let mut content_width = virtual_resolution.width() * uniform_scale;
        let mut content_height = virtual_resolution.height() * uniform_scale;
        let mut border_x = (window_width - content_width) * 0.5;
        let mut border_y = (window_height - content_height) * 0.5;

        if border_x < min_border_x || border_y < min_border_y {
            border_x = min_border_x;
            border_y = min_border_y;
            let adjusted_width = (window_width - 2.0 * border_x).max(1.0);
            let adjusted_height = (window_height - 2.0 * border_y).max(1.0);
            let uniform_scale = (adjusted_width / virtual_resolution.width())
                .min(adjusted_height / virtual_resolution.height());
            content_width = virtual_resolution.width() * uniform_scale;
            content_height = virtual_resolution.height() * uniform_scale;
            border_x = (window_width - content_width) * 0.5;
            border_y = (window_height - content_height) * 0.5;
        }

        (
            (window_width - 2.0 * border_x).max(1.0) / virtual_resolution.width(),
            (window_height - 2.0 * border_y).max(1.0) / virtual_resolution.height(),
            border_x,
            border_y,
        )
    } else {
        (
            window_width / virtual_resolution.width(),
            window_height / virtual_resolution.height(),
            0.0,
            0.0,
        )
    }
}

fn console_layout_job(snapshot: &ConsoleSnapshot) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    let mut buffer = [0u8; 4];
    let font_id = egui::FontId::monospace(CONSOLE_FONT_SIZE);

    for y in 0..snapshot.height {
        for x in 0..snapshot.width {
            let cell = snapshot.cell(x, y);
            let glyph = cell.ch.encode_utf8(&mut buffer);
            let mut format = egui::text::TextFormat::default();
            format.font_id = font_id.clone();
            format.color = palette_color(cell.fg);
            job.append(glyph, 0.0, format);
        }
        if y + 1 < snapshot.height {
            let mut format = egui::text::TextFormat::default();
            format.font_id = font_id.clone();
            format.color = palette_color(7);
            job.append("\n", 0.0, format);
        }
    }

    job
}

fn palette_color(index: u8) -> egui::Color32 {
    match index & 0x0F {
        0x00 => egui::Color32::from_rgb(0x00, 0x00, 0x00), // Black
        0x01 => egui::Color32::from_rgb(0xFF, 0xFF, 0xFF), // White
        0x02 => egui::Color32::from_rgb(0x88, 0x00, 0x00), // Red
        0x03 => egui::Color32::from_rgb(0xAA, 0xFF, 0xEE), // Cyan
        0x04 => egui::Color32::from_rgb(0xCC, 0x44, 0xCC), // Magenta
        0x05 => egui::Color32::from_rgb(0x00, 0xCC, 0x55), // Green
        0x06 => egui::Color32::from_rgb(0x00, 0x00, 0xAA), // Blue
        0x07 => egui::Color32::from_rgb(0xEE, 0xEE, 0x77), // Yellow
        0x08 => egui::Color32::from_rgb(0xDD, 0x88, 0x55), // Orange
        0x09 => egui::Color32::from_rgb(0x66, 0x44, 0x00), // Brown
        0x0A => egui::Color32::from_rgb(0xFF, 0x77, 0x77), // Light red
        0x0B => egui::Color32::from_rgb(0xAA, 0xFF, 0xEE), // Light cyan
        0x0C => egui::Color32::from_rgb(0xFF, 0xAA, 0xFF), // Light magenta
        0x0D => egui::Color32::from_rgb(0xAA, 0xFF, 0xAA), // Light green
        0x0E => egui::Color32::from_rgb(0xAA, 0xCC, 0xFF), // Light blue
        _ => egui::Color32::from_rgb(0xCC, 0xCC, 0xCC),    // Light gray
    }
}

#[cfg(feature = "native-service")]
#[derive(Resource)]
struct ServiceListener {
    receiver: Receiver<ServiceEnvelope>,
}

#[cfg(feature = "native-service")]
struct ServiceEnvelope {
    command: ServiceCommand,
    respond_to: Sender<ServiceResponseMessage>,
}

#[cfg(feature = "native-service")]
fn start_service_listener(host: &str, port: u16) -> std::io::Result<Receiver<ServiceEnvelope>> {
    let (tx, rx) = crossbeam_channel::unbounded();
    let listener = TcpListener::bind((host, port))?;
    thread::spawn(move || {
        if let Err(err) = run_service_listener(listener, tx) {
            eprintln!("service listener error: {err}");
        }
    });
    Ok(rx)
}

#[cfg(feature = "native-service")]
fn run_service_listener(listener: TcpListener, tx: Sender<ServiceEnvelope>) -> std::io::Result<()> {
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let tx = tx.clone();
                thread::spawn(move || {
                    if let Err(err) = handle_service_connection(stream, tx) {
                        eprintln!("service client error: {err}");
                    }
                });
            }
            Err(err) => eprintln!("service accept error: {err}"),
        }
    }
    Ok(())
}

#[cfg(feature = "native-service")]
fn handle_service_connection(
    stream: TcpStream,
    tx: Sender<ServiceEnvelope>,
) -> std::io::Result<()> {
    let reader_stream = stream.try_clone()?;
    let mut reader = BufReader::new(reader_stream);
    let mut writer = BufWriter::new(stream);
    let mut line = String::new();

    loop {
        line.clear();
        let read = reader.read_line(&mut line)?;
        if read == 0 {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let payload: ServiceRequestPayload = match serde_json::from_str(trimmed) {
            Ok(payload) => payload,
            Err(err) => {
                write_service_response(
                    &mut writer,
                    ServiceResponseMessage::error(format!("invalid json: {err}")),
                )?;
                continue;
            }
        };
        let command = match payload.into_command() {
            Ok(command) => command,
            Err(err) => {
                write_service_response(&mut writer, ServiceResponseMessage::error(err))?;
                continue;
            }
        };
        let (resp_tx, resp_rx) = crossbeam_channel::bounded(1);
        let envelope = ServiceEnvelope {
            command,
            respond_to: resp_tx,
        };
        if tx.send(envelope).is_err() {
            write_service_response(
                &mut writer,
                ServiceResponseMessage::error("service unavailable"),
            )?;
            break;
        }
        match resp_rx.recv() {
            Ok(response) => {
                write_service_response(&mut writer, response)?;
            }
            Err(_) => {
                write_service_response(
                    &mut writer,
                    ServiceResponseMessage::error("service unavailable"),
                )?;
                break;
            }
        }
    }

    writer.flush()?;
    Ok(())
}

#[cfg(feature = "native-service")]
fn write_service_response<W: Write>(
    writer: &mut W,
    response: ServiceResponseMessage,
) -> std::io::Result<()> {
    serde_json::to_writer(&mut *writer, &response)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?;
    writer.write_all(b"\n")?;
    writer.flush()
}
