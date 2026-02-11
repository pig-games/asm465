//! Target execution helpers for the Cross465 RTST runner.
//!
//! Bridges assembled PRGs into the in-process emulator (`bus` + `core6502`),
//! polls RTST headers until completion, and surfaces the captured region and
//! cycle count back to the runner.

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use bus::Bus;
use runtime_sdk::rtst::{
    Header, RtstError, BASE_LAYOUT_CROSS465, BASE_LAYOUT_MEGA65, BASE_LAYOUT_ULTIMATE64, HEADER_LEN,
};
use serde::Deserialize;
use serde_json::json;
use std::{
    env, fs,
    io::{BufRead, BufReader, Read, Write},
    net::{TcpStream, ToSocketAddrs},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::Mutex,
    thread,
    time::{Duration, Instant},
};
use tempfile::{NamedTempFile, TempPath};
use ureq::{Agent, AgentBuilder, Error as UreqError, Response};

use crate::RunnerError;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
/// Targets supported by the runner backend.
pub enum TargetKind {
    Cross465,
    Ultimate64,
    Mega65,
    Asm465Native,
    Asm465Wasm,
}

/// Configuration for a single execution attempt.
#[derive(Clone, Copy, Debug)]
pub struct ExecutionConfig {
    pub target: TargetKind,
    pub timeout_ms: u64,
    pub progress_timeout_ms: u64,
    pub transport_retries: u32,
    pub capture_console: bool,
    pub capture_display: bool,
    pub capture_overlay: bool,
}

/// Optional endpoint/runtime overrides threaded from CLI or CI matrix entries.
#[derive(Clone, Debug, Default)]
pub struct BackendOverrides {
    pub ultimate64_host: Option<String>,
    pub ultimate64_port: Option<u16>,
    pub asm465_native_host: Option<String>,
    pub asm465_native_port: Option<u16>,
    pub asm465_bridge_host: Option<String>,
    pub asm465_bridge_port: Option<u16>,
    pub asm465_ws_host: Option<String>,
    pub asm465_ws_port: Option<u16>,
    pub asm465_max_cycles: Option<u64>,
    pub asm465_keep_alive: Option<bool>,
}

/// Results captured from executing a PRG.
#[derive(Debug)]
pub struct ExecutionOutput {
    pub rtst_region: Vec<u8>,
    pub cycles: u64,
    pub debug: ExecutionDebug,
}

#[derive(Clone, Debug, Default)]
/// Optional MMIO debug snapshots returned by a backend.
pub struct ExecutionDebug {
    pub console_log: Option<String>,
    pub display: Option<DisplaySample>,
    pub overlay: Option<OverlaySample>,
}

#[derive(Clone, Copy, Debug, Default)]
/// Simple palette snapshot captured from display MMIO.
pub struct DisplaySample {
    pub border_color: u8,
    pub background_color: u8,
}

#[derive(Clone, Copy, Debug, Default)]
/// Optional overlay/raster snapshot captured from the target.
pub struct OverlaySample {
    pub raster: u16,
    pub sprite_collisions: u8,
    pub background_collisions: u8,
}

struct RtstProgressTracker {
    progress_deadline: Option<Duration>,
    initialized: bool,
    last_header: Option<Header>,
    last_wpos: u16,
    last_progress: Instant,
}

impl RtstProgressTracker {
    fn new(progress_deadline: Option<Duration>) -> Self {
        Self {
            progress_deadline,
            initialized: false,
            last_header: None,
            last_wpos: 0,
            last_progress: Instant::now(),
        }
    }

    fn state(&self) -> Option<runtime_sdk::rtst::State> {
        self.last_header.as_ref().map(|header| header.state())
    }

    fn wpos(&self) -> u16 {
        self.last_wpos
    }

    fn enforce_progress_deadline(&self) -> Result<(), RunnerError> {
        if self.initialized {
            if let Some(limit) = self.progress_deadline {
                if self.last_progress.elapsed() >= limit {
                    return Err(RunnerError::NoProgress {
                        state: self.state(),
                        wpos: self.last_wpos,
                        elapsed_ms: self.last_progress.elapsed().as_millis() as u64,
                    });
                }
            }
        }
        Ok(())
    }

    fn observe_header(&mut self, header: Header) {
        if !self.initialized {
            self.initialized = true;
            self.last_progress = Instant::now();
        }

        if header.write_pos() != self.last_wpos {
            self.last_wpos = header.write_pos();
            self.last_progress = Instant::now();
        } else if self.last_header.as_ref().map(|prev| prev.state()) != Some(header.state()) {
            self.last_progress = Instant::now();
        }

        self.last_header = Some(header);
    }

    fn handle_parse_error(&self, err: RtstError, timed_out: bool) -> Result<(), RunnerError> {
        if !self.initialized {
            if let Some(limit) = self.progress_deadline {
                if self.last_progress.elapsed() >= limit {
                    return Err(RunnerError::ProtocolInit { source: err });
                }
            }
            if timed_out {
                return Err(RunnerError::RtstParse(err));
            }
            Ok(())
        } else {
            Err(RunnerError::RtstParse(err))
        }
    }
}

fn poll_rtst_loop<F>(
    mut read_header: F,
    timeout: Option<Duration>,
    progress_deadline: Option<Duration>,
    poll_delay: Duration,
) -> Result<(), RunnerError>
where
    F: FnMut() -> Result<Vec<u8>, RunnerError>,
{
    let start = Instant::now();
    let mut tracker = RtstProgressTracker::new(progress_deadline);

    loop {
        tracker.enforce_progress_deadline()?;

        let header_bytes = read_header()?;
        match Header::parse(&header_bytes) {
            Ok(header) => {
                tracker.observe_header(header);
                if header.state().is_terminal() {
                    return Ok(());
                }
                if let Some(limit) = timeout {
                    if start.elapsed() >= limit {
                        return Err(RunnerError::Timeout {
                            cycles: 0,
                            state: tracker.state(),
                            wpos: tracker.wpos(),
                            pc: 0,
                        });
                    }
                }
            }
            Err(err) => {
                let timed_out = timeout
                    .map(|limit| start.elapsed() >= limit)
                    .unwrap_or(false);
                tracker.handle_parse_error(err, timed_out)?;
            }
        }

        thread::sleep(poll_delay);
    }
}

