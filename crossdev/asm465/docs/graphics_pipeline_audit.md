## Graphics/Sprite Pipeline Audit

### 6502 → Bus
- Sprite/border/background registers now live in the dedicated graphics MMIO
  window (`$DF20–$DF2F`).
- `GraphicsMmio::write` updates per-slot `SpriteState`s plus colour registers
  and mirrors them into `GraphicsOutput` immediately.
- Recent instrumentation (`log::trace!`) shows raw register values in 8.8
  fixed-point (e.g. `$3264 → 12.75`).

### Snapshot → Bevy
- `ConsoleOutput::snapshot` now carries text only; `GraphicsOutput::snapshot`
  provides sprites and colours to the frontend.
- In `asm465/src/lib.rs`, the UI system logs the raw hex and decoded float for
  each sprite before calling `sprite_world_position`.
- Both native and wasm builds share the same code path; wasm just needed assets
  copied into `web-dist` so textures resolve.

### Scaling Logic
- `sprite_world_transform` treats the registers as U8.8 fixed-point values,
  clamps them to the configured virtual resolution, normalises to `[0, 1]`, and
  scales both positions and sprite sizes based on the active window.
- Unit tests at the end of `asm465/src/lib.rs` confirm origin/midpoint/max
  placement and verify that sprite dimensions grow proportionally with the
  window.

### Observations
- The pipeline now uses the dedicated graphics MMIO. Remaining work items
  (sprite scaling, aspect ratio control, off-screen margins) can build on this
  split without touching the console MMIO.
