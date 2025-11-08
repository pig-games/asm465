
# Cross465 Runtime SDK — Module SDK Specification (01)
[← 00: Overview](00_Cross465_Runtime_SDK_Overview.md) | [→ 02: Packager Tool](02_Cross465_Runtime_SDK_Packager_Tool_Spec.md)

**Status:** Draft • **Target:** desktop & WASM • **Audience:** module authors & runtime integrators

---

## 1. Goals
- Stable surface for **first‑party** and **community** runtime modules.
- Clean separation between **core traits (Rust)** and optional **desktop dynamic loading (C‑ABI shim)**.
- Works **statically** (WASM + release builds) and **dynamically** (asm465‑dev on desktop).

---

## 2. Terminology
- **Module**: a capability provider (Display, Audio, Input, System, Storage).
- **Slot**: where a module is plugged (e.g., `ModuleSlot::Display`).
- **Descriptor**: build‑time/runtime metadata (name, vendor, semver, capabilities).
- **Capabilities**: strings describing features implemented by a module (`sprites`, `tilemap`, `bitmap`, `parallax`, `sid3voice`, etc.).

---

## 3. Rust‑side API (static and dynamic use)
`cross465_core::module` defines the traits and descriptors used by the runtime.

```rust
// core/src/module.rs
pub struct ModuleDescriptor {
    pub name: &'static str,
    pub vendor: &'static str,
    pub version: (u16, u16, u16),  // (major, minor, patch)
    pub slot: ModuleSlot,
    pub capabilities: &'static [&'static str],
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ModuleSlot { Display, Audio, Input, System, Storage }

pub trait Module {
    fn descriptor(&self) -> &ModuleDescriptor;
    fn init(&mut self, services: &mut Services) -> Result<(), ModuleError>;
    fn quiesce(&mut self) {}                  // optional: prepare for unload/hot‑swap
}

// Display slot example (other slots similar)
pub trait DisplayModule: Module {
    // Sprites
    fn set_sprite_xy(&mut self, i: usize, x: u16, y: u16);
    fn set_sprite_enable(&mut self, i: usize, on: bool);
    fn set_sprite_variant(&mut self, i: usize, sub: u16);
    fn set_sprite_scale(&mut self, i: usize, sx: f32, sy: f32);

    // Layers / tilemaps / bitmaps
    fn layer_set_tilemap(&mut self, layer: &str, id: &str);
    fn layer_set_tileset(&mut self, layer: &str, id: &str);
    fn layer_set_bitmap(&mut self, layer: &str, id: &str);
    fn layer_set_scroll(&mut self, layer: &str, x: f32, y: f32, parallax: f32);
}
```

### Services passed to modules
```rust
pub struct Services<'a> {
    pub log: &'a dyn Fn(LogLevel, &str),
    pub get_asset: &'a dyn Fn(AssetKind, &str) -> AssetHandle,
    pub schedule_callback: &'a dyn Fn(FrameTime, CallbackId),
    // ... extend as needed (time, input polling, save state, etc.)
}
```

### Errors
```rust
#[derive(thiserror::Error, Debug)]
pub enum ModuleError {
    #[error("capability missing: {0}")]
    CapabilityMissing(&'static str),
    #[error("resource not found: {0}")]
    ResourceNotFound(String),
    #[error("init failed: {0}")]
    InitFailed(&'static str),
    #[error("unsupported: {0}")]
    Unsupported(&'static str),
}
```

---

## 4. Registration (static)
For **release/WASM**, modules are **statically linked** and registered via macros.

```rust
// core/src/register.rs
#[macro_export]
macro_rules! register_module {
    (slot=$slot:expr, name=$name:expr, vendor=$vendor:expr, version=$ver:expr, ctor=$ctor:expr, capabilities=[$($cap:expr),*]) => {
        #[doc(hidden)]
        #[allow(non_upper_case_globals)]
        pub static __REGISTER: cross465_core::inventory::Submit<cross465_core::ModuleFactory> =
            cross465_core::inventory::Submit::new(cross465_core::ModuleFactory {
                slot: $slot,
                name: $name,
                vendor: $vendor,
                version: $ver,
                capabilities: &[$($cap),*],
                ctor: $ctor,
            });
    };
}

// Used by runtimes:
register_module!(
  slot = ModuleSlot::Display,
  name = "RetroModern2D",
  vendor = "PIG Games",
  version = (1,2,0),
  ctor = || Box::new(RetroModern2D::new()),
  capabilities = ["sprites","tilemap","bitmap","parallax"]
);
```

The runtime picks modules by name/slot:
```rust
let rt = Runtime::builder()
    .with_display("RetroModern2D")
    .with_audio("BasicAudio")
    .build()?;
```

---

## 5. Dynamic loading (desktop dev only, optional)
A tiny **C‑ABI shim** allows loading `.so/.dll/.dylib` modules in `asm465-dev` using `libloading`.
Not available on WASM; omitted from release builds by default.

