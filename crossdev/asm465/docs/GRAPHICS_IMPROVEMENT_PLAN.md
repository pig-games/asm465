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
  - [x] Write commit message summarising aspect-ratio/border updates

- [x] Explore sprite coordinate remapping  
  - [x] Analyse current 8.8 fixed-point mapping versus virtual resolution + desired margins  
  - [x] Document several remapping approaches (e.g. scaling factors, clamping strategies, non-linear maps) in a dedicated exploration doc  
  - [x] Determine recommended approach to support full virtual range and off-screen allowances  
  - [x] Update `graphics_pipeline_audit.md` with conclusions  
  - [x] Write commit message summarising the exploration findings

- [x] Support configurable off-screen margins for sprites  
  - [x] Add a setting describing the maximum sprite size to tolerate when off-screen  
  - [x] Implement linear MMIO→virtual mapping using the agreed scale/offset (Option 1 + per-sprite power-of-two scale) and apply configurable negative offsets for top-left margins (see `sprite_coord_remap.md`)
  - [x] Ensure sprites can move beyond right/bottom edges by mapping the full MMIO range after offsets  
  - [x] Ensure clipping/overlay still masks sprites that remain outside the visible content area  
  - [x] Cover the updated mapping with unit tests that check extreme values (negative offsets, fully-hidden sprites, etc.)  
  - [x] Update `graphics_pipeline_audit.md` to reflect the changes
  - [x] Write commit message summarising off-screen margin changes

- [x] Polish & cleanup  
  - [x] Remove temporary logging/instrumentation or behind a feature flag  
  - [x] Document the new settings in README / inline rustdoc  
  - [x] Re-verify native and wasm builds, including asset bundling  
  - [x] Ensure `graphics_pipeline_audit.md` captures the final state
  - [x] Write commit message summarising polish/cleanup

- [x] Extend MMIO interface with colour registers  
  - [x] Add read/write registers for border colour  
  - [x] Add read/write registers for background colour  
  - [x] Create a new MMIO module for generic display settings
  - [x] Rename graphics_mmio.rs to sprites_mmio.rs (and take care of dependencies)
  - [x] Move color registers to the new display MMIO module.
  - [x] Surface new values in Bevy so the enforced borders/background pick them up  
  - [x] Add tests (or assertions in instrumentation) to confirm writes propagate to the frontend  
  - [x] Update `graphics_pipeline_audit.md` to reflect the changes
  - [x] Write commit message summarising colour-register integration

- [x] Refactor the bus/MMIO architecture to support 'personalities' (see `graphics_pipeline_audit.md`)
  - [x] Define a `Personality` descriptor that lists MMIO mappings + default viewer config.
  - [x] Convert the current Modern Retro mapping into a default personality implemented via the descriptor.
  - [x] Add a loader/registry so the bus can be constructed from a selected personality (including CLI flag support in the viewer tools).
  - [x] Provide scaffold personalities (e.g. C64 mirror, modern 2D) with documentation for their MMIO ranges.
  - [x] Update docs/tests to cover personality selection and ensure MMIO modules initialise correctly per personality.

# rough outlines of additional features, these need to be further explored and documented.
- [ ] Add interrupt support from the modern UI layer, including display refresh, input, and timer sources (use `cross465/docs/6502_interrupts_overview.md` as reference).
  - [x] Capture the desired interrupt model: document which host events map to IRQ vs NMI, how acknowledgement/clearing works, and how personalities declare available sources (see `crossdev/asm465/docs/interrupt_model.md`).
  - [ ] Put the CPU on a dedicated worker thread with a controllable run loop (throttle, pause/resume hooks, graceful shutdown) so it can service interrupts continuously.
  - [ ] Introduce a thread-safe interrupt controller that the UI can call into
    - [ ] Provide APIs to raise/clear IRQ and NMI, queue multiple sources, and report pending state back to the CPU core.
    - [ ] Cover the controller with unit tests to confirm edge-triggered NMIs and level-triggered IRQs behave correctly.
  - [ ] Extend personality descriptors with interrupt definitions
    - [ ] Allow personalities to register named interrupt sources, associate them with IRQ/NMI lines, and expose configuration knobs (priority, enable bits, default masks).
    - [ ] Ensure personalities can subscribe to host-side producers (display loop, input manager, timers) and translate those events into controller calls.
  - [ ] Wire up interrupt sources for the ModernRetro personality
    - [ ] Display: raise `frame_start`/`frame_end` interrupts tied to the renderer’s vblank lifecycle and debounce duplicate triggers.
    - [ ] Timer: implement a host-driven programmable timer module in `cross465/asm465`, expose period registers via MMIO, and verify cadence with tests.
    - [ ] Keyboard: surface key state/register interface to the guest, fire interrupts on press/release, and document scan-code expectations.
    - [ ] Game controller: mirror the keyboard approach for controller state changes, including hot-plug/idle handling.
  - [ ] Update developer documentation and tooling
    - [ ] Add an interrupt wiring section to the bus/personality docs so new personalities can opt in.
    - [ ] Provide an integration test or harness that asserts ModernRetro receives interrupts when the host fires synthetic events.


- [ ] Add sprite collision support (possibly interupt driven or just some value that can be read)

- [ ] Implement color palettes and support for index colored sprites

- [ ] Implement SPRANIM support ('sub frames' per SPRNUM)
