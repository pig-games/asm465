# Repository Guidelines

## Project Structure & Module Organization
- `cross465/` is the shared workspace: `bus/` provides memory-mapped devices and `core6502/` implements the CPU (tests live in `core6502/tests/`).  
- `asm465/` hosts the Bevy/wgpu UI for desktop and web; shared textures and PRG samples sit in `assets/`.  
- `asm465-server/` exposes the TCP↔WebSocket bridge for tooling.  
- `asm465-wasm/` wraps the Bevy client for browsers; `Makefile` builds into `web-dist/` (treat as build output).

## Build, Test, and Development Commands
- `cargo run --manifest-path asm465/Cargo.toml` launches the native desktop UI with the default `native-service` feature set; add `--target wasm32-unknown-unknown` to validate wasm builds.  
- `cargo run --manifest-path asm465-server/Cargo.toml -- --help` enumerates bridge server ports and flags.  
- `make -C asm465-wasm build` produces the wasm bundle and runs `wasm-bindgen`, depositing artifacts in `asm465-wasm/web-dist/`.  
- `cargo fmt --all` and `cargo clippy --all-targets --all-features` should be clean before you push.

## Coding Style & Naming Conventions
- Follow standard Rust defaults: 4-space indentation, snake_case for modules/functions, UpperCamelCase for types, and SCREAMING_SNAKE_CASE for constants.  
- Prefer helper modules in `asm465/src/` over large functions in `main.rs`.  
- Use `rustfmt` before committing; add `#[allow]` only when accompanied by a brief inline rationale.

## Testing Guidelines
- Run `cargo test --manifest-path cross465/Cargo.toml --workspace` before opening a PR; this exercises the CPU and bus crates.  
- New CPU behaviors require targeted integration tests under `core6502/tests/` (follow the existing file-per-domain pattern like `opcodes.rs`).  
- When adding async or networking code, include smoke tests or example commands for the expected JSON payload. Document manual test steps in the PR if automation is impractical.

## Commit & Pull Request Guidelines
- Provide a commit message with a title and a summary of the changes.
- Match the existing history: concise, capitalized subjects describing the change scope (e.g. “Improve wasm file picker handling”).  
- Reference issue numbers or affected crates in the body, and note any feature flags or follow-up work.  
- PRs should link to the relevant issue, list verification commands (`cargo test`, `make build`, etc.), and attach screenshots or terminal captures for UI-facing updates.
