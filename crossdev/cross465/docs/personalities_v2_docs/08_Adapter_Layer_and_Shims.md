# Adapter Layer and Shims
[← Mapping DSL and Examples](07_Mapping_DSL_and_Examples.md) • [→ Prototype and Conformance](09_Prototype_and_Conformance.md)

Adapters form the runtime bridge between **personality definitions** and the **engine backend**.  
They ensure MMIO operations in 6502 code update modern system state correctly and efficiently.

---

## 1. Role of the Adapter Layer

Adapters interpret reads, writes, and events from the MMIO personality.  
They then translate these into backend function calls — such as updating sprite positions, setting colors, or scheduling IRQs.

They guarantee that:
- Writes to registers modify the correct engine state.  
- Reads reconstruct values that 6502 software expects.  
- Timing and latch behaviour remains semantically correct.  
- Pointer decoders and scatter/fanout rules are applied properly.

---

## 2. Core Responsibilities

| Function | Description |
|:--|:--|
| **Instance virtualization** | Map N hardware sprite registers to M backend entities. |
| **Field projection** | Join or split fields, handle scatter/gather operations. |
| **Latch handling** | Emulate read‑to‑clear or shared flag registers. |
| **Hook execution** | Trigger `on_read` or `on_write` logic for dynamic side effects. |
| **Timing coordination** | Schedule frame or raster callbacks. |
| **Pointer decode routing** | Deliver `(index, subindex)` to backend (e.g., set_sprite_variant). |
| **Scatter/fanout interpretation** | Apply per‑bit scatter updates or broadcast fanouts. |
| **Scaling integration** | Apply transform fallback if no scaled asset variant exists. |

---

## 3. Scatter vs Fanout Behaviour

- **Scatter** mappings are *bi‑directional*; writes decompose bits into fields, reads rebuild them.  
- **Fanout** mappings are *one‑way*; a write broadcasts updates to multiple destinations or modules.  
- Fanout registers are typically **synthetic** (e.g., `D500 ResetAll`) and not readable.  
- The adapter should never expect a readback value from fanout registers.

---

## 4. Pointer Decoder Interaction

Pointer decoders emit `(index, subindex)` pairs for sprites, screens, or other resources.  
The adapter translates these into backend calls.

```rust
fn set_sprite_variant(idx: u8, subindex: u8);
fn set_screen_base(addr: u16);
fn set_charset_base(addr: u16);
```

For sprite personalities using scaling extensions (see [Scaling‑aware Pointer Decode in 07](07_Mapping_DSL_and_Examples.md#12-scalingaware-pointer-decode-option-a-with-fallback)), the adapter may also check expansion bits and apply transform scaling:

```rust
fn set_sprite_variant(idx: u8, subindex: u8) {
    let anim =  subindex        & 0b11;
    let x2   = (subindex >> 2)  & 1 != 0;
    let y2   = (subindex >> 3)  & 1 != 0;

    if backend.has_variant(idx, anim, x2, y2) {
        backend.bind_variant(idx, anim, x2, y2);
    } else {
        backend.bind_variant(idx, anim, false, false);
        backend.set_scale(idx, if x2 {2.0} else {1.0}, if y2 {2.0} else {1.0});
    }
}
```

---

## 5. Testing

| Test | Expectation |
|:--|:--|
| **Write‑read consistency** | Scatter fields correctly recombine to register values. |
| **Latch handling** | Read‑to‑clear and toggle bits behave as expected. |
| **Raster/timing events** | Hooks fire at proper frame or raster timing. |
| **Pointer decoding** | `(index, subindex)` matches MMIO writes. |
| **Scaling fallback** | Fallback transform triggers when variant art missing. |
| **Fanout safety** | No readback dependency on write‑only fanout registers. |

---

## 6. Summary

Adapters give *asm465* personalities real behaviour.  
They unify legacy register logic with modern rendering, ensuring that **scatter**, **fanout**, and **scaling‑aware pointer decoders** all function consistently across C64, MEGA65, and modern backends.
