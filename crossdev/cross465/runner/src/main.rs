//! Minimal command-line runner for the cross465 6502 core.
//!
//! This binary mirrors the “load PRG + run” behaviour provided by the Bevy UI
//! but without a window. It is convenient for quick smoke tests or for
//! integrating into shell scripts.

use bus::console_mmio::ConsoleMmio;
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

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let data = fs::read(&args.prg)?;
    if data.len() < 2 {
        anyhow::bail!("PRG too small");
    }
    let load_addr = u16::from_le_bytes([data[0], data[1]]);
    let body = &data[2..];

    let mut bus = Bus::new();
    bus.load(load_addr, body);
    let start = args.start.unwrap_or(load_addr);
    bus.write(0xFFFC, (start & 0xFF) as u8);
    bus.write(0xFFFD, (start >> 8) as u8);

    let mut cpu = Cpu::new(bus);
    cpu.reset();

    cpu.run_for(5_000_000u64);
    Ok(())
}
