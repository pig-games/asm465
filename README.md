# opFoundry

A cross-development GUI IDE for 65xx-based retro computers, featuring a built-in 6502 emulator, WebSocket bridge for tooling integration, and WASM support for browser-based development.

## Components

- **opfoundry-gui** — Bevy/egui desktop application with integrated 6502 emulator
- **opfoundry-server** — TCP/WebSocket bridge server for external tooling
- **opfoundry-wasm** — WASM build for browser-based development

## Requirements

- Rust (stable)
- [opFoundryCore](https://github.com/pig-games/opFoundryCore) — 6502 emulator core (path: `../opFoundryCore`)

## Building

```bash
# Build the desktop GUI
cargo build --manifest-path opfoundry-gui/Cargo.toml

# Build the bridge server
cargo build --manifest-path opfoundry-server/Cargo.toml

# Build the WASM version
make -C opfoundry-wasm build
```

## Running

```bash
# Desktop GUI with service API
cargo run --manifest-path opfoundry-gui/Cargo.toml -- --service-port 7465

# Bridge server (for WASM clients)
cargo run --manifest-path opfoundry-server/Cargo.toml -- --tcp-port 7465 --ws-port 8800
```

## Related Projects

- [opFoundryCore](https://github.com/pig-games/opFoundryCore) — Runtime SDK, emulator cores, personality system, execution infrastructure.
- [opForge465](https://github.com/pig-games/opForge465) — Native 6502 assembler/editor

## License

See [LICENSE](LICENSE) for details.