/// Backend contract for running RTST-enabled binaries.
pub trait TargetBackend {
    fn kind(&self) -> TargetKind;
    fn run(&self, prg: &[u8], cfg: ExecutionConfig) -> Result<ExecutionOutput, RunnerError>;
}

/// Cross465 emulator backend using the in-process CPU implementation.
#[derive(Debug)]
pub struct Cross465Backend;

impl Cross465Backend {
    pub fn new() -> Self {
        Self
    }

    fn execute(&self, prg: &[u8], cfg: ExecutionConfig) -> Result<ExecutionOutput, RunnerError> {
        let base = match cfg.target {
            TargetKind::Cross465 | TargetKind::Asm465Native | TargetKind::Asm465Wasm => {
                BASE_LAYOUT_CROSS465
            }
            TargetKind::Ultimate64 => BASE_LAYOUT_ULTIMATE64,
            TargetKind::Mega65 => BASE_LAYOUT_MEGA65,
        };
        if base.address > 0xFFFF {
            return Err(RunnerError::UnsupportedTarget(format!(
                "{}",
                cfg.target.to_string()
            )));
        }
        let (load_addr, body) = parse_prg(prg)?;
        let mut bus = Bus::new();
        bus.load(load_addr, body);
        bus.set_reset_vector(load_addr);
        let mut cpu = core6502::Cpu::new(bus);
        cpu.reset();

        let mut cycles: u64 = 0;
        let poll_interval: u64 = 1024;
        let cycle_budget = cfg.timeout_ms.saturating_mul(1_000) as u64;
        let base_addr = base.address as u16;
        let progress_deadline = if cfg.progress_timeout_ms == 0 {
            None
        } else {
            Some(Duration::from_millis(cfg.progress_timeout_ms))
        };
        let mut tracker = RtstProgressTracker::new(progress_deadline);

        loop {
            let step_cycles = cpu.step() as u64;
            cycles = cycles.saturating_add(step_cycles);
            if cycle_budget > 0 && cycles > cycle_budget {
                return Err(RunnerError::Timeout {
                    cycles,
                    state: tracker.state(),
                    wpos: tracker.wpos(),
                    pc: cpu.pc,
                });
            }
            tracker.enforce_progress_deadline()?;
            if cycles % poll_interval != 0 {
                continue;
            }
            let header_bytes = read_bytes(cpu.bus_mut(), base_addr, HEADER_LEN);
            match Header::parse(&header_bytes) {
                Ok(header) => {
                    tracker.observe_header(header);
                    if header.state().is_terminal() {
                        break;
                    }
                }
                Err(err) => {
                    tracker.handle_parse_error(err, false)?;
                }
            }
        }

        let mut region = vec![0u8; base.span.min(0x10000 - base.address as usize)];
        for (offset, byte) in region.iter_mut().enumerate() {
            *byte = cpu.bus_mut().read(base_addr.wrapping_add(offset as u16));
        }

        let mut debug = ExecutionDebug::default();
        if cfg.capture_console {
            debug.console_log = cpu.bus().console_buffer();
        }
        if cfg.capture_display {
            if let Some(handle) = cpu.bus().display_output_handle() {
                if let Ok(output) = handle.lock() {
                    let snapshot = output.snapshot();
                    debug.display = Some(DisplaySample {
                        border_color: snapshot.border_color,
                        background_color: snapshot.background_color,
                    });
                }
            }
        }

        Ok(ExecutionOutput {
            rtst_region: region,
            cycles,
            debug,
        })
    }
}

impl Default for Cross465Backend {
    fn default() -> Self {
        Self::new()
    }
}

impl TargetBackend for Cross465Backend {
    fn kind(&self) -> TargetKind {
        TargetKind::Cross465
    }

    fn run(&self, prg: &[u8], cfg: ExecutionConfig) -> Result<ExecutionOutput, RunnerError> {
        self.execute(prg, cfg)
    }
}

/// Ultimate64 backend configuration (HTTP host/port/polling).
#[derive(Clone, Debug)]
pub struct Ultimate64BackendConfig {
    pub host: String,
    pub port: u16,
    pub connect_timeout: Duration,
    pub read_timeout: Duration,
    pub poll_delay: Duration,
    pub retries: usize,
}

impl Default for Ultimate64BackendConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8080,
            connect_timeout: Duration::from_secs(2),
            read_timeout: Duration::from_secs(5),
            poll_delay: Duration::from_millis(200),
            retries: 3,
        }
    }
}

impl Ultimate64BackendConfig {
    pub fn from_env() -> Self {
        let mut cfg = Self::default();
        if let Ok(host) = env::var("CROSS465_ULTIMATE64_HOST") {
            cfg.host = host;
        }
        if let Ok(port_raw) = env::var("CROSS465_ULTIMATE64_PORT") {
            if let Ok(port) = port_raw.parse::<u16>() {
                cfg.port = port;
            }
        }
        if let Ok(ms_raw) = env::var("CROSS465_ULTIMATE64_POLL_DELAY_MS") {
            if let Ok(ms) = ms_raw.parse::<u64>() {
                cfg.poll_delay = Duration::from_millis(ms);
            }
        }
        if let Ok(ms_raw) = env::var("CROSS465_ULTIMATE64_CONNECT_TIMEOUT_MS") {
            if let Ok(ms) = ms_raw.parse::<u64>() {
                cfg.connect_timeout = Duration::from_millis(ms);
            }
        }
        if let Ok(ms_raw) = env::var("CROSS465_ULTIMATE64_READ_TIMEOUT_MS") {
            if let Ok(ms) = ms_raw.parse::<u64>() {
                cfg.read_timeout = Duration::from_millis(ms);
            }
        }
        if let Ok(retries_raw) = env::var("CROSS465_ULTIMATE64_RETRIES") {
            if let Ok(retries) = retries_raw.parse::<usize>() {
                cfg.retries = retries.max(1);
            }
        }
        cfg
    }

