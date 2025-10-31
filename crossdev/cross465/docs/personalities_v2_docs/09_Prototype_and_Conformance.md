# Prototype and Conformance
[← Adapter Layer and Shims](08_Adapter_Layer_and_Shims.md) • [→ Pointer Decoders & Address Semantics](10_Pointer_Decoders_and_Address_Semantics.md)

This chapter outlines the plan for implementing and validating the expanded personality model.

---

## Milestone 6 — Semantic Prototype

### Objective
Demonstrate complete C64 register-level compatibility with modern rendering.

### Implementation Tasks
1. Instance arrays and `$D010` scatter mapping.  
2. Bitfield-level policies (`on_read`, `ro`, etc.).  
3. Write fan-out (mask registers).  
4. Mirrors and constant open-bus behaviour.  
5. Compute expressions for raster/beam and collision states.  
6. Sprite and video adapters connecting MMIO to engine.

### Deliverables
- **c64-compat-extended.toml** personality file.  
- **Functional demo** showing sprites rendered through a modern backend.  

---

## Milestone 7 — Extended Compatibility

### Goals
1. Add MEGA65 personality (8 sprites + extended attributes).  
2. Add conditional layouts and selector front-ends.  
3. Run conformance tests verifying:  
   - coordinate and color correctness  
   - enable masks and read-to-clear bits  
   - scatter/gather mapping accuracy  
4. Confirm static address tables and O(1) decode performance.

---

## Validation

| Test Area | Purpose |
|:--|:--|
| **Register coverage** | Every logical register maps uniquely. |
| **Latch/flag behaviour** | `on_read` clears correct bits. |
| **Functional parity** | Results match reference behaviour. |
| **Backend linkage** | MMIO writes match backend sprite state. |

---

## Success Criteria

| Goal | Description |
|:--|:--|
| **Functional Equivalence** | Legacy code runs unmodified at MMIO level. |
| **Backend Flexibility** | Multiple backends reuse the same personality. |
| **Performance** | Decoding and mapping remain O(1). |
| **Developer Efficiency** | Ports and remasters can extend easily. |

---

**Summary:**  
The extended model formalizes register semantics, not transistor physics.  
Its success is measured by how easily it lets developers bring 8‑bit games to richer modern environments.
