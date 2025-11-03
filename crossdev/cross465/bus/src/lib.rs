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

pub mod adapters; // higher-level adapters bridging modules to modern backends
pub mod console_mmio; // expose console device as bus::console_mmio::*
pub mod display_mmio; // expose display device as bus::display_mmio::*
pub mod input_mmio; // expose input device as bus::input_mmio::*
pub mod interrupts; // expose shared interrupt controller helpers
pub mod mmio; // shared module trait/registry scaffold
pub mod personality; // personas describing MMIO layouts
pub mod personality_v2; // data-driven personality definitions and loader
pub mod sprite_mmio; // expose sprite device as bus::sprite_mmio::*
pub mod system_mmio; // expose system-level MMIO (interrupt controller)
pub mod utils; // expose helpers as bus::utils::*

use system_mmio::SystemMmio;

pub use adapters::display::{DisplayAdapter, DisplayBackend, DisplayOutputBackend};
pub use adapters::input::{InputAdapter, InputBackend, InputBackendHandle};
pub use adapters::sprite::{SpriteAdapter, SpriteBackend, SpriteOutputBackend, SpriteRenderState};
pub use adapters::video::{
    RasterIrqState, VideoAdapter, VideoBackend, VideoState, VideoStateBackend, RASTER_IRQ_MASK,
};

/// Resolved mapping entry produced by the personality compiler.
#[derive(Clone, Debug)]
pub struct AddressMapping {
    pub addr: u16,
    pub priority: i32,
    pub module: ModuleKind,
    pub module_impl_id: String,
    pub register: RegId,
    pub register_name: &'static str,
    pub value_builder: bool,
    pub compute: Option<String>,
    pub transform: TransformInfo,
    pub mapping: MappingDetail,
    pub field_hooks: Vec<FieldHookInfo>,
    pub suppress_primary: bool,
}

/// High-level mapping kind for an address slot.
#[derive(Clone, Debug)]
pub enum MappingDetail {
    Direct,
    DirectInstance {
        instance: u8,
    },
    Scatter {
        target_bit: u8,
        source_bit: u8,
        instance: Option<u8>,
    },
}

/// Transform summary used when inspecting compiled maps.
#[derive(Clone, Debug, Default)]
pub struct TransformInfo {
    pub invert_mask: u8,
    pub ro_mask: u8,
    pub wo_mask: u8,
    pub shift: i8,
    pub on_read: Option<String>,
    pub on_write: Option<String>,
}

/// Field-level hook derived from `field_policies`.
#[derive(Clone, Debug)]
pub struct FieldHookInfo {
    pub mask: u8,
    pub on_read: Option<String>,
    pub on_write: Option<String>,
}

/// Errors raised when registering module adapters.
#[derive(Debug)]
pub enum AdapterError {
    LegacyPersonality,
    ModuleNotMapped(ModuleKind),
    AdapterAlreadyAttached(ModuleKind),
}

impl fmt::Display for AdapterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AdapterError::LegacyPersonality => {
                write!(f, "adapter registration requires a v2 personality")
            }
            AdapterError::ModuleNotMapped(kind) => {
                write!(
                    f,
                    "module kind `{}` is not mapped in this personality",
                    kind.as_str()
                )
            }
            AdapterError::AdapterAlreadyAttached(kind) => {
                write!(
                    f,
                    "an adapter is already attached to module kind `{}`",
                    kind.as_str()
                )
            }
        }
    }
}

impl std::error::Error for AdapterError {}

