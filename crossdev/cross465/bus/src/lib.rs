//! cross465 Bus: 64KB RAM + pluggable personalities.
//!
//! This module implements the system bus for a 6502-based virtual machine.
//! It contains:
//!
//! * [`Memory`] — a 64 KB RAM abstraction shared by all MMIO devices.
//! * [`MmioDevice`] — a trait for memory-mapped I/O peripherals.
//! * [`Bus`] — the actual 6502 bus, with RAM and personality-driven MMIO layout.
//! * [`personality`] — descriptors that define which MMIO modules to map and in
//!   which address ranges.
//!
//! ## Personalities & Memory Map
//!
//! By default [`Bus::new`] loads the [`personality::MODERN_RETRO`] descriptor,
//! which maps:
//!
//! ```text
//! $0000–$DFFF   RAM (read/write)
//! $DF00–$DF1F   Console MMIO (text output)
//! $DF20–$DF21   Display MMIO (border/background colours)
//! $DF30–$DF37   Sprite MMIO (sprite slots)
//! $DF38–$FFFB   RAM (read/write)
//! $FFFC–$FFFD   Reset vector
//! $FFFE–$FFFF   NMI vector
//! ```
//!
//! You can construct a bus with a different mapping by calling
//! [`Bus::with_personality`] and supplying a custom descriptor. Each
//! `PersonalityMmio` entry provides a range and a factory for the MMIO module so
//! applications can plug in alternative devices (e.g. different sprite/display
//! implementations) without modifying the bus internals.
//!
//! The [`Bus`] forwards reads/writes in MMIO ranges to their device instead of RAM.

pub mod console_mmio; // expose console device as bus::console_mmio::*
pub mod display_mmio; // expose display device as bus::display_mmio::*
pub mod interrupts; // expose shared interrupt controller helpers
pub mod mmio; // shared module trait/registry scaffold
pub mod personality; // personas describing MMIO layouts
pub mod personality_v2; // data-driven personality definitions and loader
pub mod sprite_mmio; // expose sprite device as bus::sprite_mmio::*
pub mod system_mmio; // expose system-level MMIO (interrupt controller)
pub mod utils; // expose helpers as bus::utils::*

