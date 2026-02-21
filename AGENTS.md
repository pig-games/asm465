# Repository Guidelines

**Workspace context:** This repo is part of a four-repo multi-project workspace. For workspace-wide architecture, task system, and multi-repo workflows, see [Workspace AGENTS.md](../workspaces/AGENTS.md).

## Project Overview

OpFoundry is a cross-platform interactive 6502 emulator with a desktop GUI, web server, and WASM interface. It integrates opFoundryCore for CPU/bus emulation with Bevy 0.11 and egui for rich user interaction. The system consists of three primary crates:

- **opfoundry-gui:** Desktop/WASM GUI using Bevy 0.11 and egui 0.21 for sprite editing, memory inspection, and real-time CPU stepping
- **opfoundry-server:** Tokio-based TCP/WebSocket bridge connecting CLI tools to the emulator
- **opfoundry-wasm:** WASM bundle for browser-based emulation; includes web UI integration

## Build, Test, and Development Commands

- `make all` — Build opfoundry-gui and opfoundry-server
- `cargo build --manifest-path opfoundry-gui/Cargo.toml` — Build desktop GUI
- `cargo build --manifest-path opfoundry-server/Cargo.toml` — Build server
- `make -C opfoundry-wasm build` — Build WASM bundle and web artifacts
- `cargo test --manifest-path opfoundry-gui/Cargo.toml` — Test GUI crate
- `cargo test --manifest-path opfoundry-server/Cargo.toml` — Test server crate
- `cargo clippy --all-targets --all-features --manifest-path opfoundry-gui/Cargo.toml -- -D warnings` — Lint GUI
- `cargo clippy --all-targets --all-features --manifest-path opfoundry-server/Cargo.toml -- -D warnings` — Lint server
- `cargo fmt --manifest-path opfoundry-gui/Cargo.toml` — Format GUI
- `cargo fmt --manifest-path opfoundry-server/Cargo.toml` — Format server

## Project Structure

```
OpFoundry/
├── Makefile                          # Convenience targets (build/run/clean)
├── opfoundry-gui/                    # Desktop & WASM GUI (Bevy 0.11 + egui)
│   ├── src/
│   │   ├── lib.rs                    # Main UI state, emulator integration
│   │   ├── cpu_worker.rs             # Background CPU thread (native) / single-threaded (WASM)
│   │   └── ...
│   ├── Cargo.toml                    # Features: native-service, wasm-support
│   └── assets/
├── opfoundry-server/                 # TCP/WebSocket bridge (Tokio async)
│   ├── src/
│   │   ├── main.rs                   # Server entry point
│   │   ├── bridge.rs                 # TCP↔WebSocket conversion
│   │   └── ...
│   └── Cargo.toml
├── opfoundry-wasm/                   # WASM bundle & web build
│   ├── src/
│   │   └── lib.rs                    # WASM entry point
│   ├── Cargo.toml
│   ├── web-dist/                     # Build output (gitignored)
│   ├── Makefile
│   └── index.html

Note: there is no top-level Cargo workspace manifest; build each crate via its own `Cargo.toml` (or use the root `Makefile`).
```

## Architecture & Design

### opfoundry-gui

**Purpose:** Interactive 2D viewer for CPU state, sprite editing, and program execution.

**Key components:**
- **EmulatorState:** Wraps opFoundryCore CPU/bus; manages worker thread (native) or single-threaded loop (WASM)
- **CpuWorker:** Background thread that executes CPU instructions; sends state updates to UI
- **Display viewport:** Renders MMIO video output (VIC-II) with sprite borders and collision indicators

**Features:**
- Native desktop: Background worker thread and service listener (crossbeam channels; enabled via `native-service`)
- WASM: Single-threaded with main loop integration
- Dual feature flags: `native-service` (desktop builds), `wasm-support` (WASM builds)

### opfoundry-server

**Purpose:** TCP/WebSocket bridge for remote emulator control and telemetry.

