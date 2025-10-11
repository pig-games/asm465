use std::fs;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use bus::console_mmio::{ConsoleOutput, ConsoleSnapshot};
use bus::Bus;
use clap::Parser;
use core6502::Cpu;
use crossbeam_channel::{Receiver, Sender};
use eframe::egui::{self, text::LayoutJob, text::TextFormat, Color32, Context, FontId};
use serde::{Deserialize, Serialize};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Optional 6502 PRG to execute before the console window opens.
    #[arg(long)]
    prg: Option<PathBuf>,

    /// Maximum number of CPU cycles to run the startup program for.
    #[arg(long, default_value_t = 5_000_000u64)]
    max_cycles: u64,

    /// Optional address to jump to instead of the PRG's load address.
    #[arg(long)]
    start: Option<u16>,

    /// Optional TCP port to expose the JSON service API on.
    #[arg(long)]
    service_port: Option<u16>,

    /// Host/interface to bind the JSON service API on.
    #[arg(long, default_value = "127.0.0.1")]
    service_host: String,

    /// Optional positional PRG path (shorthand for `--prg`).
    #[arg(conflicts_with = "prg")]
    program: Option<PathBuf>,
}

/// Source for a PRG to be executed.
#[derive(Clone)]
enum ProgramSource {
    File(PathBuf),
    Inline { name: Option<String>, data: Vec<u8> },
}

impl ProgramSource {
    fn load_bytes(&self) -> Result<Vec<u8>, String> {
        match self {
            ProgramSource::File(path) => {
                fs::read(path).map_err(|err| format!("Failed to read {}: {err}", path.display()))
            }
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

/// Arguments controlling automatic execution at startup.
#[derive(Clone)]
struct StartupConfig {
    /// Program source (file path or inline bytes).
    source: ProgramSource,
    /// How many CPU cycles to execute before presenting the window.
    max_cycles: u64,
    /// Optional override for the program counter after reset.
    start: Option<u16>,
}

/// Commands that can be issued via the external service API.
enum ServiceCommand {
    RunProgram {
        source: ProgramSource,
        max_cycles: Option<u64>,
        start: Option<u16>,
    },
}

/// Envelope sent from the service listener into the UI thread.
struct ServiceEnvelope {
    command: ServiceCommand,
    respond_to: Sender<ServiceResponseMessage>,
}

/// JSON payload accepted by the service listener.
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

/// Response shape returned to service clients.
#[derive(Serialize)]
struct ServiceResponseMessage {
    status: ServiceStatus,
    message: String,
}

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

/// Response status indicator (serialises to `"ok"` or `"error"`).
#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
enum ServiceStatus {
    Ok,
    Error,
}

/// egui application responsible for rendering the mirrored console.
struct Asm465App {
    /// Owning bus instance so we can forward MMIO writes.
    bus: Bus,
    /// Shared console surface mirrored from the MMIO device.
    console_output: Arc<Mutex<ConsoleOutput>>,
    /// Current text the user is preparing to send.
    input_buffer: String,
    /// Optional one-shot status banner (startup summary/error).
    status_message: Option<String>,
    /// Default cycle budget for program execution (used when commands omit it).
    default_max_cycles: u64,
    /// Optional receiver for service API commands.
    service_rx: Option<Receiver<ServiceEnvelope>>,
    /// egui context so background work can trigger repaints.
    egui_ctx: Context,
}

impl Asm465App {
    fn new(
        cc: &eframe::CreationContext<'_>,
        startup: Option<StartupConfig>,
        default_max_cycles: u64,
        service_host: String,
        service_port: Option<u16>,
    ) -> Self {
        let mut bus = Bus::new();
        let mut status_message = None;
        let egui_ctx = cc.egui_ctx.clone();

        if let Some(config) = startup {
            match run_startup_program(bus, &config) {
                Ok((new_bus, msg)) => {
                    status_message = Some(msg);
                    bus = new_bus;
                }
                Err((mut new_bus, msg)) => {
                    write_console_line(&mut new_bus, &msg);
                    status_message = Some(msg);
                    bus = new_bus;
                }
            }
        } else {
            write_console_line(&mut bus, "Welcome to the asm465 console viewer!");
        }

        let console_output = bus
            .console_output_handle()
            .expect("default console MMIO not found on bus");

        let service_rx = match service_port {
            Some(port) => match start_service_listener(&service_host, port, egui_ctx.clone()) {
                Ok(rx) => Some(rx),
                Err(err) => {
                    let msg =
                        format!("Failed to start service listener on {service_host}:{port}: {err}");
                    write_console_line(&mut bus, &msg);
                    status_message = Some(msg);
                    None
                }
            },
            None => None,
        };

        Self {
            bus,
            console_output,
            input_buffer: String::new(),
            status_message,
            default_max_cycles,
            service_rx,
            egui_ctx,
        }
    }

    /// Helper for pushing a line of host text through the MMIO console path.
    fn write_line(&mut self, line: &str) {
        write_console_line(&mut self.bus, line);
    }

    fn process_service_messages(&mut self) {
        let Some(rx_ref) = self.service_rx.as_ref() else {
            return;
        };
        let rx = rx_ref.clone();
        let mut handled = false;
        while let Ok(envelope) = rx.try_recv() {
            self.handle_service_envelope(envelope);
            handled = true;
        }
        if handled {
            self.egui_ctx.request_repaint();
        }
    }

    fn handle_service_envelope(&mut self, envelope: ServiceEnvelope) {
        let ServiceEnvelope {
            command,
            respond_to,
        } = envelope;
        let response = match command {
            ServiceCommand::RunProgram {
                source,
                max_cycles,
                start,
            } => match self.execute_program(source, max_cycles, start) {
                Ok(msg) => ServiceResponseMessage::ok(msg),
                Err(err) => ServiceResponseMessage::error(err),
            },
        };
        let _ = respond_to.send(response);
    }

    fn execute_program(
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
        match run_startup_program(bus, &config) {
            Ok((new_bus, msg)) => {
                self.status_message = Some(msg.clone());
                self.bus = new_bus;
                self.console_output = self
                    .bus
                    .console_output_handle()
                    .expect("default console MMIO not found on bus");
                self.egui_ctx.request_repaint();
                Ok(msg)
            }
            Err((mut new_bus, msg)) => {
                write_console_line(&mut new_bus, &msg);
                self.status_message = Some(msg.clone());
                self.bus = new_bus;
                self.console_output = self
                    .bus
                    .console_output_handle()
                    .expect("default console MMIO not found on bus");
                self.egui_ctx.request_repaint();
                Err(msg)
            }
        }
    }
}

impl eframe::App for Asm465App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.process_service_messages();
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Console Output");

            if let Some(msg) = &self.status_message {
                ui.label(msg);
                ui.separator();
            }

            let snapshot = self.console_output.lock().map(|out| out.snapshot()).ok();

            if let Some(snapshot) = snapshot {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let job = snapshot_to_layout(&snapshot);
                        ui.label(job);
                    });
            }

