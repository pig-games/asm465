# Crossdev Code Review

## Summary
This review focuses on the crossdev workspace crates (`asm465`, `asm465-bevy`, `asm465-server`, `asm465-wasm`, and `cross465`). The findings below call out functional defects and higher-risk design issues observed during inspection.

## Major Issues

### 1. Legacy runner crate retired
The former `cross465-runner` CLI has been superseded by the asm465 tooling (which now exposes `--list-modules`/`--dump-maps` for scripted workflows) and has been removed to reduce maintenance overhead.

### 2. `Cpu::run_for` halts before executing a `BRK`
The CPU core's `run_for` loop peeks at the next opcode and exits when it is `0x00` instead of letting the instruction execute. Real 6502 hardware runs the `BRK`, pushes the return state, and vectors through `$FFFE`. Skipping the instruction means any program that expects the handler to run (or relies on the CPU side effects) will misbehave inside every host that uses `run_for` (the GUI and CLI front-ends both do). 【F:crossdev/cross465/core6502/src/lib.rs†L816-L825】

**Suggestion:** execute the instruction first (e.g., call `step` and break if it was a `BRK`) so the observable behaviour matches the hardware and your opcode implementation.

**Execution Plan:**
- [x] Refactor `Cpu::run_for` so it invokes `step` before checking for a `BRK` exit condition.
- [x] Extend the CPU test suite with a scenario that executes `BRK` and asserts the correct stack/PC state.
- [x] Smoke-test dependent crates (GUI/CLI front-ends) to ensure no regressions after the control-flow change.

### 3. Browser build defaults to a loopback WebSocket URL
`asm465-bevy`'s web front-end hardcodes `ws://127.0.0.1:8800` as its default bridge address. When the app is hosted anywhere other than a local development machine—especially over HTTPS—modern browsers will block or fail that connection, leaving the console unusable unless the user manually supplies a `?ws=` query parameter. 【F:crossdev/asm465-bevy/src/web.rs†L20-L120】

**Suggestion:** derive the default endpoint from `window.location` (respecting scheme/host/port) and only fall back to loopback in explicit development builds.

**Execution Plan:**
- [x] Introduce a helper that constructs the WebSocket URL from the active page context with HTTPS/WSS handling.
- [x] Guard the loopback fallback behind an opt-in development flag or feature.
- [x] Validate the behavior with both local development (loopback) and hosted (remote) deployments.

## Additional Notes
* The duplicate `#[cfg(feature = "native-service")]` attribute on `ServiceEnvelope` in `asm465` is harmless but could be cleaned up for clarity. 【F:crossdev/asm465/src/lib.rs†L120-L134】