/// Build a registry populated with the built-in module implementations.
pub fn builtin_module_registry() -> mmio::ModuleRegistry {
    let mut registry = mmio::ModuleRegistry::new();
    registry.register(&console_mmio::CONSOLE_FACTORY);
    registry.register(&display_mmio::DISPLAY_FACTORY);
    registry.register(&sprite_mmio::SPRITE_FACTORY);
    registry.register(&system_mmio::SYSTEM_FACTORY);
    registry
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mmio::ModuleKind;
    use crate::personality;

    #[test]
    fn builtin_registry_contains_expected_factories() {
        let registry = builtin_module_registry();
        assert!(registry.by_id("console.text").is_some());
        assert_eq!(
            registry.by_kind(ModuleKind::Display).count(),
            1,
            "exactly one display implementation expected"
        );
    }

    #[test]
    fn bus_from_personality_def_maps_display_registers() {
        let toml = r#"
[personality]
id = "modern-retro"
title = "Modern Retro (Range)"

[modules.display]
impl = "display.basic2d"

[modules.console]
impl = "console.text"

[[map]]
decode = { range = { addr = "DF20..=DF21", kind = "display", order = ["BorderColor","BackgroundColor"] } }
"#;

        let registry = builtin_module_registry();
        let def = personality_v2::PersonalityDef::from_toml_str(toml, &registry)
            .expect("load personality");
        let mut bus = Bus::from_personality_def(def).expect("build bus");

        bus.write(0xDF20, 0x11);
        bus.write(0xDF21, 0x22);

        let snapshot = bus
            .display_output_handle()
            .expect("display handle")
            .lock()
            .unwrap()
            .snapshot();

        assert_eq!(snapshot.border_color, 0x11);
        assert_eq!(snapshot.background_color, 0x22);
    }

    #[test]
    fn value_builder_packs_signal_bits() {
        let toml = r#"
[personality]
id = "signals"
title = "Signal Test"

[modules.system]
impl = "system.interrupts"

[[map]]
decode = { sparse = [ { addr="DF40", kind="system", id="IrqPending", value_builder = { width = 1, bits = [ { bit = 0, src = "irq0" } ], const_set = "00000000" } } ] }
"#;

        let registry = builtin_module_registry();
        let def = personality_v2::PersonalityDef::from_toml_str(toml, &registry)
            .expect("load personality");
        let mut bus = Bus::from_personality_def(def).expect("build bus");

        assert_eq!(bus.read(0xDF40), 0x00);

        bus.set_signal_bool("irq0", true);
        assert_eq!(bus.read(0xDF40), 0x01);

        bus.set_signal_bool("irq0", false);
        assert_eq!(bus.read(0xDF40), 0x00);

        bus.clear_signals();
        assert_eq!(bus.read(0xDF40), 0x00);
    }

    #[test]
    fn modern_retro_range_matches_legacy() {
        let registry = builtin_module_registry();
        let def = personality_v2::PersonalityDef::from_toml_str(
            include_str!("../../personality_defs/modern-retro-range.toml"),
            &registry,
        )
        .expect("load personality");
        let mut bus_v2 = Bus::from_personality_def(def).expect("build v2 bus");
        let mut bus_legacy = Bus::with_personality(personality::default());

        // Display: writes mirror legacy colours.
        bus_v2.write(0xDF20, 0x0E);
        bus_legacy.write(0xDF20, 0x0E);
        bus_v2.write(0xDF21, 0x04);
        bus_legacy.write(0xDF21, 0x04);
        assert_eq!(bus_v2.read(0xDF20), bus_legacy.read(0xDF20));
        assert_eq!(bus_v2.read(0xDF21), bus_legacy.read(0xDF21));
        let v2_display = bus_v2
            .display_output_handle()
            .expect("v2 display handle")
            .lock()
            .unwrap()
            .snapshot();
        let legacy_display = bus_legacy
            .display_output_handle()
            .expect("legacy display handle")
            .lock()
            .unwrap()
            .snapshot();
        assert_eq!(v2_display.border_color, legacy_display.border_color);
        assert_eq!(v2_display.background_color, legacy_display.background_color);

        // Sprite MMIO (slot select + properties).
        bus_v2.write(0xDF30, 0x02);
        bus_legacy.write(0xDF30, 0x02);
        bus_v2.write(0xDF31, 0x07);
        bus_legacy.write(0xDF31, 0x07);
        bus_v2.write(0xDF32, 0x03);
        bus_legacy.write(0xDF32, 0x03);
        bus_v2.write(0xDF33, 0x12);
        bus_legacy.write(0xDF33, 0x12);
        bus_v2.write(0xDF34, 0x34);
        bus_legacy.write(0xDF34, 0x34);
        bus_v2.write(0xDF35, 0x56);
        bus_legacy.write(0xDF35, 0x56);
        bus_v2.write(0xDF36, 0x78);
        bus_legacy.write(0xDF36, 0x78);
        bus_v2.write(0xDF37, 0x21);
        bus_legacy.write(0xDF37, 0x21);
        for addr in 0xDF30..=0xDF37 {
            assert_eq!(
                bus_v2.read(addr),
                bus_legacy.read(addr),
                "sprite register {addr:#06X} mismatch"
            );
        }

        // System MMIO (IRQ enable/pending mirrors).
        bus_v2.write(0xDF41, 0x05);
        bus_legacy.write(0xDF41, 0x05);
        assert_eq!(bus_v2.read(0xDF41), bus_legacy.read(0xDF41));
        // Simulate IRQ raise via controller.
        let controller = bus_v2.interrupt_controller();
        controller.raise_irq(0x02);
        let legacy_controller = bus_legacy.interrupt_controller();
        legacy_controller.raise_irq(0x02);
        assert_eq!(bus_v2.read(0xDF40), bus_legacy.read(0xDF40));
        assert_eq!(bus_v2.read(0xDF43), bus_legacy.read(0xDF43));

        // Console pointer registers (readable slots).
        bus_v2.write(0xDF09, 0xAA);
        bus_legacy.write(0xDF09, 0xAA);
        bus_v2.write(0xDF0A, 0x55);
        bus_legacy.write(0xDF0A, 0x55);
        assert_eq!(bus_v2.read(0xDF09), bus_legacy.read(0xDF09));
        assert_eq!(bus_v2.read(0xDF0A), bus_legacy.read(0xDF0A));
    }

    #[test]
    fn conditions_toggle_sparse_overlays() {
        let toml = r#"
[personality]
id = "banking"
title = "Condition Banking"

[modules.system]
impl = "system.interrupts"

[conditions]
bank1 = { kind = "system", reg = "IrqEnable", equals = 1 }

[[map]]
priority = 0
decode = { sparse = [
  { addr = "DF40", kind = "system", id = "IrqPending" },
  { addr = "DF41", kind = "system", id = "IrqEnable" }
] }

[[map]]
priority = 1
active_when = "bank1"
decode = { sparse = [ { addr = "DF40", kind = "system", id = "IrqEnable" } ] }
"#;

        let registry = builtin_module_registry();
        let def = personality_v2::PersonalityDef::from_toml_str(toml, &registry)
            .expect("load personality");
        let mut bus = Bus::from_personality_def(def).expect("build bus");

        // Initially the lower-priority IrqPending map is active.
        assert_eq!(bus.read(0xDF40), 0x00);

        // Enable IRQ bit via the underlying module; this satisfies `bank1` and should swap to IrqEnable view.
        bus.write(0xDF41, 0x01);
        assert_eq!(bus.read(0xDF40), 0x01);

        // Clearing the enable bit should revert to the pending register mapping.
        bus.write(0xDF41, 0x00);
        assert_eq!(bus.read(0xDF40), 0x00);
    }

    #[test]
    fn c64_sparse_personality_supports_active_low_inputs() {
        let registry = builtin_module_registry();
        let def = personality_v2::PersonalityDef::from_toml_str(
            include_str!("../../personality_defs/c64-compat-sparse.toml"),
            &registry,
        )
        .expect("load c64 personality");
        let mut bus = Bus::from_personality_def(def).expect("c64 bus");

        // Display registers mapped at $D020/$D021.
        bus.write(0xD020, 0x06);
        bus.write(0xD021, 0x03);
        assert_eq!(bus.read(0xD020), 0x06);
        assert_eq!(bus.read(0xD021), 0x03);

        // Sprite 0 registers routed via select pre-sets.
        bus.write(0xD100, 0x11); // Number
        bus.write(0xD102, 0x34); // XLo
        bus.write(0xD104, 0x56); // YLo
        assert_eq!(bus.read(0xD100), 0x11);
        assert_eq!(bus.read(0xD102), 0x34);
        assert_eq!(bus.read(0xD104), 0x56);

        // Sprite 1 should remain independent.
        bus.write(0xD108, 0x22);
        bus.write(0xD10A, 0x78);
        assert_eq!(bus.read(0xD108), 0x22);
        assert_eq!(bus.read(0xD10A), 0x78);
        // Ensure sprite 0 values untouched.
        assert_eq!(bus.read(0xD100), 0x11);
        assert_eq!(bus.read(0xD102), 0x34);

        // Active-low joystick inputs via value builder at $DC00.
        assert_eq!(bus.read(0xDC00), 0xFF);
        bus.set_signal_bool("p0.button_fire", true);
        assert_eq!(bus.read(0xDC00), 0xEF);
        bus.set_signal_bool("p0.dpad_left", true);
        assert_eq!(bus.read(0xDC00), 0xEB);
        bus.clear_signals();
        assert_eq!(bus.read(0xDC00), 0xFF);
    }
}

