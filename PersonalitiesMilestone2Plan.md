# Milestone 2 Planning — Personality Data Model

## Current State Summary
- The runtime personality surface (`crossdev/cross465/bus/src/personality.rs`) exposes a static `Personality` struct containing:
  - `name`, `description`, display defaults, and a slice of `PersonalityMmio` ranges.
  - Each `PersonalityMmio` pairs a fixed address range with a factory closure returning one built-in MMIO device.
  - Optional interrupt metadata (`PersonalityInterrupt`) lists sources and defaults but lacks per-map wiring.
- Personality selection occurs at bus construction (`Bus::with_personality`), which iterates `personality.mmio` and installs devices directly; there is no address decoder or condition handling.
- No representation exists today for:
  - Module implementation selection/options (beyond the hard-coded factory closures).
  - Sparse vs range mapping, per-register order, or transforms (invert/RO/WO/hooks).
  - Banking/conditions, priority resolution, value builders, or interrupt source configuration tied to personalities.

## Gaps vs Personalities v2 Spec
- Need a data model that mirrors the TOML schema (`docs/personalities_v2_docs/03-Personality-Spec.md`), covering:
  - Personality metadata (`id`, `title`, `default_map_priority`).
  - Module selections (`[modules.<kind>] { impl, options }`).
  - Conditions for banking/overlays (`[conditions.<name>]`).
  - Map entries with `range`/`sparse` decoders, transforms, optional value builders, and priorities.
  - Interrupt wiring (sources, ack hooks, line type).
- Loader must validate references: module impl IDs, register IDs, condition names, transform hook names, builder signal names.
- Runtime structures should pre-resolve:
  - Module factories from the registry.
  - Register descriptors for quick lookup by `RegId`.
  - Condition descriptors to enable efficient change detection.
  - Value builder definitions (compiled signal lookup tables).

## Proposed Deliverables
1. **Data Structures Module** (`bus::personality_v2` or similar)
   - Define structs/enums representing the parsed TOML (e.g., `PersonalityDef`, `ModuleConfig`, `Map`, `RangeMap`, `SparseMapEntry`, `Transform`, `Condition`, `InterruptConfig`, `ValueBuilder`).
   - Include resolved handles (factory pointers, register references) to avoid string lookups at runtime.
2. **Loader API**
   - `PersonalityLoader::from_toml(&str, &ModuleRegistry)` returning a validated `PersonalityDef` or error type.
   - Structured error reporting with spans/context for invalid references.
3. **Validation Coverage**
   - Checks for duplicate IDs, unknown module kinds/impls, overlapping map ranges, invalid register names, missing transforms/hooks, builder width mismatches.
   - Unit tests covering success and failure cases.
4. **Diagnostics Strategy**
   - Error type capturing severity, location (line/column), and human-readable message to surface in CLI/tooling.

## Next Steps
- Sketch Rust types mirroring the TOML schema, deciding how to represent conditions and map variants ergonomically.
- Plan loader phases (parse → resolve modules/regs → validate maps/conditions → compile builders stubs).
- Prepare test scaffolding (sample TOML snippets) to exercise validation rules once implementation begins.
