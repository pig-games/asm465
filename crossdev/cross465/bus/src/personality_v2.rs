use std::collections::BTreeMap;

use crate::mmio::{ModuleFactory, ModuleKind, ModuleOptions, ModuleRegistry, RegId, RegisterDesc};

use serde::Deserialize;
use toml::value::{Table, Value};

/// High-level personality definition produced by the TOML loader.
pub struct PersonalityDef {
    pub metadata: PersonalityMetadata,
    /// Configured modules keyed by [`ModuleKind`].
    pub modules: BTreeMap<ModuleKind, ModuleConfig>,
    /// Named conditions used by maps to toggle address layers.
    pub conditions: BTreeMap<String, Condition>,
    /// Address maps in declaration order.
    pub maps: Vec<Map>,
    /// Optional interrupt wiring metadata.
    pub interrupts: InterruptConfig,
}

pub struct PersonalityMetadata {
    pub id: String,
    pub title: String,
    pub default_map_priority: i32,
}

pub struct ModuleConfig {
    pub kind: ModuleKind,
    pub impl_id: String,
    pub factory: &'static dyn ModuleFactory,
    pub options: ModuleOptions,
}

pub struct Condition {
    pub module: ModuleKind,
    pub register: ResolvedRegister,
    pub equals: i32,
}

#[derive(Clone)]
pub struct ResolvedRegister {
    pub id: RegId,
    pub desc: &'static RegisterDesc,
}

pub struct Map {
    pub priority: i32,
    pub active_when: Vec<String>,
    pub decode: MapDecode,
}

pub enum MapDecode {
    Range(RangeMap),
    Sparse(Vec<SparseEntry>),
    Instances(InstanceMap),
}

pub struct RangeMap {
    pub range: AddressRange,
    pub module: ModuleKind,
    pub order: Vec<ResolvedRegister>,
    pub stride: u16,
    pub default_transform: Option<Transform>,
}

pub struct AddressRange {
    pub start: u16,
    pub end: u16,
}

pub struct SparseEntry {
    pub addr: u16,
    pub module: ModuleKind,
    pub register: ResolvedRegister,
    pub transform: Option<Transform>,
    pub value_builder: Option<ValueBuilder>,
    pub field_policies: Vec<FieldPolicy>,
}

pub struct InstanceMap {
    pub module: ModuleKind,
    pub selector: Option<ResolvedRegister>,
    pub count: usize,
    pub index_var: String,
    pub layout: Vec<InstanceLayoutEntry>,
}

pub struct InstanceLayoutEntry {
    pub addr: InstanceAddressExpr,
    pub register: ResolvedRegister,
    pub transform: Option<Transform>,
    pub field: Option<InstanceField>,
    pub field_policies: Vec<FieldPolicy>,
}

#[derive(Clone)]
pub enum InstanceAddressExpr {
    Absolute(InstanceExpr),
}

#[derive(Clone)]
pub struct InstanceField {
    pub source_bit: InstanceExpr,
    pub target_bit: InstanceExpr,
}

#[derive(Clone)]
pub struct FieldPolicy {
    pub lsb: u8,
    pub msb: u8,
    pub mask: u8,
    pub on_read: Option<String>,
    pub on_write: Option<String>,
    pub ro: bool,
    pub wo: bool,
}

#[derive(Clone)]
pub struct InstanceExpr {
    source: String,
    tokens: Vec<ExprToken>,
}

#[derive(Clone)]
enum ExprToken {
    Number(u32),
    Var,
    Plus,
    Minus,
    Star,
}

#[derive(Clone, Default)]
pub struct Transform {
    pub invert_mask: u8,
    pub ro_mask: u8,
    pub wo_mask: u8,
    pub shift: i8,
    pub on_read: Option<String>,
    pub on_write: Option<String>,
    pub pre_read_sets: Vec<RegisterSet>,
    pub post_read_sets: Vec<RegisterSet>,
    pub pre_write_sets: Vec<RegisterSet>,
    pub post_write_sets: Vec<RegisterSet>,
}

#[derive(Default)]
pub struct InterruptConfig {
    pub irq_sources: Vec<InterruptSource>,
    pub nmi_sources: Vec<InterruptSource>,
    pub irq_ack: Option<InterruptAck>,
    pub nmi_ack: Option<InterruptAck>,
}

pub struct InterruptSource {
    pub module: ModuleKind,
    pub register: ResolvedRegister,
}

pub struct InterruptAck {
    pub module: ModuleKind,
    pub register: ResolvedRegister,
    pub hook: Option<String>,
}

#[derive(Clone)]
pub struct ValueBuilder {
    pub width: usize,
    pub bits: Vec<BitBinding>,
    pub const_set: Option<Vec<u8>>,
    pub invert_byte: bool,
}

#[derive(Clone)]
pub struct BitBinding {
    pub byte_index: usize,
    pub bit: u8,
    pub src: String,
    pub active_low: bool,
}

#[derive(Clone)]
pub struct RegisterSet {
    pub module: ModuleKind,
    pub register: ResolvedRegister,
    pub value: u8,
}

pub trait InputSignals {
    fn get_bool(&mut self, name: &str) -> Option<bool>;
    fn get_int(&mut self, name: &str) -> Option<i32>;
    fn get_f32(&mut self, name: &str) -> Option<f32>;
}

