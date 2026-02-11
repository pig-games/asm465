use std::cmp::Ordering;
use std::fs;
use std::path::PathBuf;

use bus::mmio::ModuleKind;
use bus::personality::{self, Personality, C64_COMPAT};
use bus::personality_v2::{self, MapDecode};

use crate::cpu_worker::PersonalitySelection;

const BUILTIN_TOML_PERSONALITIES: &[(&str, &str)] = &[
    ("modern-retro-range", "Modern Retro (Range)"),
    ("c64-compat-sparse", "C64-Compatible Sparse Layout"),
];

fn builtin_personality_entry(id: &str) -> Option<(PathBuf, Option<&'static Personality>)> {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../cross465/personality_defs");
    match id {
        "modern-retro-range" => Some((
            base.join("modern-retro-range.toml"),
            Some(personality::default()),
        )),
        "c64-compat-sparse" => Some((base.join("c64-compat-sparse.toml"), Some(&C64_COMPAT))),
        _ => None,
    }
}

pub(crate) fn print_personality_list() {
    println!("Legacy personalities:");
    for persona in personality::all() {
        println!("  {:<20} {}", persona.name, persona.description);
    }
    println!("\nTOML personalities:");
    for (id, desc) in BUILTIN_TOML_PERSONALITIES {
        println!("  {:<20} {}", id, desc);
    }
    println!("  <path>               Load personality from TOML file");
}

pub(crate) fn print_module_list() {
    println!("Registered module implementations:");
    for factory in bus::builtin_module_registry().all() {
        println!("  {:<20} kind={}", factory.id(), factory.kind().as_str());
    }
}

pub(crate) fn dump_personality_maps(name: &str) -> Result<(), String> {
    if let Some(persona) = personality::find(name) {
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

    let (path, maybe_legacy) =
        builtin_personality_entry(name).unwrap_or((PathBuf::from(name), None));

    let toml = fs::read_to_string(&path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    let registry = bus::builtin_module_registry();
    let def = personality_v2::PersonalityDef::from_toml_str(&toml, &registry)
        .map_err(|err| err.to_string())?;

    println!("Personality: {} — {}", def.metadata.id, def.metadata.title);
    println!("Modules:");
    for (kind, module) in &def.modules {
        println!("  {:<10} -> {}", kind.as_str(), module.impl_id);
    }
    if let Some(legacy) = maybe_legacy {
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
                    if !entry.field_policies.is_empty() {
                        for policy in &entry.field_policies {
                            let span = if policy.lsb == policy.msb {
                                format!("bit {}", policy.lsb)
                            } else {
                                format!("bits {}..{}", policy.lsb, policy.msb)
                            };
                            let mut details = format!("      - {}", span);
                            if let Some(hook) = &policy.on_read {
                                details.push_str(&format!(" on_read=\"{}\"", hook));
                            }
                            if let Some(hook) = &policy.on_write {
                                details.push_str(&format!(" on_write=\"{}\"", hook));
                            }
                            if policy.ro {
                                details.push_str(" ro");
                            }
                            if policy.wo {
                                details.push_str(" wo");
                            }
                            println!("{details}");
                        }
                    }
                }
            }
            MapDecode::Instances(instances) => {
                let selector = instances
                    .selector
                    .as_ref()
                    .map(|sel| sel.desc.name.to_string())
                    .unwrap_or_else(|| "Select (implicit)".to_string());
                println!(
                    "  instances kind={} count={} index_var={} selector={}",
                    instances.module.as_str(),
                    instances.count,
                    instances.index_var,
                    selector
                );
                for entry in &instances.layout {
                    let addr_expr = match &entry.addr {
                        personality_v2::InstanceAddressExpr::Absolute(expr) => expr.source(),
                    };
                    let mut line = format!("    {} -> {}", addr_expr, entry.register.desc.name);
                    if let Some(field) = &entry.field {
                        line.push_str(&format!(
                            " (target_bit={}, source_bit={})",
                            field.target_bit.source(),
                            field.source_bit.source()
                        ));
                    }
                    if !entry.field_policies.is_empty() {
                        line.push_str(" field_policies=[");
                        let mut first = true;
                        for policy in &entry.field_policies {
                            if !first {
                                line.push_str(", ");
                            }
                            first = false;
                            if policy.lsb == policy.msb {
                                line.push_str(&format!("bit {}", policy.lsb));
                            } else {
                                line.push_str(&format!("bits {}..{}", policy.lsb, policy.msb));
                            }
                            if let Some(hook) = &policy.on_read {
                                line.push_str(&format!(" on_read={}", hook));
                            }
                            if let Some(hook) = &policy.on_write {
                                line.push_str(&format!(" on_write={}", hook));
                            }
                            if policy.ro {
                                line.push_str(" ro");
                            }
                            if policy.wo {
                                line.push_str(" wo");
                            }
                        }
                        line.push(']');
                    }
                    println!("{line}");
                }
            }
        }
    }

    Ok(())
}