/// Build a registry populated with the built-in module implementations.
pub fn builtin_module_registry() -> mmio::ModuleRegistry {
    let mut registry = mmio::ModuleRegistry::new();
    registry.register(&console_mmio::CONSOLE_FACTORY);
    registry.register(&display_mmio::DISPLAY_FACTORY);
    registry.register(&input_mmio::INPUT_FACTORY);
    registry.register(&sprite_mmio::SPRITE_FACTORY);
    registry.register(&system_mmio::SYSTEM_FACTORY);
    registry
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mmio::{
        HookAction, Module, ModuleDeps, ModuleFactory, ModuleKind, ModuleOptions, RegId,
        RegisterDesc, SystemReg,
    };
    use crate::personality;
    use crate::MmioDevice;

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

        // Input ports default to inactive (all 1s).
        assert_eq!(bus.read(0xDC00), 0xFF);
        assert_eq!(bus.read(0xDC01), 0xFF);

        let input_handle = bus
            .input_output_handle()
            .expect("input output handle available");
        {
            let mut input = input_handle.lock().unwrap();
            input.set_pad_port_a(0, 0xEF);
            input.set_pad_port_a(1, 0xF7);
        }
        {
            let input = input_handle.lock().unwrap();
            assert_eq!(input.pad_snapshot(0).port_a, 0xEF);
            assert_eq!(input.pad_snapshot(1).port_a, 0xF7);
        }
        assert_eq!(bus.read(0xDC00), 0xEF);
        assert_eq!(bus.read(0xDC01), 0xF7);

        {
            let mut input = input_handle.lock().unwrap();
            input.set_pot_x(0x7F);
            input.set_pot_y(0x90);
        }
        assert_eq!(bus.read(0xD419), 0x7F);
        assert_eq!(bus.read(0xD41A), 0x90);
    }

    #[test]
    fn sprite_instance_map_supports_scatter_bits() {
        let toml = r#"
[personality]
id = "sprite-instance"
title = "Sprite Instance Demo"

[modules.sprite]
impl = "sprite.basic"

[[map]]
priority = 10

[map.decode.instances]
kind = "sprite"
count = 8
index_var = "i"

[[map.decode.instances.layout]]
addr = "D000 + (i*2)"
id = "XLo"

[[map.decode.instances.layout]]
addr = "D001 + (i*2)"
id = "YLo"

[[map.decode.instances.layout]]
addr = "D010"
id = "XHi"
field = { bit = "i" }
"#;

        let registry = builtin_module_registry();
        let def =
            personality_v2::PersonalityDef::from_toml_str(toml, &registry).expect("load instances");
        let mut bus = Bus::from_personality_def(def).expect("instance bus");

        let sprite_handle = bus
            .sprite_output_handle()
            .expect("sprite output handle available");

        let sprite_index = 3u16;
        let xlo_addr = 0xD000u16 + sprite_index * 2;
        let ylo_addr = 0xD001u16 + sprite_index * 2;

        bus.write(xlo_addr, 0x34);
        bus.write(ylo_addr, 0x78);

        // ensure writes target the selected sprite slot automatically
        {
            let snapshot = sprite_handle.lock().unwrap().snapshot();
            let sprite = snapshot.sprite(sprite_index as usize).unwrap();
            assert_eq!(sprite.x & 0x00FF, 0x34);
            assert_eq!(sprite.y & 0x00FF, 0x78);

            let sprite0 = snapshot.sprite(0).unwrap();
            assert_eq!(sprite0.x, 0);
            assert_eq!(sprite0.y, 0);
        }

        // Set high X bit through the shared scatter register at $D010.
        let hi_mask = 1u8 << (sprite_index as u8);
        bus.write(0xD010, hi_mask);
        assert_eq!(bus.read(0xD010), hi_mask);

        {
            let snapshot = sprite_handle.lock().unwrap().snapshot();
            let sprite = snapshot.sprite(sprite_index as usize).unwrap();
            assert_eq!(sprite.x, 0x134);
        }

        // Clear the high bit again.
        bus.write(0xD010, 0);
        assert_eq!(bus.read(0xD010), 0);
        {
            let snapshot = sprite_handle.lock().unwrap().snapshot();
            let sprite = snapshot.sprite(sprite_index as usize).unwrap();
            assert_eq!(sprite.x, 0x34);
        }
    }

    #[test]
    fn write_fanout_sets_sprite_enable() {
        let toml = r#"
[personality]
id = "sprite-enable"
title = "Sprite Enable Fanout"

[modules.sprite]
impl = "sprite.basic"

[[map]]
priority = 10
decode = { instances = { kind = "sprite", selector = "Select", count = 3, index_var = "i", layout = [
  { addr = "D000 + (i*4)", id = "Number" },
  { addr = "D001 + (i*4)", id = "XLo" },
  { addr = "D002 + (i*4)", id = "YLo" },
  { addr = "D003 + (i*4)", id = "Scale" }
] } }

[[map]]
priority = 5
decode = { sparse = [
  { addr = "D015", kind = "sprite", id = "Enable", suppress_write = true, write_fanout = [{ id = "Enable", instance = "*", from_bits = "0..2" }] }
] }
"#;

        let registry = builtin_module_registry();
        let def = personality_v2::PersonalityDef::from_toml_str(toml, &registry)
            .expect("load personality");
        let mut bus = Bus::from_personality_def(def).expect("build bus");

        assert_eq!(bus.read(0xD015), 0x00);

        bus.write(0xD015, 0b0000_0101);
        assert_eq!(bus.read(0xD015), 0b0000_0101);

        let snapshot = bus
            .sprite_output_handle()
            .expect("sprite output")
            .lock()
            .unwrap()
            .snapshot();
        assert!(snapshot.sprite(0).unwrap().enabled);
        assert!(!snapshot.sprite(1).unwrap().enabled);
        assert!(snapshot.sprite(2).unwrap().enabled);

        bus.write(0xD015, 0x00);
        assert_eq!(bus.read(0xD015), 0x00);
        let snapshot = bus
            .sprite_output_handle()
            .expect("sprite output")
            .lock()
            .unwrap()
            .snapshot();
        assert!(!snapshot.sprite(0).unwrap().enabled);
        assert!(!snapshot.sprite(1).unwrap().enabled);
        assert!(!snapshot.sprite(2).unwrap().enabled);
    }

    #[test]
    fn mirror_replicates_base_range() {
        let toml = r#"
[personality]
id = "mirror-test"
title = "Mirror Test"

[[mirror]]
range = "D000..=D00F"
period = 0x001

[modules.display]
impl = "display.basic2d"

[[map]]
priority = 10
decode = { sparse = [
  { addr = "D000", kind = "display", id = "BorderColor" }
] }
"#;

        let registry = builtin_module_registry();
        let def = personality_v2::PersonalityDef::from_toml_str(toml, &registry)
            .expect("load mirror personality");
        let mut bus = Bus::from_personality_def(def).expect("build bus");

        bus.write(0xD000, 0x0F);
        assert_eq!(bus.read(0xD000), 0x0F);
        assert_eq!(bus.read(0xD005), 0x0F);
    }

    #[test]
    fn open_bus_last_read_returns_previous_value() {
        let toml = r#"
[personality]
id = "open-bus"
title = "Open Bus Mirror"

[[open_bus]]
range = "D0F0..=D0FF"
policy = "last_read"

[modules.display]
impl = "display.basic2d"

[[map]]
priority = 10
decode = { sparse = [
  { addr = "D020", kind = "display", id = "BorderColor" }
] }
"#;

        let registry = builtin_module_registry();
        let def = personality_v2::PersonalityDef::from_toml_str(toml, &registry)
            .expect("load open bus personality");
        let mut bus = Bus::from_personality_def(def).expect("build bus");

        bus.write(0xD020, 0x12);
        assert_eq!(bus.read(0xD020), 0x12);

        assert_eq!(bus.read(0xD0F0), 0x12);

        bus.write(0xD0F0, 0x34); // ignored

        {
            let mem = bus.mem_mut();
            assert_eq!(mem.read(0xD0F0), 0);
        }
    }

    #[test]
    fn compute_expression_reads_signals() {
        let toml = r#"
[personality]
id = "compute-demo"
title = "Compute Expression Demo"

[modules.system]
impl = "system.interrupts"

[[map]]
priority = 10
decode = { sparse = [
  { addr = "D012", kind = "system", id = "RasterLo", compute = "beam.y & 0xFF", suppress_write = true }
] }
"#;

        let registry = builtin_module_registry();
        let def = personality_v2::PersonalityDef::from_toml_str(toml, &registry)
            .expect("load compute personality");
        let mut bus = Bus::from_personality_def(def).expect("build bus");

        assert_eq!(bus.read(0xD012), 0x00);

        bus.set_signal_int("beam.y", 0x123);
        assert_eq!(bus.read(0xD012), 0x23);

        bus.set_signal_int("beam.y", 0x00);
        assert_eq!(bus.read(0xD012), 0x00);
    }

    #[test]
    fn raster_compare_write_triggers_irq_via_adapter() {
        let toml = r#"
[personality]
id = "raster-demo"
title = "Raster IRQ Demo"

[modules.system]
impl = "system.interrupts"

[[map]]
priority = 10
decode = { range = { addr = "DF40..=DF4B", kind = "system", order = [
  "IrqPending","IrqEnable","IrqAck","IrqSource",
  "NmiPending","NmiAck","Status","RasterLo","RasterCompareLo",
  "RasterCompareHi","SpriteCollisions","BackgroundCollisions"
] } }
"#;

        let registry = builtin_module_registry();
        let def = personality_v2::PersonalityDef::from_toml_str(toml, &registry)
            .expect("load raster personality");
        let mut bus = Bus::from_personality_def(def).expect("build bus");

        let raster_state = Arc::new(RasterIrqState::new());
        bus.attach_raster_irq_state(raster_state.clone());

        let controller = bus.interrupt_controller();
        controller.set_irq_enable(RASTER_IRQ_MASK);

        let video_state = Arc::new(Mutex::new(VideoState::default()));
        let backend: Arc<dyn VideoBackend> = Arc::new(VideoStateBackend::new(video_state));
        bus.attach_adapter(
            ModuleKind::System,
            Box::new(VideoAdapter::new(
                backend,
                controller.clone(),
                Some(raster_state),
            )),
        )
        .expect("attach video adapter");

        // Write compare (lo/high) and confirm no IRQ until the raster matches.
        bus.write(0xDF48, 0x50);
        bus.write(0xDF49, 0x00);
        bus.write(0xDF47, 0x10);
        assert_eq!(controller.irq_pending(), 0);

        // Update raster to the compare value and expect the IRQ bit to assert.
        bus.write(0xDF47, 0x50);
        assert_eq!(controller.irq_pending(), RASTER_IRQ_MASK);

        // Acknowledge and ensure the source clears.
        bus.write(0xDF42, RASTER_IRQ_MASK as u8);
        assert_eq!(controller.irq_pending(), 0);

        // Move the compare high byte away from the current raster so no IRQ is raised.
        bus.write(0xDF49, 0x01);
        bus.write(0xDF47, 0x50);
        assert_eq!(controller.irq_pending(), 0);
    }

    #[test]
    fn field_policy_read_hook_clears_bits() {
        let toml = r#"
[personality]
id = "field-hooks"
title = "Field Hook Demo"

[modules.system]
impl = "system.hooks"

[[map]]
decode = { sparse = [
  { addr = "DE00", kind = "system", id = "Status", field_policies = [
      { lsb = 0, msb = 0, on_read = "clear_bits", ro = true }
  ] }
] }
"#;

        let mut registry = mmio::ModuleRegistry::new();
        registry.register(&HOOKS_FACTORY);

        let def = personality_v2::PersonalityDef::from_toml_str(toml, &registry)
            .expect("load field hook personality");
        let mut bus = Bus::from_personality_def(def).expect("build bus with hooks");

        // Initial read returns the latched bit and triggers the on_read hook to clear it.
        assert_eq!(bus.read(0xDE00), 0x01);
        assert_eq!(bus.read(0xDE00), 0x00);

        // Writes cannot set the read-only bit back to 1.
        bus.write(0xDE00, 0xFF);
        assert_eq!(bus.read(0xDE00), 0xFE);
    }

    const HOOK_REGS: &[RegisterDesc] = &[RegisterDesc::new(
        RegId::System(SystemReg::Status),
        "Status",
        1,
        0,
        true,
        true,
        &[],
    )];

    #[derive(Default)]
    struct HooksModule {
        value: u8,
    }

    impl MmioDevice for HooksModule {
        fn read(&mut self, _addr: u16) -> u8 {
            self.value
        }

        fn write(&mut self, _addr: u16, value: u8) {
            self.value = value;
        }
    }

    impl Module for HooksModule {
        fn kind(&self) -> ModuleKind {
            ModuleKind::System
        }

        fn regs(&self) -> &'static [RegisterDesc] {
            HOOK_REGS
        }

        fn read_reg(&mut self, _reg: RegId) -> u8 {
            self.value
        }

        fn write_reg(&mut self, _reg: RegId, value: u8) {
            self.value = value;
        }

        fn handle_hook(&mut self, hook: &str, action: HookAction) {
            if hook == "clear_bits" {
                if let HookAction::Read { value, .. } = action {
                    self.value &= !value;
                }
            }
        }
    }

    struct HooksFactory;

    static HOOKS_FACTORY: HooksFactory = HooksFactory;

    impl ModuleFactory for HooksFactory {
        fn id(&self) -> &'static str {
            "system.hooks"
        }

        fn kind(&self) -> ModuleKind {
            ModuleKind::System
        }

        fn create(&self, _deps: &ModuleDeps, _options: &ModuleOptions) -> Box<dyn Module> {
            Box::new(HooksModule { value: 0x01 })
        }

        fn regs(&self) -> &'static [RegisterDesc] {
            HOOK_REGS
        }
    }
}

