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

The notation is **64tass‑inspired** but not aiming for full compatibility. The custom file format preserves structural and layout data for fast incremental parsing and native UI fidelity.

## Crossdev architecture
The crossdev version features a **modern native UI** (planned: egui) with retro sensibilities, running a **6502 assembler core** inside a built‑in **6502 simulator**. A platform‑specific **MMIO bus interface** connects the assembler core to the Rust host application, allowing ~80% of assembler logic to be shared across all targets, with ~20% per‑platform wrapper code for I/O, display, and integration.

## Development philosophy
- **Native‑first, crossdev‑second:** Crossdev accelerates feature development and ensures parity across targets.
- **Consistent workflow:** Edit → assemble → test workflow is the same across Mega65, Ultimate64, and crossdev.
- **Incremental milestones:** Early focus on parity, background processing, and deterministic cross‑target builds before full self‑hosting.

## Disclaimer
This is a lofty set of goals, but development will be incremental. Knowing the target end state helps avoid early design decisions that would make reaching it harder in the future.