pub(crate) fn dump_personality_registers(name: &str) -> Result<(), String> {
    if let Some(persona) = personality::find(name) {
        println!(
            "Register-level dump is not yet available for legacy personality `{}`",
            persona.name
        );
        return Ok(());
    }

    let (path, maybe_legacy) =
        builtin_personality_entry(name).unwrap_or((PathBuf::from(name), None));

    let toml = fs::read_to_string(&path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    let registry = bus::builtin_module_registry();
    let def = personality_v2::PersonalityDef::from_toml_str(&toml, &registry)
        .map_err(|err| err.to_string())?;

    println!("Personality: {} — {}", def.metadata.id, def.metadata.title);
    println!("Modules:");
    for (kind, module) in &def.modules {
        println!("  {:<10} -> {}", kind.as_str(), module.impl_id);
    }
    if let Some(legacy) = maybe_legacy {
        println!("Legacy fallback: {}", legacy.name);
    }

    let bus = bus::Bus::from_personality_def(def).map_err(|err| err.to_string())?;
    let mut mappings = bus
        .address_mappings()
        .ok_or_else(|| "register dump requires a TOML personality".to_string())?;

    if mappings.is_empty() {
        println!("\nNo resolved mappings.");
        return Ok(());
    }

    mappings.sort_by(|a, b| {
        a.addr
            .cmp(&b.addr)
            .then(match (&a.mapping, &b.mapping) {
                (
                    bus::MappingDetail::Scatter { target_bit: ta, .. },
                    bus::MappingDetail::Scatter { target_bit: tb, .. },
                ) => ta.cmp(tb),
                (bus::MappingDetail::Scatter { .. }, _) => Ordering::Greater,
                (_, bus::MappingDetail::Scatter { .. }) => Ordering::Less,
                _ => Ordering::Equal,
            })
            .then(a.module.as_str().cmp(b.module.as_str()))
            .then(a.module_impl_id.cmp(&b.module_impl_id))
            .then(a.register_name.cmp(&b.register_name))
            .then(match (&a.mapping, &b.mapping) {
                (
                    bus::MappingDetail::DirectInstance { instance: ia },
                    bus::MappingDetail::DirectInstance { instance: ib },
                ) => ia.cmp(ib),
                (bus::MappingDetail::DirectInstance { .. }, bus::MappingDetail::Direct) => {
                    Ordering::Greater
                }
                (bus::MappingDetail::Direct, bus::MappingDetail::DirectInstance { .. }) => {
                    Ordering::Less
                }
                (
                    bus::MappingDetail::Scatter { instance: ia, .. },
                    bus::MappingDetail::Scatter { instance: ib, .. },
                ) => ia.cmp(ib),
                _ => Ordering::Equal,
            })
    });

    println!("\nResolved register mappings:");
    for mapping in mappings {
        let register_suffix = match &mapping.mapping {
            bus::MappingDetail::Direct => String::new(),
            bus::MappingDetail::DirectInstance { instance } => format!("[{}]", instance),
            bus::MappingDetail::Scatter { instance, .. } => {
                instance.map(|idx| format!("[{}]", idx)).unwrap_or_default()
            }
        };

        let mut details: Vec<String> = Vec::new();
        if let bus::MappingDetail::Scatter { source_bit, .. } = &mapping.mapping {
            details.push(format!("source_bit={}", source_bit));
        }

        if mapping.value_builder {
            details.push("value_builder".to_string());
        }
        if let Some(expr) = &mapping.compute {
            details.push(format!("compute={}", expr));
        }
        if mapping.transform.shift != 0 {
            details.push(format!("shift {}", mapping.transform.shift));
        }
        if mapping.transform.invert_mask != 0 {
            details.push(format!(
                "invert_mask=0x{:02X}",
                mapping.transform.invert_mask
            ));
        }
        if mapping.transform.ro_mask != 0 {
            details.push(format!("ro_mask=0x{:02X}", mapping.transform.ro_mask));
        }
        if mapping.transform.wo_mask != 0 {
            details.push(format!("wo_mask=0x{:02X}", mapping.transform.wo_mask));
        }
        if let Some(ref hook) = mapping.transform.on_read {
            details.push(format!("on_read={}", hook));
        }
        if let Some(ref hook) = mapping.transform.on_write {
            details.push(format!("on_write={}", hook));
        }
        for hook in &mapping.field_hooks {
            let mut parts = vec![format!("mask=0x{:02X}", hook.mask)];
            if let Some(ref name) = hook.on_read {
                parts.push(format!("on_read={}", name));
            }
            if let Some(ref name) = hook.on_write {
                parts.push(format!("on_write={}", name));
            }
            details.push(format!("field_policy({})", parts.join(" ")));
        }
        if mapping.suppress_primary {
            details.push("suppress_primary".to_string());
        }

        let detail_str = if details.is_empty() {
            String::new()
        } else {
            format!(" [{}]", details.join("; "))
        };

        let mut addr_label = format_addr(mapping.addr);
        if let bus::MappingDetail::Scatter { target_bit, .. } = mapping.mapping {
            addr_label.push_str(&format!(".bit{}", target_bit));
        }

        let module_label = format!(
            "{}::{}{}",
            mapping.module.as_str(),
            mapping.module_impl_id,
            register_suffix
        );

        println!(
            "  {} <= {}.{} (priority {}){}",
            addr_label, module_label, mapping.register_name, mapping.priority, detail_str
        );
    }

    println!();
    Ok(())
}

fn format_addr(addr: u16) -> String {
    format!("${:04X}", addr)
}

fn describe_mmio_kind(kind: ModuleKind) -> &'static str {
    match kind {
        ModuleKind::Console => "console",
        ModuleKind::Display => "display",
        ModuleKind::Sprite => "sprite",
        ModuleKind::System => "system",
        ModuleKind::Input => "input",
        ModuleKind::Video => "video",
        ModuleKind::Audio => "audio",
    }
}

pub(crate) fn resolve_personality_selection(name: &str) -> Result<PersonalitySelection, String> {
    if let Some(persona) = personality::find(name) {
        return Ok(PersonalitySelection::Legacy(persona));
    }

    if let Some((path, maybe_legacy)) = builtin_personality_entry(name) {
        return Ok(PersonalitySelection::Toml {
            path,
            legacy: maybe_legacy,
        });
    }

    let path = PathBuf::from(name);
    if path.exists() {
        return Ok(PersonalitySelection::from_path(path));
    }

    Err(format!(
        "unknown personality '{name}'. Use --list-personalities to inspect the available options.",
    ))
}
