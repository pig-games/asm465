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
    env,
    io::Read,
    thread,
    time::{Duration, Instant},
};
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

        let mut last_header: Option<Header> = None;
        let mut last_wpos: u16 = 0;

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
            if cycles % poll_interval != 0 {
                continue;
            }
            let header_bytes = read_bytes(&mut cpu.bus, base_addr, HEADER_LEN);
            if let Ok(header) = Header::parse(&header_bytes) {
                last_wpos = header.write_pos();
                last_header = Some(header);
                if last_header.as_ref().unwrap().state().is_terminal() {
                    break;
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
            port: 80,
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
        let start = Instant::now();
        let client = Ultimate64Client::connect(&self.config)?;
        client.run_program(prg)?;
        loop {
            let header_bytes = client.read_memory(layout.address, HEADER_LEN)?;
            let header = match Header::parse(&header_bytes) {
                Ok(header) => header,
                Err(err) => {
                    if let Some(limit) = timeout {
                        if start.elapsed() >= limit {
                            return Err(RunnerError::RtstParse(err));
                        }
                    }
                    thread::sleep(self.config.poll_delay);
                    continue;
                }
            };
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
        let rtst = client.read_memory(layout.address, layout.span)?;
        Ok(ExecutionOutput {
            rtst_region: rtst,
            cycles: 0,
        })
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

/// Placeholder backend for MEGA65 hardware (TODO: m65 CLI integration).
pub struct Mega65Backend;

impl Mega65Backend {
    pub fn new() -> Self {
        Self
    }
}

impl TargetBackend for Mega65Backend {
    fn kind(&self) -> TargetKind {
        TargetKind::Mega65
    }

    fn run(&self, _prg: &[u8], _cfg: ExecutionConfig) -> Result<ExecutionOutput, RunnerError> {
        Err(RunnerError::UnsupportedTarget(
            "mega65 backend not implemented yet".to_string(),
        ))
    }
}

/// Select an appropriate backend implementation for the requested target.
pub fn backend_for_target(target: TargetKind) -> Box<dyn TargetBackend> {
    match target {
        TargetKind::Cross465 => Box::new(Cross465Backend::new()),
        TargetKind::Ultimate64 => Box::new(Ultimate64Backend::default()),
        TargetKind::Mega65 => Box::new(Mega65Backend::new()),
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
                },
            )
            .expect("cross backend should execute sample");
        assert!(exec.cycles > 0);
        let stream = Stream::parse(&exec.rtst_region).expect("parse rtst");
        assert_eq!(stream.header().passed_cases(), 1);
    }
}
