# Milestone 6 – Raster IRQ & Controller Integration Plan

Intermediate checklist for the backend/adapter/personality work required before
the personality branch can merge back to `main`. These steps assume the 6502
test program will be authored separately; this document focuses on the host
plumbing.

## 1. Raster IRQ plumbing
- [x] Confirm `SystemMmio` exposes the full raster register set (`RasterLo`,
      `RasterCompareLo/Hi`) for both `modern-retro` and `c64-compat` personalities.
- [x] Ensure `VideoAdapter` raises `RASTER_IRQ_MASK` when the compare equals the
      Bevy-driven raster counter (adapter unit test
      `raster_compare_triggers_irq_when_current_matches` covers this).
- [x] Verify `SystemReg::IrqAck` clears only the masked bits so guest IRQ
      handlers can acknowledge raster interrupts reliably (`system_mmio::tests::read_write_registers`).

## 2. Controller state export
- [x] Double-check controller‑0 MMIO mappings:
      `DF50` stride for modern-retro, value builders for `$DC00/$DC01` in the
      C64 personality.
- [x] Update `Controller_Integration.md` (or personalities docs) with explicit
      bit layout/addresses so test authors know where to poll controller 0.

## 3. Sprite MMIO parity
- [x] Validate the sprite adapter/backend path mirrors instance writes,
      scatter bits (`$D010`), and fanout (`Enable`) into both the viewer snapshot
      and the underlying MMIO registers (covered by existing adapter unit tests).
- [x] Extend/refresh unit tests in `adapters/sprite.rs` and/or `sprite_mmio.rs`
      to prove scatter/fanout code paths remain functional.

## 4. System/MMIO documentation for test authors
- [x] Add a documentation snippet covering:
    - Raster IRQ register sequence (`IrqEnable`, `IrqAck`, `RasterCompare`).
    - Controller 0 register addresses and bit masks.
    - Sprite register offsets for both personalities.
    - Mention of `--enable-video-overlay` for raster debugging.



Once all boxes are checked and the assembler test confirms controller-driven
sprite motion via the raster IRQ, the milestone branch should be ready to merge
into `main`.