#[derive(Debug)]
pub struct LoaderError {
    pub message: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

impl std::fmt::Display for LoaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let (Some(line), Some(column)) = (self.line, self.column) {
            write!(f, "{} (at {}:{})", self.message, line, column)
        } else {
            write!(f, "{}", self.message)
        }
    }
}

impl std::error::Error for LoaderError {}

#[derive(Debug)]
pub enum CompileError {
    MissingModule(ModuleKind),
    AddressOverlap {
        addr: u16,
        existing_priority: i32,
        new_priority: i32,
    },
    AddressOutOfRange {
        addr: u16,
    },
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompileError::MissingModule(kind) => {
                write!(f, "module kind `{}` is not configured", kind.as_str())
            }
            CompileError::AddressOverlap {
                addr,
                existing_priority,
                new_priority,
            } => write!(
                f,
                "address {:#06X} already mapped at priority {}; new priority {} conflicts",
                addr, existing_priority, new_priority
            ),
            CompileError::AddressOutOfRange { addr } => {
                write!(f, "map writes past declared range at address {:#06X}", addr)
            }
        }
    }
}

impl std::error::Error for CompileError {}

impl From<toml::de::Error> for LoaderError {
    fn from(err: toml::de::Error) -> Self {
        LoaderError {
            message: err.to_string(),
            line: None,
            column: None,
        }
    }
}

impl PersonalityDef {
    pub fn from_toml_str(src: &str, registry: &ModuleRegistry) -> Result<Self, LoaderError> {
        let raw: RawPersonalityFile = toml::from_str(src)?;

        let RawPersonalityFile {
            personality,
            modules: raw_modules,
            maps: raw_maps,
            conditions: raw_conditions,
            interrupts: raw_interrupts,
        } = raw;

        let default_priority = personality.default_map_priority.unwrap_or(0);

        let metadata = PersonalityMetadata {
            id: personality.id,
            title: personality.title,
            default_map_priority: default_priority,
        };

        let modules = resolve_modules(raw_modules, registry)?;

        let conditions = resolve_conditions(raw_conditions, &modules)?;

        let maps = resolve_maps(raw_maps, &modules, default_priority)?;

        let interrupts = resolve_interrupts(raw_interrupts, &modules)?;

        Ok(PersonalityDef {
            metadata,
            modules,
            conditions,
            maps,
            interrupts,
        })
    }
}

fn resolve_modules(
    raw: BTreeMap<String, RawModule>,
    registry: &ModuleRegistry,
) -> Result<BTreeMap<ModuleKind, ModuleConfig>, LoaderError> {
    let mut modules = BTreeMap::new();
    for (kind_str, module) in raw {
        let kind = ModuleKind::from_str(&kind_str).ok_or_else(|| LoaderError {
            message: format!("unknown module kind `{kind_str}`"),
            line: None,
            column: None,
        })?;
        if modules.contains_key(&kind) {
            return Err(LoaderError {
                message: format!("module kind `{}` defined more than once", kind.as_str()),
                line: None,
                column: None,
            });
        }
        let RawModule { impl_id, options } = module;
        let factory = registry.by_id(&impl_id).ok_or_else(|| LoaderError {
            message: format!("module implementation `{impl_id}` not registered"),
            line: None,
            column: None,
        })?;
        if factory.kind() != kind {
            return Err(LoaderError {
                message: format!(
                    "module `{}` has kind `{}` but registry entry is `{}`",
                    impl_id,
                    kind.as_str(),
                    factory.kind().as_str()
                ),
                line: None,
                column: None,
            });
        }
        modules.insert(
            kind,
            ModuleConfig {
                kind,
                impl_id,
                factory,
                options,
            },
        );
    }
    Ok(modules)
}

fn resolve_conditions(
    raw: BTreeMap<String, RawCondition>,
    modules: &BTreeMap<ModuleKind, ModuleConfig>,
) -> Result<BTreeMap<String, Condition>, LoaderError> {
    let mut conditions = BTreeMap::new();
    for (name, cond) in raw {
        let kind = ModuleKind::from_str(&cond.kind).ok_or_else(|| LoaderError {
            message: format!(
                "condition `{name}` references unknown module kind `{}`",
                cond.kind
            ),
            line: None,
            column: None,
        })?;
        let module = modules.get(&kind).ok_or_else(|| LoaderError {
            message: format!(
                "condition `{name}` references module kind `{}` with no module configuration",
                kind.as_str()
            ),
            line: None,
            column: None,
        })?;
        let register = resolve_register(&cond.reg, module)?;
        conditions.insert(
            name,
            Condition {
                module: kind,
                register,
                equals: cond.equals,
            },
        );
    }
    Ok(conditions)
}

