
# Cross465 Runtime SDK — Display Module Scaffold (RetroModern2D) (04)
[← 03: Personality Codegen](03_Cross465_Runtime_SDK_Personality_Codegen_Spec.md) | [→ 05: Runtime Template](05_Cross465_Runtime_SDK_Runtime_Template_Spec.md)

**Status:** Draft • **Audience:** module implementers • **Target:** desktop & WASM (static); desktop dev (dynamic)

This document provides a **reference scaffold** for a first‑party display module named **RetroModern2D**.  
It implements the `DisplayModule` trait from the **Module SDK**, supports sprites, tilemaps, bitmap layers, and parallax scrolling, and is suitable for **static linking** in release/WASM as well as **dynamic loading** in `asm465-dev`.

---

## 1. Goals & Capabilities
- **Capabilities**: `["sprites","tilemap","bitmap","parallax","text"]`
- **Sprites**: position, enable, variant selection (with scaling fallback per Option‑A of pointer decode).
- **Tilemaps**: bind tilemap & tileset by string id; fast swaps for page flipping.
- **Bitmap layers**: bind static or scrolling backgrounds by id.
- **Parallax**: per-layer scroll factor (`0.0..1.0+`).

> This scaffold is renderer‑agnostic. You can back it with Bevy, WGPU, SDL2, or custom GL. The trait surface is kept minimal; map operations to your renderer internally.

---

## 2. Project Layout

```
retromodern2d/
 ├─ Cargo.toml
 └─ src/
    ├─ lib.rs
    ├─ module.rs          # trait impls
    ├─ renderer.rs        # internal renderer abstraction
    ├─ layers.rs          # tilemap, bitmap, text layers
    ├─ sprites.rs         # sprite state & batching
    ├─ assets.rs          # resolve ids via Services::get_asset
    ├─ dynamic_abi.rs     # (optional) C‑ABI shim/vtable for asm465‑dev
    └─ tests/
       └─ conformance.rs
```

---

## 3. Cargo Manifest (skeleton)

```toml
[package]
name = "retromodern2d"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["rlib", "cdylib"]  # cdylib only for desktop dynamic dev

[features]
dynamic_modules = []   # enables C-ABI shim for asm465-dev
wasm = []              # restricts to static features (no dynamic)

[dependencies]
cross465-core = { path = "../cross465-core" }
thiserror = "1"

[dev-dependencies]
pretty_assertions = "1"
```

---

## 4. Module Descriptor & Registration

```rust
// src/lib.rs
pub mod module;
#[cfg(feature = "dynamic_modules")]
pub mod dynamic_abi;

pub use module::RetroModern2D;

#[cfg(not(feature="dynamic_modules"))]
pub use cross465_core::register_module;

#[cfg(not(feature="dynamic_modules"))]
register_module!(
  slot = cross465_core::ModuleSlot::Display,
  name = "RetroModern2D",
  vendor = "PIG Games",
  version = (1,0,0),
  ctor = || Box::new(RetroModern2D::new()),
  capabilities = ["sprites","tilemap","bitmap","parallax","text"]
);
```

---

## 5. Internal State Structures

```rust
// src/sprites.rs
#[derive(Default, Clone)]
pub struct Sprite {
    pub x: u16,
    pub y: u16,
    pub enabled: bool,
    pub variant: u16,    // (index, subindex) packed per adapter policy
    pub sx: f32,         // scale X (fallback transform)
    pub sy: f32,         // scale Y (fallback transform)
}

pub struct SpriteSet {
    pub items: Vec<Sprite>,
}

impl SpriteSet {
    pub fn new(count: usize) -> Self {
        Self { items: vec![Sprite::default(); count] }
    }
}
```

```rust
// src/layers.rs
#[derive(Default, Clone)]
pub struct LayerScroll { pub x: f32, pub y: f32, pub parallax: f32 }

#[derive(Default)]
pub struct TileLayer {
    pub tilemap_id: Option<String>,
    pub tileset_id: Option<String>,
    pub scroll: LayerScroll,
}

#[derive(Default)]
pub struct BitmapLayer {
    pub image_id: Option<String>,
    pub scroll: LayerScroll,
}

pub struct Layers {
    pub tile_layers: std::collections::BTreeMap<String, TileLayer>,
    pub bitmap_layers: std::collections::BTreeMap<String, BitmapLayer>,
}

impl Layers {
    pub fn new() -> Self {
        Self {
            tile_layers: Default::default(),
            bitmap_layers: Default::default(),
        }
    }
}
```

---

## 6. Renderer Abstraction (placeholder)

