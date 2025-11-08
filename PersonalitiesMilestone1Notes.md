# Milestone 1 Notes — Module Runtime Upgrades

Summary of the scaffolding added for the Personalities v2 module runtime.

## Shared MMIO Core (`crossdev/cross465/bus/src/mmio.rs`)
- Introduced `ModuleKind`, `RegId` (with per-kind enums), and `RegisterDesc`/`BitField` metadata.
- Defined the `Module` trait (super-trait of `MmioDevice`) with register-level read/write, snapshot/tick hooks.
- Added `ModuleFactory`, `ModuleRegistry`, and `ModuleDeps` to construct modules with shared dependencies.

## Built-In Module Factories
- Console (`console.text`), Display (`display.basic2d`), Sprite (`sprite.basic`), System (`system.interrupts`) each expose:
  - Static `RegisterDesc` arrays capturing reset values and access modes.
  - `Module` trait implementations delegating to existing MMIO logic.
  - Factories wired into `builtin_module_registry()` for discovery.

## Tests
- Added sanity checks per module to exercise `Module::regs`/`Module::read_reg`/`Module::write_reg`.
- Verified registry wiring through a new unit test in `bus/src/lib.rs`.
