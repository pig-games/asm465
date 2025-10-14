use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPlugin};
use bus::console_mmio::{ConsoleOutput, ConsoleSnapshot};
use bus::{unicode_to_screen, Bus};
use core6502::Cpu;

#[cfg(feature = "native-file-dialog")]
use rfd::FileDialog;

#[cfg(feature = "native-service")]
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
#[cfg(feature = "native-service")]
use base64::Engine;
#[cfg(feature = "native-service")]
use clap::Parser;
#[cfg(feature = "native-service")]
use crossbeam_channel::{Receiver, Sender};
#[cfg(feature = "native-service")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "native-service")]
use std::io::{BufRead, BufReader, BufWriter, Write};
#[cfg(feature = "native-service")]
use std::net::{TcpListener, TcpStream};
#[cfg(feature = "native-service")]
use std::thread;

const WELCOME_MESSAGE: &str = "Welcome to the asm465 bevy console viewer!";
const CONSOLE_FONT_SIZE: f32 = 16.0;
const PLACEHOLDER_SIZE: f32 = 180.0;

#[cfg(feature = "native-service")]
#[derive(Parser, Debug)]
#[command(author, version, about = "Bevy-hosted asm465 console viewer", long_about = None)]
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

    /// Optional positional PRG path (shorthand for `--prg`).
    #[arg(conflicts_with = "prg")]
    pub program: Option<PathBuf>,
}

#[derive(Clone)]
pub enum ProgramSource {
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
pub struct StartupConfig {
    pub source: ProgramSource,
    pub max_cycles: u64,
    pub start: Option<u16>,
}

pub struct AppConfig {
    pub startup: Option<StartupConfig>,
    pub default_max_cycles: u64,
    #[cfg(feature = "native-service")]
    pub service: Option<ServiceConfig>,
}

#[cfg(feature = "native-service")]
pub struct ServiceConfig {
    pub host: String,
    pub port: u16,
}

#[cfg(feature = "native-service")]
pub fn run_native() -> Result<(), String> {
    let args = Args::parse();
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

    run_app(AppConfig {
        startup,
        default_max_cycles: args.max_cycles,
        #[cfg(feature = "native-service")]
        service,
    });

    Ok(())
}

pub fn run_app(config: AppConfig) {
    let mut emulator = EmulatorState::new(config.startup, config.default_max_cycles);

    #[cfg(feature = "native-service")]
    let mut service_listener: Option<ServiceListener> = None;

    #[cfg(feature = "native-service")]
    if let Some(service_cfg) = config.service {
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
                write_console_line(&mut emulator.bus, &msg);
            }
        }
    }

    let initial_status = emulator.status_message();

    let mut app = App::new();
    app.insert_non_send_resource(emulator);
    app.insert_resource(UiState::with_status(initial_status));
    app.insert_resource(ClearColor(Color::rgb(0.05, 0.05, 0.08)));

    #[cfg(feature = "native-service")]
    if let Some(listener) = service_listener {
        app.insert_resource(listener);
    }

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

struct EmulatorState {
    bus: Bus,
    console_output: Arc<Mutex<ConsoleOutput>>,
    default_max_cycles: u64,
    status_message: Option<String>,
}

impl EmulatorState {
    fn new(startup: Option<StartupConfig>, default_max_cycles: u64) -> Self {
        let mut bus = Bus::new();
        let status_message = if let Some(config) = startup {
            match run_program_with_config(bus, &config) {
                Ok((new_bus, msg)) => {
                    bus = new_bus;
                    Some(msg)
                }
                Err((mut new_bus, msg)) => {
                    write_console_line(&mut new_bus, &msg);
                    bus = new_bus;
                    Some(msg)
                }
            }
        } else {
            write_console_line(&mut bus, WELCOME_MESSAGE);
            Some(WELCOME_MESSAGE.to_string())
        };

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

    #[cfg(feature = "native-service")]
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
    #[cfg(feature = "native-service")] service_listener: Option<Res<ServiceListener>>,
) {
    #[cfg(feature = "native-service")]
    if let Some(listener) = service_listener {
        while let Ok(envelope) = listener.receiver.try_recv() {
            let response = emulator.handle_service_command(envelope.command);
            ui_state.status = Some(response.message.clone());
            let _ = envelope.respond_to.send(response);
        }
    }

    let ctx = contexts.ctx_mut();

    egui::TopBottomPanel::top("top_panel")
        .resizable(false)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                #[cfg(feature = "native-file-dialog")]
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

                #[cfg(not(feature = "native-file-dialog"))]
                {
                    ui.add_enabled(false, egui::Button::new("Load PRG..."));
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

#[cfg(feature = "native-service")]
#[derive(Resource)]
struct ServiceListener {
    receiver: Receiver<ServiceEnvelope>,
}

#[cfg(feature = "native-service")]
enum ServiceCommand {
    RunProgram {
        source: ProgramSource,
        max_cycles: Option<u64>,
        start: Option<u16>,
    },
}

#[cfg(feature = "native-service")]
struct ServiceEnvelope {
    command: ServiceCommand,
    respond_to: Sender<ServiceResponseMessage>,
}

#[cfg(feature = "native-service")]
#[derive(Serialize)]
struct ServiceResponseMessage {
    status: ServiceStatus,
    message: String,
}

#[cfg(feature = "native-service")]
impl ServiceResponseMessage {
    fn ok(message: impl Into<String>) -> Self {
        Self {
            status: ServiceStatus::Ok,
            message: message.into(),
        }
    }

    fn error(message: impl Into<String>) -> Self {
        Self {
            status: ServiceStatus::Error,
            message: message.into(),
        }
    }
}

#[cfg(feature = "native-service")]
#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
enum ServiceStatus {
    Ok,
    Error,
}

#[cfg(feature = "native-service")]
#[derive(Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
enum ServiceRequestPayload {
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

#[cfg(feature = "native-service")]
impl ServiceRequestPayload {
    fn into_command(self) -> Result<ServiceCommand, String> {
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
