# asm465

Asm465 is an in‑development, native‑first, self‑hosting editor/assembler for modern and expanded 65xx‑based retro computers such as the **Mega65** and **C64‑class machines** (Ultimate64/C64OS/REU/SD2IEC). It also includes a file‑compatible **cross‑development environment** written in Rust, enabling development and testing on modern platforms while maintaining identical source and build results.

The number in the name literally means *for 65 CPUs*, originally inspired by the CPU names 45GS02 and 6502.

## Design goals
1. **Run natively** on Mega65 and other expanded 6502‑based machines.
2. **Support the full 45GS02 instruction set** (including 65CE02) and addressing modes.
3. **UI/Editor**
   - Platform‑specific character modes: 80×50 (Mega65), 40×25 (Ultimate64), higher/dynamic resolutions for crossdev.
   - Purpose‑built for assembly: dense code view, tiled panels, syntax highlighting, autocomplete, multiple open files.
   - Optional per‑line overlays: assembled address, cycle counts (per line/scope), bank/page context.
4. **Modern assembler features**
   - Modules and nested labels with scope/namespace.
   - Rich pseudo‑ops (.if, .for, etc.) and macros.
5. **Background processing**
   - Continuous resolution of symbols, addresses, and cycle counts.
   - On‑the‑fly assembly for live feedback.
6. **File import/export**
   - ASCII ↔ asm465 structured format.
7. **Be fully self‑hosting** as soon as feasible.

The custom file format preserves structural and layout data for fast incremental parsing and native UI fidelity.

## Crossdev architecture
The crossdev version features a **modern native UI** (planned: egui) with retro sensibilities, running a **6502 assembler core** inside a built‑in **6502 simulator**. A platform‑specific **MMIO bus interface** connects the assembler core to the Rust host application, allowing ~80% of assembler logic to be shared across all targets, with ~20% per‑platform wrapper code for I/O, display, and integration. The Rust host application is itself fully platform independent including support for Windows, MacOS, Linux and browser.
At least cross465 started out purely as a way to do this. However, by now it has integrated a game engine framework, which will become a full alternative target for game projects as well, with the same goals as the assembler/editor. It allows for large percentages of game logic code to be reused not only between retro/vintage targets, but also modern hardware/software. This even includes running in a browser.
This all means that the goal of the modern runtime has become broader, now including becoming a modern target for retro games that can share game logic code with retro/vintage targets, while providing modern flourisches to the graphics and sound.

### Viewer configuration & MMIO reference
The desktop/wasm viewer in `crossdev/asm465` exposes several flags for tuning the virtual display:

- `--virtual-width` / `--virtual-height` control the canvas the MMIO sprite coordinates map onto (default `320×240`).
- Border handling: `--force-aspect-ratio`, `--min-border-x`, `--min-border-y`, `--border-color`, `--background-color`, `--resize-window`.
- Sprite mapping: `--sprite-margin-left/right/top/bottom` add off-screen padding (defaults ≈ one sprite width/height each axis).
- Coordinate range: `--sprite-mmio-max-x/y` (set to `0` to auto-derive from virtual size + margins).
- Culling guard rails: `--sprite-max-offscreen-width/height` decide how far sprites can stray before the viewer hides them.
- The guest can override the viewer colours at runtime by writing the graphics
  MMIO border/background registers; the viewer converts the 4-bit palette into
  modern RGB values automatically.
- Personality selection: `--personality <name>` swaps MMIO layouts (e.g.
  `modern-retro`, `c64-compat`); use
  `--list-personalities` to inspect the built-in options.

These flags apply to both the native binary and the wasm wrapper so games/tools can match the behaviour of their target hardware.

#### MMIO modules
The emulated bus dedicates separate windows to console, display, and sprite state:

- **Console MMIO (`$DF00–$DF1F`)** – console text output helpers used by tooling/tests. Key registers: `PUTC` (`$DF00`), `NL` (`$DF01`), `PUTHEX` (`$DF02`), cursor setters (`SETX/SETY/SETLOC`), colour setters (`SETCOL/SETBGCOL`), and pointer helpers (`SETLPTR/SETHPTR/PRINT`).
- **Display MMIO (`$DF20–$DF21`)** – border/background colour registers. The border is visible when the output window has a different aspect ratio than the 'virtual display'. It is also possible to provide a minimum border size, which mimics the borders on many vintage 8 bit systems.
- **Sprite MMIO (`$DF30–$DF37`)** – sprite pipeline state. `SPRSEL` selects the slot, `SPRNUM/SPRANIM` control the asset, `SPRXHI/SPRXLO` and `SPRYHI/SPRYLO` hold 8.8 positions, and `SPRSCL` stores per-axis power-of-two scale factors. The viewer reads this snapshot to position and size each sprite.

## Development philosophy
- **Parity functionality on retro/vintage and modern platforms:** Crossdev accelerates feature development and ensures parity across targets.
- **Consistent workflow:** Edit → assemble → test workflow is the same across Mega65, Ultimate64, and crossdev.
- **Incremental milestones:** Early focus on parity, background processing, and deterministic cross-target builds before full self-hosting.

## Disclaimer
This is a lofty set of goals, but development will be incremental. Knowing the target end state helps avoid early design decisions that would make reaching it harder in the future.
