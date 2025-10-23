# Personalities v2 Implementation Plan

Progress tracker for rolling out the cross465 Personalities v2 architecture.

## Milestone 0 – Baseline Audit
- [x] Catalogue current bus/MMIO wiring and document where register addresses and semantics live. (see `PersonalitiesMilestone0Audit.md`)
- [x] Inventory existing module implementations and identify missing register descriptors versus the v2 model.

## Milestone 1 – Module Runtime Upgrades
- [x] Introduce shared enums/traits (`ModuleKind`, `RegId`, `Module`, `RegisterDesc`) to support address-free registers. (see `crossdev/cross465/bus/src/mmio.rs`)
- [x] Wrap each implementation in a `ModuleFactory` and register it in a central `ModuleRegistry`. (see `crossdev/cross465/bus/src/lib.rs`)
- [x] Populate `regs()` descriptors with reset/RO/WO metadata for each module, adding spot tests where needed. (see `PersonalitiesMilestone1Notes.md`)

## Milestone 2 – Personality Data Model
- [ ] Define in-memory structures mirroring the v2 TOML schema (modules, maps, transforms, conditions, interrupts, value builders).
- [ ] Implement the TOML loader with validation for module IDs, register IDs, hooks, and transforms.
- [ ] Provide clear diagnostics for invalid personalities and surface loader errors to callers.

## Milestone 3 – Decoder Integration
- [ ] Compile range and sparse maps into an address lookup table with priority resolution.
- [ ] Update bus construction to instantiate modules via the registry and apply the compiled decoder output.
- [ ] Wire condition tracking so decoder tables rebuild when personality-defined triggers change.

## Milestone 4 – Value Builder Pipeline
- [ ] Implement the value-builder engine and input signal resolution API with cached lookups.
- [ ] Execute builders in the bus read path ahead of module `read` calls, then apply transforms.
- [ ] Cover boolean bit packing, numeric field packing, multi-byte outputs, and post-processing with targeted tests.

## Milestone 5 – Reference Personalities
- [ ] Author a range-based personality that reproduces today’s contiguous layout as a regression baseline.
- [ ] Create sparse sample personalities (e.g., C64, Atari) demonstrating active-low transforms, read-to-ack hooks, and banking.
- [ ] Expose CLI/tooling hooks (`--personality`, `--list-personalities`, map viewer stubs) for selecting and inspecting personalities.

## Milestone 6 – Conformance & Regression Tests
- [ ] Add suites validating register contracts (RO/WO, reset values, hook side effects) across module kinds.
- [ ] Test decoder behavior for ranges, sparse overlaps, and condition switching scenarios.
- [ ] Verify value builders (C64 joystick, Atari ports, float scaling) and measure decoder performance for O(1) lookups.
