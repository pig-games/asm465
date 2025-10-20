//! Minimal command-line runner for the cross465 6502 core.
//!
//! This binary mirrors the “load PRG + run” behaviour provided by the Bevy UI
//! but without a window. It is convenient for quick smoke tests or for
//! integrating into shell scripts.

use bus::Bus;
use clap::Parser;
use core6502::Cpu;
use std::{fs, path::PathBuf};

/// Command-line arguments accepted by the runner.
#[derive(Parser, Debug)]
struct Args {
    /// Path to the PRG file to execute.
    prg: PathBuf,
    /// Maximum number of CPU cycles to execute.
    #[arg(long, default_value_t = 5_000_000u64)]
    max_cycles: u64,
    /// Optional override for the start address; defaults to the PRG's load address.
    #[arg(long)]
    start: Option<u16>,
}

fn run_program(
    data: &[u8],
    max_cycles: u64,
    start_override: Option<u16>,
) -> anyhow::Result<(Cpu, core6502::RunOutcome)> {
    if data.len() < 2 {
        anyhow::bail!("PRG too small");
    }
    let load_addr = u16::from_le_bytes([data[0], data[1]]);
    let body = &data[2..];

    let mut bus = Bus::new();
    bus.load(load_addr, body);
    let start = start_override.unwrap_or(load_addr);
    bus.write(0xFFFC, (start & 0xFF) as u8);
    bus.write(0xFFFD, (start >> 8) as u8);

    let mut cpu = Cpu::new(bus);
    cpu.reset();
    let outcome = cpu.run_for(max_cycles);
    Ok((cpu, outcome))
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let data = fs::read(&args.prg)?;
    let (_cpu, _outcome) = run_program(&data, args.max_cycles, args.start)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn honors_cycle_budget() {
        // Load address $0600 followed by NOP, NOP, BRK.
        let prg = [0x00, 0x06, 0xEA, 0xEA, 0x00];
        let (cpu, outcome) = run_program(&prg, 4, None).expect("runner executes program");

        assert_eq!(cpu.cycles, 4);
        assert_eq!(outcome.limit, core6502::RunLimit::CycleBudget);
        assert_eq!(outcome.cycles, 4);
        // Two NOPs executed; PC should now point to the third byte ($0602).
        assert_eq!(cpu.pc, 0x0602);
    }
}