pub use utils::{cmb_color_to_ansi, petscii_to_unicode, screen_to_petscii, unicode_to_screen}; // convenience re-export

use std::any::Any;
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::ops::RangeInclusive;
use std::sync::{Arc, Mutex};

use crate::mmio::{
    BackendHandles, FanoutWriteEvent, HookAction, Module, ModuleAdapter, ModuleAdapterEvent,
    ModuleDeps, ModuleKind, PrimaryWriteEvent, RegId, ScatterWriteEvent,
};
use console_mmio::ConsoleMmio;
use display_mmio::DisplayMmio;
use input_mmio::InputMmio;
use interrupts::InterruptController;
use personality::{InterruptLine, Personality, PersonalityMmioKind};
use personality_v2::{
    CompileError as PersonalityCompileError, ComputeExpr, Condition, InputSignals, InterruptConfig,
    Map, MapDecode, Mirror, OpenBusPolicy, OpenBusRegion, PersonalityDef, PersonalityMetadata,
    Transform, ValueBuilder,
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
    mirrors: Vec<Mirror>,
    open_bus: Vec<OpenBusRegion>,
    address_table: Vec<Option<AddressSlot>>,
    conditions: BTreeMap<String, Condition>,
    condition_states: BTreeMap<String, bool>,
    signals: SignalStore,
    #[allow(dead_code)]
    interrupts: InterruptConfig,
    #[allow(dead_code)]
    backends: BackendHandles,
    last_read_value: u8,
}

struct ModuleInstance {
    #[allow(dead_code)]
    kind: ModuleKind,
    #[allow(dead_code)]
    impl_id: String,
    module: Box<dyn Module>,
    #[allow(dead_code)]
    options: crate::mmio::ModuleOptions,
    adapter: Option<Box<dyn ModuleAdapter>>,
}

#[derive(Clone)]
struct AddressSlot {
    priority: i32,
    kind: AddressSlotKind,
}

#[derive(Clone)]
enum AddressSlotKind {
    Direct(DirectSlot),
    Scatter(ScatterSlot),
}

