## Graphics/Sprite Pipeline Audit

### 6502 → Bus
- Sprite registers now live in the dedicated sprite MMIO window (`$DF30–$DF37`)
  while border/background colours are exposed via the display MMIO window
  (`$DF20–$DF21`).
- `SpriteMmio::write` updates per-slot `SpriteState`s and mirrors them into the
  shared sprite snapshot immediately.
- `DisplayMmio::write` updates the colour registers and mirrors them into the
  display snapshot consumed by the viewer.
- Scale nibbles are surfaced through the sprite snapshot so the frontend can
  recover per-axis power-of-two factors when decoding coordinates.

### Snapshot → Bevy
- `ConsoleOutput::snapshot` now carries text only; sprite and display snapshots
  are handled separately.
- In `asm465/src/lib.rs`, the UI system mirrors the display snapshot to tint
  the border/background, and the sprite snapshot into Bevy components. Shared
  helpers (`sprite_mmio_position`, `sprite_world_transform`) keep native/wasm
  behaviour identical.
- Both native and wasm builds share the same code path; wasm just needed assets
  copied into `web-dist` so textures resolve.

### Mapping, Margins & Clipping
- MMIO sprite coordinates are decoded as raw 16-bit values. Per-axis shift
  registers provide optional sub-pixel precision (`value / 2^shift`).
- Frontend mapping uses a linear scale+offset:
  `virtual = (mmio / mmio_max) * (virtual_extent + margins) - margin`, so the
  full configurable margin range remains reachable.
- Default configuration keeps each margin at ~one sprite width/height (40×53⅓)
  while deriving `mmio_max` from the virtual resolution + margins, so legacy
  8.8 positions still sweep the full visible area but games can immediately
  nudge sprites slightly off-screen.
- Negative top/left margins allow sprites to spawn off-screen; right/bottom
  mapping uses the same range, so writes above the virtual extent move sprites
  past the edges instead of clamping.
- `DisplaySettings` now exposes:
  - `sprite_margin_{left,right,top,bottom}` (virtual units)
  - `sprite_mmio_max_{x,y}` to describe the guest-facing coordinate span
  - `sprite_max_offscreen_{width,height}` to control when fully hidden sprites
    are culled for rendering.
- `sprite_world_transform` converts the virtual top-left into Bevy world space
  without clamping, and hides sprites once their bounding box exceeds the
  configured off-screen allowance. This keeps letterboxed borders opaque while
  still letting sprites traverse beyond the content area. A dedicated border
  overlay (four quads rendered above the sprite layer) now masks anything that
  overlaps the border region so off-screen sprites remain hidden.
- Unit tests cover origin/midpoint/border placement, margin-only spawning,
  culling behaviour, and per-axis scaling.

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

### Sprite Coordinate Mapping
- Sprite MMIO values are interpreted linearly across the virtual canvas,
  extended by configurable margins so `(0,0)` can live outside the top-left
  border.
- Each sprite slot supports a power-of-two precision scale (shift exponent)
  so games can opt into sub-pixel motion while keeping the default behaviour
  simple.

### Observations
- The pipeline now uses the dedicated graphics MMIO, honours the new mapping
  configuration, culls sprites that remain fully outside the visible content
  area, and masks anything that overlaps the enforced border.
- Remaining work (colour-register surfacing) can build on this split without
  touching the console MMIO.
