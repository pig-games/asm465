# Migration Plan & Conformance Tests
[← Value Builders](04-Value-Builders.md)

## Migration (Incremental)

1. **Introduce factories/registry**
   - Register existing modules as implementations (e.g., `display.basic2d`).

2. **Add `regs()` descriptors**
   - Provide core reg sets (and any `Ext_*`) per implementation.

3. **Implement decoder**
   - New bus lookup from personality maps (range/sparse, priority, conditions).

4. **Default personality**
   - Reproduce today’s contiguous windows with a simple `range` map.

5. **Sample vintage personality**
   - C64-style sparse map with active-low input and VIC IRQ read-to-ack.

6. **Value Builders**
   - Add packer; wire to input module signals.
   - Implement C64/Atari examples.

7. **Interrupt wiring**
   - Optional: personality-driven IRQ/NMI sources and ack hooks.

8. **CLI & tooling**
   - `--personality`, `--list-personalities`, `--list-modules`.
   - Memory map viewer; show map layers/conditions.

## Conformance Suites

### Core Register Behavior Tests
- **RO/WO** bits: enforce per descriptor.
- **Reset values**: verify after cold/warm reset.
- **Hooks**: read-to-ack/write-to-ack effects visible and idempotent.
- **Timing**: if applicable (e.g., VBlank cadence).

### Decoder & Mapping Tests
- Range: placement, stride, overflow detection.
- Sparse: overlap resolution via `priority`.
- Conditions: layer swaps on bank/write.
- Mirroring (if enabled): semantics proven.

### Value Builder Tests
- Active-low bit packing for C64 joysticks.
- Atari split directions + separate TRIGx.
- Float scaling to bytes; nibble packing.
- Multi-byte output from one builder.

## Performance
- Compile maps into **O(1)** address lookup (array/vector by address).
- Pre-resolve signal and hook names to IDs/func pointers.
- Recompose table only on condition changes.

## Deliverables Checklist
- `mmio_core` crate: traits, registry, decoder, transforms.
- Personality loader: TOML → internal structures with validation.
- Sample personalities:
  - `modern-retro-linear`
  - `c64-compat-sparse`
  - `atari-compat-sparse` (with NMI gating)
- Docs: these five spec files.