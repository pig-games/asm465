# cross465 Personalities v2 — Architecture Overview
[→ Module & Registers](02-Module-and-Registers.md) • [→ Personality File](03-Personality-Spec.md) • [→ Value Builders](04-Value-Builders.md) • [→ Migration & Tests](05-Migration-and-Tests.md)

## Purpose
A flexible, data-driven MMIO mapping system where:
- Personalities can map **individual registers** (not just a base address).
- Each **module kind** (Display/Input/Video/Audio/System) can have **multiple implementations** (e.g., `display.basic2d`, `display.2dto3d`).
- Addressing, banking, and **hardware semantics** (active-low, read-to-ack) are configured in **TOML**.
- Optional **value builders** pack logical signals (e.g., gamepad buttons/axes) into platform-specific register bitfields.

## Core Ideas
- **ModuleKind vs Implementation**: stable API surface vs specific backend logic.
- **Register descriptors** (address-free): the single source of truth for register shape/resets/RO/WO.
- **Personality maps**: `range` (contiguous) or `sparse` (per-register), with priorities and conditions for banking.
- **Transforms**: per-mapping behavior (invert, RO/WO, hooks).
- **Value Builders**: declarative bit/nibble/byte/word packing from backend signals.

## Outcomes
- Reuse MMIO modules across many personalities.
- Vintage-accurate sparse layouts or simple “mount the whole block” modern setups.
- Switch Display implementation (2D vs 2D→3D) without changing 6502-side code.
