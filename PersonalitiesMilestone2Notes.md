# Milestone 2 Notes — Personality Data Model

## Loader & Data Structures (`crossdev/cross465/bus/src/personality_v2.rs`)
- Introduced `PersonalityDef` with metadata, module catalog, conditions, maps, and interrupt wiring mirroring the v2 TOML specification.
- Map decoding supports `range` and `sparse` layouts, compiling address strings into `u16` ranges and resolving register names via the module registry descriptors.
- Conditions and interrupt sources validate module kinds and register identifiers, surfacing descriptive loader errors when references are missing or mismatched.
- Value builder definitions are parsed into strongly typed descriptors (`ValueBuilder`, `BitBinding`) tied to the owning sparse entry and register width.

## Registry Integration
- Module definitions look up factories from the shared `ModuleRegistry`, ensuring personality files only reference registered implementations of the expected `ModuleKind`.
- Register descriptors now include canonical names (`RegisterDesc::name`) so the loader can translate TOML register strings into concrete `RegId`s for downstream compilation.

## Runtime Integration (`crossdev/cross465/bus/src/lib.rs`)
- Added `Bus::from_personality_def`, which compiles a `PersonalityDef` into a `PersonalityRuntime` with instantiated modules, a 64 KB address table, and cached condition metadata.
- Range/sparse maps insert `AddressSlot`s with priority resolution (higher priorities override lower while same-priority collisions error).
- Legacy personalities remain available via `Bus::with_personality`; v2 runtime coexists alongside the legacy `mmio` mapping and reuses host helpers (console/display/sprite accessors).
- Condition descriptors are evaluated on MMIO writes; when a condition’s value changes, the runtime rebuilds the address table so `active_when` overlays activate/deactivate immediately.

## Value Builder Engine & Signals
- `ValueBuilder::build` executes bit bindings against a `SignalStore` implementing the spec’s `InputSignals` trait.
- The bus exposes `set_signal_bool`, `set_signal_int`, `set_signal_float`, and `clear_signals` so hosts can feed builder inputs ahead of read cycles.
- Current implementation supports bit bindings, optional `const_set`, and `invert_byte`; additional field/float packing hooks can build on this scaffolding.

## Diagnostics & Tests
- `LoaderError` captures human-readable validation failures leveraged by unit tests.
- Runtime tests cover mapping a display personality and exercising value builders (`tests::bus_from_personality_def_maps_display_registers`, `tests::value_builder_packs_signal_bits`).
