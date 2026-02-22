# OpFoundry Code Review

**Date:** February 22, 2026  
**Scope:** Full review of `opfoundry-gui`, `opfoundry-server`, `opfoundry-wasm`, and build configuration  
**Codebase size:** ~5,564 lines of Rust across 12 source files

---

## Summary

| Severity     | Count | Key Items |
|--------------|-------|-----------|
| **High**     | 6     | God module, code duplication, hardcoded path, panic on init, no tests, server binds `0.0.0.0` |
| **Medium**   | 10    | Mega-function, cfg proliferation, unused dep, dead expression, rfd not gated, server unwraps, RTST polling, Makefile gaps, server–GUI coupling, no auth |
| **Low**      | 9     | Dead code ×2, naming inconsistency, mutable mem for read, raster cap, redundant guard, font clone overhead, key tracking allocs, deprecated crate |
| **Info**     | 4     | NonSend rationale, clean platform split, no workspace manifest, outdated Bevy |

### Top Recommendations (ordered by impact)

1. Split `lib.rs` into focused modules (`input.rs`, `interrupts.rs`, `sprite.rs`, `viewport.rs`, `ui.rs`)
2. Extract service API types into a standalone `opfoundry-api` crate to decouple server from GUI
3. Default server binding to `127.0.0.1` instead of `0.0.0.0`
4. Add tests for `service.rs`, `program_runner.rs`, `cpu_worker.rs`, and the server
5. Remove unused `uuid` dependency
6. Fix the hardcoded `cross465/personality_defs` path in `personality_cli.rs`
7. De-duplicate `initialize_cpu` / `initialize_bus` and the two `run_program` implementations
8. Add `fmt` / `clippy` / `test` / `audit` targets to the Makefile

---

## A. Architecture & Design

### A1. God Module: `lib.rs` — HIGH

`opfoundry-gui/src/lib.rs` is **2,675 lines** and contains:

- CLI argument parsing (`Args`)
- Bevy app setup (`run_app`, `run_native`)
- All ECS resources (`EmulatorState`, `ControllerState`, `InterruptBindings`, `RasterDriver`, `KeyboardTracker`, `SpriteVirtualResolution`, `DisplayPalette`, `VideoOverlayConfig`, `TimerInterruptState`, `UiState`)
- All Bevy systems (`ui_system`, `setup_scene`, `update_sprite_viewport`, `controller_input_system`, `emit_frame_start_interrupt`, `timer_interrupt_system`, `drive_raster_counter`, `keyboard_interrupt_system`, `gamepad_interrupt_system`, `update_video_overlay_line`, `sync_controller_backend`, `update_keyboard_tracker`, `emit_frame_end_interrupt`)
- Sprite coordinate math
- Controller/gamepad mapping
- Viewport geometry
- All egui rendering

**Suggestion:** Split into focused modules — `input.rs`, `interrupts.rs`, `sprite.rs`, `viewport.rs`, `ui.rs` for readability and maintainability.

### A2. `ui_system` is a Mega-Function — MEDIUM

The `ui_system` function (~300 lines) takes 14+ parameters, handles service commands for both native and WASM, processes file picks, renders the toolbar, console, interrupts panel, and input panel all in a single function.

**Suggestion:** Split into sub-systems or helper functions per panel/tab.

### A3. `cfg` Attribute Proliferation — MEDIUM

Over 40 `#[cfg(...)]` gates and 13 `#[cfg_attr(...)]` blocks across the GUI crate, spanning six different contexts: `native-service`, `native-file-dialog`, `target_arch = "wasm32"`, `dev-loopback`, and their combinations.

The most egregious is the repeated 5-line dead-code suppression block:
```rust
#[cfg_attr(
    not(any(
        feature = "native-service",
        feature = "native-file-dialog",
        target_arch = "wasm32"
    )),
    allow(dead_code)
)]
```
This appears verbatim **~10 times** across `cpu_worker.rs` and `lib.rs`.

**Suggestion:** Define a shared `cfg` macro or consolidate the feature gating at the module/crate level with a crate-level attribute.

