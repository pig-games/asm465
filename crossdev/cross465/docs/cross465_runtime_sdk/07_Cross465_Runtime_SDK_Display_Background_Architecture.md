# Cross465 Runtime SDK — Display Background & DMA Architecture

## Overview

This document specifies the architecture for the **background, tilemap, and bitmap layer system** within the Cross465 Runtime SDK.  
It extends the **RetroModern2D** display scaffold with full support for:
- **Multi-layer backgrounds and foregrounds** (with parallax),
- **Interspersed sprites** using a shared coordinate system,
- **8-bit personality compatibility** (e.g. C64-style fine/coarse scrolling),
- **Tile→Tile** and **Tile→Bitmap** layer mappings,
- **Generic DMA module** (with REU emulation) for fast memory transfers.

---

## 1. Goals

- Provide a unified display architecture that works seamlessly across modern and legacy personalities.  
- Allow **smooth parallax backgrounds** while maintaining compatibility with classic **tilemap scrolling** behaviour.  
- Support **mapping from 8-bit MMIO tilemaps** to full **32-bit colour bitmaps** on the modern side.  
- Keep the **same coordinate and scaling model** as sprites (8.8 fixed point bus values → float rendering coordinates).  
- Provide a **DMA module** capable of both generic blitting and REU-compatible block transfers.

---

## 2. Display Adapter Architecture

### 2.1 Layer Model

Each display layer (background or foreground) has the following canonical IDs:

| Layer | Description |
|-------|--------------|
| bg0   | Far background |
| bg1   | Mid background |
| bg2   | Near background |
| fg0   | Foreground elements |
| text  | Text or overlay layer |

Each layer can be individually configured as:
- **Tilemap layer** (maps to an 8×8/16×16 tile grid),
- **Bitmap layer** (full per-pixel image),
- **Text layer** (character cells, typically mapped to a font tileset).

### 2.2 Layer Registers (module `display.basic2d`)

| Id                              | Width | Semantics |
|---------------------------------|------:|-----------|
| `L.Mode`                        |  1 B  | `0=tilemap`, `1=bitmap`, `2=text` |
| `L.ScrollX.Lo` / `.Hi`          |  2 B  | 8.8 fixed X scroll |
| `L.ScrollY.Lo` / `.Hi`          |  2 B  | 8.8 fixed Y scroll |
| `L.Parallax`                    |  1 B  | 0–255 = 0.0–1.0 parallax factor |
| `L.TilemapId` / `L.TilesetId`   |  1 B  | Asset identifiers for tileset |
| `L.BitmapId`                    |  1 B  | Asset identifier for bitmap |
| `L.Visible`                     |  1 B  | 0 = hidden, 1 = visible |

These registers are exposed to the CPU personality via MMIO mappings and are bridged by the **DisplayAdapter**.

---

## 3. Coordinate and Scaling System

- **Bus side:** 8.8 fixed point coordinates for scroll and sprite positions.  
- **Renderer side:** floating-point coordinates (converted as `value / 256.0`).  
- **Scaling:** shared coordinate system ensures that sprites and background layers remain aligned across scroll and zoom operations.  
- **Parallax:** applied per-layer via the `Parallax` register (0–255 mapped to 0.0–1.0).

---

## 4. Personality Mapping Modes

### 4.1 Modern-Retro Personality (Range Map)

The modern-retro mapping is a **contiguous range** exposing all background layers through a simple memory block.

#### Example: `modern-retro-range.toml`
```toml
[[map]]
range = "DF60..DFBF"
regs = [
  { at="DF60", id="bg0.Mode" },
  { at="DF61", id="bg0.ScrollX.Lo" }, { at="DF62", id="bg0.ScrollX.Hi" },
  { at="DF63", id="bg0.ScrollY.Lo" }, { at="DF64", id="bg0.ScrollY.Hi" },
  { at="DF65", id="bg0.Parallax" },
  { at="DF66", id="bg0.TilemapId" }, { at="DF67", id="bg0.TilesetId" },
  { at="DF68", id="bg0.BitmapId" }, { at="DF69", id="bg0.Visible" },

  # repeat for bg1, bg2, text, fg0...
]
```

This mapping is ideal for modern runtimes (desktop or WASM) where the CPU has full access to the display memory space.

---

### 4.2 C64-Compatible Personality (Sparse Map)

This mapping preserves **C64-style fine + coarse scrolling** and **tilemap memory layout**, using pointer decoders and scatter bit fields.

#### Key characteristics
- **Fine scroll:** limited to 0–7 pixels (3-bit precision).
- **Coarse scroll:** handled by copying screen RAM blocks.
- **Colour RAM:** ignored by default; optional in extended mode.
- **Tile size:** 8×8.
- **Screen map:** `0400..07E7` (40×25 characters).