**Protocol:**
- Listen on TCP newline-delimited JSON (default: 7465; overridable via CLI)
- Upgrade select connections to WebSocket for real-time updates
- JSON message format mirrors the GUI service API (request/response IDs, program load/run, memory reads, etc.)

### opfoundry-wasm

**Purpose:** Browser-based 6502 emulator; compile GUI to wasm32-unknown-unknown.

**Integration:**
- wasm-bindgen for JS interop
- HTTP server (Python SimpleHTTPServer) for local testing

## Coding Conventions

### Rust Style
- **Format:** `cargo fmt --all` (standard rustfmt)
- **Lint:** `cargo clippy --all-targets --all-features -- -D warnings`
- **Naming:** snake_case functions/variables, UpperCamelCase types/traits

### Feature Gating
- Dead-code warnings for UI-only methods/fields: Use `#[cfg_attr(not(feature = "..."), allow(dead_code))]`
- Conditional imports: `#[cfg(feature = "native-service")] use std::path::PathBuf;`

## Common Workflows

### Building for Desktop
```bash
make all
# or individually:
cargo build --manifest-path opfoundry-gui/Cargo.toml --release
cargo build --manifest-path opfoundry-server/Cargo.toml --release
```

### Building for WASM/Web
```bash
make -C opfoundry-wasm build
# Produces: opfoundry-wasm/web-dist/
# Run locally: cd opfoundry-wasm && python3 -m http.server 8080
# Visit: http://localhost:8080/
```

### Running Desktop GUI
```bash
cargo run --manifest-path opfoundry-gui/Cargo.toml --release
```

### Running Server
```bash
cargo run --manifest-path opfoundry-server/Cargo.toml -- --port 6502
```

## Validation & Quality Gates

Before submitting changes:

1. **Desktop build:** `cargo build --manifest-path opfoundry-gui/Cargo.toml --release`
2. **Server build:** `cargo build --manifest-path opfoundry-server/Cargo.toml`
3. **Format:** `cargo fmt --manifest-path <crate>/Cargo.toml`
4. **Lint:** `cargo clippy --all-targets --all-features --manifest-path <crate>/Cargo.toml -- -D warnings`
5. **Tests:** `cargo test --manifest-path <crate>/Cargo.toml`
6. **WASM build (if GUI changed):** `make -C opfoundry-wasm build`

### Fixture/reference regeneration policy
- Regenerate fixtures/references only when behavior is intentionally changed and the new output is expected by design.
- Allowed examples: deliberate protocol/response changes, intentional UI/serialization output changes, intentionally revised diagnostics.
- Never update fixtures/references only because a new failure appeared; that is a regression signal and must be fixed in code.
- Never hide regressions by redefining unexpected errors as expected outputs.

Use VS Code tasks for convenience:
- `OpFoundry: Quality Gate` — Runs all validation across GUI, Server, WASM

## Debugging & Troubleshooting

### "WASM build failed: target not installed"
**Fix:** Install wasm32 target:
```bash
rustup target add wasm32-unknown-unknown
```

### CPU Worker panics with "channel closed"
**Cause:** UI dropped emulator before worker shut down cleanly.  
**Fix:** Ensure `CpuCommand::Shutdown` is sent before drop.

### Server port already in use
**Fix:** Specify alternate port:
```bash
cargo run --manifest-path opfoundry-server/Cargo.toml -- --port 6503
```

## Integration with Other Repos

**opFoundryCore → OpFoundry:**
- GUI uses `runtime_sdk` APIs (execute programs, read memory, inspect CPU state)
- Server delegates to same APIs for remote control

**opForge → OpFoundry:**
- opForge binary produces `.prg` files
- OpFoundry loads and executes via `runtime_sdk`

## Key References

- **Workspace AGENTS:** [../workspaces/AGENTS.md](../workspaces/AGENTS.md)
- **opFoundryCore AGENTS:** [../opFoundryCore/AGENTS.md](../opFoundryCore/AGENTS.md)
- **opForge AGENTS:** [../opForge/AGENTS.md](../opForge/AGENTS.md)

---

**Last updated:** February 19, 2026