### A4. `EmulatorState` as NonSend — INFO

`EmulatorState` is inserted as `NonSendResource` because the native `CpuWorker` holds an `mpsc::Sender` (not `Send` across Bevy's schedule). This is correct for the design but limits ECS ergonomics — you can't use it in parallel systems. Worth documenting why it's NonSend.

### A5. Good Native/WASM Split in `cpu_worker.rs` — INFO

The conditional compilation approach using `mod native` / `mod wasm` with a common `CpuWorker` re-export at the bottom of the module is clean and well-structured.

---

## B. Code Quality

### B1. Code Duplication: `initialize_cpu` vs `initialize_bus` — HIGH

`initialize_cpu` (native path, `cpu_worker.rs` ~L466) and `initialize_bus` (wasm path, `cpu_worker.rs` ~L754) are nearly identical (~40 lines each), differing only in that native wraps the bus in a `Cpu`. Similarly, `CpuWorker::run_program` is duplicated between native and wasm with near-identical logic.

**Suggestion:** Extract a shared `prepare_bus_and_run()` helper, then have each platform wrap the result.

### B2. Unused `uuid` Dependency — MEDIUM

`opfoundry-gui/Cargo.toml` declares:
```toml
uuid = { version = "1.8", features = ["js"] }
```

There is **zero** `uuid` usage anywhere in the GUI source files. This adds unnecessary compile time and WASM bloat (especially with `js` feature).

**Action:** Remove from `Cargo.toml`.

### B3. `prev_ref` is Computed but Discarded — MEDIUM

In `lib.rs` ~L2049:
```rust
prev_ref.and_then(|prev| prev.pads.get(pad_index));
```
This expression produces an `Option` that is immediately discarded. It appears to be leftover from a delta-rendering approach that was never completed.

**Action:** Either use `prev_ref` for delta rendering or remove the dead expression.

### B4. `button_list` is Dead Code — LOW

`button_list` (in `lib.rs` ~L1399) is annotated `#[allow(dead_code)]` and never called.

**Action:** Remove or use it.

### B5. `CONTROLLER_BUTTON_ORDER` is Dead Code — LOW

`CONTROLLER_BUTTON_ORDER` (in `lib.rs` ~L935) is annotated `#[allow(dead_code)]`. It's only used by the dead `button_list` function.

**Action:** Remove or use it.

### B6. Hardcoded Path to Personality TOML Files — HIGH

In `personality_cli.rs` (line 17):
```rust
let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../cross465/personality_defs");
```

This resolves relative to the **build-time** source tree. But `../cross465/personality_defs` does not exist — the actual workspace path is `../../opFoundryCore/personality_defs`. This silently breaks personality TOML loading for `modern-retro-range` and `c64-compat-sparse`.

**Fix:** Update to the correct relative path (`../../opFoundryCore/personality_defs`) or make it configurable via environment variable.

### B7. `rfd` Not Behind Feature Gate — MEDIUM

`Cargo.toml` declares `rfd = "0.14"` as an unconditional dependency. On native builds without `native-file-dialog`, `rfd` is still compiled but unused. On WASM, `rfd::AsyncFileDialog` is used for file picking.

**Suggestion:** Make `rfd` optional, gated on `native-file-dialog` and/or the wasm target.

### B8. Inconsistent Naming: "asm465" vs "opFoundry" — LOW

Module doc comments and UI strings still reference the old "asm465" name:

- `lib.rs` L1: `"Bevy/egui front-end for the asm465 cross-development tooling"`
- `lib.rs` L85: `WELCOME_MESSAGE = "Welcome to the asm465 console viewer!"`
- `lib.rs` L541: `title: "asm465 Console"`
- `cpu_worker.rs` L1: `"Dedicated CPU runner for the asm465 viewer."`

**Action:** Update these to "opFoundry" for branding consistency.

---

## C. Correctness & Robustness

### C1. `EmulatorState::new` Panics on Worker Failure — HIGH

In `lib.rs` ~L630:
```rust
CpuWorker::spawn(personality, startup)
    .unwrap_or_else(|err| panic!("Failed to start CPU worker: {err}"));
```

If personality TOML is malformed or the file is missing, the entire app panics with no recovery.

**Suggestion:** Propagate the error from `run_native()` or fall back to a default personality with an error status message.

### C2. `CpuWorkerOutputs::new` Uses `.expect()` — MEDIUM

In `cpu_worker.rs` lines 134–142:
```rust
let console = bus.console_output_handle().expect("console MMIO output handle");
let display = bus.display_output_handle().expect("display MMIO output handle");
let sprite = bus.sprite_output_handle().expect("sprite MMIO output handle");
```

If a personality doesn't map these modules, the worker panics. These should be `Option` fields with graceful degradation in the UI.

### C3. Server Uses `.unwrap()` on All Mutex Locks — MEDIUM

In `opfoundry-server/src/main.rs` lines 66–98: five `lock().unwrap()` calls. If any lock is poisoned (e.g., a panic in a websocket task), the server crashes.

**Suggestion:** Use `.lock().unwrap_or_else(|e| e.into_inner())` or propagate the error.

### C4. `snapshot_memory` Uses `mem_mut()` for Read-Only Access — LOW

In `cpu_worker.rs` ~L461 and ~L700:
```rust
let mem = self.cpu.bus().mem_mut();
```

This gets a mutable reference to memory just to read from it. If the `bus` API offers `mem()` (immutable), prefer that.

### C5. `run_until_rtst_done` Polling Uses Modulo — MEDIUM

In `program_runner.rs` ~L107: The RTST polling loop calls `cpu.step()` once per iteration, then checks `cycles % poll_interval`. Because `step()` increments `cycles` by a variable amount (typically 2–7), the modulo check may miss poll intervals.

**Suggestion:** Use threshold counting (`cycles >= next_poll`) instead of modulo.

### C6. Raster Overlay Hardcodes 255-Line Max — LOW

In `lib.rs` ~L2271:
```rust
let raster = snapshot.raster.min(255) as f32;
let y_virtual = (raster / 255.0) * virtual_height;
```

The raster counter is a `u16` that can go beyond 255. Capping at 255 and dividing by 255.0 means the overlay doesn't scale correctly for resolutions > 255 lines.

**Fix:** Use the virtual height as the divisor.

### C7. Redundant Guard: `.pads.len().min(2) > 0` — LOW

In `lib.rs` ~L2050:
```rust
if snapshot.pads.len().min(2) > 0 {
```

This condition is already guarded by an `if snapshot.pads.is_empty()` check above. If `pads` is non-empty, `.len().min(2)` is always ≥ 1 — the guard is a no-op.

---

## D. Performance

### D1. `VideoState` Cloned Under Lock Every Frame — MEDIUM

In `lib.rs` ~L2001–2006:
```rust
if let Some(video_state) = emulator.video_state().lock().ok()
    .map(|guard| guard.clone())
```

`video_state()` returns an `Arc<Mutex<VideoState>>` clone, then `.lock()` acquires and `.clone()` copies the entire state. This happens every frame in the Interrupts tab.

**Suggestion:** Only clone when the tab is active, and/or consider a snapshot approach that doesn't need a full clone.

### D2. `font_id.clone()` Per Character in Console Rendering — LOW

In `console_ui.rs` ~L47–54: `font_id.clone()` is called for every character cell. `FontId` contains an owned `String` for the font family. For a 40×25 console, that's **1,000+ string clones per frame**.

**Suggestion:** Pre-allocate the `TextFormat` outside the inner loop, or use a reference.

### D3. `sync_controller_backend` Clones Arcs Every Frame — LOW

In `lib.rs` ~L1259–1262: `emulator.input_backend()` and `emulator.input_snapshot()` both clone `Arc` pointers every frame. The `sync_backend` method checks pointer equality before doing anything, so the overhead is small but avoidable.

### D4. `KeyboardTracker` Allocates Strings Every Frame — LOW

In `lib.rs` ~L1161–1186: `update_from_input` maps every pressed key to `format!("{key:?}")` every frame, creates a sorted `Vec<String>`, compares, and extends `previous`.

**Suggestion:** Use `KeyCode` directly (it's `Copy` + `Eq`) and stringify only when rendering.

---

## E. Maintainability

### E1. No Unit Tests for Most Modules — HIGH

Test coverage is thin:

| File | Lines | Tests |
|------|-------|-------|
| `cpu_worker.rs` | 831 | 0 |
| `service_listener.rs` | 123 | 0 |
| `service.rs` | 244 | 0 |
| `program_runner.rs` | 173 | 0 |
| `personality_cli.rs` | 387 | 0 |
| `opfoundry-server/src/main.rs` | 340 | 0 |
| `web.rs` | 572 | 4 (wasm-only, never run in `cargo test`) |
| `lib.rs` | 2,675 | ~10 (sprite math, viewport, palette, interrupts) |

Key untested areas:
- `ServiceRequestPayload::into_command` with all its parsing
- `parse_rtst_config` with nested optional fields
- `run_program` / `run_until_rtst_done` control flow
- Server message routing and bridging logic

### E2. No `clippy`, `fmt`, or `test` Targets in Makefile — MEDIUM

The Makefile only has `build`, `clean`, `run`, and `run-server`. The AGENTS.md lists `fmt`, `clippy`, `test`, and `audit` as validation requirements, but the Makefile doesn't provide corresponding targets.

**Action:** Add `fmt`, `clippy`, `test`, and `audit` Makefile targets for all three crates.

### E3. No Top-Level Cargo Workspace — INFO

Each crate has its own `Cargo.toml` with no workspace root. This is documented but means `cargo test --workspace` doesn't work. Each crate must be built and tested individually, increasing maintenance burden.

### E4. `web.rs` Tests Gated on `wasm32` — MEDIUM

In `web.rs` ~L541:
```rust
#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
```

The `derive_ws_url` tests are pure string manipulation and don't require WASM. They should be gated on `#[cfg(test)]` only so they run in normal `cargo test`.

---

## F. Dependencies & Build

### F1. Server Depends on Full `opfoundry-gui` for Type Sharing — MEDIUM

`opfoundry-server/Cargo.toml`:
```toml
opfoundry-gui = { path = "../opfoundry-gui", default-features = false }
```

The server only uses `ServiceRequestPayload`, `ServiceResponseMessage`, and `ServiceStatus` from `opfoundry-gui`. Pulling in the entire GUI crate (with Bevy, egui, etc. in the transitive dependency graph) is heavy.

**Suggestion:** Extract the service API types into a standalone `opfoundry-api` crate.

### F2. Bevy 0.11 and bevy_egui 0.21 Are Outdated — INFO

These were current in 2023. Bevy is at 0.15+ and bevy_egui at 0.34+. Upgrading would bring better WASM support, performance improvements, and API ergonomics. This is a large effort and may not be immediately actionable.

### F3. `instant` Crate is Deprecated — LOW

`program_runner.rs` and `web.rs` use `instant::Instant`. The `instant` crate is deprecated in favor of `web-time` (or `std::time::Instant`, which works on WASM in newer Rust toolchains).

---

## G. Security

### G1. Server Binds to `0.0.0.0` by Default — HIGH

In `opfoundry-server/src/main.rs` lines 34 and 40:
```rust
#[arg(long, default_value = "0.0.0.0")]
tcp_host: String,
#[arg(long, default_value = "0.0.0.0")]
ws_host: String,
```

Both listeners default to **all interfaces**. Anyone on the network can send commands to load and execute arbitrary PRG binaries or read emulator memory.

**Fix:** Default to `127.0.0.1`.

### G2. No Authentication or Authorization — MEDIUM

Neither the native service listener (`service_listener.rs`) nor the bridge server (`opfoundry-server`) has any authentication. Any TCP/WebSocket client can issue `run_prg`, `run_prg_data`, or `read_mem` commands.

Acceptable for a local dev tool, but should be documented as a security boundary. If the server is ever exposed to untrusted networks (see G1), this becomes critical.

### G3. `read_mem` Exposes Full Address Space — LOW

The `ReadMem` command validates length (1–65536) but allows reading any address in the emulated 64KB space. Low risk for an emulator, but worth noting.

### G4. `run_prg` Accepts Arbitrary File Paths — MEDIUM

The `RunPrg` command accepts any filesystem path. A remote client on the network (see G1) could probe the filesystem by attempting path traversal. Combined with the `0.0.0.0` default, this is exploitable on shared networks.

**Mitigation:** Restrict to a configured working directory or require explicit opt-in for remote file loading.

---

## File-by-File Notes

### `opfoundry-gui/src/lib.rs` (2,675 lines)
- Well-documented public API and Bevy system registration
- Good use of Bevy resources and ECS patterns
- However, too many responsibilities in one file (see A1)
- Test module covers sprite math and viewport geometry well

### `opfoundry-gui/src/cpu_worker.rs` (831 lines)
- Clean native/wasm platform split
- Good error propagation from worker thread
- Significant duplication between native and wasm paths (see B1)

### `opfoundry-gui/src/web.rs` (572 lines)
- Solid WebSocket bridge implementation for WASM
- File picker integration via `rfd::AsyncFileDialog` is clean
- Tests are wasm-gated unnecessarily (see E4)

### `opfoundry-gui/src/service.rs` (244 lines)
- Clear command/response protocol design
- Good use of serde for JSON serialization
- Needs unit tests for parsing edge cases (see E1)

### `opfoundry-gui/src/program_runner.rs` (173 lines)
- RTST monitor integration is well-designed
- Polling logic could be improved (see C5)

### `opfoundry-gui/src/personality_cli.rs` (387 lines)
- Broken path resolution (see B6)
- Otherwise well-structured diagnostic output

### `opfoundry-gui/src/video_backend.rs` (125 lines)
- Clean abstraction for video overlay signals
- Thread-safe via atomic operations

### `opfoundry-gui/src/service_listener.rs` (123 lines)
- Simple and correct TCP listener implementation
- No tests

### `opfoundry-gui/src/console_ui.rs` (69 lines)
- Compact MMIO-to-egui color mapping
- Per-character `font_id.clone()` is avoidable (see D2)

### `opfoundry-gui/src/main.rs` (11 lines)
- Minimal and correct

### `opfoundry-server/src/main.rs` (340 lines)
- Good async architecture with Tokio
- Proper use of channels for TCP↔WS bridging
- Security concerns with default binding (see G1)
- All mutex locks use `.unwrap()` (see C3)

### `opfoundry-wasm/src/lib.rs` (15 lines)
- Minimal and correct wasm-bindgen entry point

### `Makefile`
- Clean target structure but missing validation targets (see E2)

### `Cargo.toml` files
- Unused `uuid` dependency (see B2)
- Server depends on full GUI crate for 3 types (see F1)
- Deprecated `instant` dependency (see F3)

---

## Fix Plan

The fixes are grouped into six phases, ordered by risk (lowest-risk / highest-value first) so that each phase lands as a clean, independently testable commit. Phases 1–3 are quick wins; phases 4–6 are structural refactors.

### Execution Status (February 22, 2026)

Implementation sweep complete with incremental commits and validation after each slice.

| Phase | Status | Notes |
|-------|--------|-------|
| 1 — Safety & Hygiene | ✅ Completed | Server bind/default safety, naming/cleanup, deprecated time crate migration completed. |
| 2 — Build & Validation Infrastructure | ✅ Completed | Make/quality-gate workflow and baseline checks completed. |
| 3 — Correctness Fixes | ✅ Completed | Panic/lock robustness, RTST poll behavior, memory snapshot access, and raster scaling addressed. |
| 4 — Dependency Decoupling | ✅ Completed | `opfoundry-api` extraction, optional dependency gating, and server path restriction support landed. |
| 5 — Test Coverage & Worker Dedup | ✅ Completed | Added targeted GUI/server tests and worker dedup helpers. |
| 6 — Module Decomposition | ✅ Completed | `lib.rs` responsibilities extracted into focused modules (`input`, `interrupts`, `display`, `ui`, `emulator_state`). |

### Phase 1 — Immediate Safety & Hygiene (1–2 hours)

Small, mechanical changes with no behavioral risk. Ship as one commit.

| # | Finding | Action | Files |
|---|---------|--------|-------|
| 1 | G1 | Change server default bind from `0.0.0.0` to `127.0.0.1` | `opfoundry-server/src/main.rs` |
| 2 | B2 | Remove unused `uuid` dependency | `opfoundry-gui/Cargo.toml` |
| 3 | B8 | Rename "asm465" → "opFoundry" in doc comments, `WELCOME_MESSAGE`, window title | `lib.rs`, `cpu_worker.rs` |
| 4 | B4, B5 | Remove dead `button_list` function and `CONTROLLER_BUTTON_ORDER` constant | `lib.rs` |
| 5 | B3 | Remove or use the discarded `prev_ref` expression | `lib.rs` |
| 6 | C7 | Remove redundant `.pads.len().min(2) > 0` guard | `lib.rs` |
| 7 | F3 | Replace deprecated `instant` crate with `web-time` | `Cargo.toml`, `program_runner.rs`, `web.rs` |

### Phase 2 — Build & Validation Infrastructure (1 hour)

Add missing Makefile targets so the quality gate is runnable. No code changes.

| # | Finding | Action | Files |
|---|---------|--------|-------|
| 1 | E2 | Add `fmt`, `clippy`, `test`, `audit` targets to `Makefile` covering all three crates | `Makefile` |
| 2 | E4 | Un-gate `web.rs` `derive_ws_url` tests so they run in normal `cargo test` | `web.rs` |
| 3 | — | Run `cargo fmt`, `cargo clippy`, `cargo test` across all crates and fix any new warnings | all |

### Phase 3 — Correctness Fixes (1–2 hours)

Targeted bug fixes and robustness improvements. Each can be a separate commit.

| # | Finding | Action | Files |
|---|---------|--------|-------|
| 1 | B6 | Fix hardcoded `../cross465/personality_defs` → `../../opFoundryCore/personality_defs` (or make configurable) | `personality_cli.rs` |
| 2 | C1 | Replace `panic!` in `EmulatorState::new` with error propagation / graceful fallback | `lib.rs` |
| 3 | C2 | Make `CpuWorkerOutputs` console/display/sprite handles `Option`, degrade gracefully | `cpu_worker.rs`, `lib.rs` |
| 4 | C3 | Replace `lock().unwrap()` with `unwrap_or_else(\|e\| e.into_inner())` in server | `opfoundry-server/src/main.rs` |
| 5 | C5 | Replace modulo-based RTST poll with threshold counter | `program_runner.rs` |
| 6 | C6 | Fix raster overlay to scale by virtual height instead of hardcoded 255 | `lib.rs` |
| 7 | C4 | Use `mem()` instead of `mem_mut()` for read-only memory snapshots | `cpu_worker.rs` |

### Phase 4 — Dependency Decoupling (2–3 hours)

Structural change to the crate graph. Needs careful testing across native, server, and WASM builds.

| # | Finding | Action | Files |
|---|---------|--------|-------|
| 1 | F1 | Extract `ServiceRequestPayload`, `ServiceResponseMessage`, `ServiceStatus`, `ServiceCommand`, and related types into a new `opfoundry-api` crate | new `opfoundry-api/`, `opfoundry-gui/Cargo.toml`, `opfoundry-server/Cargo.toml` |
| 2 | B7 | Make `rfd` optional, gated behind `native-file-dialog` feature | `opfoundry-gui/Cargo.toml` |
| 3 | G4 | Add optional `--allowed-dir` flag to restrict `run_prg` paths; document security boundary (G2) | `service.rs`, `opfoundry-server/src/main.rs`, `README.md` |

### Phase 5 — Test Coverage & Worker Dedup (3–4 hours)

Fill the testing gap **before** the module decomposition in Phase 6 so that the refactor has a proper regression safety net. The existing tests only cover sprite math, viewport geometry, palette, and interrupts — leaving controller input, keyboard tracking, service parsing, and program runner untested.

| # | Finding | Action | Files |
|---|---------|--------|-------|
| 1 | E1 | Add unit tests for `ServiceRequestPayload::into_command` and `parse_rtst_config` | `service.rs` |
| 2 | E1 | Add unit tests for `program_runner` control flow (mocking CPU step) | `program_runner.rs` |
| 3 | E1 | Add integration tests for server TCP→WS bridging | `opfoundry-server/` |
| 4 | E1 | Add unit tests for `parse_color`, `ControllerState::sync_backend`, `KeyboardTracker::update_from_input` | `lib.rs` |
| 5 | E1 | Add edge-case tests for `compute_viewport_geometry` (zero-size window, extreme aspect ratios) | `lib.rs` |
| 6 | B1 | Extract shared `prepare_bus()` / `apply_program()` helper used by both native and wasm `CpuWorker` paths | `cpu_worker.rs` |
| 7 | D2 | Pre-allocate `TextFormat` outside inner loop in console rendering | `console_ui.rs` |
| 8 | D4 | Use `KeyCode` directly in `KeyboardTracker` instead of `format!("{key:?}")` | `lib.rs` |

### Phase 6 — Module Decomposition of `lib.rs` (3–4 hours)

Break the god module into focused files. Pure refactor — no behavioral changes. The tests added in Phase 5 provide the regression safety net for this move.

| # | Finding | Target module | Responsibilities moved |
|---|---------|---------------|----------------------|
| 1 | A1 | `input.rs` | `ControllerState`, `PadAssignment`, `KeyboardTracker`, `sync_controller_backend`, `controller_input_system`, `update_keyboard_tracker`, gamepad mapping helpers |
| 2 | A1 | `interrupts.rs` | `InterruptBindings`, `TimerInterruptState`, `emit_frame_start_interrupt`, `emit_frame_end_interrupt`, `timer_interrupt_system`, `keyboard_interrupt_system`, `gamepad_interrupt_system` |
| 3 | A1 | `sprite.rs` | `SpriteVirtualResolution`, sprite coordinate math, `sprite_world_transform`, `update_sprite_viewport`, `setup_scene` (sprite portion) |
| 4 | A1 | `viewport.rs` | `DisplaySettings`, `DisplayPalette`, `VideoOverlayConfig`, `RasterDriver`, `drive_raster_counter`, `update_video_overlay_line`, aspect-ratio geometry |
| 5 | A1, A2 | `ui.rs` | `UiState`, `ui_system` (split into per-panel helpers: toolbar, console, interrupts, input) |
| 6 | A3 | — | Consolidate repeated `cfg_attr` blocks into a crate-level `allow` or a shared macro |

Suggested order: extract `input.rs` first (fewest cross-dependencies), then `interrupts.rs`, `sprite.rs`, `viewport.rs`, and finally `ui.rs`. Run the full test suite after each extraction.

### Phase Summary

| Phase | Effort | Risk | Commit(s) |
|-------|--------|------|-----------|
| 1 — Safety & Hygiene | 1–2 h | Minimal | 1 |
| 2 — Build Infrastructure | 1 h | None | 1 |
| 3 — Correctness Fixes | 1–2 h | Low | 2–3 |
| 4 — Dependency Decoupling | 2–3 h | Medium | 2–3 |
| 5 — Tests & Dedup | 3–4 h | Low | 3–4 |
| 6 — Module Decomposition | 3–4 h | Medium (refactor, safety net from Phase 5) | 3–5 |
| **Total** | **~12–16 h** | | **~12–17 commits** |

### Items Deferred (Not In Plan)

| Finding | Reason |
|---------|--------|
| F2 (Bevy 0.11 → 0.15+) | Major migration; plan as a separate project with its own review |
| E3 (No workspace manifest) | Low-impact; optional quality-of-life improvement |
| G3 (`read_mem` full address space) | Acceptable for emulator use case; no action needed |
