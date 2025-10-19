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
- In `asm465/src/lib.rs`, the UI system logs the raw hex and decoded float for
  each sprite before calling `sprite_world_transform`.
- Both native and wasm builds share the same code path; wasm just needed assets
  copied into `web-dist` so textures resolve.

### Scaling Logic
- `sprite_world_transform` treats the registers as U8.8 fixed-point values,
  clamps them to the configured virtual resolution, normalises to `[0, 1]`, and
  scales both positions and sprite sizes based on the active window (or the
  enforced aspect-ratio viewport).
- Unit tests at the end of `asm465/src/lib.rs` confirm origin/midpoint/max
  placement, sprite dimensions, and the aspect-ratio viewport math.

### Aspect Ratio & Borders
- `DisplaySettings` (from CLI or defaults) controls whether we enforce the
  virtual aspect ratio, minimum border sizes, and border/background colours.
- Defaults aim to emulate a letterboxed 320×240 canvas: aspect enforcement
  enabled, 50px minimum borders, a dark-grey border (`#404040`), black
  background, and automatic window resizing on native builds.
- When enforcement is enabled, the viewport snaps to the largest letterboxed
  region that honours the requested aspect ratio and minimum borders. The
  surrounding window is cleared to the border colour, while the content region
  is drawn via a background quad tinted to the configured background colour.
- Optional native-only resizing can adjust the primary window to match the
  virtual aspect ratio on startup.

### Observations
- The pipeline now uses the dedicated graphics MMIO and honours the new
  display settings. Remaining work items (off-screen margins, final polish)
  can build on this split without touching the console MMIO.