fn resolve_maps(
    raw_maps: Vec<RawMap>,
    modules: &BTreeMap<ModuleKind, ModuleConfig>,
    default_priority: i32,
) -> Result<Vec<Map>, LoaderError> {
    let mut maps = Vec::new();
    for raw in raw_maps {
        let priority = raw.priority.unwrap_or(default_priority);
        let active_when = raw.active_when.map(|s| vec![s]).unwrap_or_default();
        let decode = {
            let RawDecode {
                range,
                sparse,
                instances,
            } = raw.decode;
            match (range, sparse, instances) {
                (Some(range), None, None) => {
                    let (kind, module) = module_for_kind(&range.kind, modules)?;
                    let resolved_order = resolve_register_order(&range.order, module)?;
                    let stride = range.stride.unwrap_or(1);
                    let address_range = parse_range(&range.addr)?;
                    let default_transform = range
                        .default_transform
                        .map(|t| resolve_transform(t, modules))
                        .transpose()?;
                    MapDecode::Range(RangeMap {
                        range: address_range,
                        module: kind,
                        order: resolved_order,
                        stride,
                        default_transform,
                    })
                }
                (None, Some(entries), None) => {
                    let mut resolved = Vec::new();
                    for entry in entries {
                        let (kind, module) = module_for_kind(&entry.kind, modules)?;
                        let register = resolve_register(&entry.id, module)?;
                        let addr = parse_addr(&entry.addr)?;
                        let transform = entry
                            .transform
                            .map(|t| resolve_transform(t, modules))
                            .transpose()?;
                        let value_builder = entry
                            .value_builder
                            .map(|value| parse_value_builder(value, register.desc))
                            .transpose()?;
                        let field_policies = resolve_field_policies(&entry.field_policies, &register)?;
                        resolved.push(SparseEntry {
                            addr,
                            module: kind,
                            register,
                            transform,
                            value_builder,
                            field_policies,
                        });
                    }
                    MapDecode::Sparse(resolved)
                }
                (None, None, Some(instances)) => {
                    MapDecode::Instances(resolve_instance_map(instances, modules)?)
                }
                _ => {
                    return Err(LoaderError {
                        message:
                            "each [[map]] must define exactly one of decode.range, decode.sparse, or decode.instances"
                                .to_string(),
                        line: None,
                        column: None,
                    });
                }
            }
        };

        maps.push(Map {
            priority,
            active_when,
            decode,
        });
    }
    Ok(maps)
}

fn resolve_instance_map(
    raw: RawInstanceMap,
    modules: &BTreeMap<ModuleKind, ModuleConfig>,
) -> Result<InstanceMap, LoaderError> {
    if raw.count == 0 {
        return Err(LoaderError {
            message: "decode.instances.count must be > 0".to_string(),
            line: None,
            column: None,
        });
    }

    let (kind, module) = module_for_kind(&raw.kind, modules)?;

    let selector = if let Some(name) = raw.selector {
        Some(resolve_register(&name, module)?)
    } else {
        module
            .factory
            .regs()
            .iter()
            .find(|desc| desc.matches_name("Select"))
            .map(|desc| ResolvedRegister {
                id: desc.id,
                desc,
            })
    };

    if raw.layout.is_empty() {
        return Err(LoaderError {
            message: "decode.instances.layout must contain at least one entry".to_string(),
            line: None,
            column: None,
        });
    }

    let mut layout = Vec::with_capacity(raw.layout.len());
    for entry in raw.layout {
        let register = resolve_register(&entry.id, module)?;
        let addr_expr = parse_instance_expr(&entry.addr, &raw.index_var)?;
        let transform = entry
            .transform
            .map(|t| resolve_transform(t, modules))
            .transpose()?;
        let field = entry
            .field
            .map(|field| parse_instance_field(field, &raw.index_var))
            .transpose()?;
        let field_policies = resolve_field_policies(&entry.field_policies, &register)?;

        layout.push(InstanceLayoutEntry {
            addr: InstanceAddressExpr::Absolute(addr_expr),
            register,
            transform,
            field,
            field_policies,
        });
    }

    Ok(InstanceMap {
        module: kind,
        selector,
        count: raw.count,
        index_var: raw.index_var,
        layout,
    })
}

fn parse_instance_field(
    raw: RawInstanceField,
    index_var: &str,
) -> Result<InstanceField, LoaderError> {
    let target_expr_str = raw
        .target_bit
        .or(raw.bit)
        .ok_or_else(|| LoaderError {
            message: "field must specify `bit` or `target_bit`".to_string(),
            line: None,
            column: None,
        })?;
    let target_bit = parse_instance_expr(&target_expr_str, index_var)?;

    let source_expr_str = raw.source_bit.unwrap_or_else(|| "0".to_string());
    let source_bit = parse_instance_expr(&source_expr_str, index_var)?;

    Ok(InstanceField {
        source_bit,
        target_bit,
    })
}

fn resolve_interrupts(
    raw: Option<RawInterrupts>,
    modules: &BTreeMap<ModuleKind, ModuleConfig>,
) -> Result<InterruptConfig, LoaderError> {
    let mut config = InterruptConfig::default();
    if let Some(raw) = raw {
        for source in raw.irq_sources {
            let (kind, module) = module_for_kind(&source.kind, modules)?;
            let register = resolve_register(&source.id, module)?;
            config.irq_sources.push(InterruptSource {
                module: kind,
                register,
            });
        }
        for source in raw.nmi_sources {
            let (kind, module) = module_for_kind(&source.kind, modules)?;
            let register = resolve_register(&source.id, module)?;
            config.nmi_sources.push(InterruptSource {
                module: kind,
                register,
            });
        }
        if let Some(ack) = raw.irq_ack {
            let (kind, module) = module_for_kind(&ack.kind, modules)?;
            let register = resolve_register(&ack.id, module)?;
            config.irq_ack = Some(InterruptAck {
                module: kind,
                register,
                hook: ack.hook,
            });
        }
        if let Some(ack) = raw.nmi_ack {
            let (kind, module) = module_for_kind(&ack.kind, modules)?;
            let register = resolve_register(&ack.id, module)?;
            config.nmi_ack = Some(InterruptAck {
                module: kind,
                register,
                hook: ack.hook,
            });
        }
    }
    Ok(config)
}

