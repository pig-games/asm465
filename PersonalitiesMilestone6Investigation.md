# Milestone 6 Plan — C64 Semantic Prototype

## Updated Objective
Deliver a register-accurate Commodore 64 personality that runs unmodified 6502 code while driving the modern rendering backend. The milestone graduates from investigation to a working semantic prototype, as outlined in `docs/personalities_v2_docs/06-09`.

## Scope and Philosophy
- Focus on **logical register compatibility**; frame/raster timing hooks are coarse-grained.
- Implement sprite, IRQ, and collision semantics; defer cycle-accurate DMA or BAD line modelling.
- Cover VIC-II + CIA pathways that impact sprite graphics, raster IRQs, and latch behaviour.

## Feature Requirements
1. **Instance Arrays + Scatter/Gather** — Sprite register blocks (`$D000-$D01F`) with `$D010` high bits per Chapter 7.
2. **Bitfield Policies & Hooks** — `on_read` / `on_write` semantics for IRQ and collision registers (`$D019`, `$D01E/$D01F`).
3. **Write Fan-out** — Mask registers (`$D015`, `$D01C`) broadcasting enable bits across sprite instances.
4. **Mirrors & Open Bus** — Declare mirrored ranges and constant/last-read behaviour for unmapped areas (`$DE00-$DEFF`).
5. **Compute Expressions** — Evaluate raster beam positions and collision summaries for readbacks.
6. **Adapter Layer** — Sprite/video adapters that translate MMIO operations to engine calls (Chapter 8).

## Implementation Tasks
- Extend the TOML loader/runtime to materialise instance arrays, scatter fields, and fan-out wiring in the compiled decoder.
- Wire up bitfield policies with hook callbacks so read-to-clear and latch semantics run through adapters.
- Implement compute-expression evaluation for beam/collision registers with cached module state.
- Introduce `SpriteAdapter`/`VideoAdapter` traits (or equivalent) and connect them to the bus runtime.
- Model mirrors and open-bus defaults in the decoder tables and register metadata.
- Produce `c64-compat-extended.toml` covering VIC-II sprites, raster IRQ, CIA joystick/keyboard matrix (reuse existing builders), and open-bus policy.
- Build a functional showcase (CLI or integration test) that exercises sprite movement, IRQ ACK, and collision reads using the modern backend.

## Adapter Shim Plan — Progress Log
- [x] Inventory rendering/backend sprite and video entry points so adapter hooks know which callback to trigger.
  - Sprite flow: adapters ultimately need to drive `SpriteOutput::set_sprite` so `SpriteState` snapshots reach Bevy (`crossdev/cross465/bus/src/sprite_mmio.rs:68`) and remain compatible with the polling path in `EmulatorState::sprite_snapshot` and `ui_system` (`crossdev/asm465/src/lib.rs:1177`, `crossdev/asm465/src/lib.rs:1654`).
  - Positioning & variant semantics: the viewer maps `SpriteState` into world transforms/texture slots via `sprite_world_transform` and the texture selection block inside `ui_system` (`crossdev/asm465/src/lib.rs:1788`, `crossdev/asm465/src/lib.rs:1966`), so adapters must populate `number`, `x/y`, and `scale_*` consistently with scatter/fanout rules.
  - Video palette bridge: border/background colours propagate through `DisplayOutput::set_border_color` / `set_background_color` (`crossdev/cross465/bus/src/display_mmio.rs:47`, `crossdev/cross465/bus/src/display_mmio.rs:51`) into `DisplayPalette::apply_snapshot` inside the viewer (`crossdev/asm465/src/lib.rs:818`), defining the colour hooks for a future video adapter.
  - Timing/IRQ signalling: modern frame/timer hooks surface through the shared `InterruptController` API (`crossdev/cross465/bus/src/interrupts.rs:38`) and are raised from the Bevy side via `InterruptBindings::raise_*` helpers (`crossdev/asm465/src/lib.rs:1322`), anchoring where video adapters should acknowledge raster-style events.