### C‑ABI types
```rust
#[repr(C)]
pub struct ModuleCDescriptor {
    pub name: *const c_char,
    pub vendor: *const c_char,
    pub version_major: u16,
    pub version_minor: u16,
    pub version_patch: u16,
    pub slot: u32, // matches ModuleSlot
    pub capabilities: *const *const c_char, // null‑terminated array
}

#[repr(C)]
pub struct ModuleCExports {
    pub descriptor: extern "C" fn() -> *const ModuleCDescriptor,
    pub create: extern "C" fn() -> *mut c_void,
    pub destroy: extern "C" fn(ptr: *mut c_void),
    pub init: extern "C" fn(ptr: *mut c_void, services: *mut ServicesC) -> i32,
    pub quiesce: extern "C" fn(ptr: *mut c_void),

    pub display_vtbl: *const DisplayCVtbl, // null if not display
    pub audio_vtbl:   *const AudioCVtbl,   // null if not audio
    // ...
}
```

### Display vtable (example)
```rust
#[repr(C)]
pub struct DisplayCVtbl {
    pub set_sprite_xy: extern "C" fn(ptr: *mut c_void, i: u32, x: u16, y: u16),
    pub set_sprite_enable: extern "C" fn(ptr: *mut c_void, i: u32, on: bool),
    pub set_sprite_variant: extern "C" fn(ptr: *mut c_void, i: u32, sub: u16),
    pub set_sprite_scale: extern "C" fn(ptr: *mut c_void, i: u32, sx: f32, sy: f32),
    pub layer_set_tilemap: extern "C" fn(ptr: *mut c_void, layer: *const c_char, id: *const c_char),
    pub layer_set_tileset: extern "C" fn(ptr: *mut c_void, layer: *const c_char, id: *const c_char),
    pub layer_set_bitmap: extern "C" fn(ptr: *mut c_void, layer: *const c_char, id: *const c_char),
    pub layer_set_scroll: extern "C" fn(ptr: *mut c_void, layer: *const c_char, x: f32, y: f32, parallax: f32),
}
```

### Module export symbol
```rust
#[no_mangle]
pub extern "C" fn cross465_module_exports() -> *const ModuleCExports {
    &RETROMODERN2D_EXPORTS
}
```

**Safety**: mark dev loader as “unsafe third‑party code allowed”; sandboxing not guaranteed.

---

## 6. Lifecycle
1) **Discover** (static registry or dynamic loader).  
2) **Construct** (`ctor` or `create`).  
3) **Init** (receives `Services`).  
4) **Run** (methods invoked by adapter/runtime).  
5) **Quiesce** (optional; prepare for unload/hot‑swap).  
6) **Destroy**.

Hot‑reload (dev): snapshot state → `quiesce` → `destroy` → load new → `init` → replay snapshot.

---

## 7. Versioning & Compatibility
- **Semver** on both **module** and **SDK**.  
- Runtime performs **capability negotiation**:
  - Personality requires: `["sprites","tilemap"]`.
  - Module advertises: `["sprites","tilemap","bitmap"]`.
  - If a required capability is missing → `ModuleError::CapabilityMissing("tilemap")`.
- **Minimum SDK version** can be embedded in `ModuleCDescriptor`.

---

## 8. WASM Constraints
- Dynamic loading **disabled**.  
- Only **static registration** with `register_module!`.  
- Services must avoid blocking/threads; use async or frame callbacks as needed.

---

## 9. Testing
- Unit tests for module conformance (mock Services).  
- Golden tests for sprite I/O, tilemap binding, scroll, and scale fallback.  
- ABI tests for dynamic vtable (desktop CI only).

---

## 10. Security Notes
- Dynamic modules are native code: load only from trusted sources.  
- Prefer static linking for releases (single binary; fewer attack vectors).

---

## 11. Example: Minimal Display Module Skeleton
```rust
use cross465_core::prelude::*;

pub struct RetroModern2D { /* internal state */ }

impl RetroModern2D {
    pub fn new() -> Self { Self { /* ... */ } }
}

impl Module for RetroModern2D {
    fn descriptor(&self) -> &ModuleDescriptor {
        static DESC: ModuleDescriptor = ModuleDescriptor {
            name: "RetroModern2D",
            vendor: "PIG Games",
            version: (1,2,0),
            slot: ModuleSlot::Display,
            capabilities: &["sprites","tilemap","bitmap","parallax"],
        };
        &DESC
    }
    fn init(&mut self, _s: &mut Services) -> Result<(), ModuleError> { Ok(()) }
}

impl DisplayModule for RetroModern2D {
    fn set_sprite_xy(&mut self, i: usize, x: u16, y: u16) { /* ... */ }
    fn set_sprite_enable(&mut self, i: usize, on: bool) { /* ... */ }
    fn set_sprite_variant(&mut self, i: usize, sub: u16) { /* ... */ }
    fn set_sprite_scale(&mut self, i: usize, sx: f32, sy: f32) { /* ... */ }
    fn layer_set_tilemap(&mut self, l: &str, id: &str) { /* ... */ }
    fn layer_set_tileset(&mut self, l: &str, id: &str) { /* ... */ }
    fn layer_set_bitmap(&mut self, l: &str, id: &str) { /* ... */ }
    fn layer_set_scroll(&mut self, l: &str, x: f32, y: f32, p: f32) { /* ... */ }
}
```


---

*Part of the **Cross465 Runtime SDK** doc set.*  
Index: [00 Overview](00_Cross465_Runtime_SDK_Overview.md)