```rust
// src/renderer.rs
pub trait Renderer {
    fn init(&mut self) -> anyhow::Result<()>;
    fn resize(&mut self, _w: u32, _h: u32) {}
    fn submit(&mut self, sprites: &[crate::sprites::Sprite], layers: &crate::layers::Layers);
}

/// A no-op renderer for headless tests
pub struct NullRenderer;
impl Renderer for NullRenderer {
    fn init(&mut self) -> anyhow::Result<()> { Ok(()) }
    fn submit(&mut self, _s: &[crate::sprites::Sprite], _l: &crate::layers::Layers) {}
}
```

Replace with your chosen backend (Bevy ECS systems, WGPU pipelines, etc.).

---

## 7. Module Implementation

```rust
// src/module.rs
use cross465_core::prelude::*;
use crate::{sprites::SpriteSet, layers::Layers, renderer::{Renderer, NullRenderer}};

pub struct RetroModern2D<R: Renderer = NullRenderer> {
    desc: ModuleDescriptor,
    sprites: SpriteSet,
    layers: Layers,
    renderer: R,
}

impl<R: Renderer + Default> RetroModern2D<R> {
    pub fn new() -> Self {
        Self {
            desc: ModuleDescriptor {
                name: "RetroModern2D",
                vendor: "PIG Games",
                version: (1,0,0),
                slot: ModuleSlot::Display,
                capabilities: &["sprites","tilemap","bitmap","parallax","text"],
            },
            sprites: SpriteSet::new(8), // default sprite count; can be overridden by personality
            layers: Layers::new(),
            renderer: R::default(),
        }
    }
}

impl<R: Renderer> Module for RetroModern2D<R> {
    fn descriptor(&self) -> &ModuleDescriptor { &self.desc }
    fn init(&mut self, _s: &mut Services) -> Result<(), ModuleError> {
        self.renderer.init().map_err(|_| ModuleError::InitFailed("renderer"))?;
        Ok(())
    }
    fn quiesce(&mut self) {}
}

impl<R: Renderer> DisplayModule for RetroModern2D<R> {
    fn set_sprite_xy(&mut self, i: usize, x: u16, y: u16) {
        if let Some(sp) = self.sprites.items.get_mut(i) { sp.x = x; sp.y = y; }
    }
    fn set_sprite_enable(&mut self, i: usize, on: bool) {
        if let Some(sp) = self.sprites.items.get_mut(i) { sp.enabled = on; }
    }
    fn set_sprite_variant(&mut self, i: usize, sub: u16) {
        if let Some(sp) = self.sprites.items.get_mut(i) { sp.variant = sub; }
    }
    fn set_sprite_scale(&mut self, i: usize, sx: f32, sy: f32) {
        if let Some(sp) = self.sprites.items.get_mut(i) { sp.sx = sx; sp.sy = sy; }
    }

    fn layer_set_tilemap(&mut self, layer: &str, id: &str) {
        self.layers.tile_layers.entry(layer.to_string()).or_default().tilemap_id = Some(id.to_string());
    }
    fn layer_set_tileset(&mut self, layer: &str, id: &str) {
        self.layers.tile_layers.entry(layer.to_string()).or_default().tileset_id = Some(id.to_string());
    }
    fn layer_set_bitmap(&mut self, layer: &str, id: &str) {
        self.layers.bitmap_layers.entry(layer.to_string()).or_default().image_id = Some(id.to_string());
    }
    fn layer_set_scroll(&mut self, layer: &str, x: f32, y: f32, parallax: f32) {
        if let Some(l) = self.layers.tile_layers.get_mut(layer) {
            l.scroll = crate::layers::LayerScroll { x, y, parallax };
            return;
        }
        self.layers.bitmap_layers.entry(layer.to_string())
            .or_default().scroll = crate::layers::LayerScroll { x, y, parallax };
    }
}
```

> **Note:** The runtime will call a `present()` or equivalent method per frame. You can expose it via Services or a separate trait; for brevity it’s omitted here.

---

## 8. Scaling‑aware variant handling (Option‑A + fallback)
In your **adapter**, decode `(anim, x2, y2)` into the sprite `variant` and `sx/sy` fields. If the asset isn’t present, apply `sx/sy` as a transform.

---

## 9. Dynamic Loading Shim (asm465‑dev)
Only with `--features dynamic_modules` export `cross465_module_exports` and mirror the C‑ABI vtable per **01 Module SDK**.

---

## 10. Conformance Tests (suggested)
```rust
#[test]
fn sprites_move_and_scale() {
    let mut m = RetroModern2D::<renderer::NullRenderer>::new();
    // ... init & assert behaviour
}
```


---

*Part of the **Cross465 Runtime SDK** doc set.*  
Index: [00 Overview](00_Cross465_Runtime_SDK_Overview.md)