- [x] Extend `ModuleDeps` (or companion builder) to carry backend handles down to module factories and adapters.
  - Added a typed `BackendHandles` registry so factories/adapters can request pre-wired host objects (`crossdev/cross465/bus/src/mmio.rs:206`).
  - Plumbed those handles through `ModuleDeps::new`/`ModuleDeps::backend` and into the personality runtime so every module instantiation receives the same host context (`crossdev/cross465/bus/src/mmio.rs:259`, `crossdev/cross465/bus/src/mmio.rs:267`).
  - Exposed `Bus::from_personality_def_with_backends` so the viewer can supply concrete rendering/timing backends when building a v2 personality (`crossdev/cross465/bus/src/lib.rs:2037`).
- [x] Teach the bus runtime to register adapter callbacks alongside module instances, feeding scatter/fanout resolutions and hook names into the dispatcher.
  - Introduced adapter event types and trait so modules can emit primary/scatter/fanout updates plus hook notifications (`crossdev/cross465/bus/src/mmio.rs:324`).
  - Personality runtime now owns optional adapter slots per module and dispatches events from direct writes, scatter paths, fanouts, and field hooks (`crossdev/cross465/bus/src/lib.rs:866`, `crossdev/cross465/bus/src/lib.rs:1685`, `crossdev/cross465/bus/src/lib.rs:1759`, `crossdev/cross465/bus/src/lib.rs:1830`).
  - Added `Bus::attach_adapter` to let hosts register shims against specific module kinds in v2 personalities (`crossdev/cross465/bus/src/lib.rs:2194`).
- [x] Implement sprite adapter logic for instance arrays, `$D010` scatter bits, enable masks, scaling flags, and pointer decoder outputs.
  - Created `SpriteAdapter` that tracks per-instance state, consuming primary/scatter/fanout events and forwarding consolidated updates to a pluggable backend (`crossdev/cross465/bus/src/adapters/sprite.rs:52`).
  - Provided `SpriteBackend` trait plus a `SpriteOutputBackend` helper that mirrors adapter updates into the existing `SpriteOutput` snapshots for viewer parity (`crossdev/cross465/bus/src/adapters/sprite.rs:21`).
  - Added regression tests covering direct coordinate writes, `$D010` scatter bits, and fanout enable updates to ensure backend notifications stay in sync (`crossdev/cross465/bus/src/adapters/sprite.rs:166`).
- [x] Implement video adapter logic covering raster compare, IRQ status, collision latches, and read-to-clear semantics.
  - Added a generic `VideoAdapter` that forwards primary/scatter/fanout writes and hook notifications for system/video registers to pluggable backends (`crossdev/cross465/bus/src/adapters/video.rs:24`).
  - Defined the `VideoBackend` contract plus a `VideoStateBackend` recorder so runtimes can observe IRQ masks, collision flags, and read-to-clear hooks (`crossdev/cross465/bus/src/adapters/video.rs:14`, `crossdev/cross465/bus/src/adapters/video.rs:76`).
  - Backed the adapter with unit tests exercising direct writes, hook forwarding, and optional scatter/fanout callbacks to prove coverage of latch semantics (`crossdev/cross465/bus/src/adapters/video.rs:110`).
  - The viewer's interrupt panel now surfaces raster/IRQ state captured via the adapter-backed backend (`crossdev/asm465/src/lib.rs:1882`).
- [x] Update C64 personalities to bind the new adapters, then drive them through integration tests or demo harness runs to confirm behaviour.
  - Toml-driven builds now auto-attach sprite/video adapters when instantiating the bus, wiring the `SpriteOutput` snapshot and a video backend hook before the CPU starts (`crossdev/asm465/src/cpu_worker.rs:36`, `crossdev/asm465/src/cpu_worker.rs:611`).
