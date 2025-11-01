
# Cross465 Runtime SDK — Packager Tool Specification (02)
[← 01: Module SDK](01_Cross465_Runtime_SDK_Module_SDK_Spec.md) | [→ 03: Personality Codegen](03_Cross465_Runtime_SDK_Personality_Codegen_Spec.md)

**Status:** Draft • **Audience:** Runtime builders • **Targets:** Desktop & WASM

---

## 1. Purpose
`cross465-pack` transforms a development setup (TOML personalities + modules + assets) into a **self-contained runtime**:
1) Parse & validate **personality.toml**.  
2) Generate **macro-embedded Rust** (`personality_gen.rs`).  
3) Select & **statically link modules** (Display/Audio/Input/...).  
4) Optionally **embed assets**.  
5) Build a **single binary** (desktop) or a **single .wasm bundle** (web).

---

## 2. CLI Reference

### 2.1 Basic Syntax
```
cross465-pack --personality <file> [options]
```

### 2.2 Options
| Flag | Description |
|------|-------------|
| `--personality <file>` | Path to TOML personality. |
| `--embed-personality` | Embed via codegen (default `true`). |
| `--modules <k=v,...>` | Slot bindings, e.g. `display=RetroModern2D,audio=BasicAudio`. |
| `--assets <dir>` | Directory to embed (optional). |
| `--manifest <file>` | Use a packager manifest (see §3). |
| `--out <path>` | Output folder/file (default `build/runtime`). |
| `--wasm` | Build for WebAssembly (forces static & embed). |
| `--release` | Build optimized binary. |
| `--debug` | Include verbose logs/dev flags. |

### 2.3 Examples
**Desktop**
```
cross465-pack   --personality personalities/c64_modern.toml   --modules display=RetroModern2D,audio=BasicAudio,input=GamepadInput   --assets ./assets   --out build/mygame_desktop --release
```

**WASM**
```
cross465-pack   --personality personalities/c64_modern.toml   --modules display=RetroModern2D   --wasm --out build/mygame_web
```

---

## 3. Packager Manifest

`packager.toml`:
```toml
[runtime]
name = "MyModernRetroGame"
personality = "c64_modern.toml"
embed_personality = true
target = "desktop"   # or "wasm"
release = true

[modules]
display = "RetroModern2D"
audio   = "BasicAudio"
input   = "GamepadInput"
system  = "BasicSystem"

[assets]
include = ["assets/**"]
compress = true
```

Invoke:
```
cross465-pack --manifest packager.toml
```

---

## 4. Workflow (Detailed)

```
TOML ──validate──▶ IR ──codegen──▶ personality_gen.rs ──cargo──▶ runtime
            ▲                         ▲ modules
            │                         └─ static register_module!() selection
     schema + rules
```

### 4.1 Phase A — Parse & Validate
- Schema check (addresses, kinds, overlaps, pointer blocks).  
- Semantic checks (scatter width consistent; fanout read forbidden).  
- Diagnostics include source locations and error codes (`P00x`).

### 4.2 Phase B — Codegen
- Produce `src/personality_gen.rs` with `personality!{}` / `reg!` / `scatter!` / `ptrdec!` macros.  
- Deterministic ordering (by address → id → declaration order).

### 4.3 Phase C — Module Linking
- Resolve requested modules (by name) in static registry.  
- Verify **capabilities** vs personality requirements (e.g., `sprites`, `tilemap`).

### 4.4 Phase D — Asset Embedding (optional)
- Embed via `include_bytes!()` or `rust-embed`.  
- Provide a `Services::get_asset` callback for modules/adapters.

### 4.5 Phase E — Build
- Emit a minimal runtime crate in `target/runtime/`.  
- Run `cargo build` with feature flags (`--features wasm`, `--features embed_assets`).

---

## 5. Output Layout

### 5.1 Desktop
```
build/mygame_desktop/
 ├─ mygame            # single executable
 ├─ manifest.json
 └─ (optional) assets/
```

### 5.2 WASM
```
build/mygame_web/
 ├─ mygame.wasm
 ├─ mygame.js         # tiny loader (optional)
 └─ manifest.json
```

**manifest.json**
```json
{
  "runtime": "cross465-core 1.0.0",
  "personality": "c64_modern",
  "modules": ["RetroModern2D","BasicAudio","GamepadInput","BasicSystem"],
  "build": "2025-11-01T14:32:00Z",
  "wasm": true
}
```

---

## 6. Generated Rust Preview
`target/runtime/src/personality_gen.rs` (excerpt):
```rust
use cross465_core::prelude::*;

pub struct C64ModernPersonality;

impl PersonalityDef for C64ModernPersonality {
    fn describe() -> Personality {
        personality! {
            name = "c64_modern",
            version = (1,0,0),
            mmio = [
                reg!(addr="D000", id="sprite[0].XLo", kind="sprite"),
                reg!(addr="D001", id="sprite[0].YLo", kind="sprite"),
                scatter!(addr="D010", bits=8, kind="sprite", field="XHi"),
                scatter!(addr="D015", bits=8, kind="sprite", field="Enable")
            ],
            pointer_decoders = [
                ptrdec! {
                  name="vic2_sprites",
                  source=table!("07F8..07FF", entries=8, kind="sprite"),
                  block=64,
                  subindex=sub!(bits=2),
                  index=idx!(from="pointer", shift=2)
                }
            ]
        }
    }
}
```

---

## 7. Dual-Target Build (Desktop + WASM)

**Desktop release:**
```
cross465-pack --manifest packager.toml --out build/desktop --release
```

**WASM:**
```
cross465-pack --manifest packager.toml --wasm --out build/web
```

Feature toggles (packager chooses):
- Desktop: `--features embed_assets` (optional), no `wasm` feature.  
- WASM: `--features wasm,embed_assets` (dynamic loading disabled).

---

## 8. Error Reference
| Code | Meaning | Fix |
|------|--------|-----|
| `E001` | Missing module or capability | Adjust `--modules` or module versions. |
| `E002` | Invalid TOML schema | Fix addresses/kinds; see diagnostics with line numbers. |
| `E003` | Asset not found | Ensure `[assets] include` paths are correct. |
| `E004` | Build failed | Check `target/runtime/build.log`. |

---

## 9. Best Practices
- Keep assets logically named; use stable ids in pointer decoders/bindings.  
- Prefer **static** personalities for distribution.  
- Use `asm465-dev` + dynamic modules **only** for local iteration.


---

*Part of the **Cross465 Runtime SDK** doc set.*  
Index: [00 Overview](00_Cross465_Runtime_SDK_Overview.md)
