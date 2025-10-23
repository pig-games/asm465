# Module & Register Model
[← Overview](01-Architecture-Overview.md) • [→ Personality File](03-Personality-Spec.md) • [→ Value Builders](04-Value-Builders.md) • [→ Migration & Tests](05-Migration-and-Tests.md)

This chapter defines the runtime model behind cross465 Personalities v2:
- **Module kinds** vs **implementations**
- **Address-free register descriptors**
- **Transforms** (hardware semantics) applied at the *mapping* layer
- **Module registry & factories**
- **Contracts, conformance & capabilities**
- **Save-state & determinism**

---

## 1) Module Kinds & Implementations

A **ModuleKind** is the logical category visible to the bus and personality maps.  
A **Module implementation** is a concrete backend selected by the personality (e.g., `display.basic2d` or `display.2dto3d`).

```rust
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum ModuleKind {
    Display,   // screen-facing MMIO (colors, mode, page flip, etc.)
    Input,     // gamepads/joysticks/paddles/keys
    Video,     // raster/vblank IRQs, status, beam/raster regs
    Audio,     // sound chip(s)
    System,    // timers, banking, IRQ/NMI gating
}
```

### Module Runtime Interface

Each implementation must expose a minimal bus-facing interface. The bus reads/writes **by RegId** (not by address).

```rust
pub trait Module: Send {
    fn read(&mut self, id: RegId) -> u8;          // fallback read (used when no value_builder)
    fn write(&mut self, id: RegId, val: u8);      // write path
    fn tick(&mut self, cycles: u32);              // time-based updates (optional)
    fn snapshot(&self) -> ModuleState;            // save-state
    fn restore(&mut self, s: ModuleState);
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct ModuleState {
    pub version: u16,
    pub bytes: Vec<u8>,
}
```

> Reads can be overridden by **Value Builders** when a personality wants to synthesize a register value (see `04-Value-Builders.md`). If no builder is present, the bus delegates to `Module::read`.

---

## 2) Address-Free Register Descriptors

Implementations must publish a **canonical list of registers** they support — *without addresses*.  
This is the single source of truth for width, reset values, RO/WO bits, and bitfield metadata.

```rust
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum RegId {
    // Display (core)
    BorderColor, BackgroundColor, PageSelect, Mode,
    // Display (optional extensions)
    Ext_Command, Ext_Param, Ext_Status,

    // Input (examples)
    JoyPort1, JoyPort2, PaddleX, PaddleY,

    // Video/IRQ (examples)
    IrqEnable, IrqStatus, NmiEnable, NmiStatus,

    // Add further per-kind IDs as needed…
}

pub struct RegisterDesc {
    pub id: RegId,
    pub width: u8,                 // bytes (1..=N)
    pub reset: u32,                // power-on value (masked by width)
    pub readable: bool,
    pub writable: bool,
    pub bitfields: &'static [BitField], // optional metadata for UI/tools
}

pub struct BitField {
    pub name: &'static str,        // e.g., "IRQ_RASTER", "SPRITE_BG_COLL"
    pub lsb: u8,                   // bit position start
    pub width: u8,                 // bit width
    pub ro: bool,                  // read-only at the bitfield level
    pub active_low: bool,          // UI hint; mapping can still transform
}

pub trait RegisterBlock {
    fn registers() -> &'static [RegisterDesc];
}
```

### Core vs Extension RegIds

Core RegIds define a **contract** shared by all implementations of a kind.  
Extensions (`Ext_*`) may appear only in some implementations. Personalities can map them if available.

---

## 3) Transforms (hardware semantics)

Transforms are part of the *mapping*, not the implementation. They express how the CPU-facing value behaves.

```rust
pub struct Transform {
    pub invert_mask: u8,                 // active-low bits
    pub ro_mask: u8,                     // read-only bits
    pub wo_mask: u8,                     // write-only bits
    pub shift: i8,                       // bit shift offset
    pub on_read: Option<&'static str>,   // module hook for read side-effects
    pub on_write: Option<&'static str>,  // module hook for write side-effects
}
```

Example usages:
- Invert joystick inputs (`invert_mask = 0x1F`)
- VIC-II IRQ clear-on-read (`on_read = "vic_ack_irq"`)

---

## 4) Module Factories & Registry

Implementations are registered via `ModuleFactory`.

```rust
pub trait ModuleFactory: Send + Sync + 'static {
    fn id(&self) -> &'static str;                // e.g., "display.basic2d"
    fn kind(&self) -> ModuleKind;                // ModuleKind::Display
    fn create(&self, opts: &ModuleOpts) -> Box<dyn Module>;
    fn regs(&self) -> &'static [RegisterDesc];   // descriptor table
}

pub struct ModuleRegistry { builders: Vec<&'static dyn ModuleFactory> }

impl ModuleRegistry {
    pub fn by_id(&self, id: &str) -> Option<&'static dyn ModuleFactory> { /* ... */ }
    pub fn by_kind(&self, k: ModuleKind) -> impl Iterator<Item=&dyn ModuleFactory> { /* ... */ }
}
```

Personality then selects which implementation to instantiate:

```toml
[modules.display]
impl    = "display.2dto3d"
options = { width=320, height=200 }
```

---

## 5) Capabilities and Introspection

Implementations can advertise optional features for UI/tools:

```rust
bitflags::bitflags! {
    pub struct Caps: u32 {
        const PAGE_FLIP = 1<<0;
        const SPRITES   = 1<<1;
        const TILEMAP   = 1<<2;
        const CMD_PORT  = 1<<3; // supports Ext_Command/Ext_Param
        const RENDER_3D = 1<<4;
    }
}

pub trait ModuleIntrospect {
    fn name(&self) -> &'static str;
    fn caps(&self) -> Caps;
}
```

---

## 6) Save-State and Determinism

Each module implements serialization for its internal state:

```rust
#[derive(serde::Serialize, serde::Deserialize)]
pub struct ModuleState {
    pub version: u16,
    pub bytes: Vec<u8>,
}
```

This allows deterministic replay and debugging for all module kinds.

---

## 7) Conformance Tests

Each core module kind defines a **behavior contract** suite that tests:
- RO/WO enforcement
- Reset values
- Hook side effects (e.g., read-to-ack)
- Timing consistency

```rust
pub trait BehaviorSpec {
    fn conformance_suite(create: impl Fn() -> Box<dyn Module>);
}
```

These suites ensure that multiple implementations (e.g., `display.basic2d` and `display.2dto3d`) behave identically for core RegIds.

---

## Summary

- Modules are **instantiated via factories**; personalities pick implementations.
- **Registers** define stable logical APIs; personalities place them in memory.
- **Transforms** and **value builders** handle platform quirks (bit inversions, hooks, scaling).
- **Registry** and **capabilities** provide introspection and tooling hooks.
- **Conformance tests** guarantee consistent cross-implementation behavior.