fn module_for_kind<'a>(
    kind_str: &str,
    modules: &'a BTreeMap<ModuleKind, ModuleConfig>,
) -> Result<(ModuleKind, &'a ModuleConfig), LoaderError> {
    let kind = ModuleKind::from_str(kind_str).ok_or_else(|| LoaderError {
        message: format!("unknown module kind `{kind_str}`"),
        line: None,
        column: None,
    })?;
    let module = modules.get(&kind).ok_or_else(|| LoaderError {
        message: format!(
            "module kind `{}` referenced before it is defined in [modules]",
            kind.as_str()
        ),
        line: None,
        column: None,
    })?;
    Ok((kind, module))
}

fn resolve_register(name: &str, module: &ModuleConfig) -> Result<ResolvedRegister, LoaderError> {
    let desc = module
        .factory
        .regs()
        .iter()
        .find(|desc| desc.matches_name(name))
        .ok_or_else(|| LoaderError {
            message: format!(
                "module `{}` (kind `{}`) has no register named `{}`",
                module.impl_id,
                module.kind.as_str(),
                name
            ),
            line: None,
            column: None,
        })?;
    Ok(ResolvedRegister { id: desc.id, desc })
}

fn resolve_register_order(
    order: &[String],
    module: &ModuleConfig,
) -> Result<Vec<ResolvedRegister>, LoaderError> {
    let mut resolved = Vec::new();
    for name in order {
        resolved.push(resolve_register(name, module)?);
    }
    Ok(resolved)
}

fn resolve_transform(
    raw: RawTransform,
    modules: &BTreeMap<ModuleKind, ModuleConfig>,
) -> Result<Transform, LoaderError> {
    let mut transform = Transform {
        invert_mask: raw.invert_mask.unwrap_or(0),
        ro_mask: raw.ro_mask.unwrap_or(0),
        wo_mask: raw.wo_mask.unwrap_or(0),
        shift: raw.shift.unwrap_or(0),
        on_read: raw.on_read,
        on_write: raw.on_write,
        pre_read_sets: Vec::new(),
        post_read_sets: Vec::new(),
        pre_write_sets: Vec::new(),
        post_write_sets: Vec::new(),
    };

    transform.pre_read_sets = resolve_register_sets(&raw.pre_read_sets, modules)?;
    transform.post_read_sets = resolve_register_sets(&raw.post_read_sets, modules)?;
    transform.pre_write_sets = resolve_register_sets(&raw.pre_write_sets, modules)?;
    transform.post_write_sets = resolve_register_sets(&raw.post_write_sets, modules)?;

    Ok(transform)
}

fn resolve_register_sets(
    raws: &[RawRegisterSet],
    modules: &BTreeMap<ModuleKind, ModuleConfig>,
) -> Result<Vec<RegisterSet>, LoaderError> {
    let mut sets = Vec::with_capacity(raws.len());
    for raw in raws {
        let kind = ModuleKind::from_str(&raw.kind).ok_or_else(|| LoaderError {
            message: format!("register set references unknown module kind `{}`", raw.kind),
            line: None,
            column: None,
        })?;
        let module = modules.get(&kind).ok_or_else(|| LoaderError {
            message: format!(
                "register set references module kind `{}` without configuration",
                kind.as_str()
            ),
            line: None,
            column: None,
        })?;
        let register = resolve_register(&raw.id, module)?;
        sets.push(RegisterSet {
            module: kind,
            register,
            value: raw.value,
        });
    }
    Ok(sets)
}

fn resolve_field_policies(
    raws: &[RawFieldPolicy],
    register: &ResolvedRegister,
) -> Result<Vec<FieldPolicy>, LoaderError> {
    if raws.is_empty() {
        return Ok(Vec::new());
    }

    if register.desc.width != 1 {
        return Err(LoaderError {
            message: format!(
                "field_policies currently support 1-byte registers; `{}` is {} bytes wide",
                register.desc.name, register.desc.width
            ),
            line: None,
            column: None,
        });
    }

    let mut policies = Vec::with_capacity(raws.len());
    for raw in raws {
        let lsb = raw.lsb.ok_or_else(|| LoaderError {
            message: "field policy missing `lsb`".to_string(),
            line: None,
            column: None,
        })?;
        let msb = raw.msb.unwrap_or(lsb);
        if msb < lsb {
            return Err(LoaderError {
                message: format!(
                    "field policy has msb {} < lsb {} for register `{}`",
                    msb, lsb, register.desc.name
                ),
                line: None,
                column: None,
            });
        }
        if msb >= register.desc.width.saturating_mul(8) {
            return Err(LoaderError {
                message: format!(
                    "field policy bit {} out of range for register `{}` (width {} bytes)",
                    msb, register.desc.name, register.desc.width
                ),
                line: None,
                column: None,
            });
        }

        let span = msb - lsb + 1;
        let mask = (((1u16 << span) - 1) << lsb) as u8;
        policies.push(FieldPolicy {
            lsb,
            msb,
            mask,
            on_read: raw.on_read.clone(),
            on_write: raw.on_write.clone(),
            ro: raw.ro.unwrap_or(false),
            wo: raw.wo.unwrap_or(false),
        });
    }

    Ok(policies)
}

