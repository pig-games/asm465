## Sprite Coordinate Remapping Exploration

### Current behaviour (baseline)
- Guest writes sprite X/Y in 8.8 fixed-point (`$FFFF` ≈ 255.996). We scale to the
  virtual resolution (`virtual_width × virtual_height`) directly, so the usable
  range is effectively capped at the virtual resolution, not the 16-bit max.
- Default virtual resolution (320×240) therefore yields a “dead zone” beyond
  ~255 units on each axis (sprites can’t reach the far edge).
- Upcoming off-screen-margins work will further stretch the logical space, so
  the mapping needs to accommodate out-of-bounds positions gracefully.

### Requirements
- Full virtual range must be reachable (e.g. rightmost column at `virtual_width`).
- Allow configurable margin so sprites can start/exit off-screen while still
  obeying min-border overlays.
- Keep behaviour deterministic for tooling/tests (avoid floating surprises).
- Avoid breaking existing ROM/tests (consider backwards compatibility or
  transitional scaling).

### Option 1 – Linear scaling with adjustable max
- Treat MMIO values as **raw** (0..65535). Use a scalar mapping: `world = raw * scale + offset`.
- Choose scale = `virtual_width / max_mmio_value` where `max_mmio_value` is
  configurable (e.g. default 65535). This can be set to `virtual_width + margin`.
- Pros: simple, linear, easy to reason; margins just adjust scale/offset.
- Cons: loses 8.8 precision semantics (but we can treat high byte as integer,
  low byte as fractional after applying new scale).

### Recommended approach
- Baseline mapping: treat the 16-bit value as a linear coordinate spanning the
  full virtual width/height. Off-screen margins can then be expressed via
  configurable offsets.
  - We **accept** this mapping (Option 1) to ensure the entire virtual canvas is
    reachable and simplifying offset math.
- Add an optional **power-of-two scale factor per sprite slot** (e.g. 1×, 2×,
  4×). The graphics MMIO can expose new registers so games on modern systems
  can use sub-pixel movement while retro-style titles leave the scale at 1×.
  - We **accept** this extension: per-sprite scale registers provide higher
    resolution without complicating the baseline case.
- Options 2, 3, and 4 (piecewise scaling, new fixed-point formats, configurable
  MMIO ranges) are **rejected** for now because they introduce extra complexity
  or break backwards compatibility without clear benefit.
  - They remain noted for future consideration if requirements change.

### Implementation sketch
- `graphics_mmio` gains `spr_scale_x/spr_scale_y` registers (encoded as shift
  exponents so guests can write values cheaply).
- The Bevy frontend multiplies the decoded position by `2^scale` before applying
  offsets. Defaults stay at 1× for compatibility.
- Document the effective formula so tooling/tests can calculate
  `world = ((raw_value as f32) * 2^scale / virtual_extent) - offset` (exact
  expression TBD during implementation).

### Option 2 – Maintain 8.8 but apply post-scaling clamp/stretch
- Keep 8.8 decode, then apply a stretch factor if decoded value exceeds the
  virtual size (e.g. `clamped = min(decoded, virtual_width); world = clamped + margin`).
- Could also use piecewise mapping (0..virtual -> linear, virtual..max -> map
  progressively to margin). Pros: retains “pixel” semantics for first 255 units.
- Cons: piecewise logic may introduce non-linear speed of sprites near edges;
  might be harder to reason about/test.

### Option 3 – Dedicated fixed-point format
- Define a new fixed-point format (e.g. 12.4 or 10.6) so `max` corresponds to
  virtual dimension + margin. Guests would target the new format. Pros: precise.
- Cons: requires ROM/ASM updates; new format might be harder to encode manually.

### Option 4 – Virtual-to-MMIO remap via configuration
- Add configuration that defines a “MMIO coordinate space” (e.g. 0..1023). Keep
  guests writing integer values, but we scale to virtual width proportionally.
- Off-screen margins just expand the virtual width/height we map from.
- Pros: expresses mapping explicitly; guests can refer to a documented range.
- Cons: requires documentation & potential update for existing ROMs.

### Considerations
- Backwards compatibility: Option 1 with default `max_mmio_value=virtual_width`
  matches old behaviour but allows tuning. We could default to new value
  `max_mmio_value=virtual_width + 2*margin`, preserving old semantics with margin=0.
- Precision: using floats is acceptable (Bevy works in f32), but we should
  ensure tests verify edges and check rounding.
- Tooling: docs should show the formula so ROM authors know how to target the new space.

### Implementation snapshot
- Sprite scale register at `$DF29` packs X shift in the high nibble and Y shift
  in the low nibble; the frontend divides the 16-bit MMIO value by `2^shift`
  before applying offsets.
- Host configuration now exposes `sprite_margin_*`, `sprite_mmio_max_*`, and
  `sprite_max_offscreen_*` so guests can tune the reachable range and the
  culling threshold independently.
- Leaving the MMIO maxima at `0` defers to the virtual-resolution+margin span;
  with the default margins (one sprite width/height) this preserves the legacy
  8.8 sweep while allowing a small off-screen buffer.
- Mapping uses the agreed linear form  
  `virtual = (mmio / mmio_max) * (virtual_extent + margins) - margin`, feeding
  directly into the viewport scaling logic.
- Off-screen sprites are hidden once their bounding box exceeds the configured
  `sprite_max_offscreen_*` allowance, ensuring the border overlay remains opaque.
