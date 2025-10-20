# Platform Examples: Commodore 64, Atari 8-bit, and Apple II

This document describes how the Commodore 64, Atari 8-bit family, and Apple II implement interrupt handling, DMA coordination, and device integration.

---

## Commodore 64

### Overview

- **CPU:** MOS 6510 (6502 variant)
- **Main Interrupt Sources:**
  - VIC-II (video controller)
  - CIA1/CIA2 (I/O, timers)
- **Shared IRQ Line:** VIC-II and CIA chips both assert IRQ to CPU.

### Registers

| Device | Enable  | Status  | Notes                     |
| ------ | ------- | ------- | ------------------------- |
| VIC-II | `$D01A` | `$D019` | Bit 0 = Raster IRQ        |
| CIA1   | `$DC0D` | `$DC0D` | Bit 0 = Timer A underflow |
| CIA2   | `$DD0D` | `$DD0D` | Bit 0 = Timer A underflow |

### VIC-II Raster IRQ Example

```asm
install_vic_irq
    sei
    lda #<vic_irq
    sta $0314
    lda #>vic_irq
    sta $0315

    lda #100
    sta $d012
    lda $d011
    and #%01111111
    sta $d011

    lda #%00000001
    sta $d01a
    lda #%00000001
    sta $d019

    cli
    rts

vic_irq
    pha
    txa
    pha
    tya
    pha

    lda $d019
    and #%00000001
    beq .not_vic
    lda #%00000001
    sta $d019

    ; Raster effect logic

.not_vic:
    pla
    tay
    pla
    tax
    pla
    rti
```

### CIA1 Timer A IRQ Example

```asm
install_cia1_ta_irq
    sei
    lda #<cia_irq
    sta $0314
    lda #>cia_irq
    sta $0315

    lda #<$0400
    sta $dc04
    lda #>$0400
    sta $dc05

    lda #%10000001
    sta $dc0d
    lda #%00010001
    sta $dc0e

    cli
    rts

cia_irq
    pha
    txa
    pha
    tya
    pha

    lda $dc0d
    and #%00000001
    beq .not_ta

    ; Periodic work

.not_ta:
    pla
    tay
    pla
    tax
    pla
    rti
```

---

## Atari 8-bit Family (400/800/XL/XE)

### Overview

- **CPU:** SALLY (6502 variant)
- **Video:** ANTIC chip with DMA and interrupt capabilities.
- **Interrupt Sources:**
  - ANTIC → NMI (Display List Interrupts, Vertical Blanks)
  - POKEY/PIA → IRQ

### NMI Gating Explanation

The 6502 CPU cannot mask NMI itself, but the **ANTIC chip** can control which events are allowed to trigger it using `$D40E` (**NMIEN**):

| Bit | Meaning     | Effect                                         |
| --- | ----------- | ---------------------------------------------- |
| 7   | DLI enable  | Allows Display List Interrupts to assert NMI   |
| 6   | VBI enable  | Allows Vertical Blank Interrupts to assert NMI |
| 5   | Reset clear | Clears pending NMI flags                       |

The CPU always responds to an NMI edge, but ANTIC gates whether it asserts one. Reading `$D40F` (**NMIST**) acknowledges and clears the source.

### DLI Example

```asm
VDSLST  = $0200
NMIEN   = $D40E
NMIST   = $D40F

install_dli
    sei
    lda #<dli_handler
    sta VDSLST
    lda #>dli_handler
    sta VDSLST+1

    lda NMIEN
    ora #%10000000
    sta NMIEN
    cli
    rts

dli_handler
    pha
    txa
    pha
    tya
    pha

    lda NMIST   ; acknowledge

    ; per-scanline work

    pla
    tay
    pla
    tax
    pla
    rti
```

---

## Apple II

### Overview

- **CPU:** MOS 6502
- **Interrupt Sources:** Slot cards (disk, serial, sound, etc.)
- **Interrupt Lines:** Shared IRQ/NMI; dispatched via software vectoring.

### Slot IRQ Example

```asm
BRKVEC      = $03FE
SLOT4_BASE  = $C080
IRQ_STATUS  = SLOT4_BASE
IRQ_CLEAR   = SLOT4_BASE+1

install_slot_irq
    sei
    lda #<slot_irq
    sta BRKVEC
    lda #>slot_irq
    sta BRKVEC+1
    cli
    rts

slot_irq
    pha
    txa
    pha
    tya
    pha

    lda IRQ_STATUS
    and #%00000001
    beq .not_ours

    lda #$00
    sta IRQ_CLEAR

    ; Service code

.not_ours:
    pla
    tay
    pla
    tax
    pla
    rti
```

---

## Comparison Summary

| Feature         | Commodore 64          | Atari 8-bit     | Apple II        |
| --------------- | --------------------- | --------------- | --------------- |
| **IRQ Sources** | VIC-II, CIA1/2        | POKEY, PIA      | Slot cards      |
| **NMI Sources** | RESTORE key           | ANTIC (DLI/VBI) | RESET/slot      |
| **DMA**         | VIC-II cycle stealing | ANTIC HALT DMA  | Minimal         |
| **Enable Reg**  | `$D01A`, `$DC0D`      | `$D40E`         | Device specific |
| **Status Reg**  | `$D019`, `$DC0D`      | `$D40F`         | Device specific |