            ui.separator();

            ui.horizontal(|ui| {
                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.input_buffer)
                        .hint_text("Type text to send to the console"),
                );

                let enter_pressed =
                    response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));

                if enter_pressed || ui.button("Send").clicked() {
                    let line = self.input_buffer.trim_end_matches('\n').to_owned();
                    if !line.is_empty() {
                        self.write_line(&line);
                    }
                    self.input_buffer.clear();
                    ui.ctx().request_repaint();
                }

                if ui.button("Clear").clicked() {
                    self.bus.clear_console_buffer();
                    ui.ctx().request_repaint();
                }
            });
        });
    }
}

/// Push a line of host text through the MMIO console path.
fn write_console_line(bus: &mut Bus, line: &str) {
    for byte in line.as_bytes() {
        bus.write(0xDF00, *byte);
    }
    bus.write(0xDF01, 0);
}

/// Convert a console snapshot into an egui layout that preserves colour/spacing.
fn snapshot_to_layout(snapshot: &ConsoleSnapshot) -> LayoutJob {
    let mut job = LayoutJob::default();
    let font = FontId::monospace(16.0);
    let mut newline_format = TextFormat::default();
    newline_format.font_id = font.clone();

    for y in 0..snapshot.height {
        for x in 0..snapshot.width {
            let cell = snapshot.cell(x, y);
            let mut format = TextFormat::default();
            format.font_id = font.clone();
            format.color = palette_color(cell.fg);
            format.background = palette_color(cell.bg);
            let text = cell.ch.to_string();
            job.append(&text, 0.0, format);
        }
        if y + 1 < snapshot.height {
            job.append("\n", 0.0, newline_format.clone());
        }
    }

    job
}