    pub fn from_sources(overrides: &BackendOverrides) -> Self {
        let mut cfg = Self::from_env();
        if let Some(host) = &overrides.ultimate64_host {
            cfg.host = host.clone();
        }
        if let Some(port) = overrides.ultimate64_port {
            cfg.port = port;
        }
        cfg
    }
}

/// REST backend for Ultimate64 hardware.
#[derive(Debug)]
pub struct Ultimate64Backend {
    config: Ultimate64BackendConfig,
}

impl Ultimate64Backend {
    pub fn new(config: Ultimate64BackendConfig) -> Self {
        Self { config }
    }

    fn execute(&self, prg: &[u8], cfg: ExecutionConfig) -> Result<ExecutionOutput, RunnerError> {
        let layout = match cfg.target {
            TargetKind::Cross465 | TargetKind::Asm465Native | TargetKind::Asm465Wasm => {
                BASE_LAYOUT_CROSS465
            }
            TargetKind::Ultimate64 => BASE_LAYOUT_ULTIMATE64,
            TargetKind::Mega65 => BASE_LAYOUT_MEGA65,
        };
        let _ = parse_prg(prg)?;
        let timeout = if cfg.timeout_ms == 0 {
            None
        } else {
            Some(Duration::from_millis(cfg.timeout_ms))
        };
        let progress_deadline = if cfg.progress_timeout_ms == 0 {
            None
        } else {
            Some(Duration::from_millis(cfg.progress_timeout_ms))
        };
        let client = Ultimate64Client::connect(&self.config)?;
        self.retry_transport(|| client.run_program(prg), cfg.transport_retries)?;
        poll_rtst_loop(
            || {
                self.retry_transport(
                    || client.read_memory(layout.address, HEADER_LEN),
                    cfg.transport_retries,
                )
            },
            timeout,
            progress_deadline,
            self.config.poll_delay,
        )?;
        let rtst = self.retry_transport(
            || client.read_memory(layout.address, layout.span),
            cfg.transport_retries,
        )?;
        Ok(ExecutionOutput {
            rtst_region: rtst,
            cycles: 0,
            debug: ExecutionDebug::default(),
        })
    }

    fn retry_transport<T, F>(&self, mut op: F, retries: u32) -> Result<T, RunnerError>
    where
        F: FnMut() -> Result<T, RunnerError>,
    {
        let mut attempts = 0;
        loop {
            match op() {
                Ok(value) => return Ok(value),
                Err(err) => {
                    if matches!(err, RunnerError::Ultimate64Error { .. }) && attempts < retries {
                        attempts += 1;
                        thread::sleep(self.config.poll_delay);
                        continue;
                    }
                    return Err(err);
                }
            }
        }
    }
}

impl Default for Ultimate64Backend {
    fn default() -> Self {
        Self::new(Ultimate64BackendConfig::from_sources(
            &BackendOverrides::default(),
        ))
    }
}

impl TargetBackend for Ultimate64Backend {
    fn kind(&self) -> TargetKind {
        TargetKind::Ultimate64
    }

    fn run(&self, prg: &[u8], cfg: ExecutionConfig) -> Result<ExecutionOutput, RunnerError> {
        let mut last_err = None;
        for attempt in 0..self.config.retries {
            match self.execute(prg, cfg) {
                Ok(output) => return Ok(output),
                Err(err) => {
                    last_err = Some(err);
                    if attempt + 1 < self.config.retries {
                        thread::sleep(Duration::from_millis(250));
                    }
                }
            }
        }
        Err(last_err.unwrap_or_else(|| RunnerError::Ultimate64Error {
            message: "unable to execute program on Ultimate64".to_string(),
        }))
    }
}

/// MEGA65 backend configuration (m65 CLI path, serial options).
#[derive(Clone, Debug)]
pub struct Mega65BackendConfig {
    pub m65_path: PathBuf,
    pub serial_port: Option<String>,
    pub baud_rate: Option<u32>,
    pub poll_delay: Duration,
    pub retries: usize,
}

impl Default for Mega65BackendConfig {
    fn default() -> Self {
        Self {
            m65_path: PathBuf::from("m65"),
            serial_port: None,
            baud_rate: None,
            poll_delay: Duration::from_millis(200),
            retries: 3,
        }
    }
}

impl Mega65BackendConfig {
    pub fn from_env() -> Self {
        let mut cfg = Self::default();
        if let Ok(path) = env::var("CROSS465_MEGA65_M65_PATH") {
            if !path.trim().is_empty() {
                cfg.m65_path = PathBuf::from(path);
            }
        }
        if let Ok(port) = env::var("CROSS465_MEGA65_SERIAL") {
            if !port.trim().is_empty() {
                cfg.serial_port = Some(port);
            }
        }
        if let Ok(baud_raw) = env::var("CROSS465_MEGA65_BAUD") {
            if let Ok(baud) = baud_raw.parse::<u32>() {
                cfg.baud_rate = Some(baud);
            }
        }
        if let Ok(ms_raw) = env::var("CROSS465_MEGA65_POLL_DELAY_MS") {
            if let Ok(ms) = ms_raw.parse::<u64>() {
                cfg.poll_delay = Duration::from_millis(ms);
            }
        }
        if let Ok(retries_raw) = env::var("CROSS465_MEGA65_RETRIES") {
            if let Ok(retries) = retries_raw.parse::<usize>() {
                cfg.retries = retries.max(1);
            }
        }
        cfg
    }
}

/// Backend implementation that shells out to the `m65` CLI.
#[derive(Debug)]
pub struct Mega65Backend {
    config: Mega65BackendConfig,
}

impl Mega65Backend {
    pub fn new(config: Mega65BackendConfig) -> Self {
        Self { config }
    }

    fn retry_transport<T, F>(&self, mut op: F, retries: u32) -> Result<T, RunnerError>
    where
        F: FnMut() -> Result<T, RunnerError>,
    {
        let mut attempts = 0;
        loop {
            match op() {
                Ok(value) => return Ok(value),
                Err(err) => {
                    if matches!(err, RunnerError::Mega65Error { .. }) && attempts < retries {
                        attempts += 1;
                        thread::sleep(self.config.poll_delay);
                        continue;
                    }
                    return Err(err);
                }
            }
        }
    }

