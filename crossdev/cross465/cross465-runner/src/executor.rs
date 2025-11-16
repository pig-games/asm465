//! Target execution helpers for the Cross465 RTST runner.
//!
//! Bridges assembled PRGs into the in-process emulator (`bus` + `core6502`),
//! polls RTST headers until completion, and surfaces the captured region and
//! cycle count back to the runner.

use bus::Bus;
use runtime_sdk::rtst::{
    Header, BASE_LAYOUT_CROSS465, BASE_LAYOUT_MEGA65, BASE_LAYOUT_ULTIMATE64, HEADER_LEN,
};

use crate::RunnerError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Targets supported by the runner backend.
pub enum TargetKind {
    Cross465,
    Ultimate64,
    Mega65,
}

/// Configuration for a single execution attempt.
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

/// Placeholder backend for Ultimate64 hardware (TODO: Telnet/UCI wiring).
pub struct Ultimate64Backend;

impl Ultimate64Backend {
    pub fn new() -> Self {
        Self
    }
}

impl TargetBackend for Ultimate64Backend {
    fn kind(&self) -> TargetKind {
        TargetKind::Ultimate64
    }

    fn run(&self, _prg: &[u8], _cfg: ExecutionConfig) -> Result<ExecutionOutput, RunnerError> {
        Err(RunnerError::UnsupportedTarget(
            "ultimate64 backend not implemented yet".to_string(),
        ))
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
        TargetKind::Ultimate64 => Box::new(Ultimate64Backend::new()),
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
