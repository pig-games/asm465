## Sprite Scaling & Layout Improvements

- [x] Audit current sprite coordinate pipeline  
  - [x] Trace how `virtual_width/height`, camera scaling, and sprite sizes interact in native & wasm builds  
  - [x] Capture baseline behaviour with instrumentation to reference expected world positions
  - [x] Document findings (see `graphics_pipeline_audit.md`)

- [x] Move sprite MMIO into a dedicated graphics module  
  - [x] Create a new MMIO device responsible for sprites/background/border state  
  - [x] Map it to a separate address window (freeing the console MMIO range)  
  - [x] Update the bus initialisation to register both console and graphics MMIO devices  
  - [x] Add migration glue so existing ROM/tests keep working (update include files / constants)  
  - [x] Ensure the new module exposes shared state handles for the Bevy frontend (sprites, colours, etc.)  
  - [x] Remove sprite-specific code from `ConsoleMmio` and verify console text behaviour remains intact  
  - [x] Extend the new module with border/background colour registers as part of the graphics MMIO  
  - [x] Add tests covering register reads/writes and interaction with the snapshot exposed to Bevy

- [x] Scale sprite dimensions alongside coordinates  
  - [x] Decide on configuration surface (automatic vs. explicit scale factor per axis)  
  - [x] Update `sprite_world_position` (or companion helper) to return a size multiplier in addition to position  
  - [x] Apply the multiplier to each sprite’s `Sprite::custom_size` while preserving aspect ratio  
  - [x] Add focused unit tests that validate size scaling for a few window/virtual-resolution combinations  
  - [x] Update `graphics_pipeline_audit.md` to reflect the changes
  - [x] Write commit message summarising scaling changes and tests

- [x] Introduce aspect-ratio controls & border handling  
  - [x] Extend configuration to support:  
    - [x] Enforcing virtual aspect ratio on the output canvas  
    - [x] Custom border colour when letterboxing/pillarboxing  
    - [x] Custom background colour separate from the content  
    - [x] Optional window resizing to match the enforced aspect ratio (native only)  
    - [x] Minimum horizontal/vertical border sizes so we can emulate fixed hardware margins  
  - [x] Implement viewport calculations that determine content bounds and border regions  
  - [x] Render a border overlay (or clear colour) that sits above sprites so hidden regions stay concealed  
  - [x] Unit-test the viewport/border math to ensure centered content and correct padding sizes  
  - [x] Update `graphics_pipeline_audit.md` to reflect the changes  
  - [ ] Write commit message summarising aspect-ratio/border updates

- [ ] Support configurable off-screen margins for sprites  
  - [ ] Add a setting describing the maximum sprite size to tolerate when off-screen  
  - [ ] Modify coordinate mapping to expand the virtual canvas by that margin, keeping `x=0/y=0` just outside top-left and `x=max` outside bottom-right after borders are applied  
  - [ ] Ensure clipping/overlay still masks sprites that remain outside the visible content area  
  - [ ] Cover the updated mapping with unit tests that check extreme values (negative offsets, fully-hidden sprites, etc.)  
  - [ ] Update `graphics_pipeline_audit.md` to reflect the changes
  - [ ] Write commit message summarising off-screen margin changes

- [ ] Polish & cleanup  
  - [ ] Remove temporary logging/instrumentation or behind a feature flag  
  - [ ] Document the new settings in README / inline rustdoc  
  - [ ] Re-verify native and wasm builds, including asset bundling  
  - [ ] Ensure `graphics_pipeline_audit.md` captures the final state
  - [ ] Write commit message summarising polish/cleanup

- [ ] Extend MMIO interface with colour registers  
  - [x] Add read/write registers for border colour  
  - [x] Add read/write registers for background colour  
  - [ ] Surface new values in Bevy so the enforced borders/background pick them up  
  - [x] Add tests (or assertions in instrumentation) to confirm writes propagate to the frontend  
  - [ ] Update `graphics_pipeline_audit.md` to reflect the changes
  - [ ] Write commit message summarising colour-register integration
