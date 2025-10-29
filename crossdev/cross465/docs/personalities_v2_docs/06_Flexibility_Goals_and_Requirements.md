# Personalities v2 — Flexibility Goals & Requirements
[← Migration and Tests](05-Migration-and-Tests.md) • [→ Mapping DSL and Examples](07_Mapping_DSL_and_Examples.md)

## Context

The Personalities v2 implementation of *asm465* already enables a large degree of flexibility for defining platform‑specific MMIO personalities. However, to support ports from **vintage 6502‑based systems** like the Commodore 64 or MEGA65 while targeting **modern enhanced backends**, we need richer semantic mapping capabilities.

This version of the spec focuses on *logical register compatibility*, not cycle‑accurate emulation. It allows you to express how software expects registers to behave — not how hardware signals are timed.

---

## 1. Philosophy and Scope

| Aspect | In Scope | Out of Scope |
|:--|:--:|:--:|
| Register layouts, read/write masks, latch and acknowledge behaviour | ✅ | |
| Raster or frame‑level timing hooks | ⚙ optional | |
| Cycle‑level or DMA timing | | ❌ |
| Sprite multiplexing (mid‑frame reprogramming) | | ❌ |
| Bank/mode switching, conditional layouts | ✅ | |
| Open‑bus and mirror semantics | ⚙ optional, simplified | |

The result is a **portable register‑semantic model** that can feed both accurate cross‑development targets (Mega65, Ultimate64) and modern rendering backends (2D/3D pipelines).

---

## 2. Why More Flexibility Is Needed

Vintage systems differ drastically in how their I/O blocks are organized:

- The **C64 VIC‑II** exposes eight sprite instances, each with individual registers plus shared bitfields (`$D010` for X high bits, `$D015` for enable bits).  
- The **MEGA65 VIC‑IV** still provides **8 hardware sprites**, but adds extended coordinates, scaling, colour depth and banked registers.  
- Modern engines often use a single sprite array with **selector‑based updates** or **virtual sprite pools**.  

Bridging those models requires a mapping system that can express:

1. **Instance arrays** – groups of registers repeated per entity (e.g., sprite, channel).  
2. **Scatter/gather mappings** – joining or splitting bits across multiple registers.  
3. **Field‑level policies** – read‑to‑acknowledge, read‑only, or inverted bits.  
4. **Compute fields** – derived values like beam position or collision flags.  
5. **Conditional layouts** – bank or mode dependent register sets.  
6. **Mirrors/open‑bus rules** – optional for undefined regions.  

---

## 3. Design Goals

| Goal | Description |
|------|--------------|
| **Logical Compatibility** | Vintage 6502 code can run without modification. |
| **Modern Integration** | The same mapping works with new graphics, sound, or input backends. |
| **Predictable Behaviour** | Registers respond deterministically; timing detail is coarse‑grained. |
| **Configurability** | Personalities can mix predefined modules and redefine register addresses freely. |
| **Reusability** | Shared MMIO modules (Display, Input, Audio) can serve multiple platforms. |

---

## 4. Key Requirements Summary

1. **Instance Arrays with Address Templates**  
   Express repeated structures (e.g., 8 sprite objects). Each instance defines its own offset layout.

2. **Scatter/Gather Field Mapping**  
   Link one logical value to multiple register locations (e.g., C64 X‑Hi bits).

3. **Bitfield‑Level Access Policies**  
   Define per‑bit read/write rules, allowing fine‑grained latch or acknowledge semantics.

4. **Multi‑Target Writes**  
   Fan‑out a single MMIO write (e.g., enable mask) across several logical registers.

5. **Compute Expressions**  
   Generate register values from module state (e.g., raster counter or collision summary).

6. **Conditional Instance Sets**  
   Swap layouts when the system changes mode or bank.

7. **Mirrors and Open Bus**  
   Declare repeated address ranges and default return values for unmapped reads.

---

## 5. Conceptual Examples

| Legacy Behaviour | Modern Mapping Strategy |
|:--|:--|
| `$D000..$D01F`: 8 sprite register blocks | `instances = { count = 8, layout = [...] }` |
| `$D010`: sprite X‑high bits (one bit per sprite) | Scatter mapping to each instance’s `XHi` |
| `$D01C`: sprite enable mask | Write‑fan‑out to 8 sprite `Enable` bits |
| `$D019`: interrupt flags | Field policies with read‑to‑clear bits |
| `$D41B`: raster counter | Compute expression `beam.y` |
| Sprite multiplexing | Use selector‑based backend (no timing required) |

---

## 6. Open‑Bus Simplification

An *open bus* occurs when the CPU reads from an address with no responding device.  
The physical lines retain the previous byte, producing pseudo‑random results.  
In this framework, open‑bus behaviour is simplified:

```toml
[[open_bus]]
range = "DE00..=DEFF"
policy = "const"
const  = "00"  # Return a constant value for undefined reads
```

Optionally:
```toml
policy = "last_read"
```
to mimic vintage floating‑bus behaviour (useful only for test compatibility).

---

## 7. Summary

The extended mapping model gives each personality the tools to express **complex register structures** (like C64’s sprite or IRQ blocks) while remaining flexible enough for **modern rendering and input architectures**.

The philosophy: *keep the semantics, modernize the backend.*