pub use utils::{cmb_color_to_ansi, petscii_to_unicode, screen_to_petscii, unicode_to_screen}; // convenience re-export

use std::any::Any;
use std::collections::{BTreeMap, HashMap};
use std::ops::RangeInclusive;
use std::sync::{Arc, Mutex};

use crate::mmio::{Module, ModuleDeps, ModuleKind, RegId};
use console_mmio::ConsoleMmio;
use display_mmio::DisplayMmio;
use interrupts::InterruptController;
use personality::{InterruptLine, Personality, PersonalityMmioKind};
use personality_v2::{
    CompileError as PersonalityCompileError, Condition, InputSignals, InterruptConfig, Map,
    MapDecode, PersonalityDef, PersonalityMetadata, Transform, ValueBuilder,
};
use sprite_mmio::SpriteMmio;

/// Represents the flat 64KB RAM array of the 6502 address space.
///
/// [`Memory`] is a thin wrapper around a fixed `[u8; 65536]` array.
/// It provides basic read/write methods and a helper to load binary blobs.
///
/// It does **not** perform any bounds checking beyond masking the address to 16 bits.
/// All bus-level MMIO logic lives in [`Bus`].
pub struct Memory {
    /// Raw 64 KB addressable storage.
    pub data: [u8; 0x10000],
}

impl Memory {
    /// Create a new zero-initialized memory.
    pub fn new() -> Self {
        Self { data: [0; 0x10000] }
    }

    /// Load a contiguous slice of bytes into memory starting at `start`.
    ///
    /// The load wraps around at `$FFFF` if the data slice is longer than the
    /// remaining space in memory.
    pub fn load(&mut self, start: u16, bytes: &[u8]) {
        let mut a = start as usize;
        for &b in bytes {
            self.data[a & 0xFFFF] = b;
            a += 1;
        }
    }

    /// Read a byte from memory at `addr`.
    #[inline]
    pub fn read(&self, addr: u16) -> u8 {
        self.data[addr as usize]
    }

