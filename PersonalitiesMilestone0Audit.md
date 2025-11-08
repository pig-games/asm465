# Milestone 0 Audit — Current MMIO Layout

Reference notes capturing the existing bus/personality wiring and MMIO register semantics prior to the Personalities v2 migration.

## Bus & Personality Wiring
- `crossdev/cross465/bus/src/lib.rs` owns the 64 KB memory array and a list of `(RangeInclusive<u16>, Box<dyn MmioDevice>)` mappings (`MappedDevice`).
- `Bus::with_personality` takes a `&'static Personality`, iterates `personality.mmio`, invokes each mapping’s factory, and stores the resulting device/range pair. Interrupt defaults are seeded via `personality.interrupts`.
- `crossdev/cross465/bus/src/personality.rs` defines the current descriptor surface:
  - `Personality` fields: `name`, `description`, `mmio` slice, display defaults, interrupt list.
  - `PersonalityMmio` couples a fixed address range with a factory closure returning one of the four built-in MMIO devices.
  - Built-in personalities:
    - `MODERN_RETRO`: contiguous map at `$DF00–$DF46`.
    - `C64_COMPAT`: sparse-style map mirroring C64 ranges (`$D000–$D04E`).
  - No runtime decoding: addresses are hard-coded per range; there is no per-register metadata beyond the device implementations.

## Module Implementations & Registers

### Console MMIO (`crossdev/cross465/bus/src/console_mmio.rs`)
- Mapped range: `$DF00–$DF1F` (mask `addr & 0x001F`).
- Key registers (write path):
  - `$DF00`: push byte as PETSCII/Unicode character.
  - `$DF01`: newline command.
  - `$DF02`: emit byte as hex pair.
  - `$DF03`: clear screen.
  - `$DF04/$DF05`: set cursor `x`/`y`.
  - `$DF06`: apply cursor (uses latched `x`/`y`).
  - `$DF07/$DF08`: set foreground/background colour indices.
  - `$DF09/$DF0A`: low/high pointer for block printing.
  - `$DF0B`: print block starting at pointer, length inferred.
- Read semantics:
  - `$DF00–$DF02`: always `0` (write-only).
  - `$DF04–$DF0B`: expose current cursor, colours, and block-print state; other slots return `0`.
- Host integration maintains an `Arc<Mutex<ConsoleOutput>>` for tooling/tests.

### Display MMIO (`crossdev/cross465/bus/src/display_mmio.rs`)
- Mapped range: `$DF20–$DF21` (mask `addr & 0x0001`).
- Registers:
  - `$DF20`: border colour (`border_color`).
  - `$DF21`: background colour (`background_color`).
- Read/write mirrors the two bytes directly; host snapshot published on write.

### Sprite MMIO (`crossdev/cross465/bus/src/sprite_mmio.rs`)
- Mapped range: `$DF30–$DF37` (mask `addr & 0x0007`), eight slots total (selected via `spr_select`).
- Registers:
  - `$DF30`: `spr_select` — index of active sprite slot (0–7).
  - `$DF31`: sprite asset number for selected slot.
  - `$DF32`: animation index.
  - `$DF33/$DF34`: X position (high/low byte) in 8.8 fixed point.
  - `$DF35/$DF36`: Y position (high/low byte).
  - `$DF37`: packed nibble `scale_x<<4 | scale_y`.
- Reads mirror cached sprite state for currently selected slot; writes mutate slot then publish to shared snapshot.

### System/Interrupt MMIO (`crossdev/cross465/bus/src/system_mmio.rs`)
- Mapped range: `$DF40–$DF46` (mask `addr & 0x0007`), proxying the shared `InterruptController`.
- Registers:
  - `$DF40`: IRQ pending bitmap (low 8 bits).
  - `$DF41`: IRQ enable bitmap (RW; write sets enable bits).
  - `$DF42`: IRQ pending mirror; writing a mask clears pending IRQ bits.
  - `$DF43`: reports lowest numbered enabled + pending IRQ source (or `0xFF` when none).
  - `$DF44`: NMI pending bitmap (low 8 bits).
  - `$DF45`: NMI pending mirror; writing mask clears pending NMIs.
  - `$DF46`: status flags (`bit0` = IRQ line level, `bit1` = NMI line level, `bit2` = NMI edge latched).
- Reads default to `0xFF` for unmapped offsets; writes ignore unsupported addresses.

## Observations
- Address decoding is entirely per-device with constant masks; there is no shared register descriptor or transform layer yet.
- Module factories are plain constructors embedded inside `PersonalityMmio::create`; expanding to dynamic registries will require refactoring this surface.
- Interrupt metadata already exists in `PersonalityInterrupt` but is limited to enable defaults and naming; wiring is manual within `SystemMmio`.