    fn execute(&self, prg: &[u8], cfg: ExecutionConfig) -> Result<ExecutionOutput, RunnerError> {
        let layout = match cfg.target {
            TargetKind::Cross465 | TargetKind::Asm465Native | TargetKind::Asm465Wasm => {
                BASE_LAYOUT_CROSS465
            }
            TargetKind::Ultimate64 => BASE_LAYOUT_ULTIMATE64,
            TargetKind::Mega65 => BASE_LAYOUT_MEGA65,
        };
        let _ = parse_prg(prg)?;
        let mut temp_prg = NamedTempFile::new().map_err(|e| RunnerError::Mega65Error {
            message: format!("failed to create temp program: {e}"),
        })?;
        temp_prg
            .write_all(prg)
            .map_err(|e| RunnerError::Mega65Error {
                message: format!("failed to write temp program: {e}"),
            })?;
        let temp_path = temp_prg.into_temp_path();
        let client = Mega65Client::new(&self.config);
        self.retry_transport(
            || client.run_program(temp_path.as_ref()),
            cfg.transport_retries,
        )?;
        let _ = temp_path.close();

        let timeout = if cfg.timeout_ms == 0 {
            None
        } else {
            Some(Duration::from_millis(cfg.timeout_ms))
        };
        let progress_deadline = if cfg.progress_timeout_ms == 0 {
            None
        } else {
            Some(Duration::from_millis(cfg.progress_timeout_ms))
        };
        poll_rtst_loop(
            || {
                self.retry_transport(
                    || client.read_memory(layout.address, HEADER_LEN),
                    cfg.transport_retries,
                )
            },
            timeout,
            progress_deadline,
            self.config.poll_delay,
        )?;
        let rtst = self.retry_transport(
            || client.read_memory(layout.address, layout.span),
            cfg.transport_retries,
        )?;
        Ok(ExecutionOutput {
            rtst_region: rtst,
            cycles: 0,
            debug: ExecutionDebug::default(),
        })
    }
}

impl Default for Mega65Backend {
    fn default() -> Self {
        Self::new(Mega65BackendConfig::from_env())
    }
}

impl TargetBackend for Mega65Backend {
    fn kind(&self) -> TargetKind {
        TargetKind::Mega65
    }

    fn run(&self, prg: &[u8], cfg: ExecutionConfig) -> Result<ExecutionOutput, RunnerError> {
        let mut last_err = None;
        for attempt in 0..self.config.retries {
            match self.execute(prg, cfg) {
                Ok(output) => return Ok(output),
                Err(err) => {
                    last_err = Some(err);
                    if attempt + 1 < self.config.retries {
                        thread::sleep(self.config.poll_delay);
                    }
                }
            }
        }
        Err(last_err.unwrap_or_else(|| RunnerError::Mega65Error {
            message: "unable to execute program on MEGA65".to_string(),
        }))
    }
}

/// Select an appropriate backend implementation for the requested target.
pub fn backend_for_target(
    target: TargetKind,
    workspace_root: Option<PathBuf>,
    overrides: Option<&BackendOverrides>,
) -> Box<dyn TargetBackend> {
    let overrides = overrides.cloned().unwrap_or_default();
    match target {
        TargetKind::Cross465 => Box::new(Cross465Backend::new()),
        TargetKind::Ultimate64 => Box::new(Ultimate64Backend::new(
            Ultimate64BackendConfig::from_sources(&overrides),
        )),
        TargetKind::Mega65 => Box::new(Mega65Backend::default()),
        TargetKind::Asm465Native | TargetKind::Asm465Wasm => Box::new(Asm465Backend::new(
            Asm465BackendConfig::from_sources(target, workspace_root, &overrides),
        )),
    }
}

fn read_bytes(bus: &mut Bus, base: u16, len: usize) -> Vec<u8> {
    let mut buf = vec![0u8; len];
    for i in 0..len {
        buf[i] = bus.read(base.wrapping_add(i as u16));
    }
    buf
}

fn parse_prg(prg: &[u8]) -> Result<(u16, &[u8]), RunnerError> {
    if prg.len() < 3 {
        return Err(RunnerError::MalformedPrg);
    }
    let load_addr = u16::from_le_bytes([prg[0], prg[1]]);
    Ok((load_addr, &prg[2..]))
}

impl TargetKind {
    pub fn to_string(self) -> &'static str {
        match self {
            TargetKind::Cross465 => "cross465",
            TargetKind::Ultimate64 => "ultimate64",
            TargetKind::Mega65 => "mega65",
            TargetKind::Asm465Native => "asm465",
            TargetKind::Asm465Wasm => "asm465-wasm",
        }
    }

    pub fn as_define_suffix(self) -> &'static str {
        match self {
            TargetKind::Cross465 => "CROSS465",
            TargetKind::Ultimate64 => "ULTIMATE64",
            TargetKind::Mega65 => "MEGA65",
            TargetKind::Asm465Native => "ASM465_NATIVE",
            TargetKind::Asm465Wasm => "ASM465_WASM",
        }
    }

    pub fn all() -> [Self; 5] {
        [
            Self::Cross465,
            Self::Ultimate64,
            Self::Mega65,
            Self::Asm465Native,
            Self::Asm465Wasm,
        ]
    }
}

impl std::str::FromStr for TargetKind {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "cross465" => Ok(TargetKind::Cross465),
            "ultimate64" => Ok(TargetKind::Ultimate64),
            "mega65" => Ok(TargetKind::Mega65),
            "asm465" => Ok(TargetKind::Asm465Native),
            "asm465-wasm" => Ok(TargetKind::Asm465Wasm),
            _ => Err(()),
        }
    }
}

struct Ultimate64Client {
    agent: Agent,
    base_url: String,
}

impl Ultimate64Client {
    fn connect(cfg: &Ultimate64BackendConfig) -> Result<Self, RunnerError> {
        let agent = AgentBuilder::new()
            .timeout_connect(cfg.connect_timeout)
            .timeout_read(cfg.read_timeout)
            .timeout_write(cfg.read_timeout)
            .build();
        let base_url = if cfg.port == 80 {
            format!("http://{}", cfg.host)
        } else {
            format!("http://{}:{}", cfg.host, cfg.port)
        };
        Ok(Self { agent, base_url })
    }