#[derive(Clone)]
struct DirectSlot {
    module_index: usize,
    reg: RegId,
    transform: Transform,
    value_builder: Option<ValueBuilder>,
    compute: Option<ComputeExpr>,
    pre_read_sets: Vec<AddressRegisterSet>,
    post_read_sets: Vec<AddressRegisterSet>,
    pre_write_sets: Vec<AddressRegisterSet>,
    post_write_sets: Vec<AddressRegisterSet>,
    field_hooks: Vec<FieldHook>,
    instance: Option<u8>,
    write_fanout: Vec<WriteFanoutAction>,
    suppress_primary: bool,
}

#[derive(Clone)]
struct ScatterSlot {
    entries: Vec<ScatterEntry>,
}

#[derive(Clone)]
struct ScatterEntry {
    module_index: usize,
    reg: RegId,
    transform: Transform,
    pre_read_sets: Vec<AddressRegisterSet>,
    post_read_sets: Vec<AddressRegisterSet>,
    pre_write_sets: Vec<AddressRegisterSet>,
    post_write_sets: Vec<AddressRegisterSet>,
    source_bit: u8,
    target_bit: u8,
    instance: Option<u8>,
}

#[derive(Clone)]
struct FieldHook {
    mask: u8,
    on_read: Option<String>,
    on_write: Option<String>,
}

#[derive(Clone)]
struct WriteFanoutAction {
    target_module_index: usize,
    reg: RegId,
    source_lsb: u8,
    target_lsb: u8,
    width: u8,
    selector: Option<RegId>,
    instance: FanoutInstanceResolved,
}

#[derive(Clone, Copy)]
enum FanoutInstanceResolved {
    None,
    Fixed(u8),
    SelfInstance,
}

impl FanoutInstanceResolved {
    fn index(self, self_instance: Option<u8>) -> Option<u8> {
        match self {
            FanoutInstanceResolved::None => None,
            FanoutInstanceResolved::Fixed(idx) => Some(idx),
            FanoutInstanceResolved::SelfInstance => self_instance,
        }
    }
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

struct FanoutContext {
    self_instance: Option<u8>,
    instance_count: Option<u8>,
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
    mirrors: &[Mirror],
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
            MapDecode::Instances(instances) => {
                compile_instance_map(&mut table, map.priority, instances, lookup)?;
            }
        }
    }
    apply_mirrors(&mut table, mirrors);
    Ok(table)
}

fn apply_mirrors(table: &mut [Option<AddressSlot>], mirrors: &[Mirror]) {
    for mirror in mirrors {
        let period = mirror.period as u32;
        if period == 0 {
            continue;
        }
        let span = (mirror.range.end - mirror.range.start) as u32;
        for offset in 0..=span {
            let addr = mirror.range.start + offset as u16;
            if table[addr as usize].is_some() {
                continue;
            }
            let base_offset = (offset % period) as u16;
            let base_addr = mirror.range.start + base_offset;
            if let Some(base_slot) = table.get(base_addr as usize).and_then(|slot| slot.clone()) {
                table[addr as usize] = Some(base_slot);
            }
        }
    }
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
            kind: AddressSlotKind::Direct(DirectSlot {
                module_index,
                reg: register.id,
                transform,
                value_builder: None,
                compute: None,
                pre_read_sets,
                post_read_sets,
                pre_write_sets,
                post_write_sets,
                field_hooks: Vec::new(),
                instance: None,
                write_fanout: Vec::new(),
                suppress_primary: false,
            }),
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

        let mut transform = entry.transform.clone().unwrap_or_default();
        let pre_read_sets = map_register_sets(&transform.pre_read_sets, lookup)?;
        let post_read_sets = map_register_sets(&transform.post_read_sets, lookup)?;
        let pre_write_sets = map_register_sets(&transform.pre_write_sets, lookup)?;
        let post_write_sets = map_register_sets(&transform.post_write_sets, lookup)?;
        let field_hooks = compile_field_hooks(&entry.field_policies, &mut transform);

        let write_fanout = compile_write_fanout_actions(
            &entry.write_fanout,
            lookup,
            FanoutContext {
                self_instance: None,
                instance_count: None,
            },
        )?;

        let slot = AddressSlot {
            priority,
            kind: AddressSlotKind::Direct(DirectSlot {
                module_index,
                reg: entry.register.id,
                transform,
                value_builder: entry.value_builder.clone(),
                compute: entry.compute.clone(),
                pre_read_sets,
                post_read_sets,
                pre_write_sets,
                post_write_sets,
                field_hooks,
                instance: None,
                write_fanout,
                suppress_primary: entry.suppress_primary,
            }),
        };

        insert_slot(table, entry.addr, slot)?;
    }

    Ok(())
}