fn parse_value_builder(value: Value, register: &RegisterDesc) -> Result<ValueBuilder, LoaderError> {
    let table = value.as_table().ok_or_else(|| LoaderError {
        message: "value_builder must be a table".to_string(),
        line: None,
        column: None,
    })?;

    let width = table
        .get("width")
        .and_then(Value::as_integer)
        .ok_or_else(|| LoaderError {
            message: "value_builder.width must be an integer".to_string(),
            line: None,
            column: None,
        })?;

    if width <= 0 {
        return Err(LoaderError {
            message: "value_builder.width must be positive".to_string(),
            line: None,
            column: None,
        });
    }

    if width as u8 != register.width {
        return Err(LoaderError {
            message: format!(
                "value_builder width {} does not match register width {}",
                width, register.width
            ),
            line: None,
            column: None,
        });
    }

    let width = width as usize;

    let bits = table
        .get("bits")
        .map(|value| parse_bit_bindings(value, width))
        .transpose()?
        .unwrap_or_default();

    let const_set = table
        .get("const_set")
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| LoaderError {
                    message: "value_builder.const_set must be a string".to_string(),
                    line: None,
                    column: None,
                })
                .and_then(|bits| parse_const_set(bits, width))
        })
        .transpose()?;

    let invert_byte = table
        .get("post")
        .and_then(Value::as_table)
        .and_then(|post| post.get("invert_byte"))
        .and_then(Value::as_bool)
        .unwrap_or(false);

    Ok(ValueBuilder {
        width,
        bits,
        const_set,
        invert_byte,
    })
}

fn parse_bit_bindings(value: &Value, width: usize) -> Result<Vec<BitBinding>, LoaderError> {
    let array = value.as_array().ok_or_else(|| LoaderError {
        message: "value_builder.bits must be an array".to_string(),
        line: None,
        column: None,
    })?;

    let mut bindings = Vec::with_capacity(array.len());
    for entry in array {
        let table = entry.as_table().ok_or_else(|| LoaderError {
            message: "value_builder.bits entries must be tables".to_string(),
            line: None,
            column: None,
        })?;

        let bit = table
            .get("bit")
            .and_then(Value::as_integer)
            .ok_or_else(|| LoaderError {
                message: "value_builder.bits entries require `bit`".to_string(),
                line: None,
                column: None,
            })?;

        if !(0..=7).contains(&bit) {
            return Err(LoaderError {
                message: format!("value_builder bit position {} is out of range (0..=7)", bit),
                line: None,
                column: None,
            });
        }

        let src = table
            .get("src")
            .and_then(Value::as_str)
            .ok_or_else(|| LoaderError {
                message: "value_builder.bits entries require `src`".to_string(),
                line: None,
                column: None,
            })?
            .to_string();

        let active_low = table
            .get("active_low")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let byte_index = table
            .get("byte_index")
            .and_then(Value::as_integer)
            .unwrap_or(0);

        if byte_index < 0 || byte_index as usize >= width {
            return Err(LoaderError {
                message: format!(
                    "value_builder.bit byte_index {} is out of range (width {})",
                    byte_index, width
                ),
                line: None,
                column: None,
            });
        }

        bindings.push(BitBinding {
            byte_index: byte_index as usize,
            bit: bit as u8,
            src,
            active_low,
        });
    }

    Ok(bindings)
}

fn parse_const_set(bits: &str, width: usize) -> Result<Vec<u8>, LoaderError> {
    let expected_len = width * 8;
    if bits.len() != expected_len {
        return Err(LoaderError {
            message: format!(
                "value_builder.const_set length {} does not match width {} ({} bits expected)",
                bits.len(),
                width,
                expected_len
            ),
            line: None,
            column: None,
        });
    }

    let mut bytes = vec![0u8; width];
    for (idx, ch) in bits.chars().enumerate() {
        let value = match ch {
            '0' => 0,
            '1' => 1,
            _ => {
                return Err(LoaderError {
                    message: "value_builder.const_set must contain only '0' or '1'".to_string(),
                    line: None,
                    column: None,
                })
            }
        };
        let byte = idx / 8;
        let bit = 7 - (idx % 8);
        if value == 1 {
            bytes[byte] |= 1 << bit;
        } else {
            bytes[byte] &= !(1 << bit);
        }
    }
    Ok(bytes)
}

fn parse_addr(addr: &str) -> Result<u16, LoaderError> {
    let trimmed = addr.trim_start_matches("0x");
    u16::from_str_radix(trimmed, 16).map_err(|_| LoaderError {
        message: format!("invalid address literal `{addr}`"),
        line: None,
        column: None,
    })
}

