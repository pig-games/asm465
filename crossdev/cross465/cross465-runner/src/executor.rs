//! Target execution helpers for the Cross465 RTST runner.
//!
//! Bridges assembled PRGs into the in-process emulator (`bus` + `core6502`),
//! polls RTST headers until completion, and surfaces the captured region and
//! cycle count back to the runner.

use bus::Bus;
use runtime_sdk::rtst::{
    Header, BASE_LAYOUT_CROSS465, BASE_LAYOUT_MEGA65, BASE_LAYOUT_ULTIMATE64, HEADER_LEN,
};
use std::{
    env, fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, Instant},
};
use tempfile::{NamedTempFile, TempPath};
use ureq::{Agent, AgentBuilder, Error as UreqError, Response};

use crate::RunnerError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Targets supported by the runner backend.
pub enum TargetKind {
    Cross465,
    Ultimate64,
    Mega65,
}

/// Configuration for a single execution attempt.
#[derive(Clone, Copy, Debug)]
pub struct ExecutionConfig {
    pub target: TargetKind,
    pub timeout_ms: u64,
    pub progress_timeout_ms: u64,
    pub transport_retries: u32,
}

/// Results captured from executing a PRG.
pub struct ExecutionOutput {
    pub rtst_region: Vec<u8>,
    pub cycles: u64,
}

/// Backend contract for running RTST-enabled binaries.
pub trait TargetBackend {
    fn kind(&self) -> TargetKind;
    fn run(&self, prg: &[u8], cfg: ExecutionConfig) -> Result<ExecutionOutput, RunnerError>;
}

/// Cross465 emulator backend using the in-process CPU implementation.
pub struct Cross465Backend;

impl Cross465Backend {
    pub fn new() -> Self {
        Self
    }

    fn execute(&self, prg: &[u8], cfg: ExecutionConfig) -> Result<ExecutionOutput, RunnerError> {
        let base = match cfg.target {
            TargetKind::Cross465 => BASE_LAYOUT_CROSS465,
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

        let mut last_header: Option<Header> = None;
        let mut last_wpos: u16 = 0;
        let mut initialized = false;
        let mut last_progress = Instant::now();

        loop {
            let step_cycles = cpu.step() as u64;
            cycles = cycles.saturating_add(step_cycles);
            if cycle_budget > 0 && cycles > cycle_budget {
                let state = last_header.as_ref().map(|h| h.state());
                return Err(RunnerError::Timeout {
                    cycles,
                    state,
                    wpos: last_wpos,
                    pc: cpu.pc,
                });
            }
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
            if cycles % poll_interval != 0 {
                continue;
            }
            let header_bytes = read_bytes(&mut cpu.bus, base_addr, HEADER_LEN);
            match Header::parse(&header_bytes) {
                Ok(header) => {
                    if !initialized {
                        initialized = true;
                        last_progress = Instant::now();
                    }
                    if header.write_pos() != last_wpos {
                        last_wpos = header.write_pos();
                        last_progress = Instant::now();
                    } else if last_header.as_ref().map(|prev| prev.state()) != Some(header.state())
                    {
                        last_progress = Instant::now();
                    }
                    last_header = Some(header);
                    if last_header.as_ref().unwrap().state().is_terminal() {
                        break;
                    }
                }
                Err(err) => {
                    if !initialized {
                        if let Some(limit) = progress_deadline {
                            if last_progress.elapsed() >= limit {
                                return Err(RunnerError::ProtocolInit { source: err });
                            }
                        }
                    } else {
                        return Err(RunnerError::RtstParse(err));
                    }
                }
            }
        }

        let mut region = vec![0u8; base.span.min(0x10000 - base.address as usize)];
        for (offset, byte) in region.iter_mut().enumerate() {
            *byte = cpu.bus.read(base_addr.wrapping_add(offset as u16));
        }

        Ok(ExecutionOutput {
            rtst_region: region,
            cycles,
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
}

/// REST backend for Ultimate64 hardware.
pub struct Ultimate64Backend {
    config: Ultimate64BackendConfig,
}

impl Ultimate64Backend {
    pub fn new(config: Ultimate64BackendConfig) -> Self {
        Self { config }
    }

    fn execute(&self, prg: &[u8], cfg: ExecutionConfig) -> Result<ExecutionOutput, RunnerError> {
        let layout = match cfg.target {
            TargetKind::Cross465 => BASE_LAYOUT_CROSS465,
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
        let mut last_progress = Instant::now();
        let mut initialized = false;
        let mut last_header: Option<Header> = None;
        let mut last_wpos: u16 = 0;
        let start = Instant::now();
        let client = Ultimate64Client::connect(&self.config)?;
        self.retry_transport(|| client.run_program(prg), cfg.transport_retries)?;
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
                || client.read_memory(layout.address, HEADER_LEN),
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
            || client.read_memory(layout.address, layout.span),
            cfg.transport_retries,
        )?;
        Ok(ExecutionOutput {
            rtst_region: rtst,
            cycles: 0,
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
        Self::new(Ultimate64BackendConfig::from_env())
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
            TargetKind::Cross465 => BASE_LAYOUT_CROSS465,
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
        let mut last_progress = Instant::now();
        let mut initialized = false;
        let mut last_header: Option<Header> = None;
        let mut last_wpos: u16 = 0;
        let start = Instant::now();
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
                || client.read_memory(layout.address, HEADER_LEN),
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
            || client.read_memory(layout.address, layout.span),
            cfg.transport_retries,
        )?;
        Ok(ExecutionOutput {
            rtst_region: rtst,
            cycles: 0,
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
pub fn backend_for_target(target: TargetKind) -> Box<dyn TargetBackend> {
    match target {
        TargetKind::Cross465 => Box::new(Cross465Backend::new()),
        TargetKind::Ultimate64 => Box::new(Ultimate64Backend::default()),
        TargetKind::Mega65 => Box::new(Mega65Backend::default()),
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
        }
    }

    pub fn as_define_suffix(self) -> &'static str {
        match self {
            TargetKind::Cross465 => "CROSS465",
            TargetKind::Ultimate64 => "ULTIMATE64",
            TargetKind::Mega65 => "MEGA65",
        }
    }

    pub fn all() -> [Self; 3] {
        [Self::Cross465, Self::Ultimate64, Self::Mega65]
    }
}

impl std::str::FromStr for TargetKind {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "cross465" => Ok(TargetKind::Cross465),
            "ultimate64" => Ok(TargetKind::Ultimate64),
            "mega65" => Ok(TargetKind::Mega65),
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
        let output = cmd.output().map_err(RunnerError::Spawn)?;
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
        let prg = match assemble_sample("crossdev/cross465/tests/cases/math_add_basic.s") {
            Some(prg) => prg,
            None => return,
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
                },
            )
            .expect("cross backend should execute sample");
        assert!(exec.cycles > 0);
        let stream = Stream::parse(&exec.rtst_region).expect("parse rtst");
        assert_eq!(stream.header().passed_cases(), 1);
    }
}
