# Personality File (TOML) — Spec
[← Modules](02-Module-and-Registers.md) • [→ Value Builders](04-Value-Builders.md) • [→ Migration & Tests](05-Migration-and-Tests.md)

This chapter describes the TOML-based personality definition format.

---

## 1) High-Level Structure
```toml
[personality]
id = "modern-retro-3d"
title = "Modern Retro (2D MMIO, 3D renderer)"
default_map_priority = 10  # optional

[modules.display]
impl    = "display.2dto3d"
options = { width=320, height=200, camera="iso" }

[modules.input]
impl    = "input.cia-style"
options = { }

[conditions]  # optional banking/overlays
bank0 = { kind="system", reg="BankSel", equals=0 }
bank1 = { kind="system", reg="BankSel", equals=1 }
```

---

## 2) Maps (`[[map]]`)

Each `[[map]]` entry defines how one or more registers are mapped into address space.  
It contains exactly **one** of `decode.range` or `decode.sparse`.

### A) Contiguous Range Map
```toml
[[map]]
priority     = 10                # higher wins on overlap (default = personality.default_map_priority)
active_when  = "bank0"           # optional condition name
decode = { range = {
  addr  = "DF20..=DF2F",         # hex string range
  kind  = "display",             # ModuleKind
  order = ["BorderColor","BackgroundColor","PageSelect","Mode"],
  stride = 1,                    # optional (default 1)
  default_transform = { invert_mask = 0 }  # optional
}}
```

### B) Sparse Per-Register Map
```toml
[[map]]
priority = 20
decode = { sparse = [
  { addr="D020", kind="display", id="BorderColor" },
  { addr="D021", kind="display", id="BackgroundColor" },
  { addr="DC00", kind="input", id="JoyPort2", transform={ invert_mask=31 } },
  { addr="DC01", kind="input", id="JoyPort1", transform={ invert_mask=31 } },
  { addr="DF80", kind="display", id="Ext_Command" },
  { addr="DF81", kind="display", id="Ext_Param"   },
  { addr="DF82", kind="display", id="Ext_Status"  }
]}
```

---

## 3) Transforms (per map entry)

Transforms modify how the mapped value behaves:
```toml
transform = {
  invert_mask = 31,
  ro_mask = 0,
  wo_mask = 0,
  shift = 0,
  on_read = "vic_ack_irq",
  on_write = "cia_start_timer"
}
```

---

## 4) Interrupt Wiring (optional)

```toml
[interrupts]
irq_sources = [
  { kind="video", id="IrqStatus" },
  { kind="cia",   id="IrqStatus" }
]
nmi_sources = [
  { kind="video", id="NmiStatus" }
]
irq_ack = { kind="video", id="IrqStatus", hook="vic_ack_irq" }
nmi_ack = { kind="video", id="NmiStatus", hook="antic_ack_nmi" }
```

---

## 5) Conditions (banking/overlays)

Conditions determine which maps are active.  
Example:

```toml
[conditions]
bank0 = { kind="system", reg="BankSel", equals=0 }
bank1 = { kind="system", reg="BankSel", equals=1 }
```

When a condition changes, the decoder recomposes the active address table.

---

## 6) Validation Rules

- Each `[[map]]` must define **exactly one** of `decode.range` or `decode.sparse`.
- `range.order` entries must exist in the chosen module impl’s `regs()`.
- `sparse[].(kind,id)` must exist; else fail or bind stub (configurable).
- No overlapping addresses at the same priority (unless mirroring is allowed).
- Hooks must resolve to functions within the implementation.

---

## 7) Decoder Behavior

At load time, the personality compiles to:
```
address → (ModuleKind, RegId, Transform, value_builder?)
```
- Overlaps resolved by `priority`.
- Conditional maps (via `active_when`) recomposed when condition values change.

---

## 8) Example Personalities

### C64-Compatible (Sparse)
```toml
[personality]
id="c64-compat"
title="C64-like sparse layout"

[modules.display]
impl="display.basic2d"

[modules.input]
impl="input.cia-style"

[[map]]
decode = { sparse = [
  { addr="D020", kind="display", id="BorderColor" },
  { addr="D021", kind="display", id="BackgroundColor" },
  { addr="DC00", kind="input", id="JoyPort2", transform={ invert_mask=31 } },
  { addr="DC01", kind="input", id="JoyPort1", transform={ invert_mask=31 } },
  { addr="D019", kind="video", id="IrqStatus", transform={ on_read="vic_ack_irq" } },
  { addr="D01A", kind="video", id="IrqEnable" }
]}
```

### Modern Retro (Contiguous)
```toml
[personality]
id="modern-retro-3d"
title="2D control regs, 3D renderer"

[modules.display]
impl="display.2dto3d"
options={ camera="iso", width=320, height=200 }

[[map]]
decode = { range = {
  addr="DF20..=DF2F", kind="display",
  order=["BorderColor","BackgroundColor","PageSelect","Mode"], stride=1
}}
```
