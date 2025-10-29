# Adapter Layer and Shims
[← Mapping DSL and Examples](07_Mapping_DSL_and_Examples.md) • [→ Prototype and Conformance](09_Prototype_and_Conformance.md)

Adapters form the runtime layer that connects the **declarative mapping** to the **backend logic**.  
They interpret register operations and convert them into modern engine calls.

---

## 1. Purpose

Adapters decouple legacy MMIO behaviour from modern backend logic.

They ensure that:
- Writes to registers modify the correct engine state.
- Reads from registers return values derived from that state.
- Flags, IRQs, and latches behave consistently with vintage expectations.

---

## 2. Responsibilities

| Task | Example |
|:--|:--|
| **Instance virtualization** | 8 sprite registers controlling 8 or more virtual sprites. |
| **Field projection** | Split X into XLo/XHi or merge into 16-bit coordinate. |
| **Latch handling** | Read-to-clear collision flags. |
| **Hook routing** | Trigger VIC-II-style IRQ acknowledge on read. |
| **Timing hooks** | Execute per-frame or raster callbacks. |
| **Compute evaluation** | Evaluate `beam.y` or `coll.spr_spr` expressions. |
| **Open-bus/mirror fallback** | Handle unmapped reads. |

---

## 3. Adapter Traits

```rust
pub trait SpriteAdapter: Module {
    fn set_xy(&mut self, i: usize, x: u16, y: u16);
    fn get_xy(&self, i: usize) -> (u16,u16);
    fn set_enable(&mut self, i: usize, on: bool);
    fn get_enable(&self, i: usize) -> bool;
    fn set_color(&mut self, i: usize, c: u8);
}

pub trait VideoAdapter: Module {
    fn beam(&self) -> (u16,u16);
    fn irq_status(&self) -> u16;
    fn irq_ack_bits(&mut self, mask: u16);
}
```

### Explanation
- The **SpriteAdapter** connects C64-style sprite registers to any rendering backend.  
- The **VideoAdapter** handles raster positions, IRQ flags, and related state.

---

## 4. Design Philosophy

- **Frame-level accuracy** is enough. Cycle-accuracy not required.  
- **Selector front-ends** can simulate sprite multiplexing.  
- **Multiple vintage modules** can share a single backend module.

---

## 5. Testing

| Test | Expectation |
|:--|:--|
| Write-read consistency | Writing register updates backend state. |
| Bit ack behaviour | Bits flagged `on_read` clear correctly. |
| Raster hook | Events trigger at correct raster or frame. |
| Open-bus | Reads from unmapped range return expected constant. |

---

## 6. Summary

Adapters turn the abstract MMIO map into a living bridge between old and new.  
They enable *asm465* to preserve logic while modernizing presentation and control.
