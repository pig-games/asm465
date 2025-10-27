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
- [x] Define in-memory structures mirroring the v2 TOML schema (modules, maps, transforms, conditions, interrupts, value builders). (see `crossdev/cross465/bus/src/personality_v2.rs`)
- [x] Implement the TOML loader with validation for module IDs, register IDs, hooks, and transforms. (see `crossdev/cross465/bus/src/personality_v2.rs`)
- [x] Provide clear diagnostics for invalid personalities and surface loader errors to callers. (see `PersonalitiesMilestone2Notes.md`)

## Milestone 3 – Decoder Integration
- [x] Compile range and sparse maps into an address lookup table with priority resolution. (see `crossdev/cross465/bus/src/lib.rs`)
- [x] Update bus construction to instantiate modules via the registry and apply the compiled decoder output. (see `crossdev/cross465/bus/src/lib.rs`)
- [x] Wire condition tracking so decoder tables rebuild when personality-defined triggers change. (see `crossdev/cross465/bus/src/lib.rs`)

## Milestone 4 – Value Builder Pipeline
- [x] Implement the value-builder engine and input signal resolution API with cached lookups. (see `crossdev/cross465/bus/src/personality_v2.rs`)
- [x] Execute builders in the bus read path ahead of module `read` calls, then apply transforms. (see `crossdev/cross465/bus/src/lib.rs`)
- [x] Cover boolean bit packing, numeric field packing, multi-byte outputs, and post-processing with targeted tests. (see `crossdev/cross465/bus/src/lib.rs` tests)

## Milestone 5 – Reference Personalities
- [x] Author a range-based personality that reproduces today’s contiguous layout as a regression baseline. (see `crossdev/cross465/personality_defs/modern-retro-range.toml`)
- [x] Create sparse sample personalities (e.g., C64, Atari) demonstrating active-low transforms, read-to-ack hooks, and banking. (see `crossdev/cross465/personality_defs/c64-compat-sparse.toml`)
- [x] Expose CLI/tooling hooks (`--personality`, `--list-personalities`, `--list-modules`, map dumps) for selecting and inspecting personalities. (runner & asm465 CLI options)

## Milestone 6 – Conformance & Regression Tests
- [ ] Add suites validating register contracts (RO/WO, reset values, hook side effects) across module kinds.
- [ ] Test decoder behavior for ranges, sparse overlaps, and condition switching scenarios.
- [ ] Verify value builders (C64 joystick, Atari ports, float scaling) and measure decoder performance for O(1) lookups.