fn parse_range(range: &str) -> Result<AddressRange, LoaderError> {
    let mut parts = range.split("..");
    let start = parts
        .next()
        .ok_or_else(|| LoaderError {
            message: format!("invalid address range `{range}`"),
            line: None,
            column: None,
        })
        .and_then(parse_addr)?;
    let end_str = parts.next().ok_or_else(|| LoaderError {
        message: format!("invalid address range `{range}`"),
        line: None,
        column: None,
    })?;
    if parts.next().is_some() {
        return Err(LoaderError {
            message: format!("invalid address range `{range}`"),
            line: None,
            column: None,
        });
    }
    let end_trimmed = end_str.trim_start_matches('=');
    let end = parse_addr(end_trimmed)?;
    if end < start {
        return Err(LoaderError {
            message: format!("address range end {end:#06x} < start {start:#06x}"),
            line: None,
            column: None,
        });
    }
    Ok(AddressRange { start, end })
}

impl InstanceExpr {
    pub fn constant(value: u32) -> Self {
        Self {
            source: format!("{value}"),
            tokens: vec![ExprToken::Number(value)],
        }
    }

    pub fn evaluate(&self, index: u32) -> Result<u32, LoaderError> {
        let mut output = Vec::with_capacity(self.tokens.len());
        for token in &self.tokens {
            output.push(match token {
                ExprToken::Number(n) => ValueOrOp::Value(*n),
                ExprToken::Var => ValueOrOp::Value(index),
                ExprToken::Plus => ValueOrOp::Op(Op::Add),
                ExprToken::Minus => ValueOrOp::Op(Op::Sub),
                ExprToken::Star => ValueOrOp::Op(Op::Mul),
            });
        }
        eval_rpn(&output)
    }

    pub fn source(&self) -> &str {
        &self.source
    }
}

enum ValueOrOp {
    Value(u32),
    Op(Op),
}

#[derive(Copy, Clone)]
enum Op {
    Add,
    Sub,
    Mul,
}

fn eval_rpn(tokens: &[ValueOrOp]) -> Result<u32, LoaderError> {
    let mut stack: Vec<i64> = Vec::new();
    for token in tokens {
        match token {
            ValueOrOp::Value(v) => stack.push(*v as i64),
            ValueOrOp::Op(op) => {
                if stack.len() < 2 {
                    return Err(LoaderError {
                        message: "invalid expression".to_string(),
                        line: None,
                        column: None,
                    });
                }
                let rhs = stack.pop().unwrap();
                let lhs = stack.pop().unwrap();
                let result = match op {
                    Op::Add => lhs + rhs,
                    Op::Sub => lhs - rhs,
                    Op::Mul => lhs * rhs,
                };
                stack.push(result);
            }
        }
    }
    if stack.len() != 1 {
        return Err(LoaderError {
            message: "invalid expression".to_string(),
            line: None,
            column: None,
        });
    }
    let value = stack[0];
    if value < 0 {
        return Err(LoaderError {
            message: "expression evaluated to negative value".to_string(),
            line: None,
            column: None,
        });
    }
    Ok(value as u32)
}

fn parse_instance_expr(expr: &str, index_var: &str) -> Result<InstanceExpr, LoaderError> {
    let tokens = tokenize_expr(expr, index_var)?;
    let rpn = shunting_yard(&tokens)?;
    Ok(InstanceExpr {
        source: expr.trim().to_string(),
        tokens: rpn,
    })
}

#[derive(Clone)]
enum Token {
    Number(u32),
    Var,
    Op(Op),
    LParen,
    RParen,
}

fn tokenize_expr(expr: &str, index_var: &str) -> Result<Vec<Token>, LoaderError> {
    let mut tokens = Vec::new();
    let mut chars = expr.chars().peekable();
    while let Some(ch) = chars.peek().copied() {
        match ch {
            ' ' | '\t' | '\n' | '\r' => {
                chars.next();
            }
            '+' => {
                chars.next();
                tokens.push(Token::Op(Op::Add));
            }
            '-' => {
                chars.next();
                tokens.push(Token::Op(Op::Sub));
            }
            '*' => {
                chars.next();
                tokens.push(Token::Op(Op::Mul));
            }
            '(' => {
                chars.next();
                tokens.push(Token::LParen);
            }
            ')' => {
                chars.next();
                tokens.push(Token::RParen);
            }
            _ => {
                if ch.is_ascii_digit() || ch.is_ascii_hexdigit() {
                    let mut literal = String::new();
                    if ch == '0' {
                        chars.next();
                        if let Some(next) = chars.peek() {
                            if *next == 'x' || *next == 'X' {
                                chars.next();
                                while let Some(c) = chars.peek() {
                                    if c.is_ascii_hexdigit() {
                                        literal.push(*c);
                                        chars.next();
                                    } else {
                                        break;
                                    }
                                }
                                if literal.is_empty() {
                                    return Err(LoaderError {
                                        message: format!(
                                            "invalid literal `0x` in expression `{expr}`"
                                        ),
                                        line: None,
                                        column: None,
                                    });
                                }
                                let value =
                                    u32::from_str_radix(&literal, 16).map_err(|_| LoaderError {
                                        message: format!(
                                            "invalid literal `0x{}` in expression `{expr}`",
                                            literal
                                        ),
                                        line: None,
                                        column: None,
                                    })?;
                                tokens.push(Token::Number(value));
                                continue;
                            } else {
                                literal.push('0');
                            }
                        } else {
                            literal.push('0');
                        }
                    } else {
                        chars.next();
                        literal.push(ch);
                    }
                    while let Some(c) = chars.peek() {
                        if c.is_ascii_hexdigit() {
                            literal.push(*c);
                            chars.next();
                        } else {
                            break;
                        }
                    }
                    let value = u32::from_str_radix(&literal, 16).map_err(|_| LoaderError {
                        message: format!("invalid literal `{}` in expression `{expr}`", literal),
                        line: None,
                        column: None,
                    })?;
                    tokens.push(Token::Number(value));
                } else if ch.is_alphabetic() || ch == '_' {
                    let mut ident = String::new();
                    while let Some(c) = chars.peek() {
                        if c.is_alphanumeric() || *c == '_' {
                            ident.push(*c);
                            chars.next();
                        } else {
                            break;
                        }
                    }
                    if ident == index_var {
                        tokens.push(Token::Var);
                    } else {
                        return Err(LoaderError {
                            message: format!(
                                "unknown identifier `{}` in expression `{expr}`",
                                ident
                            ),
                            line: None,
                            column: None,
                        });
                    }
                } else {
                    return Err(LoaderError {
                        message: format!("unexpected character `{}` in expression `{expr}`", ch),
                        line: None,
                        column: None,
                    });
                }
            }
        }
    }
    Ok(tokens)
}

