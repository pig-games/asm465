# Mapping DSL and Examples
[← Flexibility Goals](06_Flexibility_Goals_and_Requirements.md) • [→ Adapter Layer and Shims](08_Adapter_Layer_and_Shims.md)

This chapter defines the **extended TOML mapping DSL** for asm465 personalities: how to describe MMIO ranges, shared bitfields, computed registers, and higher-level behaviours that bridge classic 6502 platforms (e.g., C64 / MEGA65) with modern backends.

---

## 1. Instance Arrays with Address Templates

### Concept
An **instance array** describes a set of repeated registers, one per logical entity (like sprites or voices).  
Each instance shares the same internal structure but has its own address offsets.

### Why It Matters
In systems such as the C64 and MEGA65, sprites and sound channels each occupy a fixed register range.  
Instead of hardcoding these, we declare them parametrically — improving reusability.

### Example
```toml
[[map]]
decode = {
  instances = {
    count = 8,                  # 8 hardware sprites (C64/MEGA65)
    index_var = "i",            # template variable
    kind = "sprite",
    base = "D000",
    layout = [
      { addr = "D000 + (i*2)", id = "XLo" },
      { addr = "D001 + (i*2)", id = "YLo" },
      { addr = "D027 + i",     id = "Color" },
      { addr = "D015 bit i",   id = "Enable", field = { bit = "i" } }
    ]
  }
}
```

### Benefit
One definition supports any engine that uses multiple sprite objects, including selector-based backends that abstract the sprite index.

---

## 2. Scatter and Gather Fields

### Concept
Split or merge fields across multiple registers.

### Why It Matters
C64 sprites have 9-bit X coordinates where bit 8 of all sprites is stored in `$D010`.  
Scatter/gather supports such shared fields.

### Scatter Example — C64 Sprite X-High
```toml
{ addr = "D010 bit i", id="XHi" }
```
The mapping system masks and updates only the bit belonging to instance `i`.

### Gather Example — Combine Hi/Lo
```toml
{ addr="DF20", kind="sprite", id="X16",
  compose = {
    parts = [
      { from = { id="XLo" }, lsb=0,  msb=7 },
      { from = { id="XHi" }, lsb=8,  msb=8 }
    ]
  }
}
```
### Benefit
Works seamlessly with modern backends using 16-bit coordinates or larger logical ranges.

---

## 3. Bitfield and Access Policies

### Concept
Bitfield policies define read/write behaviour per bit range.

### Example
```toml
{ addr="D019", kind="video", id="IrqStatus",
  field_policies = [
    { lsb=0, msb=0, on_read="vic_ack_raster" },
    { lsb=1, msb=1, on_read="vic_ack_sprite" },
    { lsb=2, msb=2, ro=true }
  ]
}
```

### Why It Matters
Many registers clear or trigger bits individually.  
This ensures correct behaviour for flags and latches.

---

## 4. ⚙️ Scatter vs Fanout Semantics

Both `scatter` and `write_fanout` allow one register to affect multiple logical fields, but they serve different purposes.

| Concept | **Scatter** | **Fanout** |
|----------|--------------|------------|
| Direction | Bi-directional (read & write) | Write-only |
| Purpose | Models hardware bitfields | Models broadcast or command behaviour |
| Readback | Yes, reconstructs from fields | No, unless explicitly defined |
| Cross-module | No | Yes |
| Use case | `$D015`, `$D010` (C64) | Helper registers like `EnableAllSprites`, `ResetSystem` |
| Scope | Hardware fidelity | Modern convenience |

### Example — Hardware (`$D015`, C64 Sprite Enable) — **scatter only**
```toml
{ addr="D015", kind="sprite", id="Enable",
  scatter = [
    { instance=0, bit=0 },
    { instance=1, bit=1 },
    { instance=2, bit=2 },
    { instance=3, bit=3 },
    { instance=4, bit=4 },
    { instance=5, bit=5 },
    { instance=6, bit=6 },
    { instance=7, bit=7 }
  ]
}
```
**Behaviour:** writing `%01010101` enables sprites 0,2,4,6; reading `$D015` returns the same value.  
This is authentic VIC-II behaviour and should **not** be modeled with `fanout`.

### Example — Synthetic (`$D500`, Global Reset) — **fanout**
*(Non-C64 example — for modern personalities or testing tools)*
```toml
{ addr="D500", id="ResetAll",
  write_fanout = [
    { to={ module="video", id="Reset" }, const_value=1 },
    { to={ module="audio", id="Reset" }, const_value=1 },
    { to={ module="input", id="Reset" }, const_value=1 }
  ]
}
```
Writing any value resets all modules; `$D500` is write-only.

### Example — Synthetic (`$D501`, Enable All Sprites) — **fanout**
```toml
{ addr="D501", id="EnableAllSprites",
  write_fanout = [
    { to={ instance="*", id="Enable" }, const_value=1 }
  ]
}
```
Writing any value enables all sprites; this is a **modern helper**, not a C64 register.

> **Guideline:** Use `scatter` for authentic hardware registers.  
> Use `write_fanout` only for synthetic helper or control registers.

---

## 5. Multi-Target Writes (Fanout)

### Concept
A write to one register updates several underlying fields.  
See *Scatter vs Fanout Semantics* for a full explanation of when to use this feature.

### Example (Synthetic Modern Helper)
```toml
{ addr="D501", id="EnableAllSprites",
  write_fanout = [
    { to={ instance="*", id="Enable" }, const_value=1 }
  ]
}
```

---

## 6. Timing Hooks (Simplified)

### Concept
Associate a register write or read with a scheduled event — frame or raster level.

