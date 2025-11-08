
# Cross465 Runtime SDK — asm465 Dev Loader Specification (06)
[← 05: Runtime Template](05_Cross465_Runtime_SDK_Runtime_Template_Spec.md)

**Status:** Draft • **Audience:** asm465 integrators & module authors • **Target:** desktop dev only (no WASM)

The **asm465 Dev Loader** enables **dynamic loading and hot‑reloading** of runtime modules during development and debugging. It is intentionally limited to **desktop** builds and is not used in released runtimes or WASM.

---

## 1. Goals
- Rapid iteration on **Display**, **Audio**, **Input**, **System**, **Storage** modules.
- Load `.so/.dll/.dylib` at runtime using a **stable C‑ABI shim**.
- **Hot‑reload** modules without restarting the dev session.
- Provide clear **capability negotiation** and **error diagnostics**.

---

## 2. Build & Feature Flags
- `asm465-dev` binary is built **only** with the `dynamic_modules` feature enabled.
- `cross465-core` exposes the C‑ABI shim types behind `dynamic_modules`.
- WASM builds **do not** include the dev loader.

```toml
[features]
dynamic_modules = ["libloading"]
wasm = []
```

---

## 3. CLI
```
asm465-dev --personality <file.toml>   --module <slot>=<path or name> [...]   [--manifest devloader.toml] [--console] [--watch] [--hot-reload] [--log-level <lvl>]
```

### Options
| Flag | Description |
|------|-------------|
| `--personality` | Path to TOML personality (dynamic_personalities only). |
| `--module display=...` | For each `slot`, provide a static **name** or a **path** to a shared library. |
| `--manifest` | Optional TOML manifest (see below). |
| `--console` | Embedded dev console (stdin REPL). |
| `--watch` | Watches module files and reloads on change. |
| `--hot-reload` | Enables manual hot‑reload commands. |
| `--log-level` | `error|warn|info|debug|trace` (default: `info`). |

---

## 4. Dev Loader Manifest
```toml
[personality]
file = "c64_modern.toml"

[modules]
display = "./mods/retromodern2d.dylib"
audio   = "./mods/basicaudio.dll"
input   = "GamepadInput"
system  = "BasicSystem"

[console]
enabled = true
watch = true
hot_reload = true
log_level = "debug"
```

---

## 5. Dynamic Module ABI (summary)
Resolve export:
```
cross465_module_exports() -> *const ModuleCExports
```
Read descriptor, create/destroy/init/quiesce fns, and the slot vtables.

---

## 6. Capability Negotiation
Compare personality‑required capabilities with module‑advertised ones; error on missing capabilities with actionable diagnostics.

---

## 7. Hot‑Reload Flow
1) Snapshot relevant state.  
2) `quiesce()` → `destroy()` → unload.  
3) Load new lib → `create()` → `init(Services)`.  
4) Replay snapshot; resume frame updates.

---

## 8. Dev Console
```
:module list
:module info <slot>
:module reload <slot>
:module swap <slot> <name-or-path>
:personality reload
:snapshot save <file>
:snapshot load <file>
:log level <level>
:diag caps
```
Returns structured status and logs.

---

## 9. Error Reference
| Code | Condition | Action |
|------|-----------|--------|
| `E010` | Shared library not found | Print path; retry if `--watch`. |
| `E011` | Missing symbol | Suggest correct SDK version. |
| `E012` | Slot mismatch | Refuse; show expected slot. |
| `E013` | Capability missing | Refuse; list missing capabilities. |
| `E014` | Init failed | Unload and report module logs. |
| `E015` | Replay failed | Warn; continue with defaults. |
| `E016` | Personality parse error | Show TOML diagnostic; keep previous. |

---

## 10. Security
Banner: “Dynamic modules run native code. Load only from trusted sources.”  
No dynamic loading in release/WASM.


---

*Part of the **Cross465 Runtime SDK** doc set.*  
Index: [00 Overview](00_Cross465_Runtime_SDK_Overview.md)
