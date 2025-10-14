use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPlugin};
use bus::console_mmio::{ConsoleOutput, ConsoleSnapshot};
use bus::{unicode_to_screen, Bus};
use clap::Parser;
use core6502::Cpu;
use rfd::FileDialog;

const WELCOME_MESSAGE: &str = "Welcome to the asm465 bevy console viewer!";

#[derive(Parser, Debug)]
#[command(author, version, about = "Bevy-hosted asm465 console viewer", long_about = None)]
struct Args {
    /// Optional 6502 PRG to execute before the window opens.
    #[arg(long)]
    prg: Option<PathBuf>,

    /// Maximum number of CPU cycles to run the startup program for.
    #[arg(long, default_value_t = 5_000_000u64)]
    max_cycles: u64,

    /// Optional address to jump to instead of the PRG's load address.
    #[arg(long)]
    start: Option<u16>,

    /// Optional positional PRG path (shorthand for `--prg`).
    #[arg(conflicts_with = "prg")]
    program: Option<PathBuf>,
}

#[derive(Clone)]
enum ProgramSource {
    File(PathBuf),
    Inline { name: Option<String>, data: Vec<u8> },
}

impl ProgramSource {
    fn load_bytes(&self) -> Result<Vec<u8>, String> {
        match self {
            ProgramSource::File(path) => std::fs::read(path)
                .map_err(|err| format!("Failed to read {}: {err}", path.display())),
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

#[derive(Clone)]
struct StartupConfig {
    source: ProgramSource,
    max_cycles: u64,
    start: Option<u16>,
}

struct EmulatorState {
    bus: Bus,
    console_output: Arc<Mutex<ConsoleOutput>>,
    default_max_cycles: u64,
    status_message: Option<String>,
}

impl EmulatorState {
    fn new(startup: Option<StartupConfig>, default_max_cycles: u64) -> Self {
        let mut bus = Bus::new();
        let mut status_message = None;

        if let Some(config) = startup {
            match run_program_with_config(bus, &config) {
                Ok((new_bus, msg)) => {
                    status_message = Some(msg.clone());
                    bus = new_bus;
                }
                Err((mut new_bus, msg)) => {
                    write_console_line(&mut new_bus, &msg);
                    status_message = Some(msg);
                    bus = new_bus;
                }
            }
        } else {
            write_console_line(&mut bus, WELCOME_MESSAGE);
            status_message = Some(WELCOME_MESSAGE.to_string());
        }

        let console_output = bus
            .console_output_handle()
            .expect("default console MMIO not found on bus");

        Self {
            bus,
            console_output,
            default_max_cycles,
            status_message,
        }
    }

    fn status_message(&self) -> Option<String> {
        self.status_message.clone()
    }

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
        let bus = std::mem::replace(&mut self.bus, Bus::new());
        match run_program_with_config(bus, &config) {
            Ok((new_bus, msg)) => {
                self.bus = new_bus;
                self.console_output = self
                    .bus
                    .console_output_handle()
                    .expect("default console MMIO not found on bus");
                self.status_message = Some(msg.clone());
                Ok(msg)
            }
            Err((mut new_bus, msg)) => {
                write_console_line(&mut new_bus, &msg);
                self.bus = new_bus;
                self.console_output = self
                    .bus
                    .console_output_handle()
                    .expect("default console MMIO not found on bus");
                self.status_message = Some(msg.clone());
                Err(msg)
            }
        }
    }

    fn snapshot(&self) -> Option<ConsoleSnapshot> {
        self.console_output
            .lock()
            .map(|output| output.snapshot())
            .ok()
    }
}

#[derive(Resource)]
struct UiState {
    status: Option<String>,
    console_open: bool,
}

impl UiState {
    fn with_status(status: Option<String>) -> Self {
        Self {
            status,
            console_open: true,
        }
    }
}

const CONSOLE_FONT_SIZE: f32 = 16.0;
const PLACEHOLDER_SIZE: f32 = 180.0;

fn main() {
    let args = Args::parse();
    let startup_path = args.prg.clone().or_else(|| args.program.clone());
    let startup = startup_path.map(|path| StartupConfig {
        source: ProgramSource::File(path),
        max_cycles: args.max_cycles,
        start: args.start,
    });

    let mut emulator = EmulatorState::new(startup, args.max_cycles);
    let initial_status = emulator.status_message();
    let mut app = App::new();
    app.insert_non_send_resource(emulator);
    app.insert_resource(UiState::with_status(initial_status));

    app.insert_resource(ClearColor(Color::rgb(0.05, 0.05, 0.08)));

    app.add_plugins((
        DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "asm465 Bevy Console".to_string(),
                ..Default::default()
            }),
            ..Default::default()
        }),
        EguiPlugin,
    ))
    .add_systems(Startup, setup_scene)
    .add_systems(Update, ui_system)
    .run();
}

fn setup_scene(mut commands: Commands) {
    commands.spawn(Camera2dBundle::default());

    commands.spawn(SpriteBundle {
        sprite: Sprite {
            color: Color::rgb(0.2, 0.4, 0.8),
            custom_size: Some(Vec2::splat(PLACEHOLDER_SIZE)),
            ..Default::default()
        },
        transform: Transform::from_xyz(0.0, 0.0, 0.0),
        ..Default::default()
    });
}

fn ui_system(
    mut contexts: EguiContexts,
    mut emulator: NonSendMut<EmulatorState>,
    mut ui_state: ResMut<UiState>,
) {
    let ctx = contexts.ctx_mut();

    egui::TopBottomPanel::top("top_panel")
        .resizable(false)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
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

                if let Some(status) = &ui_state.status {
                    ui.label(status);
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let toggle_label = if ui_state.console_open {
                        "Hide Console"
                    } else {
                        "Show Console"
                    };
                    if ui.button(toggle_label).clicked() {
                        ui_state.console_open = !ui_state.console_open;
                    }
                });
            });
        });

    let console_snapshot = emulator.snapshot();

    egui::TopBottomPanel::bottom("console_panel")
        .resizable(true)
        .default_height(220.0)
        .min_height(120.0)
        .show_animated(ctx, ui_state.console_open, |ui| {
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
        });
}

fn run_program_with_config(
    bus: Bus,
    config: &StartupConfig,
) -> Result<(Bus, String), (Bus, String)> {
    let label = config.source.label();
    let data = match config.source.load_bytes() {
        Ok(bytes) => bytes,
        Err(err) => return Err((bus, err)),
    };

    if data.len() < 2 {
        return Err((
            bus,
            format!("Program {label} is too small to contain a load address"),
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
    cpu.run_for(config.max_cycles);
    let cycles = cpu.cycles;
    let bus = cpu.bus;

    let summary = format!(
        "Loaded {label} at ${:04X} and ran for {} cycles (start=${:04X})",
        load_addr, cycles, start
    );

    Ok((bus, summary))
}

fn write_console_line(bus: &mut Bus, line: &str) {
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
