# Crossdev Architecture Notes

High-level walkthrough of the crossdev side of the repo (Rust-based emulator/viewer and tooling) with pointers to where each piece lives in code.

## Workspace layout
- `cross465/` (Rust workspace)
  - `bus/` – pluggable 64 KB MMIO bus + device registry and personality system.
  - `core6502/` – table-driven 6502 CPU core.
  - `cross465-runner/` – host CLI/library that assembles and runs RTST-based 6502 tests on emulator or hardware targets.
  - `runtime_sdk/` – shared RTST protocol types used by both host and 6502-side macros.
- `asm465/` – Bevy/egui native+wasm viewer that embeds the bus/CPU, renders MMIO outputs, and exposes service APIs.
- `asm465-server/` – TCP↔WebSocket bridge so external tools can talk to the wasm viewer like the native UI.
- `asm465-wasm/` – thin wrapper to package the viewer for browsers.

## cross465 bus (crossdev/cross465/bus)
- Core entry: `bus::Bus` (`bus/src/lib.rs`). Owns `Memory` (flat `[u8; 65536]`) and a set of mapped MMIO devices. `read`/`write` routes to RAM unless the address falls inside a mapped MMIO range.
- MMIO modules: console (`console_mmio.rs`), display (`display_mmio.rs`), sprite (`sprite_mmio.rs`), input (`input_mmio.rs`), and system/interrupt controller (`system_mmio.rs`). Each module implements `MmioDevice` plus `Module` (from `mmio.rs`) so it can be addressed via register IDs. Snapshots (`ConsoleOutput`, `DisplayOutput`, `SpriteOutput`, `InputOutput`) are held behind `Arc<Mutex<...>>` for UI polling.
- Personalities (memory maps):
  - Legacy descriptors live in `personality.rs`.
  - V2 TOML-driven personalities (`personality_v2.rs`) compile to `AddressMapping` tables with priority, scatter/fanout, mirrors, open-bus policies, computed fields, and per-field hooks. Built-ins: `personality_defs/modern-retro-range.toml` and `c64-compat-sparse.toml`.
  - `builtin_module_registry` wires known module factories so TOML can pick implementations by ID.
- Adapters (`adapters/*.rs`) bind MMIO modules to host-facing backends: display/sprite to renderer states, input to host controller feed, system to video/raster IRQ bridge. `Bus::attach_adapter` installs these when a v2 personality exposes the module.
- Interrupts: `interrupts.rs` models IRQ/NMI masks/pending bits and exposes the controller shared with adapters and UI.
- Tests in `bus/src/lib.rs` ensure TOML personalities compile as expected (mirrors, scatter bits, value builders, raster IRQ compute paths, etc.), and parity with legacy maps.

## 6502 core (crossdev/cross465/core6502)
- `core6502::Cpu` (`core6502/src/lib.rs`) is a table-driven implementation of the official 151 opcodes, including decimal-mode behaviour and the JMP(indirect) page-wrap bug. `step` executes one opcode; `run_for` runs until a cycle budget or `BRK`. Interrupts are queued via `pending_irq/nmi` flags and serviced before executing the next opcode when enabled.
- Uses the bus trait object for all memory I/O, so MMIO-visible side effects (sprites, display colours, IRQ controller) naturally occur through `Bus::read/write`.
- Tests under `core6502/tests/` validate instruction semantics, BCD rules, opcode coverage, console MMIO behaviour, and integration with the bus layout.

## Runtime SDK (crossdev/cross465/runtime_sdk)
- `rtst.rs` defines the Runtime Test Stream protocol shared with 6502 test macros (`native/src/include/test_rtst.h`). `Header` encodes stream state, counts, and write cursor; records follow in the payload area.
- Constants define canonical layouts for Cross465, C64, Ultimate64, and MEGA65 targets (`BASE_LAYOUT_*`, sizes). Parsing/encoding helpers keep host tools and guest code in sync on byte layout and versioning.