#### Example: `c64-compat-sparse.toml`
```toml
# Pointer decoder for screen memory
[pointer_decoder.bg0_map]
source = { table="0400..07E7", kind="tile", entries=1000 }
block  = 1
index  = { from="pointer" }

# Sparse MMIO mapping
[[map]]
regs = [
  # Fine scroll bits
  { at="D016.0:2", id="bg0.ScrollX.Fine" },  # bits 0-2 of $D016
  { at="D011.0:2", id="bg0.ScrollY.Fine" },  # bits 0-2 of $D011

  # Optional parallax repurposed from border color for demo
  { at="D021", id="bg0.Parallax" },

  # Mode select: tilemap (0) or bitmap (1)
  { at="DF70", id="bg0.Mode" },
]

# Optional color RAM (ignored by default)
#[pointer_decoder.color_ram]
#source = { table="D800..DBE7", kind="color", entries=1000 }
```

---

## 5. C64 Fine and Coarse Scroll Handling

### 5.1 Fine Scroll (0–7 pixels)
- Mapped from the lower 3 bits of `$D016` (X) and `$D011` (Y).
- The adapter converts these to a sub-tile fractional offset (0.0–0.875 of tile width/height).
- Passed to the renderer as a fractional scroll delta.

### 5.2 Coarse Scroll (Tile Copy)
- Occurs when fine scroll wraps from 7→0.
- The CPU or DMA module copies screen RAM to shift the visible area.
- The adapter detects this memory copy (via write listeners or DMA completion) and adjusts the **tile origin** or marks the **bitmap layer** dirty.
- On the modern side, the result is a seamless scroll.

### 5.3 Colour RAM
- Ignored by default (modern layers support full RGBA).
- Optional pointer decoder if needed for tint/shadow mapping.

---

## 6. Generic DMA / REU Module

### 6.1 Purpose
A unified DMA system that:
- Accelerates memory copies and tilemap updates,
- Emulates REU block transfers for legacy code,
- Provides a fast blitting mechanism for modern personalities.

### 6.2 DMA Module Registers
| Register        | Description |
|-----------------|--------------|
| `SrcLo/Hi/Bank` | Source address (24-bit) |
| `DstLo/Hi/Bank` | Destination address (24-bit) |
| `LenLo/Hi`      | Transfer length |
| `StrideSrc`     | Stride between source rows (for 2D) |
| `StrideDst`     | Stride between destination rows |
| `Ctrl`          | Control bits (`GO`, `FILL`, `INVERT`, `WRAP`, `IRQEN`, etc.) |
| `IRQStatus`     | Interrupt status flags |
| `Enable`        | DMA enable flag |
| `ChanSel`       | Active channel select |

The module uses a small FIFO command queue to batch transfers and synchronize with the render cycle.

### 6.3 REU Compatibility Personality
A secondary mapping that emulates the Commodore **RAM Expansion Unit** (REU) interface.

#### Example: `c64-reu.toml`
```toml
[[map]]
range = "DF00..DF1F"
regs = [
  { at="DF00", id="dma.Command" },
  { at="DF01", id="dma.Status" },
  { at="DF02", id="dma.Len.Lo" },
  { at="DF03", id="dma.Len.Hi" },
  { at="DF04", id="dma.C64Addr.Lo" },
  { at="DF05", id="dma.C64Addr.Hi" },
  { at="DF06", id="dma.REUAddr.Lo" },
  { at="DF07", id="dma.REUAddr.Mid" },
  { at="DF08", id="dma.REUAddr.Hi" },
  { at="DF09", id="dma.Bank" },
]
```

---

## 7. Adapter Event Flow

1. CPU writes `$D016/$D011` → adapter updates fractional scroll.
2. Fine scroll wraps → game performs coarse copy via DMA or CPU → shadow screen RAM updates.
3. Adapter detects updated screen RAM → updates integral scroll or rebinds tilemap.
4. Display module composites updated layers → modern renderer displays 32-bit colour output.
5. Optional parallax applied per-layer in the same coordinate space as sprites.

---

## 8. Deliverables

| Component | Description |
|------------|-------------|
| `DisplayAdapter` | Bridges bus writes to DisplayModule. Handles fine/coarse scroll, tilemap updates, and bitmap conversion. |
| `DMA Module` | Generic memory copy engine. Supports REU-compatible window. |
| `modern-retro-range.toml` | Contiguous register mapping for modern builds. |
| `c64-compat-sparse.toml` | Sparse mapping with fine/coarse scroll and pointer decoders. |
| `c64-reu.toml` | REU-compatible mapping for DMA commands. |
| `Tests` | Integration tests verifying correct scroll and DMA-based coarse moves. |

---

## 9. Integration Notes

- Works with existing `RetroModern2D` display scaffold and `DisplayModule` trait.
- Fully compatible with current `SpriteAdapter` and coordinate system.
- DMA engine can be extended to service other modules (e.g., sound buffer fill, VRAM copies).
- Colour RAM can be reintroduced via pointer decoders if colour-based gameplay elements require it.

---

## 10. Summary

This design provides a robust and extensible background and tilemap system bridging legacy MMIO models and modern 2D rendering pipelines.  
It maintains full C64-style compatibility for fine and coarse scrolls while enabling 32-bit colour, alpha blending, parallax, and DMA acceleration on modern targets.