fn compile_instance_map(
    table: &mut [Option<AddressSlot>],
    priority: i32,
    map: &personality_v2::InstanceMap,
    lookup: &BTreeMap<ModuleKind, usize>,
) -> Result<(), PersonalityCompileError> {
    let module_index = *lookup
        .get(&map.module)
        .ok_or(PersonalityCompileError::MissingModule(map.module))?;

    for instance in 0..map.count {
        let index_u32 = instance as u32;
        if index_u32 > u8::MAX as u32 {
            return Err(PersonalityCompileError::AddressOutOfRange { addr: 0 });
        }
        let selector_value = index_u32 as u8;
        for entry in &map.layout {
            let addr = match &entry.addr {
                personality_v2::InstanceAddressExpr::Absolute(expr) => {
                    let value = expr
                        .evaluate(index_u32)
                        .map_err(|_| PersonalityCompileError::AddressOutOfRange { addr: 0 })?;
                    if value > u16::MAX as u32 {
                        return Err(PersonalityCompileError::AddressOutOfRange { addr: u16::MAX });
                    }
                    value as u16
                }
            };

            let mut transform = entry.transform.clone().unwrap_or_default();
            if let Some(selector) = &map.selector {
                let set = personality_v2::RegisterSet {
                    module: map.module,
                    register: selector.clone(),
                    value: selector_value,
                };
                transform.pre_read_sets.insert(0, set.clone());
                transform.pre_write_sets.insert(0, set);
            }

            let pre_read_sets = map_register_sets(&transform.pre_read_sets, lookup)?;
            let post_read_sets = map_register_sets(&transform.post_read_sets, lookup)?;
            let pre_write_sets = map_register_sets(&transform.pre_write_sets, lookup)?;
            let post_write_sets = map_register_sets(&transform.post_write_sets, lookup)?;
            let write_fanout = compile_write_fanout_actions(
                &entry.write_fanout,
                lookup,
                FanoutContext {
                    self_instance: Some(selector_value),
                    instance_count: Some(map.count as u8),
                },
            )?;

            if let Some(field) = &entry.field {
                let target_bit = field
                    .target_bit
                    .evaluate(index_u32)
                    .map_err(|_| PersonalityCompileError::AddressOutOfRange { addr })?;
                let source_bit = field
                    .source_bit
                    .evaluate(index_u32)
                    .map_err(|_| PersonalityCompileError::AddressOutOfRange { addr })?;
                if target_bit > 7 || source_bit > 7 {
                    return Err(PersonalityCompileError::AddressOutOfRange { addr });
                }
                let scatter_entry = ScatterEntry {
                    module_index,
                    reg: entry.register.id,
                    transform,
                    pre_read_sets,
                    post_read_sets,
                    pre_write_sets,
                    post_write_sets,
                    source_bit: source_bit as u8,
                    target_bit: target_bit as u8,
                    instance: Some(selector_value),
                };
                let slot = AddressSlot {
                    priority,
                    kind: AddressSlotKind::Scatter(ScatterSlot {
                        entries: vec![scatter_entry],
                    }),
                };
                insert_slot(table, addr, slot)?;
            } else {
                let field_hooks = compile_field_hooks(&entry.field_policies, &mut transform);
                let slot = AddressSlot {
                    priority,
                    kind: AddressSlotKind::Direct(DirectSlot {
                        module_index,
                        reg: entry.register.id,
                        transform,
                        value_builder: None,
                        compute: entry.compute.clone(),
                        pre_read_sets,
                        post_read_sets,
                        pre_write_sets,
                        post_write_sets,
                        field_hooks,
                        instance: Some(selector_value),
                        write_fanout,
                        suppress_primary: entry.suppress_primary,
                    }),
                };
                insert_slot(table, addr, slot)?;
            }
        }
    }

    Ok(())
}

fn compile_write_fanout_actions(
    fanouts: &[personality_v2::WriteFanout],
    lookup: &BTreeMap<ModuleKind, usize>,
    context: FanoutContext,
) -> Result<Vec<WriteFanoutAction>, PersonalityCompileError> {
    let mut actions = Vec::new();
    for fanout in fanouts {
        let target_module_index = *lookup
            .get(&fanout.module)
            .ok_or(PersonalityCompileError::MissingModule(fanout.module))?;

        let selector = fanout.selector.as_ref().map(|sel| sel.id);

        match fanout.instance {
            personality_v2::FanoutInstance::None => {
                actions.push(WriteFanoutAction {
                    target_module_index,
                    reg: fanout.register.id,
                    source_lsb: fanout.source_lsb,
                    target_lsb: fanout.target_lsb,
                    width: fanout.width,
                    selector,
                    instance: FanoutInstanceResolved::None,
                });
            }
            personality_v2::FanoutInstance::Fixed(index) => {
                actions.push(WriteFanoutAction {
                    target_module_index,
                    reg: fanout.register.id,
                    source_lsb: fanout.source_lsb,
                    target_lsb: fanout.target_lsb,
                    width: fanout.width,
                    selector,
                    instance: FanoutInstanceResolved::Fixed(index),
                });
            }
            personality_v2::FanoutInstance::Current => {
                if context.self_instance.is_none() {
                    return Err(PersonalityCompileError::FanoutMissingInstance);
                }
                actions.push(WriteFanoutAction {
                    target_module_index,
                    reg: fanout.register.id,
                    source_lsb: fanout.source_lsb,
                    target_lsb: fanout.target_lsb,
                    width: fanout.width,
                    selector,
                    instance: FanoutInstanceResolved::SelfInstance,
                });
            }
            personality_v2::FanoutInstance::All => {
                let count = context
                    .instance_count
                    .or_else(|| default_instance_count(fanout.module))
                    .ok_or(PersonalityCompileError::FanoutMissingInstance)?;
                for offset in 0..count {
                    actions.push(WriteFanoutAction {
                        target_module_index,
                        reg: fanout.register.id,
                        source_lsb: fanout.source_lsb + offset,
                        target_lsb: fanout.target_lsb,
                        width: 1,
                        selector,
                        instance: FanoutInstanceResolved::Fixed(offset),
                    });
                }
            }
        }
    }
    Ok(actions)
}