    fn run_program(&self, prg: &[u8]) -> Result<(), RunnerError> {
        let url = format!("{}/v1/runners:run_prg", self.base_url);
        Self::map_response(
            self.agent
                .post(&url)
                .set("Content-Type", "application/octet-stream")
                .set("Expect", "")
                .send_bytes(prg),
            "POST /v1/runners:run_prg",
        )?;
        Ok(())
    }

    fn read_memory(&self, addr: u32, len: usize) -> Result<Vec<u8>, RunnerError> {
        let url = format!(
            "{}/v1/machine:readmem?address={:04X}&length={}",
            self.base_url, addr, len
        );
        let response = Self::map_response(self.agent.get(&url).call(), "GET /v1/machine:readmem")?;
        let mut reader = response.into_reader();
        let mut bytes = vec![0u8; len];
        reader
            .read_exact(&mut bytes)
            .map_err(|e| RunnerError::Ultimate64Error {
                message: format!("failed to read memory payload: {e}"),
            })?;
        Ok(bytes)
    }

    fn map_response(
        result: Result<Response, UreqError>,
        action: &str,
    ) -> Result<Response, RunnerError> {
        match result {
            Ok(resp) => Ok(resp),
            Err(UreqError::Status(code, response)) => {
                let body = response.into_string().unwrap_or_default();
                Err(RunnerError::Ultimate64Error {
                    message: format!("{action} returned HTTP {code}: {body}"),
                })
            }
            Err(UreqError::Transport(err)) => Err(RunnerError::Ultimate64Error {
                message: format!("{action} transport error: {err}"),
            }),
        }
    }
}

struct Mega65Client {
    config: Mega65BackendConfig,
}

impl Mega65Client {
    fn new(config: &Mega65BackendConfig) -> Self {
        Self {
            config: config.clone(),
        }
    }

    fn run_program(&self, prg_path: &Path) -> Result<(), RunnerError> {
        let mut cmd = self.base_command();
        cmd.arg("--binary").arg("--run").arg(prg_path);
        self.run_command(cmd, "run program")
    }

    fn read_memory(&self, addr: u32, len: usize) -> Result<Vec<u8>, RunnerError> {
        let temp_file = NamedTempFile::new().map_err(|e| RunnerError::Mega65Error {
            message: format!("failed to create memsave temp file: {e}"),
        })?;
        let temp_path = temp_file.into_temp_path();
        let end = addr.saturating_add(len as u32);
        let range = format!("{:X}:{:X}={}", addr, end, temp_path.display());
        let mut cmd = self.base_command();
        cmd.arg("--memsave").arg(&range);
        self.run_command(cmd, "read memory")?;
        let path = <TempPath as AsRef<Path>>::as_ref(&temp_path).to_path_buf();
        let data = fs::read(&path).map_err(|e| RunnerError::Mega65Error {
            message: format!("failed to read memsave output: {e}"),
        })?;
        match temp_path.close() {
            Ok(()) => {}
            Err(err) => {
                return Err(RunnerError::Mega65Error {
                    message: format!("failed to delete memsave temp file: {err}"),
                })
            }
        }
        if data.len() < len {
            return Err(RunnerError::Mega65Error {
                message: format!(
                    "memsave returned {} bytes but {} were requested",
                    data.len(),
                    len
                ),
            });
        }
        Ok(data)
    }

    fn base_command(&self) -> Command {
        let mut cmd = Command::new(&self.config.m65_path);
        cmd.arg("--quiet");
        if let Some(port) = &self.config.serial_port {
            cmd.arg("--device").arg(port);
        }
        if let Some(baud) = self.config.baud_rate {
            cmd.arg("--speed").arg(baud.to_string());
        }
        cmd
    }

