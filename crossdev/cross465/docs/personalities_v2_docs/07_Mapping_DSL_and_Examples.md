# Mapping DSL and Examples
[← Flexibility Goals](06_Flexibility_Goals_and_Requirements.md) • [→ Adapter Layer and Shims](08_Adapter_Layer_and_Shims.md)

This section defines the extended TOML syntax used to describe address decoding and behaviour for personality files. The goal is to be descriptive enough for classic systems (C64, MEGA65) yet flexible for modern engines.

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
    kind = "sprite",
    selector = "Select",        # optional; pre-sets the slot before each access
    count = 8,                  # 8 hardware sprites (C64/MEGA65)
    index_var = "i",            # template variable
    layout = [
      { addr = "D100 + (i*8)", id = "Number" },
      { addr = "D101 + (i*8)", id = "Anim" },
      { addr = "D102 + (i*8)", id = "XLo" },
      { addr = "D103 + (i*8)", id = "XHi" },
      { addr = "D104 + (i*8)", id = "YLo" },
      { addr = "D105 + (i*8)", id = "Scale" },
      { addr = "D010",         id = "XHi", field = { bit = "i" } }
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

## 4. Multi-Target Writes (Fanout)

### Concept
A write to one register updates several underlying fields.

### Example
```toml
{ addr = "D015", kind = "sprite", id = "Enable", suppress_write = true,
  write_fanout = [
    { id = "Enable", instance = "*", from_bits = "0..7" }
  ]
}
```
### Use Case
C64 sprite enable mask `$D015`.  
The personality suppresses the primary write (since the MMIO has no mask register) and fans the bits out into each sprite's `Enable` flag.

---

## 5. Timing Hooks (Simplified)

### Concept
Associate a register write or read with a scheduled event — frame or raster level.

### Example
```toml
{ addr="D012", kind="video", id="RasterCompare",
  transform = { on_write="vic_set_raster_cmp" },
  timing = { align="raster_start" }
}
```
### Why It Matters
Timing hooks allow mapping raster interrupts or similar triggers without cycle-level precision.

---

## 6. Conditional Instance Sets

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

## 7. Compute Expressions

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
### Benefit
Useful for dynamic status or derived metrics (beam position, collision state).

---

## 8. Mirrors and Open-Bus

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

## 9. Example — C64 Sprite Block

### Complete Definition
```toml
[[map]]
priority = 10
decode = { instances = {
  kind = "sprite",
  selector = "Select",
  count = 8,
  index_var = "i",
  layout = [
    { addr = "D100 + (i*8)", id = "Number" },
    { addr = "D101 + (i*8)", id = "Anim" },
    { addr = "D102 + (i*8)", id = "XLo" },
    { addr = "D103 + (i*8)", id = "XHi" },
    { addr = "D104 + (i*8)", id = "YLo" },
    { addr = "D105 + (i*8)", id = "Scale" },
    { addr = "D010",         id = "XHi", field = { bit = "i" } }
  ]
} }

[[map]]
priority = 10
decode = { sparse = [
  { addr = "D015", kind = "sprite", id = "Enable", suppress_write = true,
    write_fanout = [{ id = "Enable", instance = "*", from_bits = "0..7" }] }
] }
```

### How It Works
- Each sprite gets its own registers.
- Shared registers (e.g., `$D010`) are scattered automatically.
- `$D015` writes behave like the real C64 mask: the personality fans out the bits, and reads reconstruct the mask from the per-sprite enable flags.
- Modern backends can map this directly to sprite objects.
- **Note:** The reference `c64-compat` personality deliberately maps the sprite block to `$D100+`. We expose more per-sprite registers than the VIC-II originally provided, so the classic `$D000..=D03F` range would be too small to host the extended data.

---

## 10. Summary

The Mapping DSL bridges classic register organization with modern engine architectures.  
It supports both *faithful semantics* and *creative reinterpretation* while remaining human-readable and portable.