    /// Write a byte to memory at `addr`.
    #[inline]
    pub fn write(&mut self, addr: u16, val: u8) {
        self.data[addr as usize] = val;
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait for memory-mapped I/O devices.
///
/// Devices must implement:
/// - `read`: called on bus reads from the device's address range.
/// - `write`: called on bus writes to the device's address range.
///
/// ### Trait bounds
/// - `Any`: so devices can be downcast for inspection/configuration in tests.
/// - `Send`: so devices can be moved across threads if needed in the future.
pub trait MmioDevice: Any + Send {
    /// Read a byte from the device.
    fn read(&mut self, addr: u16) -> u8;

    /// Write a byte to the device.
    fn write(&mut self, addr: u16, value: u8);
}

impl dyn MmioDevice {
    /// Downcast helper for `&dyn MmioDevice`.
    #[inline]
    pub fn as_any(&self) -> &dyn Any {
        self
    }

    /// Downcast helper for `&mut dyn MmioDevice`.
    #[inline]
    pub fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// The main 6502 system bus: RAM plus pluggable MMIO devices.
///
/// The bus owns:
/// - A [`Memory`] instance for the 64 KB RAM.
/// - A list of `(address_range, device)` pairs for MMIO mapping.
///
/// Reads/writes to addresses inside a mapped MMIO range are delegated to that device.
/// Everything else goes to RAM.
pub struct Bus {
    ram: Arc<Mutex<Memory>>,
    personality_legacy: Option<&'static Personality>,
    mmio: Vec<MappedDevice>,
    controller: Arc<InterruptController>,
    runtime_v2: Option<PersonalityRuntime>,
}

struct MappedDevice {
    range: RangeInclusive<u16>,
    device: Box<dyn MmioDevice>,
    kind: Option<PersonalityMmioKind>,
}

struct PersonalityRuntime {
    #[allow(dead_code)]
    metadata: PersonalityMetadata,
    modules: Vec<ModuleInstance>,
    module_lookup: BTreeMap<ModuleKind, usize>,
    maps: Vec<Map>,
    address_table: Vec<Option<AddressSlot>>,
    conditions: BTreeMap<String, Condition>,
    condition_states: BTreeMap<String, bool>,
    signals: SignalStore,
    #[allow(dead_code)]
    interrupts: InterruptConfig,
}

struct ModuleInstance {
    #[allow(dead_code)]
    kind: ModuleKind,
    #[allow(dead_code)]
    impl_id: String,
    module: Box<dyn Module>,
    #[allow(dead_code)]
    options: crate::mmio::ModuleOptions,
}

#[derive(Clone)]
struct AddressSlot {
    priority: i32,
    module_index: usize,
    reg: RegId,
    transform: Transform,
    value_builder: Option<ValueBuilder>,
    pre_read_sets: Vec<AddressRegisterSet>,
    post_read_sets: Vec<AddressRegisterSet>,
    pre_write_sets: Vec<AddressRegisterSet>,
    post_write_sets: Vec<AddressRegisterSet>,
}

#[derive(Clone)]
struct AddressRegisterSet {
    module_index: usize,
    reg: RegId,
    value: u8,
}

#[derive(Default)]
struct SignalStore {
    bools: HashMap<String, bool>,
    ints: HashMap<String, i32>,
    floats: HashMap<String, f32>,
}

impl SignalStore {
    fn set_bool<S: Into<String>>(&mut self, name: S, value: bool) {
        self.bools.insert(name.into(), value);
    }

    fn set_int<S: Into<String>>(&mut self, name: S, value: i32) {
        self.ints.insert(name.into(), value);
    }

    fn set_float<S: Into<String>>(&mut self, name: S, value: f32) {
        self.floats.insert(name.into(), value);
    }

    fn clear(&mut self) {
        self.bools.clear();
        self.ints.clear();
        self.floats.clear();
    }
}

impl InputSignals for SignalStore {
    fn get_bool(&mut self, name: &str) -> Option<bool> {
        self.bools.get(name).copied()
    }

    fn get_int(&mut self, name: &str) -> Option<i32> {
        self.ints.get(name).copied()
    }

    fn get_f32(&mut self, name: &str) -> Option<f32> {
        self.floats.get(name).copied()
    }
}

fn compile_address_table(
    maps: &[Map],
    lookup: &BTreeMap<ModuleKind, usize>,
    condition_states: &BTreeMap<String, bool>,
) -> Result<Vec<Option<AddressSlot>>, PersonalityCompileError> {
    let mut table = vec![None; 0x10000];
    for map in maps {
        if !map
            .active_when
            .iter()
            .all(|name| *condition_states.get(name).unwrap_or(&false))
        {
            continue;
        }
        match &map.decode {
            MapDecode::Range(range) => {
                compile_range_map(&mut table, map.priority, range, lookup)?;
            }
            MapDecode::Sparse(entries) => {
                compile_sparse_map(&mut table, map.priority, entries, lookup)?;
            }
        }
    }
    Ok(table)
}

fn compile_range_map(
    table: &mut [Option<AddressSlot>],
    priority: i32,
    range: &personality_v2::RangeMap,
    lookup: &BTreeMap<ModuleKind, usize>,
) -> Result<(), PersonalityCompileError> {
    let module_index = *lookup
        .get(&range.module)
        .ok_or(PersonalityCompileError::MissingModule(range.module))?;

    let stride = if range.stride == 0 { 1 } else { range.stride };
    for (idx, register) in range.order.iter().enumerate() {
        let offset = (idx as u32)
            .checked_mul(stride as u32)
            .and_then(|v| u16::try_from(v).ok())
            .ok_or(PersonalityCompileError::AddressOutOfRange {
                addr: range.range.start,
            })?;

        let target_addr = range.range.start.checked_add(offset).ok_or(
            PersonalityCompileError::AddressOutOfRange {
                addr: range.range.start,
            },
        )?;

        if target_addr > range.range.end {
            return Err(PersonalityCompileError::AddressOutOfRange { addr: target_addr });
        }

        let transform = range.default_transform.clone().unwrap_or_default();
        let pre_read_sets = map_register_sets(&transform.pre_read_sets, lookup)?;
        let post_read_sets = map_register_sets(&transform.post_read_sets, lookup)?;
        let pre_write_sets = map_register_sets(&transform.pre_write_sets, lookup)?;
        let post_write_sets = map_register_sets(&transform.post_write_sets, lookup)?;

        let slot = AddressSlot {
            priority,
            module_index,
            reg: register.id,
            transform,
            value_builder: None,
            pre_read_sets,
            post_read_sets,
            pre_write_sets,
            post_write_sets,
        };

        insert_slot(table, target_addr, slot)?;
    }

    Ok(())
}

fn compile_sparse_map(
    table: &mut [Option<AddressSlot>],
    priority: i32,
    entries: &[personality_v2::SparseEntry],
    lookup: &BTreeMap<ModuleKind, usize>,
) -> Result<(), PersonalityCompileError> {
    for entry in entries {
        let module_index = *lookup
            .get(&entry.module)
            .ok_or(PersonalityCompileError::MissingModule(entry.module))?;

        let transform = entry.transform.clone().unwrap_or_default();
        let pre_read_sets = map_register_sets(&transform.pre_read_sets, lookup)?;
        let post_read_sets = map_register_sets(&transform.post_read_sets, lookup)?;
        let pre_write_sets = map_register_sets(&transform.pre_write_sets, lookup)?;
        let post_write_sets = map_register_sets(&transform.post_write_sets, lookup)?;

        let slot = AddressSlot {
            priority,
            module_index,
            reg: entry.register.id,
            transform,
            value_builder: entry.value_builder.clone(),
            pre_read_sets,
            post_read_sets,
            pre_write_sets,
            post_write_sets,
        };

        insert_slot(table, entry.addr, slot)?;
    }

    Ok(())
}

fn map_register_sets(
    sets: &[personality_v2::RegisterSet],
    lookup: &BTreeMap<ModuleKind, usize>,
) -> Result<Vec<AddressRegisterSet>, PersonalityCompileError> {
    let mut resolved = Vec::with_capacity(sets.len());
    for set in sets {
        let module_index = *lookup
            .get(&set.module)
            .ok_or(PersonalityCompileError::MissingModule(set.module))?;
        resolved.push(AddressRegisterSet {
            module_index,
            reg: set.register.id,
            value: set.value,
        });
    }
    Ok(resolved)
}

fn insert_slot(
    table: &mut [Option<AddressSlot>],
    addr: u16,
    slot: AddressSlot,
) -> Result<(), PersonalityCompileError> {
    let priority = slot.priority;
    let cell = &mut table[addr as usize];
    match cell {
        Some(existing) => {
            if existing.priority == priority {
                return Err(PersonalityCompileError::AddressOverlap {
                    addr,
                    existing_priority: existing.priority,
                    new_priority: priority,
                });
            }
            if priority > existing.priority {
                *existing = slot;
            }
        }
        None => {
            *cell = Some(slot);
        }
    }
    Ok(())
}

impl PersonalityRuntime {
    fn from_def(
        def: PersonalityDef,
        ram: Arc<Mutex<Memory>>,
        controller: Arc<InterruptController>,
    ) -> Result<Self, PersonalityCompileError> {
        let PersonalityDef {
            metadata,
            modules,
            conditions,
            maps,
            interrupts,
        } = def;

        let mut instances = Vec::with_capacity(modules.len());
        let mut lookup = BTreeMap::new();

        for (kind, config) in modules {
            let deps = ModuleDeps::new(ram.clone(), controller.clone());
            let module = config.factory.create(&deps, &config.options);
            let index = instances.len();
            lookup.insert(kind, index);
            instances.push(ModuleInstance {
                kind,
                impl_id: config.impl_id,
                module,
                options: config.options,
            });
        }

        let mut runtime = Self {
            metadata,
            modules: instances,
            module_lookup: lookup,
            maps,
            address_table: Vec::new(),
            conditions,
            condition_states: BTreeMap::new(),
            signals: SignalStore::default(),
            interrupts,
        };

        runtime.update_condition_states()?;
        runtime.rebuild_address_table()?;

        Ok(runtime)
    }

    fn read(&mut self, addr: u16) -> Option<u8> {
        let slot = match self
            .address_table
            .get(addr as usize)
            .and_then(|cell| cell.as_ref())
        {
            Some(slot) => slot.clone(),
            None => return None,
        };

        self.apply_register_sets(&slot.pre_read_sets);

        let mut value = if let Some(builder) = slot.value_builder.as_ref() {
            builder
                .build(&mut self.signals)
                .get(0)
                .copied()
                .unwrap_or(0)
        } else {
            let module = self.modules.get_mut(slot.module_index)?.module.as_mut();
            module.read_reg(slot.reg)
        };

        value = apply_shift(value, slot.transform.shift);
        value ^= slot.transform.invert_mask;
        if slot.transform.wo_mask != 0 {
            value &= !slot.transform.wo_mask;
        }

        self.apply_register_sets(&slot.post_read_sets);

        Some(value)
    }

    fn write(&mut self, addr: u16, mut value: u8) -> bool {
        let slot = match self
            .address_table
            .get(addr as usize)
            .and_then(|cell| cell.as_ref())
        {
            Some(slot) => slot.clone(),
            None => return false,
        };

        self.apply_register_sets(&slot.pre_write_sets);

        let module = match self.modules.get_mut(slot.module_index) {
            Some(m) => m.module.as_mut(),
            None => return false,
        };

        value = reverse_shift(value, slot.transform.shift);
        value ^= slot.transform.invert_mask;

        if slot.transform.ro_mask != 0 {
            let current = module.read_reg(slot.reg);
            value = (value & !slot.transform.ro_mask) | (current & slot.transform.ro_mask);
        }

        module.write_reg(slot.reg, value);
        self.apply_register_sets(&slot.post_write_sets);
        self.recompute_conditions_if_needed();
        true
    }

    fn apply_register_sets(&mut self, sets: &[AddressRegisterSet]) {
        for set in sets {
            if let Some(instance) = self.modules.get_mut(set.module_index) {
                instance.module.write_reg(set.reg, set.value);
            }
        }
    }

    fn module_index(&self, kind: ModuleKind) -> Option<usize> {
        self.module_lookup.get(&kind).copied()
    }

    fn update_condition_states(&mut self) -> Result<bool, PersonalityCompileError> {
        let mut changed = false;
        for (name, cond) in &self.conditions {
            let module_index = *self
                .module_lookup
                .get(&cond.module)
                .ok_or(PersonalityCompileError::MissingModule(cond.module))?;
            let module = &mut self.modules[module_index].module;
            let value = module.read_reg(cond.register.id);
            let satisfied = (value as i32) == cond.equals;
            let previous = self.condition_states.insert(name.clone(), satisfied);
            if previous.map(|prev| prev != satisfied).unwrap_or(true) {
                changed = true;
            }
        }
        Ok(changed)
    }

    fn rebuild_address_table(&mut self) -> Result<(), PersonalityCompileError> {
        self.address_table =
            compile_address_table(&self.maps, &self.module_lookup, &self.condition_states)?;
        Ok(())
    }

    fn recompute_conditions_if_needed(&mut self) {
        match self.update_condition_states() {
            Ok(true) => {
                if let Err(err) = self.rebuild_address_table() {
                    panic!(
                        "failed to rebuild address table after condition update: {}",
                        err
                    );
                }
            }
            Ok(false) => {}
            Err(err) => panic!("failed to update condition states: {}", err),
        }
    }

    fn module(&self, kind: ModuleKind) -> Option<&dyn Module> {
        let index = self.module_index(kind)?;
        Some(self.modules[index].module.as_ref())
    }

    fn module_mut(&mut self, kind: ModuleKind) -> Option<&mut dyn Module> {
        let index = self.module_index(kind)?;
        Some(self.modules[index].module.as_mut())
    }

    fn module_downcast<T: 'static>(&self, kind: ModuleKind) -> Option<&T> {
        self.module(kind).and_then(|module| {
            let device: &dyn crate::MmioDevice = module;
            device.as_any().downcast_ref::<T>()
        })
    }

    fn module_downcast_mut<T: 'static>(&mut self, kind: ModuleKind) -> Option<&mut T> {
        self.module_mut(kind).and_then(|module| {
            let device: &mut dyn crate::MmioDevice = module;
            device.as_any_mut().downcast_mut::<T>()
        })
    }

    fn console_output_handle(&self) -> Option<Arc<Mutex<console_mmio::ConsoleOutput>>> {
        self.module_downcast::<ConsoleMmio>(ModuleKind::Console)
            .map(|console| console.output())
    }

    fn display_output_handle(&self) -> Option<Arc<Mutex<display_mmio::DisplayOutput>>> {
        self.module_downcast::<DisplayMmio>(ModuleKind::Display)
            .map(|display| display.output())
    }

    fn sprite_output_handle(&self) -> Option<Arc<Mutex<sprite_mmio::SpriteOutput>>> {
        self.module_downcast::<SpriteMmio>(ModuleKind::Sprite)
            .map(|sprite| sprite.output())
    }

    fn clear_console_buffer(&mut self) {
        if let Some(console) = self.module_downcast_mut::<ConsoleMmio>(ModuleKind::Console) {
            console.clear();
        }
    }

    fn set_console_petscii(&mut self, petscii: bool) {
        if let Some(console) = self.module_downcast_mut::<ConsoleMmio>(ModuleKind::Console) {
            console.petscii_mode = petscii;
        }
    }

    fn set_signal_bool<S: Into<String>>(&mut self, name: S, value: bool) {
        self.signals.set_bool(name, value);
    }

    fn set_signal_int<S: Into<String>>(&mut self, name: S, value: i32) {
        self.signals.set_int(name, value);
    }

    fn set_signal_float<S: Into<String>>(&mut self, name: S, value: f32) {
        self.signals.set_float(name, value);
    }

    fn clear_signals(&mut self) {
        self.signals.clear();
    }
}

fn apply_shift(value: u8, shift: i8) -> u8 {
    if shift > 0 {
        value.wrapping_shl(shift as u32)
    } else if shift < 0 {
        value.wrapping_shr((-shift) as u32)
    } else {
        value
    }
}

fn reverse_shift(value: u8, shift: i8) -> u8 {
    if shift > 0 {
        value.wrapping_shr(shift as u32)
    } else if shift < 0 {
        value.wrapping_shl((-shift) as u32)
    } else {
        value
    }
}

impl Bus {
    /// Active personality descriptor backing this bus.
    pub fn personality(&self) -> Option<&'static Personality> {
        self.personality_legacy
    }