### Example
```toml
{ addr="D012", kind="video", id="RasterCompareLo",
  transform = { on_write="vic_set_raster_cmp" },
  timing = { align="raster_start" }
}
```
### Why It Matters
Timing hooks allow mapping raster interrupts or similar triggers without cycle-level precision.

---

## 7. Conditional Instance Sets

### Concept
Activate alternate layouts based on mode or bank.

### Example
```toml
[conditions]
mega_mode = { kind="system", reg="Mode", bit=4, equals=1 }

[[map]]
active_when="!mega_mode"
decode = { instances={ count=8, index_var="i", kind="sprite", base="D000", layout=[ ... ] } }

[[map]]
active_when="mega_mode"
decode = { instances={ count=8, index_var="i", kind="sprite", base="D000", layout=[ ... ] } }
```
### Why It Matters
Allows conditional mapping for MEGA65 or similar platforms with banked registers.

---

## 8. Compute Expressions

### Concept
Derive register values dynamically.

### Example
```toml
{ addr="D41B", kind="video", id="BeamX", compute="beam.x & 0xFF" }
{ addr="D41C", kind="video", id="BeamY", compute="beam.y" }
{ addr="D01E", kind="sprite", id="CollSpriteSprite",
  compute="coll.spr_spr & 0xFF",
  field_policies=[{ on_read="coll_ack_sprspr" }]
}
```

---

## 9. Mirrors and Open-Bus

### Concept
Define how unmapped or repeated address ranges behave.

### Example
```toml
[[mirror]]
range = "D000..=D3FF"
period = 0x040

[[open_bus]]
range = "DE00..=DEFF"
policy = "const"
const  = "00"
```

### Why It Matters
Some platforms mirror register blocks, others return undefined values — this keeps behaviour predictable.

---

## 10. Example — C64 Sprite Block (authentic + scaling flags)

The complete C64 sprite block with **X/Y expand** flags (scatter), so modern backends can use transform scaling or variant selection.

```toml
[[map]]
decode = { instances = {
  count=8, index_var="i", kind="sprite", base="D000",
  layout = [
    # Position (lo bytes per sprite)
    { addr="D000 + (i*2)", id="XLo" },
    { addr="D001 + (i*2)", id="YLo" },

    # Shared per-sprite high bit for X (scatter)
    { addr="D010 bit i",   id="XHi" },

    # Enable on/off (scatter)
    { addr="D015 bit i",   id="Enable" },

    # Per-sprite color
    { addr="D027 + i",     id="Color" },

    # Sprite scaling (double size flags, scatter)
    { addr="D01D bit i",   id="XExpand" },  # 1 = double width
    { addr="D017 bit i",   id="YExpand" }   # 1 = double height
  ]
}}
```

---

## 11. Pointer Decoders (Overview) — Using Subindex

Pointer decoders split a pointer into:
- **index**: the main resource selection (e.g., sprite number, screen page)
- **subindex**: the variant within that resource (e.g., animation frame, tilebank variant)

See: [Pointer Decoders & Address Semantics](10_Pointer_Decoders_and_Address_Semantics.md).

---

## 12. Scaling‑aware Pointer Decode (Option A with Fallback)

You can include **sprite scaling flags** (C64 `$D01D` X‑expand, `$D017` Y‑expand) into the pointer decoder by **extending the subindex**. This lets the backend pick different art variants automatically. If a variant is missing, fall back to a transform scale at runtime.

### MMIO (C64 sprite block with scaling flags — scatter)
```toml
[[map]]
decode = { instances = {
  count=8, index_var="i", kind="sprite", base="D000",
  layout = [
    { addr="D000 + (i*2)", id="XLo" },
    { addr="D001 + (i*2)", id="YLo" },
    { addr="D010 bit i",   id="XHi" },
    { addr="D015 bit i",   id="Enable" },
    { addr="D027 + i",     id="Color" },
    { addr="D01D bit i",   id="XExpand" },  # double width
    { addr="D017 bit i",   id="YExpand" }   # double height
  ]
}}
```

### Pointer decoder (append expand bits above animation bits)
```toml
[pointer_decoder.vic2_sprites]
source   = { table="07F8..07FF", kind="sprite", entries=8 }
block    = 64
subindex = {
  bits = 2,                 # animation 0..3 (S=2)
  extend = [
    { from = "D01D bit i" },  # XExpand → subindex bit S
    { from = "D017 bit i" }   # YExpand → subindex bit S+1
  ]
}
index    = { from="pointer", shift=2 }      # idx = P >> S
```

### Adapter hook with variant + transform fallback
```rust
fn set_sprite_variant(idx: u8, subindex: u8) {
    let anim =  subindex        & 0b11; // S=2
    let x2   = (subindex >> 2)  & 1 != 0;
    let y2   = (subindex >> 3)  & 1 != 0;

    if backend.has_variant(idx, anim, x2, y2) {
        backend.bind_variant(idx, anim, x2, y2);
        backend.set_scale(idx, 1.0, 1.0);
    } else {
        backend.bind_variant(idx, anim, false, false);
        backend.set_scale(idx, if x2 { 2.0 } else { 1.0 },
                               if y2 { 2.0 } else { 1.0 });
    }
}
```

### 64tass helper (content)
```asm
; S=2 anim bits (0..3); scaling comes from flags, not the pointer
.macro SPR_PTR sprite, anim, Sbits=2
    .byte ((sprite) << Sbits) | ((anim) & ((1 << Sbits) - 1))
.endmacro
```

---

## 13. Summary

The Mapping DSL bridges classic register organization with modern engine architectures.  
It supports both *faithful semantics* and *creative reinterpretation* while remaining human-readable and portable.