## cross465-runner (crossdev/cross465/cross465-runner)
- CLI entry `cross465-test-runner` (`src/bin/cross465-test-runner.rs`) wraps library functions to list or run RTST test cases.
- Library pieces:
  - `catalog.rs` parses `tests/catalog.toml` plus optional workspace overrides and CI matrix definitions.
  - `assembler.rs` wraps 64tass invocation and includes extra include paths/defines.
  - `executor.rs` orchestrates target selection (emulator via native asm465, wasm bridge, Ultimate64 hardware), connects transports, loads binaries, and streams RTST records.
  - `report.rs`/`artifacts.rs`/`fixtures.rs` handle reporting, optional fixture updates, and saving artifacts under `target/cross465-runner/`.
  - `expect.rs` encodes expectations for display/console/logging assertions referenced by tests.
- Test cases live in `crossdev/cross465/tests/cases/*.s` (assembly programs) and are referenced by `tests/catalog.toml`. The runner assembles them, executes via the selected target, and consumes the RTST stream to produce pass/fail summaries.

## asm465 viewer (crossdev/asm465)
- Bevy/egui app that embeds the CPU+bus to present a modern UI for retro programs.
  - Entry (`src/main.rs`) enables only when compiled with `native-service` feature.
  - `lib.rs` wires Bevy setup: loads personalities (`--personality` flag with built-ins matching `personality_defs`), builds the virtual display, console view, controller inspector, and developer tools (interrupt and input snapshots). Accepts various MMIO/viewer tuning flags (virtual size, border colours, sprite mapping limits, raster overlay).
  - `cpu_worker.rs` runs the 6502 core on a background thread (native) or synchronously (wasm). It builds a `Bus` with the chosen personality, attaches adapters (`attach_default_adapters`) to expose display/sprite/input/system state, and executes batches with throttling while sharing MMIO snapshots back to the UI.
  - `video_backend.rs` converts sprite/display/raster snapshots into Bevy render state and optional raster overlay signals.
  - `web.rs` (wasm) exposes `start_web_app` and websocket bridge management.
- Assets (fonts/sprites) live under `asm465/assets/`. Docs in `asm465/docs/` cover graphics pipeline, interrupt model, and improvement plans.

## asm465-server (crossdev/asm465-server)
- Tokio-based bridge that accepts newline-delimited JSON commands over TCP and fans them out to all connected WebSocket clients (`src/main.rs`). Tracks pending responses (by ID or FIFO) so replies from the wasm viewer can be routed back to the original TCP sender. Mirrors the service API the native viewer exposes locally.

## asm465-wasm (crossdev/asm465-wasm)
- Minimal wrapper that builds the Bevy viewer for browsers (see `Makefile` and `src/lib.rs`). Uses `wasm-bindgen` to expose entry points and packages output into `web-dist/`.

## Test/fixture docs
- `docs/` at repo root and `crossdev/cross465/docs/` detail the test runner, personality specs, SDK module scaffolds, and controller/display mapping plans. `docs/Authoring_6502_Tests_HowTo.md` is a good entry for adding RTST tests.
- Native assembly platform variants live under `native/src/platform/*` but share RTST macros with crossdev via `include/test_rtst.h`.

## Data flow summary
1. The viewer (`asm465`) starts and builds a `Bus` with a selected personality; `cpu_worker` attaches adapters so MMIO outputs feed UI state and host inputs drive MMIO registers.
2. `core6502::Cpu` executes against the bus; MMIO writes update snapshots and IRQ controller state that the UI polls/renderers consume. Raster IRQ adapter reads compare registers and asserts IRQs when host video state hits the programmed line.
3. For tests, `cross465-runner` assembles cases, loads them into the selected target (native/wasm/hardware), and watches RTST buffers using `runtime_sdk` types to produce reports/fixtures.
4. When using the wasm viewer, `asm465-server` lets host tooling talk to the browser via WebSocket while reusing the same service protocol the native viewer speaks locally.