    /// Create a RAM-only bus and map a default [`ConsoleMmio`] at `$DF00–$DF1F`.
    pub fn new() -> Self {
        Self::with_personality(personality::default())
    }

    /// Construct the bus using the specified personality.
    pub fn with_personality(personality: &'static Personality) -> Self {
        let ram = Arc::new(Mutex::new(Memory::new()));
        let controller = Arc::new(InterruptController::new());
        let mut bus = Self {
            ram: ram.clone(),
            personality_legacy: Some(personality),
            mmio: Vec::new(),
            controller: controller.clone(),
            runtime_v2: None,
        };
        for mapping in personality.mmio {
            let device = (mapping.create)(&ram, &controller);
            bus.map_mmio_internal(mapping.range.clone(), device, Some(mapping.kind));
        }
        let mut irq_enable = 0u32;
        for interrupt in personality.interrupts {
            if interrupt.default_enable && matches!(interrupt.line, InterruptLine::Irq) {
                irq_enable |= 1u32 << interrupt.id;
            }
        }
        bus.controller.set_irq_enable(irq_enable);
        bus
    }

    /// Construct the bus from a data-driven [`PersonalityDef`].
    pub fn from_personality_def(def: PersonalityDef) -> Result<Self, PersonalityCompileError> {
        let ram = Arc::new(Mutex::new(Memory::new()));
        let controller = Arc::new(InterruptController::new());
        let runtime = PersonalityRuntime::from_def(def, ram.clone(), controller.clone())?;
        Ok(Self {
            ram,
            personality_legacy: None,
            mmio: Vec::new(),
            controller,
            runtime_v2: Some(runtime),
        })
    }

