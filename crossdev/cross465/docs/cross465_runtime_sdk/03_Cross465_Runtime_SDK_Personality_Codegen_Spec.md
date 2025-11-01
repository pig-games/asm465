
# Cross465 Runtime SDK — Personality Codegen Specification (03)
[← 02: Packager Tool](02_Cross465_Runtime_SDK_Packager_Tool_Spec.md) | [→ 04: Display Module Scaffold](04_Cross465_Runtime_SDK_Display_Module_Scaffold_RetroModern2D.md)

**Status:** Draft • **Audience:** Framework/Tool authors • **Targets:** Desktop & WASM

---

## 1. Purpose
Transform a declarative **personality.toml** into **static Rust** that behaves identically to the dynamic loader, enabling single-binary and WASM runtimes.

---

## 2. Inputs & Outputs
- **Input:** `personality.toml` (+ includes, future).  
- **Output:** `personality_gen.rs` with a `PersonalityDef` implementor and `personality!{}` macros.

---

## 3. Pipeline
1) Parse TOML → AST  
2) Validate (schema + semantics)  
3) Lower to IR (blocks, regs, scatter/fanout, decoders)  
4) Emit deterministic Rust

```
TOML ─▶ AST ─▶ IR ─▶ Rust (macros)
```

---

## 4. TOML → Macro Mapping (Summary)
| TOML Construct | Generated Macro |
|----------------|------------------|
| `[[map]]` | `reg!`, `scatter!`, `compose!` |
| `fanout` | write-only synthetic register glue |
| `pointer_decoder.*` | `ptrdec!` with `sub!` and `idx!` helpers |
| `policy` / `read_clears` | `policy!` field rules |
| `timing` hooks | `hook!` with `on_write`/`on_read` |
| `conditions` | `cond!` + guarded blocks |

---

## 5. Examples

### 5.1 Registers + Scatter
**Input (TOML):**
```toml
[[map]]
decode = { instances = {
  count=8, index_var="i", kind="sprite", base="D000",
  layout = [
    { addr="D000 + (i*2)", id="XLo" },
    { addr="D001 + (i*2)", id="YLo" },
    { addr="D010 bit i",   id="XHi" },
    { addr="D015 bit i",   id="Enable" }
  ]
}}
```

**Output (Rust excerpt):**
```rust
mmio = [
  reg!(addr="D000", id="sprite[0].XLo", kind="sprite"),
  reg!(addr="D001", id="sprite[0].YLo", kind="sprite"),
  // sprite[1..7] expanded...
  scatter!(addr="D010", bits=8, kind="sprite", field="XHi"),
  scatter!(addr="D015", bits=8, kind="sprite", field="Enable"),
],
```

### 5.2 Fanout (write-only broadcast)
**Input (TOML):**
```toml
[[map]]
fanout = { addr="D5F0", id="ResetAll",
  write = [
    { to = { module="display", id="ResetSprites" } },
    { to = { module="audio",   id="ResetVoices"  } }
  ]
}
```

**Output (Rust glue):**
```rust
fanout!(addr="D5F0", id="ResetAll", routes=[
  route!(to="display.ResetSprites"),
  route!(to="audio.ResetVoices")
]);
```

### 5.3 Pointer Decoder + Extension Bits
**Input (TOML):**
```toml
[pointer_decoder.vic2_sprites]
source   = { table="07F8..07FF", kind="sprite", entries=8 }
block    = 64
subindex = { bits=2, extend=[ { from="D01D bit i" }, { from="D017 bit i" } ] }
index    = { from="pointer", shift=2 }
```

**Output (Rust):**
```rust
ptrdec! {
  name="vic2_sprites",
  source=table!("07F8..07FF", entries=8, kind="sprite"),
  block=64,
  subindex=sub!(bits=2, extend=[ from!("D01D bit i"), from!("D017 bit i") ]),
  index=idx!(from="pointer", shift=2)
}
```

### 5.4 Policies & Timing Hooks
**Input (TOML):**
```toml
[policy.vic_irq]
id="vic.IrqStatus"
fields=[
  { bit=0, on_read="vic_ack_raster" },
  { bit=1, on_read="vic_ack_sprite" },
  { bit=2, ro=true }
]

[hook.raster_compare]
addr="D012"
on_write="vic_set_raster_cmp"
timing="raster_start"
```

**Output (Rust):**
```rust
policy!(id="vic.IrqStatus", fields=[
  field!(bit=0, on_read="vic_ack_raster"),
  field!(bit=1, on_read="vic_ack_sprite"),
  field!(bit=2, ro=true)
]);

hook!(addr="D012", on_write="vic_set_raster_cmp", timing="raster_start");
```

---

## 6. Determinism & Validation
- Sorted output: address → id → source order.  
- Errors:
  - `P001` overlapping non-scatter registers.  
  - `P002` scatter width mismatch.  
  - `P003` fanout readable (forbidden).  
  - `P004` invalid pointer block (non power-of-two).  
  - `P005` unresolved symbol/expr.

---

## 7. Dynamic vs Generated: Parity Check
| Aspect | Dynamic TOML | Generated Macro |
|--------|---------------|------------------|
| Load time | Parse at startup | Zero (compiled) |
| Errors | Runtime diagnostics | Build-time diagnostics |
| WASM | Needs bundling or fetch | Single `.wasm` |
| Behaviour | Identical (snapshot tests ensure parity) | Identical |

---

## 8. Testing
- **Snapshot tests**: TOML → Rust must match golden output.  
- **Roundtrip tests**: dynamic vs generated runtime behaviour matches on fixture ROMs.  
- **Negative tests**: confirm validation errors trigger correctly.

---

## 9. Future Extensions
- Source includes; schema versions.  
- Richer timing windows (scanline windows).  
- Auto-doc generation (register tables from TOML).


---

*Part of the **Cross465 Runtime SDK** doc set.*  
Index: [00 Overview](00_Cross465_Runtime_SDK_Overview.md)
