//! Minimal command-line runner for the cross465 6502 core.
//!
//! This binary mirrors the “load PRG + run” behaviour provided by the Bevy UI
//! but without a window. It is convenient for quick smoke tests or for
//! integrating into shell scripts.

use bus::{
    personality,
    personality_v2::{self, MapDecode},
    Bus,
};
use clap::Parser;
use core6502::Cpu;
use std::{fs, path::PathBuf};

/// Command-line arguments accepted by the runner.
#[derive(Parser, Debug)]
struct Args {
    /// Path to the PRG file to execute.
    #[arg(
        value_name = "PRG",
        required_unless_present_any = ["list_personalities", "list_modules", "dump_maps"]
    )]
    prg: PathBuf,
    /// Maximum number of CPU cycles to execute.
    #[arg(long, default_value_t = 5_000_000u64)]
    max_cycles: u64,
    /// Optional override for the start address; defaults to the PRG's load address.
    #[arg(long)]
    start: Option<u16>,
    /// Personality to load (builtin id or path to TOML).
    #[arg(long, value_name = "PERSONALITY")]
    personality: Option<String>,
    /// List available personalities and exit.
    #[arg(long)]
    list_personalities: bool,
    /// List available module implementations and exit.
    #[arg(long)]
    list_modules: bool,
    /// Dump map layout for a personality and exit.
    #[arg(long, value_name = "PERSONALITY")]
    dump_maps: Option<String>,
}

fn run_program(
    data: &[u8],
    max_cycles: u64,
    start_override: Option<u16>,
    personality: PersonalitySelection,
) -> anyhow::Result<(Cpu, core6502::RunOutcome)> {
    if data.len() < 2 {
        anyhow::bail!("PRG too small");
    }
    let load_addr = u16::from_le_bytes([data[0], data[1]]);
    let body = &data[2..];

    let mut bus = personality.build_bus()?;
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

    if args.list_modules {
        list_modules();
        return Ok(());
    }

    if let Some(ref id) = args.dump_maps {
        dump_maps(id)?;
        return Ok(());
    }

    if args.list_personalities {
        list_personalities();
        return Ok(());
    }

    let data = fs::read(&args.prg)?;
    let personality = PersonalitySelection::from_arg(args.personality.as_deref())?;
    let (_cpu, _outcome) = run_program(&data, args.max_cycles, args.start, personality)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn honors_cycle_budget() {
        // Load address $0600 followed by NOP, NOP, BRK.
        let prg = [0x00, 0x06, 0xEA, 0xEA, 0x00];
        let personality = PersonalitySelection::Legacy(personality::default());
        let (cpu, outcome) =
            run_program(&prg, 4, None, personality).expect("runner executes program");

        assert_eq!(cpu.cycles, 4);
        assert_eq!(outcome.limit, core6502::RunLimit::CycleBudget);
        assert_eq!(outcome.cycles, 4);
        // Two NOPs executed; PC should now point to the third byte ($0602).
        assert_eq!(cpu.pc, 0x0602);
    }
}
#[derive(Clone)]
enum PersonalitySelection {
    Legacy(&'static personality::Personality),
    Toml {
        path: PathBuf,
        legacy: Option<&'static personality::Personality>,
    },
}

impl PersonalitySelection {
    fn from_arg(arg: Option<&str>) -> anyhow::Result<Self> {
        if let Some(value) = arg {
            if let Some(persona) = personality::find(value) {
                return Ok(Self::Legacy(persona));
            }
            if let Some((path, legacy)) = builtin_personality_path(value) {
                return Ok(Self::Toml { path, legacy });
            }
            Ok(Self::Toml {
                path: PathBuf::from(value),
                legacy: None,
            })
        } else {
            Ok(Self::Legacy(personality::default()))
        }
    }