fn shunting_yard(tokens: &[Token]) -> Result<Vec<ExprToken>, LoaderError> {
    let mut output = Vec::new();
    let mut ops: Vec<Token> = Vec::new();
    for token in tokens {
        match token {
            Token::Number(n) => output.push(ExprToken::Number(*n)),
            Token::Var => output.push(ExprToken::Var),
            Token::Op(op) => {
                while let Some(top) = ops.last() {
                    let push = match (top, op) {
                        (Token::Op(prev), op) if precedence(prev) >= precedence(op) => true,
                        _ => false,
                    };
                    if push {
                        let popped = ops.pop().unwrap();
                        if let Token::Op(o) = popped {
                            output.push(match o {
                                Op::Add => ExprToken::Plus,
                                Op::Sub => ExprToken::Minus,
                                Op::Mul => ExprToken::Star,
                            });
                        }
                    } else {
                        break;
                    }
                }
                ops.push(Token::Op(*op));
            }
            Token::LParen => ops.push(Token::LParen),
            Token::RParen => {
                while let Some(top) = ops.pop() {
                    match top {
                        Token::LParen => break,
                        Token::Op(o) => output.push(match o {
                            Op::Add => ExprToken::Plus,
                            Op::Sub => ExprToken::Minus,
                            Op::Mul => ExprToken::Star,
                        }),
                        _ => {}
                    }
                }
            }
        }
    }
    while let Some(op) = ops.pop() {
        match op {
            Token::Op(o) => output.push(match o {
                Op::Add => ExprToken::Plus,
                Op::Sub => ExprToken::Minus,
                Op::Mul => ExprToken::Star,
            }),
            Token::LParen | Token::RParen => {
                return Err(LoaderError {
                    message: "mismatched parentheses in expression".to_string(),
                    line: None,
                    column: None,
                });
            }
            Token::Number(_) | Token::Var => unreachable!("operands are not pushed to operator stack"),
        }
    }
    Ok(output)
}

fn precedence(op: &Op) -> u8 {
    match op {
        Op::Add | Op::Sub => 1,
        Op::Mul => 2,
    }
}

#[derive(Debug, Deserialize)]
struct RawPersonalityFile {
    personality: RawMetadata,
    #[serde(default)]
    modules: BTreeMap<String, RawModule>,
    #[serde(default, rename = "map")]
    maps: Vec<RawMap>,
    #[serde(default)]
    conditions: BTreeMap<String, RawCondition>,
    #[serde(default)]
    interrupts: Option<RawInterrupts>,
}

