use bus::Bus;
use clap::Parser;
use core6502::Cpu;
use std::{fs, path::PathBuf};

#[derive(Parser, Debug)]
struct Args {
    prg: PathBuf,
    #[arg(long, default_value_t = 5_000_000u64)]
    max_cycles: u64,
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

    for _ in 0..5 {
        let pc = cpu.pc;
        let op = cpu.bus.read(pc);
        println!(
            "PC={:04X} OP={:02X} A={:02X} X={:02X} Y={:02X} P={:02X} SP={:02X}",
            pc,
            op,
            cpu.a,
            cpu.x,
            cpu.y,
            cpu.p.bits(),
            cpu.sp
        );
        cpu.step();
    }

    cpu.run_for(args.max_cycles);
    println!("Done cycles={}", cpu.cycles);
    Ok(())
}