/// Translate a cross465 colour index into an egui `Color32`.
fn palette_color(index: u8) -> Color32 {
    const PALETTE: [Color32; 16] = [
        Color32::from_rgb(0x00, 0x00, 0x00), // Black
        Color32::from_rgb(0xFF, 0xFF, 0xFF), // White
        Color32::from_rgb(0x88, 0x00, 0x00), // Red
        Color32::from_rgb(0xAA, 0xFF, 0xEE), // Cyan
        Color32::from_rgb(0xCC, 0x44, 0xCC), // Magenta
        Color32::from_rgb(0x00, 0xCC, 0x55), // Green
        Color32::from_rgb(0x00, 0x00, 0xAA), // Blue
        Color32::from_rgb(0xEE, 0xEE, 0x77), // Yellow
        Color32::from_rgb(0xDD, 0x88, 0x55), // Orange
        Color32::from_rgb(0x66, 0x44, 0x00), // Brown
        Color32::from_rgb(0xFF, 0x77, 0x77), // Light red
        Color32::from_rgb(0xAA, 0xFF, 0xEE), // Light cyan
        Color32::from_rgb(0xFF, 0xAA, 0xFF), // Light magenta
        Color32::from_rgb(0xAA, 0xFF, 0xAA), // Light green
        Color32::from_rgb(0xAA, 0xCC, 0xFF), // Light blue
        Color32::from_rgb(0xCC, 0xCC, 0xCC), // Light gray
    ];
    PALETTE[index as usize & 0x0F]
}

fn start_service_listener(
    host: &str,
    port: u16,
    ctx: Context,
) -> std::io::Result<Receiver<ServiceEnvelope>> {
    let (tx, rx) = crossbeam_channel::unbounded();
    let listener = TcpListener::bind((host, port))?;
    thread::spawn(move || {
        if let Err(err) = run_service_listener(listener, tx, ctx) {
            eprintln!("service listener error: {err}");
        }
    });
    Ok(rx)
}

fn run_service_listener(
    listener: TcpListener,
    tx: Sender<ServiceEnvelope>,
    ctx: Context,
) -> std::io::Result<()> {
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let tx = tx.clone();
                let ctx = ctx.clone();
                thread::spawn(move || {
                    if let Err(err) = handle_service_connection(stream, tx, ctx) {
                        eprintln!("service client error: {err}");
                    }
                });
            }
            Err(err) => eprintln!("service accept error: {err}"),
        }
    }
    Ok(())
}

fn handle_service_connection(
    stream: TcpStream,
    tx: Sender<ServiceEnvelope>,
    ctx: Context,
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
        ctx.request_repaint();
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

fn write_service_response<W: Write>(
    writer: &mut W,
    response: ServiceResponseMessage,
) -> std::io::Result<()> {
    serde_json::to_writer(&mut *writer, &response)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))?;
    writer.write_all(b"\n")?;
    writer.flush()
}

/// Load a PRG, execute it for a fixed cycle budget, and return a status summary.
fn run_startup_program(
    mut bus: Bus,
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
    bus.load(load_addr, body);
    let start = config.start.unwrap_or(load_addr);
    bus.set_reset_vector(start);

    let mut cpu = Cpu::new(bus);
    cpu.reset();
    cpu.run_for(config.max_cycles);
    let bus = cpu.bus;

    let summary = format!(
        "Loaded {label} at ${:04X} and ran for {} cycles (start=${:04X})",
        load_addr, config.max_cycles, start
    );

    Ok((bus, summary))
}

fn main() -> eframe::Result<()> {
    let args = Args::parse();
    let startup_path = args.prg.clone().or_else(|| args.program.clone());
    let startup = startup_path.map(|path| StartupConfig {
        source: ProgramSource::File(path),
        max_cycles: args.max_cycles,
        start: args.start,
    });
    let default_max_cycles = args.max_cycles;
    let service_port = args.service_port;
    let service_host = args.service_host.clone();

    let options = eframe::NativeOptions::default();
    let startup_cfg = startup.clone();
    eframe::run_native(
        "asm465 Console",
        options,
        Box::new(move |cc| {
            Box::new(Asm465App::new(
                cc,
                startup_cfg.clone(),
                default_max_cycles,
                service_host.clone(),
                service_port,
            ))
        }),
    )
}