#[derive(Debug, Deserialize)]
struct RawMetadata {
    id: String,
    title: String,
    default_map_priority: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct RawModule {
    #[serde(rename = "impl")]
    impl_id: String,
    #[serde(default)]
    options: Table,
}

#[derive(Debug, Deserialize)]
struct RawMap {
    priority: Option<i32>,
    #[serde(rename = "active_when")]
    active_when: Option<String>,
    decode: RawDecode,
}

#[derive(Debug, Deserialize)]
struct RawDecode {
    #[serde(default)]
    range: Option<RawRange>,
    #[serde(default)]
    sparse: Option<Vec<RawSparseEntry>>,
    #[serde(default)]
    instances: Option<RawInstanceMap>,
}

#[derive(Debug, Deserialize)]
struct RawRange {
    addr: String,
    kind: String,
    order: Vec<String>,
    #[serde(default)]
    stride: Option<u16>,
    #[serde(default)]
    default_transform: Option<RawTransform>,
}

#[derive(Debug, Deserialize)]
struct RawSparseEntry {
    addr: String,
    kind: String,
    id: String,
    #[serde(default)]
    transform: Option<RawTransform>,
    #[serde(default)]
    value_builder: Option<Value>,
    #[serde(default)]
    field_policies: Vec<RawFieldPolicy>,
}

#[derive(Debug, Deserialize)]
struct RawInstanceMap {
    kind: String,
    count: usize,
    #[serde(default = "default_index_var")]
    index_var: String,
    #[serde(default)]
    #[allow(dead_code)]
    base: Option<String>,
    #[serde(default)]
    selector: Option<String>,
    #[serde(default)]
    layout: Vec<RawInstanceLayoutEntry>,
}

fn default_index_var() -> String {
    "i".to_string()
}

#[derive(Debug, Deserialize)]
struct RawInstanceLayoutEntry {
    addr: String,
    id: String,
    #[serde(default)]
    transform: Option<RawTransform>,
    #[serde(default)]
    field: Option<RawInstanceField>,
    #[serde(default)]
    field_policies: Vec<RawFieldPolicy>,
}

#[derive(Debug, Deserialize, Default)]
struct RawInstanceField {
    #[serde(default)]
    bit: Option<String>,
    #[serde(default)]
    source_bit: Option<String>,
    #[serde(default)]
    target_bit: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct RawFieldPolicy {
    #[serde(default)]
    lsb: Option<u8>,
    #[serde(default)]
    msb: Option<u8>,
    #[serde(default)]
    on_read: Option<String>,
    #[serde(default)]
    on_write: Option<String>,
    #[serde(default)]
    ro: Option<bool>,
    #[serde(default)]
    wo: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
struct RawTransform {
    #[serde(default)]
    invert_mask: Option<u8>,
    #[serde(default)]
    ro_mask: Option<u8>,
    #[serde(default)]
    wo_mask: Option<u8>,
    #[serde(default)]
    shift: Option<i8>,
    #[serde(default)]
    on_read: Option<String>,
    #[serde(default)]
    on_write: Option<String>,
    #[serde(default)]
    pre_read_sets: Vec<RawRegisterSet>,
    #[serde(default)]
    post_read_sets: Vec<RawRegisterSet>,
    #[serde(default)]
    pre_write_sets: Vec<RawRegisterSet>,
    #[serde(default)]
    post_write_sets: Vec<RawRegisterSet>,
}

#[derive(Debug, Deserialize)]
struct RawRegisterSet {
    kind: String,
    id: String,
    value: u8,
}

#[derive(Debug, Deserialize)]
struct RawCondition {
    kind: String,
    reg: String,
    equals: i32,
}

#[derive(Debug, Deserialize, Default)]
struct RawInterrupts {
    #[serde(default)]
    irq_sources: Vec<RawInterruptSource>,
    #[serde(default)]
    nmi_sources: Vec<RawInterruptSource>,
    #[serde(default)]
    irq_ack: Option<RawInterruptAck>,
    #[serde(default)]
    nmi_ack: Option<RawInterruptAck>,
}

#[derive(Debug, Deserialize)]
struct RawInterruptSource {
    kind: String,
    id: String,
}

#[derive(Debug, Deserialize)]
struct RawInterruptAck {
    kind: String,
    id: String,
    #[serde(default)]
    hook: Option<String>,
}

impl ValueBuilder {
    pub fn build(&self, signals: &mut dyn InputSignals) -> Vec<u8> {
        let mut bytes = self
            .const_set
            .clone()
            .unwrap_or_else(|| vec![0u8; self.width.max(1)]);

        if bytes.len() < self.width {
            bytes.resize(self.width, 0);
        }

        for bit in &self.bits {
            let mut active = signals.get_bool(&bit.src).unwrap_or(false);
            if bit.active_low {
                active = !active;
            }
            if active {
                bytes[bit.byte_index] |= 1 << bit.bit;
            } else {
                bytes[bit.byte_index] &= !(1 << bit.bit);
            }
        }

        if self.invert_byte {
            for byte in &mut bytes {
                *byte = !*byte;
            }
        }

        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin_module_registry;

    #[test]
    fn loads_basic_personality() {
        let toml = r#"
[personality]
id = "modern-retro"
title = "Modern Retro Layout"
default_map_priority = 10

[modules.console]
impl = "console.text"

[modules.display]
impl = "display.basic2d"

[modules.sprite]
impl = "sprite.basic"

[modules.system]
impl = "system.interrupts"

[[map]]
priority = 5
decode = { range = { addr = "DF20..=DF21", kind = "display", order = ["BorderColor","BackgroundColor"] } }

[[map]]
decode = { sparse = [
  { addr="DF00", kind="console", id="WriteChar" },
  { addr="DF01", kind="console", id="Newline" }
] }
"#;

        let registry = builtin_module_registry();
        let personality =
            PersonalityDef::from_toml_str(toml, &registry).expect("should load personality");

        assert_eq!(personality.metadata.id, "modern-retro");
        assert_eq!(personality.modules.len(), 4);
        assert_eq!(personality.maps.len(), 2);
        assert!(personality.conditions.is_empty());

        let display_map = match &personality.maps[0].decode {
            MapDecode::Range(range) => range,
            _ => panic!("expected range map"),
        };
        assert_eq!(display_map.order.len(), 2);
        assert_eq!(display_map.order[0].desc.name, "BorderColor");
    }

    #[test]
    fn rejects_unknown_module_kind() {
        let toml = r#"
[personality]
id = "broken"
title = "Broken"

[modules.unknown]
impl = "console.text"
"#;

        let registry = builtin_module_registry();
        let err = match PersonalityDef::from_toml_str(toml, &registry) {
            Ok(_) => panic!("expected loader to fail"),
            Err(err) => err,
        };
        assert!(
            err.message.contains("unknown module kind `unknown`"),
            "unexpected error: {}",
            err
        );
    }
}