fn default_instance_count(kind: ModuleKind) -> Option<u8> {
    match kind {
        ModuleKind::Sprite => Some(sprite_mmio::SPRITE_SLOTS as u8),
        _ => None,
    }
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

fn compile_field_hooks(
    policies: &[personality_v2::FieldPolicy],
    transform: &mut Transform,
) -> Vec<FieldHook> {
    let mut hooks = Vec::new();
    for policy in policies {
        if policy.ro {
            transform.ro_mask |= policy.mask;
        }
        if policy.wo {
            transform.wo_mask |= policy.mask;
        }
        if policy.on_read.is_some() || policy.on_write.is_some() {
            hooks.push(FieldHook {
                mask: policy.mask,
                on_read: policy.on_read.clone(),
                on_write: policy.on_write.clone(),
            });
        }
    }
    hooks
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
                match (&mut existing.kind, slot.kind) {
                    (
                        AddressSlotKind::Scatter(existing_scatter),
                        AddressSlotKind::Scatter(mut new_scatter),
                    ) => {
                        existing_scatter.entries.append(&mut new_scatter.entries);
                    }
                    _ => {
                        return Err(PersonalityCompileError::AddressOverlap {
                            addr,
                            existing_priority: existing.priority,
                            new_priority: priority,
                        });
                    }
                }
            } else if priority > existing.priority {
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
    fn dispatch_adapter_event<'a>(&mut self, module_index: usize, event: ModuleAdapterEvent<'a>) {
        if let Some(entry) = self.modules.get_mut(module_index) {
            if let Some(adapter) = entry.adapter.as_deref_mut() {
                adapter.handle_event(event);
            }
        }
    }

    fn attach_adapter(
        &mut self,
        kind: ModuleKind,
        adapter: Box<dyn ModuleAdapter>,
    ) -> Result<(), AdapterError> {
        let index = match self.module_lookup.get(&kind) {
            Some(index) => *index,
            None => return Err(AdapterError::ModuleNotMapped(kind)),
        };
        let entry = self
            .modules
            .get_mut(index)
            .ok_or(AdapterError::ModuleNotMapped(kind))?;
        if entry.adapter.is_some() {
            return Err(AdapterError::AdapterAlreadyAttached(kind));
        }
        entry.adapter = Some(adapter);
        Ok(())
    }

    fn from_def(
        def: PersonalityDef,
        ram: Arc<Mutex<Memory>>,
        controller: Arc<InterruptController>,
        backends: BackendHandles,
    ) -> Result<Self, PersonalityCompileError> {
        let PersonalityDef {
            metadata,
            modules,
            conditions,
            maps,
            mirrors,
            open_bus,
            interrupts,
        } = def;

        let mut instances = Vec::with_capacity(modules.len());
        let mut lookup = BTreeMap::new();

        for (kind, config) in modules {
            let deps = ModuleDeps::new(ram.clone(), controller.clone(), backends.clone());
            let module = config.factory.create(&deps, &config.options);
            let index = instances.len();
            lookup.insert(kind, index);
            instances.push(ModuleInstance {
                kind,
                impl_id: config.impl_id,
                module,
                options: config.options,
                adapter: None,
            });
        }

        let mut runtime = Self {
            metadata,
            modules: instances,
            module_lookup: lookup,
            maps,
            mirrors,
            open_bus,
            address_table: Vec::new(),
            conditions,
            condition_states: BTreeMap::new(),
            signals: SignalStore::default(),
            interrupts,
            backends,
            last_read_value: 0,
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
            None => {
                if let Some(value) = self.open_bus_value(addr) {
                    self.record_last_read(value);
                    return Some(value);
                }
                return None;
            }
        };

        let value = match slot.kind {
            AddressSlotKind::Direct(direct) => self.read_direct(direct),
            AddressSlotKind::Scatter(scatter) => self.read_scatter(scatter),
        };
        if let Some(value) = value {
            self.record_last_read(value);
        }
        value
    }

    fn write(&mut self, addr: u16, value: u8) -> bool {
        let slot = match self
            .address_table
            .get(addr as usize)
            .and_then(|cell| cell.as_ref())
        {
            Some(slot) => slot.clone(),
            None => {
                return if self.is_open_bus_addr(addr) {
                    true
                } else {
                    false
                };
            }
        };

        match slot.kind {
            AddressSlotKind::Direct(direct) => self.write_direct(value, direct),
            AddressSlotKind::Scatter(scatter) => self.write_scatter(value, scatter),
        }
    }

    fn read_direct(&mut self, slot: DirectSlot) -> Option<u8> {
        self.apply_register_sets(&slot.pre_read_sets);

        let module_index = slot.module_index;
        let aggregated = if slot.suppress_primary && !slot.write_fanout.is_empty() {
            Some(self.read_fanout_value(slot.instance, &slot.write_fanout))
        } else {
            None
        };

        let mut value = if let Some(expr) = slot.compute.as_ref() {
            expr.evaluate(&mut self.signals)
        } else if let Some(agg) = aggregated {
            agg
        } else if let Some(builder) = slot.value_builder.as_ref() {
            builder
                .build(&mut self.signals)
                .get(0)
                .copied()
                .unwrap_or(0)
        } else {
            let read_value = {
                let entry = self.modules.get_mut(module_index)?;
                entry.module.read_reg(slot.reg)
            };
            read_value
        };

        value = apply_shift(value, slot.transform.shift);
        value ^= slot.transform.invert_mask;
        if slot.transform.wo_mask != 0 {
            value &= !slot.transform.wo_mask;
        }

        let mut hook_events: Vec<(String, HookAction)> = Vec::new();
        if !slot.field_hooks.is_empty() {
            if let Some(entry) = self.modules.get_mut(module_index) {
                let module = entry.module.as_mut();
                for hook in &slot.field_hooks {
                    if let Some(name) = &hook.on_read {
                        let masked = value & hook.mask;
                        if masked != 0 {
                            let action = HookAction::Read {
                                mask: hook.mask,
                                value: masked,
                            };
                            module.handle_hook(name, action);
                            hook_events.push((name.clone(), action));
                        }
                    }
                }
            }
        }

        self.apply_register_sets(&slot.post_read_sets);

        for (name, action) in hook_events {
            self.dispatch_adapter_event(
                module_index,
                ModuleAdapterEvent::Hook {
                    hook: name.as_str(),
                    action,
                },
            );
        }

        Some(value)
    }

    fn read_scatter(&mut self, slot: ScatterSlot) -> Option<u8> {
        let mut result = 0u8;
        for entry in slot.entries {
            self.apply_register_sets(&entry.pre_read_sets);
            let module = self.modules.get_mut(entry.module_index)?.module.as_mut();
            let mut value = module.read_reg(entry.reg);
            value = apply_shift(value, entry.transform.shift);
            value ^= entry.transform.invert_mask;
            if entry.transform.wo_mask != 0 {
                value &= !entry.transform.wo_mask;
            }
            let bit = (value.wrapping_shr(entry.source_bit as u32) & 1) as u8;
            if bit != 0 {
                result |= 1u8 << entry.target_bit;
            }
            self.apply_register_sets(&entry.post_read_sets);
        }
        Some(result)
    }

    fn write_direct(&mut self, value: u8, slot: DirectSlot) -> bool {
        self.apply_register_sets(&slot.pre_write_sets);

        let cpu_value = value;
        let module_index = slot.module_index;
        let (module_value, hook_events) = {
            let entry = match self.modules.get_mut(module_index) {
                Some(entry) => entry,
                None => return false,
            };
            let module = entry.module.as_mut();

            let mut module_value = reverse_shift(value, slot.transform.shift);
            module_value ^= slot.transform.invert_mask;

            if slot.transform.ro_mask != 0 {
                let current = module.read_reg(slot.reg);
                module_value =
                    (module_value & !slot.transform.ro_mask) | (current & slot.transform.ro_mask);
            }

            let mut hook_events = Vec::new();

            if !slot.suppress_primary {
                module.write_reg(slot.reg, module_value);

                if !slot.field_hooks.is_empty() {
                    for hook in &slot.field_hooks {
                        if let Some(name) = &hook.on_write {
                            let masked = cpu_value & hook.mask;
                            if masked != 0 {
                                let action = HookAction::Write {
                                    mask: hook.mask,
                                    value: masked,
                                };
                                module.handle_hook(name, action);
                                hook_events.push((name.clone(), action));
                            }
                        }
                    }
                }
            }

            (module_value, hook_events)
        };

        self.dispatch_adapter_event(
            module_index,
            ModuleAdapterEvent::PrimaryWrite(PrimaryWriteEvent {
                reg: slot.reg,
                cpu_value,
                module_value,
                instance: slot.instance,
            }),
        );

        for (name, action) in hook_events {
            self.dispatch_adapter_event(
                module_index,
                ModuleAdapterEvent::Hook {
                    hook: name.as_str(),
                    action,
                },
            );
        }

        if !slot.write_fanout.is_empty() {
            self.apply_write_fanouts(cpu_value, slot.instance, &slot.write_fanout);
        }
        self.apply_register_sets(&slot.post_write_sets);
        self.recompute_conditions_if_needed();
        true
    }

    fn write_scatter(&mut self, value: u8, slot: ScatterSlot) -> bool {
        let mut any = false;
        for entry in slot.entries {
            self.apply_register_sets(&entry.pre_write_sets);
            let module_index = entry.module_index;
            let mut adapter_event = None;
            let module_present = {
                if let Some(module_entry) = self.modules.get_mut(module_index) {
                    let module = module_entry.module.as_mut();
                    let current = module.read_reg(entry.reg);
                    let mut cpu_value = apply_shift(current, entry.transform.shift);
                    cpu_value ^= entry.transform.invert_mask;
                    if entry.transform.wo_mask != 0 {
                        cpu_value &= !entry.transform.wo_mask;
                    }
                    let bit_value = (value.wrapping_shr(entry.target_bit as u32) & 1) != 0;
                    let mask = 1u8 << entry.source_bit;
                    if bit_value {
                        cpu_value |= mask;
                    } else {
                        cpu_value &= !mask;
                    }
                    let mut new_value = reverse_shift(cpu_value, entry.transform.shift);
                    new_value ^= entry.transform.invert_mask;
                    let mut final_value = new_value;
                    if entry.transform.ro_mask != 0 {
                        final_value = (new_value & !entry.transform.ro_mask)
                            | (current & entry.transform.ro_mask);
                    }
                    module.write_reg(entry.reg, final_value);
                    adapter_event = Some(ModuleAdapterEvent::ScatterWrite(ScatterWriteEvent {
                        reg: entry.reg,
                        cpu_value,
                        module_value: final_value,
                        bit_value,
                        source_bit: entry.source_bit,
                        target_bit: entry.target_bit,
                        instance: entry.instance,
                    }));
                    true
                } else {
                    false
                }
            };

            if !module_present {
                continue;
            }

            self.apply_register_sets(&entry.post_write_sets);

            if let Some(event) = adapter_event {
                self.dispatch_adapter_event(module_index, event);
            }

            any = true;
        }
        if any {
            self.recompute_conditions_if_needed();
        }
        any
    }

    fn apply_register_sets(&mut self, sets: &[AddressRegisterSet]) {
        for set in sets {
            if let Some(instance) = self.modules.get_mut(set.module_index) {
                instance.module.write_reg(set.reg, set.value);
            }
        }
    }

    fn apply_write_fanouts(
        &mut self,
        cpu_value: u8,
        self_instance: Option<u8>,
        fanouts: &[WriteFanoutAction],
    ) {
        let mut events: Vec<(usize, FanoutWriteEvent)> = Vec::new();
        for action in fanouts {
            if let Some(module_entry) = self.modules.get_mut(action.target_module_index) {
                let module = module_entry.module.as_mut();
                let value = extract_fanout_value(cpu_value, action);
                let target_instance = action.instance.index(self_instance);
                if let Some(index) = target_instance {
                    if let Some(selector) = action.selector {
                        module.write_reg(selector, index);
                    }
                }
                module.write_reg(action.reg, value);
                events.push((
                    action.target_module_index,
                    FanoutWriteEvent {
                        reg: action.reg,
                        value,
                        source_value: cpu_value,
                        source_instance: self_instance,
                        target_instance,
                    },
                ));
            }
        }

        for (module_index, event) in events {
            self.dispatch_adapter_event(module_index, ModuleAdapterEvent::FanoutWrite(event));
        }
    }

    fn read_fanout_value(
        &mut self,
        self_instance: Option<u8>,
        fanouts: &[WriteFanoutAction],
    ) -> u8 {
        let mut result = 0u8;
        let mut selector_restore: Vec<(usize, RegId, u8)> = Vec::new();

        for action in fanouts {
            if let Some(module_entry) = self.modules.get_mut(action.target_module_index) {
                let module = module_entry.module.as_mut();

                if let Some(selector_reg) = action.selector {
                    if let Some(index) = action.instance.index(self_instance) {
                        if !selector_restore.iter().any(|(module_idx, reg, _)| {
                            *module_idx == action.target_module_index && *reg == selector_reg
                        }) {
                            let previous = module.read_reg(selector_reg);
                            selector_restore.push((
                                action.target_module_index,
                                selector_reg,
                                previous,
                            ));
                        }
                        module.write_reg(selector_reg, index);
                    }
                }

                let value = module.read_reg(action.reg);
                let mask = if action.width >= 8 {
                    u8::MAX
                } else {
                    ((1u16 << action.width) - 1) as u8
                };
                result |= ((value >> action.target_lsb) & mask) << action.source_lsb;
            }
        }

        for (module_index, selector, previous) in selector_restore.into_iter().rev() {
            if let Some(module_entry) = self.modules.get_mut(module_index) {
                module_entry.module.write_reg(selector, previous);
            }
        }

        result
    }

    fn open_bus_value(&self, addr: u16) -> Option<u8> {
        self.open_bus
            .iter()
            .find(|region| region.range.contains(addr))
            .map(|region| match &region.policy {
                OpenBusPolicy::Const(value) => *value,
                OpenBusPolicy::LastRead => self.last_read_value,
            })
    }

    fn is_open_bus_addr(&self, addr: u16) -> bool {
        self.open_bus
            .iter()
            .any(|region| region.range.contains(addr))
    }

    fn record_last_read(&mut self, value: u8) {
        self.last_read_value = value;
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
        self.address_table = compile_address_table(
            &self.maps,
            &self.mirrors,
            &self.module_lookup,
            &self.condition_states,
        )?;
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

    fn set_raster_irq_state(&mut self, state: Arc<RasterIrqState>) {
        if let Some(system) = self.module_downcast_mut::<SystemMmio>(ModuleKind::System) {
            system.set_raster_irq_state(state);
        }
    }

    fn console_output_handle(&self) -> Option<Arc<Mutex<console_mmio::ConsoleOutput>>> {
        self.module_downcast::<ConsoleMmio>(ModuleKind::Console)
            .map(|console| console.output())
    }

    fn display_output_handle(&self) -> Option<Arc<Mutex<display_mmio::DisplayOutput>>> {
        self.module_downcast::<DisplayMmio>(ModuleKind::Display)
            .map(|display| display.output())
    }

    fn input_output_handle(&self) -> Option<Arc<Mutex<input_mmio::InputOutput>>> {
        self.module_downcast::<InputMmio>(ModuleKind::Input)
            .map(|input| input.output())
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

    #[allow(dead_code)]
    fn address_mappings(&self) -> Vec<AddressMapping> {
        let mut result = Vec::new();
        for (addr, slot) in self.address_table.iter().enumerate() {
            let Some(slot) = slot else { continue };
            match &slot.kind {
                AddressSlotKind::Direct(direct) => {
                    let module = &self.modules[direct.module_index];
                    let mapping_detail = if let Some(instance) = direct.instance {
                        MappingDetail::DirectInstance { instance }
                    } else {
                        MappingDetail::Direct
                    };
                    result.push(AddressMapping {
                        addr: addr as u16,
                        priority: slot.priority,
                        module: module.kind,
                        module_impl_id: module.impl_id.clone(),
                        register: direct.reg,
                        register_name: register_name(module, direct.reg),
                        value_builder: direct.value_builder.is_some(),
                        compute: direct
                            .compute
                            .as_ref()
                            .map(|expr| expr.source().to_string()),
                        transform: TransformInfo::from(&direct.transform),
                        mapping: mapping_detail,
                        field_hooks: direct
                            .field_hooks
                            .iter()
                            .map(|hook| FieldHookInfo {
                                mask: hook.mask,
                                on_read: hook.on_read.clone(),
                                on_write: hook.on_write.clone(),
                            })
                            .collect(),
                        suppress_primary: direct.suppress_primary,
                    });
                }
                AddressSlotKind::Scatter(scatter) => {
                    for entry in &scatter.entries {
                        let module = &self.modules[entry.module_index];
                        result.push(AddressMapping {
                            addr: addr as u16,
                            priority: slot.priority,
                            module: module.kind,
                            module_impl_id: module.impl_id.clone(),
                            register: entry.reg,
                            register_name: register_name(module, entry.reg),
                            value_builder: false,
                            compute: None,
                            transform: TransformInfo::from(&entry.transform),
                            mapping: MappingDetail::Scatter {
                                target_bit: entry.target_bit,
                                source_bit: entry.source_bit,
                                instance: entry.instance,
                            },
                            field_hooks: Vec::new(),
                            suppress_primary: false,
                        });
                    }
                }
            }
        }
        result
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
        Self::from_personality_def_with_backends(def, BackendHandles::default())
    }

    /// Construct the bus from a [`PersonalityDef`] and supply backend handles for adapters.
    pub fn from_personality_def_with_backends(
        def: PersonalityDef,
        backends: BackendHandles,
    ) -> Result<Self, PersonalityCompileError> {
        let ram = Arc::new(Mutex::new(Memory::new()));
        let controller = Arc::new(InterruptController::new());
        let runtime = PersonalityRuntime::from_def(def, ram.clone(), controller.clone(), backends)?;
        Ok(Self {
            ram,
            personality_legacy: None,
            mmio: Vec::new(),
            controller,
            runtime_v2: Some(runtime),
        })
    }

    /// Attach a module adapter that mirrors MMIO activity to a backend system.
    pub fn attach_adapter(
        &mut self,
        kind: ModuleKind,
        adapter: Box<dyn ModuleAdapter>,
    ) -> Result<(), AdapterError> {
        match self.runtime_v2.as_mut() {
            Some(runtime) => runtime.attach_adapter(kind, adapter),
            None => Err(AdapterError::LegacyPersonality),
        }
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
        if let Some(value) = {
            if let Some(dev) = self.find_mmio(addr) {
                Some(dev.read(addr))
            } else {
                None
            }
        } {
            if let Some(runtime) = self.runtime_v2.as_mut() {
                runtime.record_last_read(value);
            }
            return value;
        }
        let value = {
            let mem = self.ram.lock().unwrap();
            mem.read(addr)
        };
        if let Some(runtime) = self.runtime_v2.as_mut() {
            runtime.record_last_read(value);
        }
        value
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

    /// Expose the input device's shared snapshot buffer.
    pub fn input_output_handle(&self) -> Option<Arc<Mutex<input_mmio::InputOutput>>> {
        if let Some(runtime) = &self.runtime_v2 {
            if let Some(handle) = runtime.input_output_handle() {
                return Some(handle);
            }
        }
        for mapped in self.mmio.iter() {
            if mapped.kind == Some(PersonalityMmioKind::Input) {
                if let Some(i) = mapped.device.as_any().downcast_ref::<InputMmio>() {
                    return Some(i.output());
                }
            }
        }
        None
    }

    /// Share the raster IRQ state with the system MMIO implementation so
    /// adapters and host backends can coordinate compare/current updates.
    pub fn attach_raster_irq_state(&mut self, state: Arc<RasterIrqState>) {
        if let Some(runtime) = self.runtime_v2.as_mut() {
            runtime.set_raster_irq_state(state.clone());
        }

        for mapped in self.mmio.iter_mut() {
            if mapped.kind == Some(PersonalityMmioKind::System) {
                if let Some(system) = mapped.device.as_any_mut().downcast_mut::<SystemMmio>() {
                    system.set_raster_irq_state(state.clone());
                }
            }
        }
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

    /// Snapshot the resolved address table for the active TOML personality.
    pub fn address_mappings(&self) -> Option<Vec<AddressMapping>> {
        self.runtime_v2
            .as_ref()
            .map(|runtime| runtime.address_mappings())
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

#[allow(dead_code)]
fn register_name(instance: &ModuleInstance, reg: RegId) -> &'static str {
    instance
        .module
        .regs()
        .iter()
        .find(|desc| desc.id == reg)
        .map(|desc| desc.name)
        .unwrap_or("<unknown>")
}

fn extract_fanout_value(cpu_value: u8, action: &WriteFanoutAction) -> u8 {
    let mask = if action.width >= 8 {
        u8::MAX
    } else {
        ((1u16 << action.width) - 1) as u8
    };
    ((cpu_value >> action.source_lsb) & mask) << action.target_lsb
}

impl From<&Transform> for TransformInfo {
    fn from(transform: &Transform) -> Self {
        Self {
            invert_mask: transform.invert_mask,
            ro_mask: transform.ro_mask,
            wo_mask: transform.wo_mask,
            shift: transform.shift,
            on_read: transform.on_read.clone(),
            on_write: transform.on_write.clone(),
        }
    }
}
