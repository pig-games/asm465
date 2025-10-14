use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
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

#[derive(Resource, Default)]
struct UiState {
    status: Option<String>,
}

#[derive(Component)]
struct ConsoleText;

#[derive(Component)]
struct StatusText;

#[derive(Component)]
struct LoadButton;

#[derive(Resource, Clone)]
struct UiAssets {
    font: Handle<Font>,
}

const CONSOLE_FONT_SIZE: f32 = 18.0;

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
    app.insert_resource(UiState {
        status: initial_status,
    });

    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "asm465 Bevy Console".to_string(),
            ..Default::default()
        }),
        ..Default::default()
    }))
    .add_systems(Startup, setup_ui)
    .add_systems(
        Update,
        (handle_load_button, update_status_text, update_console_text),
    )
    .run();
}

fn setup_ui(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(Camera2dBundle::default());

    let font = asset_server.load("fonts/Hack-Regular.ttf");
    commands.insert_resource(UiAssets { font: font.clone() });
    commands
        .spawn(NodeBundle {
            style: Style {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Stretch,
                padding: UiRect::all(Val::Px(12.0)),
                row_gap: Val::Px(8.0),
                ..Default::default()
            },
            background_color: BackgroundColor(Color::rgba(0.05, 0.05, 0.08, 1.0)),
            ..Default::default()
        })
        .with_children(|parent| {
            parent
                .spawn((
                    ButtonBundle {
                        style: Style {
                            width: Val::Px(160.0),
                            height: Val::Px(32.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..Default::default()
                        },
                        background_color: BackgroundColor(Color::rgb(0.2, 0.4, 0.8)),
                        ..Default::default()
                    },
                    LoadButton,
                ))
                .with_children(|button| {
                    button.spawn(TextBundle::from_section(
                        "Load PRG...",
                        TextStyle {
                            font: font.clone(),
                            font_size: 18.0,
                            color: Color::WHITE,
                        },
                    ));
                });

            parent.spawn((
                TextBundle::from_section(
                    "",
                    TextStyle {
                        font: font.clone(),
                        font_size: 16.0,
                        color: Color::rgb(0.9, 0.9, 0.6),
                    },
                ),
                StatusText,
            ));

            parent.spawn((
                TextBundle {
                    text: Text::from_sections([TextSection {
                        value: String::new(),
                        style: TextStyle {
                            font: font.clone(),
                            font_size: CONSOLE_FONT_SIZE,
                            color: Color::rgb(0.85, 0.85, 0.85),
                        },
                    }])
                    .with_alignment(TextAlignment::Left),
                    style: Style {
                        flex_grow: 1.0,
                        flex_shrink: 1.0,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                ConsoleText,
            ));
        });
}

fn handle_load_button(
    mut interaction_query: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<LoadButton>),
    >,
    mut emulator: NonSendMut<EmulatorState>,
    mut ui_state: ResMut<UiState>,
) {
    for (interaction, mut color) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                *color = BackgroundColor(Color::rgb(0.1, 0.3, 0.7));
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
            Interaction::Hovered => {
                *color = BackgroundColor(Color::rgb(0.3, 0.5, 0.9));
            }
            Interaction::None => {
                *color = BackgroundColor(Color::rgb(0.2, 0.4, 0.8));
            }
        }
    }
}

fn update_status_text(mut query: Query<&mut Text, With<StatusText>>, ui_state: Res<UiState>) {
    if let Ok(mut text) = query.get_single_mut() {
        let content = ui_state.status.as_ref().map(|s| s.as_str()).unwrap_or("");
        text.sections[0].value = content.to_string();
    }
}

fn update_console_text(
    mut query: Query<&mut Text, With<ConsoleText>>,
    emulator: NonSend<EmulatorState>,
    assets: Res<UiAssets>,
) {
    let Some(snapshot) = emulator.snapshot() else {
        return;
    };

    if let Ok(mut text) = query.get_single_mut() {
        text.sections = snapshot_to_sections(&snapshot, assets.font.clone());
    }
}

fn snapshot_to_sections(snapshot: &ConsoleSnapshot, font: Handle<Font>) -> Vec<TextSection> {
    let mut sections = Vec::with_capacity(snapshot.width * snapshot.height + snapshot.height);
    for y in 0..snapshot.height {
        for x in 0..snapshot.width {
            let cell = snapshot.cell(x, y);
            sections.push(TextSection {
                value: cell.ch.to_string(),
                style: TextStyle {
                    font: font.clone(),
                    font_size: CONSOLE_FONT_SIZE,
                    color: palette_color(cell.fg),
                },
            });
        }
        if y + 1 < snapshot.height {
            sections.push(TextSection {
                value: "\n".to_string(),
                style: TextStyle {
                    font: font.clone(),
                    font_size: CONSOLE_FONT_SIZE,
                    color: palette_color(7),
                },
            });
        }
    }
    sections
}

fn palette_color(index: u8) -> Color {
    match index & 0x0F {
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
        _ => Color::rgb_u8(0xCC, 0xCC, 0xCC),    // Light gray
    }
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