    fn build_bus(&self) -> anyhow::Result<Bus> {
        match self {
            PersonalitySelection::Legacy(p) => Ok(Bus::with_personality(p)),
            PersonalitySelection::Toml { path, .. } => {
                let toml = fs::read_to_string(path)?;
                let registry = bus::builtin_module_registry();
                let def = personality_v2::PersonalityDef::from_toml_str(&toml, &registry)?;
                Ok(Bus::from_personality_def(def)?)
            }
        }
    }
}

const BUILTIN_TOML_PERSONALITIES: &[(&str, &str)] = &[
    ("modern-retro-range", "Modern Retro (Range)"),
    ("c64-compat-sparse", "C64-Compatible Sparse Layout"),
];

fn builtin_personality_path(
    id: &str,
) -> Option<(PathBuf, Option<&'static personality::Personality>)> {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../personality_defs");
    match id {
        "modern-retro-range" => Some((
            base.join("modern-retro-range.toml"),
            Some(personality::default()),
        )),
        "c64-compat-sparse" => Some((
            base.join("c64-compat-sparse.toml"),
            Some(&personality::C64_COMPAT),
        )),
        _ => None,
    }
}

fn list_personalities() {
    println!("Legacy personalities:");
    for persona in personality::all() {
        println!("  {:<20} — {}", persona.name, persona.description);
    }
    println!("\nTOML personalities:");
    for (id, desc) in BUILTIN_TOML_PERSONALITIES {
        println!("  {:<20} {}", id, desc);
    }
    println!("  <path>               Load personality from TOML file");
}
fn list_modules() {
    let registry = bus::builtin_module_registry();
    println!("Registered module implementations:");
    for factory in registry.all() {
        println!("  {:<20} kind={}", factory.id(), factory.kind().as_str());
    }
}

fn dump_maps(id: &str) -> anyhow::Result<()> {
    if let Some(persona) = personality::find(id) {
        println!(
            "Legacy personality: {} — {}",
            persona.name, persona.description
        );
        for mmio in persona.mmio {
            println!(
                "  {}..={} -> {}",
                format_addr(*mmio.range.start()),
                format_addr(*mmio.range.end()),
                describe_mmio_kind(mmio.kind)
            );
        }
        return Ok(());
    }

    let (path, legacy_hint) = builtin_personality_path(id).unwrap_or((PathBuf::from(id), None));

    let toml = fs::read_to_string(&path)?;
    let registry = bus::builtin_module_registry();
    let def = personality_v2::PersonalityDef::from_toml_str(&toml, &registry)?;

    println!("Personality: {} — {}", def.metadata.id, def.metadata.title);
    println!("Modules:");
    for (kind, module) in &def.modules {
        println!("  {:<10} -> {}", kind.as_str(), module.impl_id);
    }
    if let Some(legacy) = legacy_hint {
        println!("Legacy fallback: {}", legacy.name);
    }

    println!("Maps:");
    for map in &def.maps {
        println!("- priority {}", map.priority);
        if !map.active_when.is_empty() {
            println!("  active_when = {:?}", map.active_when);
        }
        match &map.decode {
            MapDecode::Range(range) => {
                println!(
                    "  range {}..={} kind={} stride={}",
                    format_addr(range.range.start),
                    format_addr(range.range.end),
                    range.module.as_str(),
                    range.stride
                );
                for reg in &range.order {
                    println!("    - {}", reg.desc.name);
                }
            }
            MapDecode::Sparse(entries) => {
                for entry in entries {
                    println!(
                        "  {} -> {}::{}",
                        format_addr(entry.addr),
                        entry.module.as_str(),
                        entry.register.desc.name
                    );
                }
            }
        }
    }
    Ok(())
}

fn format_addr(addr: u16) -> String {
    format!("${:04X}", addr)
}

fn describe_mmio_kind(kind: personality::PersonalityMmioKind) -> &'static str {
    match kind {
        personality::PersonalityMmioKind::Console => "console",
        personality::PersonalityMmioKind::Display => "display",
        personality::PersonalityMmioKind::Sprite => "sprite",
        personality::PersonalityMmioKind::System => "system",
    }
}