    fn run_command(&self, mut cmd: Command, action: &str) -> Result<(), RunnerError> {
        let output = cmd.output().map_err(|err| RunnerError::Mega65Error {
            message: format!(
                "failed to invoke m65 (path: {:?}): {err}. Set CROSS465_MEGA65_M65_PATH to the CLI location",
                self.config.m65_path
            ),
        })?;
        if output.status.success() {
            return Ok(());
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let mut message = format!("m65 {action} failed");
        if !stdout.trim().is_empty() {
            message.push_str("\nstdout:\n");
            message.push_str(stdout.trim_end());
        }
        if !stderr.trim().is_empty() {
            message.push_str("\nstderr:\n");
            message.push_str(stderr.trim_end());
        }
        Err(RunnerError::Mega65Error { message })
    }
}

/// Configuration for launching/talking to the asm465 native/bridge services.
#[derive(Clone, Debug)]
pub struct Asm465BackendConfig {
    target: TargetKind,
    endpoint: Asm465EndpointConfig,
    max_cycles: u64,
    wait_after_spawn: Duration,
    retries: u32,
    retry_delay: Duration,
    poll_delay: Duration,
    workspace_root: Option<PathBuf>,
    cargo_bin: String,
    log_dir: PathBuf,
    keep_alive: bool,
}

#[derive(Clone, Debug)]
enum Asm465EndpointConfig {
    Native(NativeServiceConfig),
    Bridge(BridgeServiceConfig),
}

#[derive(Clone, Debug)]
struct NativeServiceConfig {
    host: String,
    port: u16,
}

#[derive(Clone, Debug)]
struct BridgeServiceConfig {
    tcp_host: String,
    tcp_port: u16,
    ws_host: String,
    ws_port: u16,
}

impl Asm465BackendConfig {
    pub fn from_sources(
        target: TargetKind,
        workspace_root: Option<PathBuf>,
        overrides: &BackendOverrides,
    ) -> Self {
        let max_cycles = env::var("CROSS465_MAX_CYCLES")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(5_000_000);
        let wait_secs = env::var("CROSS465_WAIT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(2);
        let retries = env::var("CROSS465_RETRIES")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(10);
        let delay_secs = env::var("CROSS465_DELAY")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1);
        let poll_ms = env::var("CROSS465_POLL_DELAY_MS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(200);
        let cargo_bin = env::var("CROSS465_CARGO_BIN").unwrap_or_else(|_| "cargo".to_string());
        let log_dir = workspace_root
            .as_ref()
            .map(|root| root.join("target/cross465-runner"))
            .unwrap_or_else(|| PathBuf::from("target/cross465-runner"));
        let keep_alive = env::var("CROSS465_ASM465_KEEP_ALIVE")
            .map(|val| val != "0")
            .unwrap_or(false);
        let endpoint = match target {
            TargetKind::Asm465Native => {
                let host =
                    env::var("CROSS465_NATIVE_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
                let port = env::var("CROSS465_NATIVE_PORT")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(7465);
                Asm465EndpointConfig::Native(NativeServiceConfig { host, port })
            }
            TargetKind::Asm465Wasm => {
                let tcp_host =
                    env::var("CROSS465_BRIDGE_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
                let tcp_port = env::var("CROSS465_BRIDGE_PORT")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(7565);
                let ws_host =
                    env::var("CROSS465_BRIDGE_WS_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
                let ws_port = env::var("CROSS465_BRIDGE_WS_PORT")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(8800);
                Asm465EndpointConfig::Bridge(BridgeServiceConfig {
                    tcp_host,
                    tcp_port,
                    ws_host,
                    ws_port,
                })
            }
            _ => Asm465EndpointConfig::Native(NativeServiceConfig {
                host: "127.0.0.1".to_string(),
                port: 7465,
            }),
        };
        Self {
            target,
            endpoint,
            max_cycles,
            wait_after_spawn: Duration::from_secs(wait_secs.max(0) as u64),
            retries: retries.max(1),
            retry_delay: Duration::from_secs(delay_secs.max(1) as u64),
            poll_delay: Duration::from_millis(poll_ms.max(1)),
            workspace_root,
            cargo_bin,
            log_dir,
            keep_alive,
        }
        .with_overrides(overrides)
    }

    fn with_overrides(mut self, overrides: &BackendOverrides) -> Self {
        if let Some(cycles) = overrides.asm465_max_cycles {
            self.max_cycles = cycles;
        }
        if let Some(keep_alive) = overrides.asm465_keep_alive {
            self.keep_alive = keep_alive;
        }

        match &mut self.endpoint {
            Asm465EndpointConfig::Native(endpoint) => {
                if let Some(host) = &overrides.asm465_native_host {
                    endpoint.host = host.clone();
                }
                if let Some(port) = overrides.asm465_native_port {
                    endpoint.port = port;
                }
            }
            Asm465EndpointConfig::Bridge(endpoint) => {
                if let Some(host) = &overrides.asm465_bridge_host {
                    endpoint.tcp_host = host.clone();
                }
                if let Some(port) = overrides.asm465_bridge_port {
                    endpoint.tcp_port = port;
                }
                if let Some(host) = &overrides.asm465_ws_host {
                    endpoint.ws_host = host.clone();
                }
                if let Some(port) = overrides.asm465_ws_port {
                    endpoint.ws_port = port;
                }
            }
        }

        self
    }
}

/// Backend that talks to the asm465 service API (native or bridge).
pub struct Asm465Backend {
    config: Asm465BackendConfig,
    native_child: Mutex<Option<Child>>,
    bridge_child: Mutex<Option<Child>>,
}

impl Asm465Backend {
    pub fn new(config: Asm465BackendConfig) -> Self {
        Self {
            config,
            native_child: Mutex::new(None),
            bridge_child: Mutex::new(None),
        }
    }

    fn ensure_service(&self) -> Result<(), RunnerError> {
        match &self.config.endpoint {
            Asm465EndpointConfig::Native(cfg) => self.ensure_native(cfg),
            Asm465EndpointConfig::Bridge(cfg) => self.ensure_bridge(cfg),
        }
    }

    fn ensure_native(&self, cfg: &NativeServiceConfig) -> Result<(), RunnerError> {
        if !host_is_loopback(&cfg.host) {
            return self.wait_for_port(&cfg.host, cfg.port);
        }
        if self.port_is_open(&cfg.host, cfg.port) {
            return Ok(());
        }
        let manifest = self.resolve_manifest("crossdev/asm465/Cargo.toml")?;
        let mut cmd = Command::new(&self.config.cargo_bin);
        cmd.arg("run")
            .arg("--manifest-path")
            .arg(&manifest)
            .arg("--")
            .arg("--service-port")
            .arg(cfg.port.to_string())
            .arg("--service-host")
            .arg(&cfg.host)
            .arg("--max-cycles")
            .arg(self.config.max_cycles.to_string());
        self.spawn_process(cmd, ServiceKind::Native, "asm465_native")?;
        self.wait_for_port(&cfg.host, cfg.port)
    }

    fn ensure_bridge(&self, cfg: &BridgeServiceConfig) -> Result<(), RunnerError> {
        if !host_is_loopback(&cfg.tcp_host) {
            return self.wait_for_port(&cfg.tcp_host, cfg.tcp_port);
        }
        if self.port_is_open(&cfg.tcp_host, cfg.tcp_port) {
            return Ok(());
        }
        let manifest = self.resolve_manifest("crossdev/asm465-server/Cargo.toml")?;
        let mut cmd = Command::new(&self.config.cargo_bin);
        cmd.arg("run")
            .arg("--manifest-path")
            .arg(&manifest)
            .arg("--")
            .arg("--tcp-host")
            .arg(&cfg.tcp_host)
            .arg("--tcp-port")
            .arg(cfg.tcp_port.to_string())
            .arg("--ws-host")
            .arg(&cfg.ws_host)
            .arg("--ws-port")
            .arg(cfg.ws_port.to_string());
        self.spawn_process(cmd, ServiceKind::Bridge, "asm465_bridge")?;
        self.wait_for_port(&cfg.tcp_host, cfg.tcp_port)?;
        self.wait_for_port(&cfg.ws_host, cfg.ws_port)
    }

    fn resolve_manifest(&self, relative: &str) -> Result<PathBuf, RunnerError> {
        let root = self
            .config
            .workspace_root
            .clone()
            .ok_or_else(|| RunnerError::Asm465Error {
                message: "workspace root required to auto-launch asm465; rerun with --workspace or start the service manually".to_string(),
            })?;
        let manifest = root.join(relative);
        if manifest.exists() {
            Ok(manifest)
        } else {
            Err(RunnerError::Asm465Error {
                message: format!("asm465 manifest not found at {}", manifest.display()),
            })
        }
    }

    fn spawn_process(
        &self,
        mut cmd: Command,
        kind: ServiceKind,
        log_prefix: &str,
    ) -> Result<(), RunnerError> {
        if let Some(root) = &self.config.workspace_root {
            cmd.current_dir(root);
        }
        fs::create_dir_all(&self.config.log_dir).map_err(|err| RunnerError::Asm465Error {
            message: format!(
                "failed to create log dir {}: {err}",
                self.config.log_dir.display()
            ),
        })?;
        let mut log_path = self.config.log_dir.clone();
        log_path.push(format!("{log_prefix}.log"));
        let log_file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map_err(|err| RunnerError::Asm465Error {
                message: format!("failed to open {}: {err}", log_path.display()),
            })?;
        let stderr_file = log_file
            .try_clone()
            .map_err(|err| RunnerError::Asm465Error {
                message: format!("failed to clone log file: {err}"),
            })?;
        cmd.stdout(Stdio::from(log_file));
        cmd.stderr(Stdio::from(stderr_file));
        let child = cmd.spawn().map_err(RunnerError::Spawn)?;
        println!(
            ">> starting {} service (pid={})",
            kind.display_name(),
            child.id()
        );
        match kind {
            ServiceKind::Native => {
                let mut slot = self.native_child.lock().unwrap();
                *slot = Some(child);
            }
            ServiceKind::Bridge => {
                let mut slot = self.bridge_child.lock().unwrap();
                *slot = Some(child);
            }
        }
        thread::sleep(self.config.wait_after_spawn);
        Ok(())
    }

    fn wait_for_port(&self, host: &str, port: u16) -> Result<(), RunnerError> {
        for _ in 0..self.config.retries {
            if self.port_is_open(host, port) {
                return Ok(());
            }
            thread::sleep(self.config.retry_delay);
        }
        Err(RunnerError::Asm465Error {
            message: format!("asm465 service at {host}:{port} did not become ready"),
        })
    }

    fn port_is_open(&self, host: &str, port: u16) -> bool {
        try_connect(host, port).is_ok()
    }

    fn client(&self) -> Asm465Client {
        match &self.config.endpoint {
            Asm465EndpointConfig::Native(cfg) => Asm465Client::new(cfg.host.clone(), cfg.port),
            Asm465EndpointConfig::Bridge(cfg) => {
                Asm465Client::new(cfg.tcp_host.clone(), cfg.tcp_port)
            }
        }
    }

    fn retry_transport<T, F>(&self, mut op: F, retries: u32) -> Result<T, RunnerError>
    where
        F: FnMut() -> Result<T, RunnerError>,
    {
        let mut attempts = 0;
        loop {
            match op() {
                Ok(value) => return Ok(value),
                Err(err) => {
                    if attempts >= retries {
                        return Err(err);
                    }
                    attempts += 1;
                    thread::sleep(self.config.retry_delay);
                }
            }
        }
    }
}

impl TargetBackend for Asm465Backend {
    fn kind(&self) -> TargetKind {
        self.config.target
    }

    fn run(&self, prg: &[u8], cfg: ExecutionConfig) -> Result<ExecutionOutput, RunnerError> {
        self.ensure_service()?;
        let client = self.client();
        let run_response = self.retry_transport(
            || client.run_program(prg, self.config.max_cycles, cfg.progress_timeout_ms),
            cfg.transport_retries,
        )?;
        let layout = BASE_LAYOUT_CROSS465;
        let timeout = if cfg.timeout_ms == 0 {
            None
        } else {
            Some(Duration::from_millis(cfg.timeout_ms))
        };
        let progress_deadline = if cfg.progress_timeout_ms == 0 {
            None
        } else {
            Some(Duration::from_millis(cfg.progress_timeout_ms))
        };
        let start = Instant::now();
        let mut initialized = false;
        let mut last_header: Option<Header> = None;
        let mut last_progress = Instant::now();
        let mut last_wpos = 0u16;
        loop {
            if initialized {
                if let Some(limit) = progress_deadline {
                    if last_progress.elapsed() >= limit {
                        return Err(RunnerError::NoProgress {
                            state: last_header.as_ref().map(|h| h.state()),
                            wpos: last_wpos,
                            elapsed_ms: last_progress.elapsed().as_millis() as u64,
                        });
                    }
                }
            }
            let header_bytes = self.retry_transport(
                || client.read_memory(layout.address as u32, HEADER_LEN),
                cfg.transport_retries,
            )?;
            let header = match Header::parse(&header_bytes) {
                Ok(header) => header,
                Err(err) => {
                    if !initialized {
                        if let Some(limit) = progress_deadline {
                            if last_progress.elapsed() >= limit {
                                return Err(RunnerError::ProtocolInit { source: err });
                            }
                        }
                        if let Some(limit) = timeout {
                            if start.elapsed() >= limit {
                                return Err(RunnerError::RtstParse(err));
                            }
                        }
                    } else {
                        return Err(RunnerError::RtstParse(err));
                    }
                    thread::sleep(self.config.poll_delay);
                    continue;
                }
            };
            if !initialized {
                initialized = true;
                last_progress = Instant::now();
            }
            if header.write_pos() != last_wpos {
                last_wpos = header.write_pos();
                last_progress = Instant::now();
            } else if last_header.as_ref().map(|prev| prev.state()) != Some(header.state()) {
                last_progress = Instant::now();
            }
            last_header = Some(header);
            if header.state().is_terminal() {
                break;
            }
            if let Some(limit) = timeout {
                if start.elapsed() >= limit {
                    return Err(RunnerError::Timeout {
                        cycles: 0,
                        state: Some(header.state()),
                        wpos: header.write_pos(),
                        pc: 0,
                    });
                }
            }
            thread::sleep(self.config.poll_delay);
        }
        let rtst = self.retry_transport(
            || client.read_memory(layout.address as u32, layout.span),
            cfg.transport_retries,
        )?;
        Ok(ExecutionOutput {
            rtst_region: rtst,
            cycles: run_response.cycles.unwrap_or(0),
            debug: ExecutionDebug::default(),
        })
    }
}

impl Drop for Asm465Backend {
    fn drop(&mut self) {
        if !self.config.keep_alive {
            terminate_child(&self.native_child);
            terminate_child(&self.bridge_child);
        }
    }
}

fn terminate_child(slot: &Mutex<Option<Child>>) {
    if let Some(mut child) = slot.lock().unwrap().take() {
        if let Ok(None) = child.try_wait() {
            let _ = child.kill();
        }
        let _ = child.wait();
    }
}

fn host_is_loopback(host: &str) -> bool {
    let normalized = host.to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "127.0.0.1" | "localhost" | "::1" | "0.0.0.0"
    )
}

fn try_connect(host: &str, port: u16) -> std::io::Result<()> {
    let addr = (host, port)
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::Other, "invalid address"))?;
    TcpStream::connect_timeout(&addr, Duration::from_millis(250)).map(|_| ())
}

struct Asm465Client {
    host: String,
    port: u16,
}

impl Asm465Client {
    fn new(host: String, port: u16) -> Self {
        Self { host, port }
    }

