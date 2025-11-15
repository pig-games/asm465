use bus::Bus;
use runtime_sdk::rtst::{
    Header, BASE_LAYOUT_CROSS465, BASE_LAYOUT_MEGA65, BASE_LAYOUT_ULTIMATE64, HEADER_LEN,
};

use crate::RunnerError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetKind {
    Cross465,
    Ultimate64,
    Mega65,
}

pub struct ExecutionConfig {
    pub target: TargetKind,
    pub timeout_ms: u64,
}

pub struct ExecutionOutput {
    pub rtst_region: Vec<u8>,
    pub cycles: u64,
}

pub struct CpuBackend;

impl CpuBackend {
    pub fn execute(prg: &[u8], cfg: ExecutionConfig) -> Result<ExecutionOutput, RunnerError> {
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
