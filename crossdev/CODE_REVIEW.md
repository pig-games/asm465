# Crossdev Code Review

## Summary
This review focuses on the crossdev workspace crates (`asm465`, `asm465-bevy`, `asm465-server`, `asm465-wasm`, and `cross465`). The findings below call out functional defects and higher-risk design issues observed during inspection.

## Major Issues

### 1. `cross465-runner` ignores the requested cycle budget
The CLI in `crossdev/cross465/runner` exposes a `--max-cycles` flag, but the main routine always runs the CPU for a hard-coded five million cycles. As a result, callers cannot shorten or extend execution, which breaks the advertised contract and wastes time when they explicitly ask for fewer cycles. 【F:crossdev/cross465/runner/src/main.rs†L7-L34】

**Suggestion:** pass `args.max_cycles` to `cpu.run_for` so the runner honors the user's request.

**Execution Plan:**
- [ ] Update `main.rs` to forward the parsed `--max-cycles` value into `cpu.run_for`.
- [ ] Add a regression test (unit or integration) that runs with a tiny cycle budget and asserts early termination.
- [ ] Verify that the runner binary still builds cleanly for all supported targets.

### 2. `Cpu::run_for` halts before executing a `BRK`
The CPU core's `run_for` loop peeks at the next opcode and exits when it is `0x00` instead of letting the instruction execute. Real 6502 hardware runs the `BRK`, pushes the return state, and vectors through `$FFFE`. Skipping the instruction means any program that expects the handler to run (or relies on the CPU side effects) will misbehave inside every host that uses `run_for` (the GUI apps and runner both do). 【F:crossdev/cross465/core6502/src/lib.rs†L816-L825】

**Suggestion:** execute the instruction first (e.g., call `step` and break if it was a `BRK`) so the observable behaviour matches the hardware and your opcode implementation.

**Execution Plan:**
- [ ] Refactor `Cpu::run_for` so it invokes `step` before checking for a `BRK` exit condition.
- [ ] Extend the CPU test suite with a scenario that executes `BRK` and asserts the correct stack/PC state.
- [ ] Smoke-test dependent crates (runner, GUI) to ensure no regressions after the control-flow change.

### 3. Browser build defaults to a loopback WebSocket URL
`asm465-bevy`'s web front-end hardcodes `ws://127.0.0.1:8800` as its default bridge address. When the app is hosted anywhere other than a local development machine—especially over HTTPS—modern browsers will block or fail that connection, leaving the console unusable unless the user manually supplies a `?ws=` query parameter. 【F:crossdev/asm465-bevy/src/web.rs†L20-L120】

**Suggestion:** derive the default endpoint from `window.location` (respecting scheme/host/port) and only fall back to loopback in explicit development builds.

**Execution Plan:**
- [ ] Introduce a helper that constructs the WebSocket URL from the active page context with HTTPS/WSS handling.
- [ ] Guard the loopback fallback behind an opt-in development flag or feature.
- [ ] Validate the behavior with both local development (loopback) and hosted (remote) deployments.

## Additional Notes
* Consider removing the unused `ConsoleMmio` import from `cross465-runner` once the cycle-budget fix is applied to silence compiler warnings. 【F:crossdev/cross465/runner/src/main.rs†L1-L34】
* The duplicate `#[cfg(feature = "native-service")]` attribute on `ServiceEnvelope` in `asm465` is harmless but could be cleaned up for clarity. 【F:crossdev/asm465/src/lib.rs†L120-L134】