    fn run_program(
        &self,
        prg: &[u8],
        max_cycles: u64,
        progress_timeout_ms: u64,
    ) -> Result<ServiceResponse, RunnerError> {
        let layout = BASE_LAYOUT_CROSS465;
        let payload = json!({
            "cmd": "run_prg_data",
            "data": BASE64_STANDARD.encode(prg),
            "name": "cross465_case.prg",
            "max_cycles": max_cycles,
            "rtst_base": layout.address,
            "rtst_span": layout.span,
            "progress_timeout_ms": progress_timeout_ms,
        });
        self.send_request(payload)
    }

    fn read_memory(&self, address: u32, length: usize) -> Result<Vec<u8>, RunnerError> {
        let payload = json!({
            "cmd": "read_mem",
            "address": address,
            "length": length as u32,
        });
        let response = self.send_request(payload)?;
        let data_b64 = response.data.ok_or_else(|| RunnerError::Asm465Error {
            message: "read_mem response missing data payload".to_string(),
        })?;
        BASE64_STANDARD
            .decode(data_b64.as_bytes())
            .map_err(|err| RunnerError::Asm465Error {
                message: format!("failed to decode read_mem payload: {err}"),
            })
    }

    fn send_request(&self, payload: serde_json::Value) -> Result<ServiceResponse, RunnerError> {
        let addr = format!("{}:{}", self.host, self.port);
        let mut stream = TcpStream::connect(&addr).map_err(|err| RunnerError::Asm465Error {
            message: format!("failed to connect to asm465 service at {addr}: {err}"),
        })?;
        let request = serde_json::to_string(&payload).map_err(|err| RunnerError::Asm465Error {
            message: format!("failed to encode asm465 request: {err}"),
        })?;
        stream
            .write_all(request.as_bytes())
            .and_then(|_| stream.write_all(b"\n"))
            .map_err(|err| RunnerError::Asm465Error {
                message: format!("asm465 request write failed: {err}"),
            })?;
        stream.flush().map_err(|err| RunnerError::Asm465Error {
            message: format!("asm465 request flush failed: {err}"),
        })?;
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .map_err(|err| RunnerError::Asm465Error {
                message: format!("asm465 response read failed: {err}"),
            })?;
        if line.trim().is_empty() {
            return Err(RunnerError::Asm465Error {
                message: "asm465 service returned an empty response".to_string(),
            });
        }
        let response: ServiceResponse =
            serde_json::from_str(&line).map_err(|err| RunnerError::Asm465Error {
                message: format!("asm465 response parse error: {err}"),
            })?;
        match response.status {
            ServiceResponseStatus::Ok => Ok(response),
            ServiceResponseStatus::Error => Err(RunnerError::Asm465Error {
                message: response.message,
            }),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ServiceResponse {
    status: ServiceResponseStatus,
    message: String,
    #[serde(default)]
    data: Option<String>,
    #[serde(default)]
    cycles: Option<u64>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum ServiceResponseStatus {
    Ok,
    Error,
}

#[derive(Clone, Copy)]
enum ServiceKind {
    Native,
    Bridge,
}

impl ServiceKind {
    fn display_name(self) -> &'static str {
        match self {
            ServiceKind::Native => "asm465 native",
            ServiceKind::Bridge => "asm465 bridge",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use runtime_sdk::rtst::Stream;
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;
    use tempfile::NamedTempFile;

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .canonicalize()
            .unwrap()
    }

    fn assemble_sample(case_rel: &str) -> Option<Vec<u8>> {
        if Command::new("64tass").arg("--version").output().is_err() {
            eprintln!("skipping cross backend test (64tass not found)");
            return None;
        }
        let root = repo_root();
        let source = root.join(case_rel);
        let temp_output = NamedTempFile::new().ok()?;
        let include_common = root.join("native/src/include");
        let include_platform = root.join("native/src/platform/cross465/include");
        let status = Command::new("64tass")
            .arg("-q")
            .arg("-C")
            .arg("-a")
            .arg("-B")
            .arg("-I")
            .arg(include_common)
            .arg("-I")
            .arg(include_platform)
            .arg(&source)
            .arg("-o")
            .arg(temp_output.path())
            .status()
            .expect("failed to spawn 64tass");
        if !status.success() {
            return None;
        }
        fs::read(temp_output.path()).ok()
    }

    #[test]
    fn cross465_backend_executes_sample_case() {
        let Some(prg) = assemble_sample("crossdev/cross465/tests/cases/math_add_basic.s") else {
            return;
        };
        let backend = Cross465Backend::new();
        let exec = backend
            .run(
                &prg,
                ExecutionConfig {
                    target: TargetKind::Cross465,
                    timeout_ms: 5_000,
                    progress_timeout_ms: 500,
                    transport_retries: 3,
                    capture_console: false,
                    capture_display: false,
                    capture_overlay: false,
                },
            )
            .expect("cross backend should execute sample");
        assert!(exec.cycles > 0);
        let stream = Stream::parse(&exec.rtst_region).expect("parse rtst");
        assert_eq!(stream.header().passed_cases(), 1);
    }
}