    /// Map an MMIO device to a specific address range (inclusive).
    pub fn map_mmio(&mut self, range: RangeInclusive<u16>, dev: Box<dyn MmioDevice>) {
        self.map_mmio_internal(range, dev, None);
    }

    /// Load a contiguous slice into RAM starting at `at`.
    pub fn load(&mut self, at: u16, bytes: &[u8]) {
        let mut mem = self.ram.lock().unwrap();
        mem.load(at, bytes);
    }

    /// Set the reset vector (`$FFFC/$FFFD`) to `addr`.
    pub fn set_reset_vector(&mut self, addr: u16) {
        self.write(0xFFFC, (addr & 0xFF) as u8);
        self.write(0xFFFD, (addr >> 8) as u8);
    }

    /// Read a byte from the bus (MMIO devices intercept their ranges).
    pub fn read(&mut self, addr: u16) -> u8 {
        if let Some(runtime) = self.runtime_v2.as_mut() {
            if let Some(value) = runtime.read(addr) {
                return value;
            }
        }
        if let Some(dev) = self.find_mmio(addr) {
            return dev.read(addr);
        }
        let mem = self.ram.lock().unwrap();
        mem.read(addr)
    }

    /// Write a byte to the bus (MMIO devices intercept their ranges).
    pub fn write(&mut self, addr: u16, value: u8) {
        if let Some(runtime) = self.runtime_v2.as_mut() {
            if runtime.write(addr, value) {
                return;
            }
        }
        if let Some(dev) = self.find_mmio(addr) {
            dev.write(addr, value);
            return;
        }
        let mut mem = self.ram.lock().unwrap();
        mem.write(addr, value);
    }

