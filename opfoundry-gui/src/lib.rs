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
use std::time::Duration;

use bevy::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use bevy::window::WindowResolution;
use bevy_egui::EguiPlugin;
use bus::input_mmio::InputSnapshot;

mod console_ui;
mod cpu_worker;
mod display;
mod emulator_state;
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
use self::display::{
    drive_raster_counter, setup_scene, sprite_world_transform, update_sprite_viewport,
    update_video_overlay_line, DisplayPalette, RasterDriver, SpriteCatalog, SpriteSlot,
    SpriteViewport, SpriteVirtualResolution, VideoOverlayConfig, SPRITE_DEFAULT_MARGIN_X,
    SPRITE_DEFAULT_MARGIN_Y, SPRITE_VIRTUAL_HEIGHT, SPRITE_VIRTUAL_WIDTH,
};
use self::emulator_state::EmulatorState;
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
use cpu_worker::PersonalitySelection;

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
                emulator.set_status_message(msg.clone());
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

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_color_eq(a: Color, b: Color) {
        let a = a.as_linear_rgba_f32();
        let b = b.as_linear_rgba_f32();
        for i in 0..4 {
            assert!((a[i] - b[i]).abs() <= 1e-3, "component {i}: {a:?} vs {b:?}");
        }
    }

    #[test]
    fn parse_color_accepts_hex_formats() {
        let hash = parse_color("#FFCC00").expect("#RRGGBB should parse");
        let prefixed = parse_color("0x00AAFF").expect("0xRRGGBB should parse");
        let plain = parse_color("112233").expect("plain RRGGBB should parse");

        assert_color_eq(hash, Color::rgb_u8(0xFF, 0xCC, 0x00));
        assert_color_eq(prefixed, Color::rgb_u8(0x00, 0xAA, 0xFF));
        assert_color_eq(plain, Color::rgb_u8(0x11, 0x22, 0x33));
    }

    #[test]
    fn parse_color_rejects_invalid_values() {
        let len_err = parse_color("FFFF").expect_err("short value should fail");
        assert!(len_err.contains("6 hex digits"));

        let radix_err = parse_color("GG0000").expect_err("non-hex value should fail");
        assert!(!radix_err.is_empty());
    }
}