- [x] Wire the sprite/video adapters to the modern 2D rendering backend so both legacy and custom personalities use the shared infrastructure.
  - Introduced a Bevy-facing `ModernVideoBackend` that streams VIC register updates to a shared overlay, drives the raster indicator, and surfaces collision status in the viewer UI (`crossdev/asm465/src/video_backend.rs`, `crossdev/asm465/src/lib.rs:1888`).
  - Added a display adapter/back-end pairing so border/background colour writes flow through the shared adapter pipeline (`crossdev/cross465/bus/src/adapters/display.rs`, `crossdev/asm465/src/cpu_worker.rs:32`).
- [x] Extend the video adapter to trigger raster IRQs via the interrupt controller when VIC compare conditions are met.
  - Added a shared `RasterIrqState` so system MMIO writes and the video adapter observe the same compare/beam values (`crossdev/cross465/bus/src/adapters/video.rs:17`, `crossdev/cross465/bus/src/system_mmio.rs:10`).
  - `VideoAdapter` now raises IRQ bit 0 when the current raster equals the programmed compare, with regression tests and bus integration coverage (`crossdev/cross465/bus/src/adapters/video.rs:246`, `crossdev/cross465/bus/src/lib.rs:690`).
  - Personalities map the new `RasterCompare` register so guests can program the compare value alongside the read-only raster counter (`crossdev/cross465/personality_defs/modern-retro-range.toml:39`, `crossdev/cross465/personality_defs/c64-compat-sparse.toml:38`).
- [x] Route display border/background colours through the adapter pathway so the modern renderer and legacy snapshots stay in sync.
- [x] Integrate controller input via adapters, including modern-retro personalities (MMIO layout + runtime wiring per `Controller_Integration.md`).
  - `CpuWorker` now auto-attaches the input adapter and exposes the backend handle so the Bevy UI can stream controller state (`crossdev/asm465/src/cpu_worker.rs:32`, `crossdev/asm465/src/cpu_worker.rs:66`).
  - The viewer feeds Bevy gamepad events into the `InputBackend`, mirroring port/paddle values for both legacy and TOML personalities (`crossdev/asm465/src/lib.rs:155`, `crossdev/asm465/src/lib.rs:1991`).
  - [x] Ensure the C64-compatible TOML personality maps CIA joystick/paddle registers through the new input adapter pipeline.
    - `modern-retro-range.toml` and `c64-compat-sparse.toml` declare the `input.joystick` module and expose PortA/PortB/POT registers so guests observe adapter-backed state (`crossdev/cross465/personality_defs/modern-retro-range.toml:33`, `crossdev/cross465/personality_defs/c64-compat-sparse.toml:25`).
- [x] Verify `modern-retro-range.toml` meets the latest v2 schema (instance arrays, fanout, transforms) and flows through the adapter-backed rendering path.
  - Extended the sprite/system ranges to expose `Enable`, `RasterLo`, and collision registers so adapter events propagate through the modern backend (`crossdev/cross465/personality_defs/modern-retro-range.toml:19`).

## Deliverables
- `crossdev/cross465/personality_defs/c64-compat-extended.toml`.
- Adapter implementations bridging the new mapping features to the rendering/input subsystems.
- Demo or automated scenario proving behavioural parity with legacy expectations.
- Documentation updates summarising the mapping DSL usage for C64 (link back to Chapter 7).

## Validation Strategy
- Register-by-register parity checks: X/Y coordinates, enable masks, colours, IRQ status, collision flags.
- Tests for latch/ack behaviour: verify `on_read` policies clear the correct bits.
- Scatter/gather accuracy: ensure `$D010` bits map to individual sprite high bits.
- Performance spot-check: decoder lookups remain O(1) despite new indirections.

## Dependencies & Open Questions
- Confirm existing value builders cover CIA joystick matrix; extend if multi-column scanning is required.
- Decide whether raster timing hooks need frame-level scheduling now or can defer to Milestone 7.
- Determine integration path for adapters within current module registry (trait objects vs. generics).