    /// Access the shared interrupt controller.
    pub fn interrupt_controller(&self) -> Arc<InterruptController> {
        self.controller.clone()
    }

    /// Optional timing hook (no-op). Can be overridden to simulate cycles.
    pub fn tick(&mut self, _cycles: u32) {}

    /// Search for an MMIO device covering `addr`.
    pub fn find_mmio(&mut self, addr: u16) -> Option<&mut dyn MmioDevice> {
        for mapped in self.mmio.iter_mut() {
            if mapped.range.contains(&addr) {
                return Some(mapped.device.as_mut());
            }
        }
        None
    }

    /// Mutable access to the underlying RAM (returns a lock guard).
    pub fn mem_mut(&self) -> std::sync::MutexGuard<'_, Memory> {
        self.ram.lock().unwrap()
    }

    // ===== Helpers for tests / host integration =====

    /// Enable/disable PETSCII translation on the default console device.
    pub fn with_console_petscii(mut self, petscii: bool) -> Self {
        if let Some(runtime) = self.runtime_v2.as_mut() {
            runtime.set_console_petscii(petscii);
            return self;
        }
        for mapped in self.mmio.iter_mut() {
            if mapped.kind == Some(PersonalityMmioKind::Console) {
                if let Some(c) = mapped.device.as_any_mut().downcast_mut::<ConsoleMmio>() {
                    c.petscii_mode = petscii;
                }
            }
        }
        self
    }

    /// Expose the console device's shared output buffer.
    pub fn console_output_handle(&self) -> Option<Arc<Mutex<console_mmio::ConsoleOutput>>> {
        if let Some(runtime) = &self.runtime_v2 {
            if let Some(handle) = runtime.console_output_handle() {
                return Some(handle);
            }
        }
        for mapped in self.mmio.iter() {
            if mapped.kind == Some(PersonalityMmioKind::Console) {
                if let Some(c) = mapped.device.as_any().downcast_ref::<ConsoleMmio>() {
                    return Some(c.output());
                }
            }
        }
        None
    }

    /// Expose the display device's shared output buffer (border/background).
    pub fn display_output_handle(&self) -> Option<Arc<Mutex<display_mmio::DisplayOutput>>> {
        if let Some(runtime) = &self.runtime_v2 {
            if let Some(handle) = runtime.display_output_handle() {
                return Some(handle);
            }
        }
        for mapped in self.mmio.iter() {
            if mapped.kind == Some(PersonalityMmioKind::Display) {
                if let Some(d) = mapped.device.as_any().downcast_ref::<DisplayMmio>() {
                    return Some(d.output());
                }
            }
        }
        None
    }

    /// Expose the sprite device's shared output buffer.
    pub fn sprite_output_handle(&self) -> Option<Arc<Mutex<sprite_mmio::SpriteOutput>>> {
        if let Some(runtime) = &self.runtime_v2 {
            if let Some(handle) = runtime.sprite_output_handle() {
                return Some(handle);
            }
        }
        for mapped in self.mmio.iter() {
            if mapped.kind == Some(PersonalityMmioKind::Sprite) {
                if let Some(s) = mapped.device.as_any().downcast_ref::<SpriteMmio>() {
                    return Some(s.output());
                }
            }
        }
        None
    }

    /// Read back the console buffer (if present) as a snapshot string.
    pub fn console_buffer(&self) -> Option<String> {
        self.console_output_handle()
            .and_then(|handle| handle.lock().ok().map(|guard| guard.to_plain_string()))
    }

    /// Clear the console buffer (if present).
    pub fn clear_console_buffer(&mut self) {
        if let Some(runtime) = self.runtime_v2.as_mut() {
            runtime.clear_console_buffer();
            return;
        }
        for mapped in self.mmio.iter_mut() {
            if mapped.kind == Some(PersonalityMmioKind::Console) {
                if let Some(c) = mapped.device.as_any_mut().downcast_mut::<ConsoleMmio>() {
                    c.clear();
                }
            }
        }
    }

    pub fn set_signal_bool<S: Into<String>>(&mut self, name: S, value: bool) {
        if let Some(runtime) = self.runtime_v2.as_mut() {
            runtime.set_signal_bool(name, value);
        }
    }

    pub fn set_signal_int<S: Into<String>>(&mut self, name: S, value: i32) {
        if let Some(runtime) = self.runtime_v2.as_mut() {
            runtime.set_signal_int(name, value);
        }
    }

    pub fn set_signal_float<S: Into<String>>(&mut self, name: S, value: f32) {
        if let Some(runtime) = self.runtime_v2.as_mut() {
            runtime.set_signal_float(name, value);
        }
    }

    pub fn clear_signals(&mut self) {
        if let Some(runtime) = self.runtime_v2.as_mut() {
            runtime.clear_signals();
        }
    }
}

impl Bus {
    fn map_mmio_internal(
        &mut self,
        range: RangeInclusive<u16>,
        dev: Box<dyn MmioDevice>,
        kind: Option<PersonalityMmioKind>,
    ) {
        self.mmio.push(MappedDevice {
            range,
            device: dev,
            kind,
        });
    }
}
