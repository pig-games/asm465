# 6502 System Architecture: MMIO and Interrupt Coordination

This document provides a deep explanation of how 6502-based systems coordinate CPU and peripheral communication via memory-mapped I/O (MMIO) and interrupts, followed by concrete examples for the **Commodore 64**, **Atari 8-bit series**, and **Apple II**.

---

## General Architecture: CPU, MMIO, and Interrupts

### CPU Interrupt Architecture (the 6502)

The MOS 6502 CPU family provides two main hardware interrupt inputs (plus a reset) and a fixed vector table:

- **IRQ** – Maskable interrupt request (active low)
- **NMI** – Non-maskable interrupt (edge-triggered, active on falling edge)
- **RESET** – Reset input

#### Interrupt Vectors

| Interrupt | Address      | Description                       |
| --------- | ------------ | --------------------------------- |
| NMI       | `$FFFA/FFFB` | Non-maskable interrupt vector     |
| RESET     | `$FFFC/FFFD` | System reset vector               |
| IRQ/BRK   | `$FFFE/FFFF` | Maskable interrupt and BRK vector |

**References:**

- [MOS Technology 6502 Programmer’s Manual](https://archive.org/details/MOS6502ProgrammersManual)
- [Wilson Minesco – 6502 Interrupts Explained](https://wilsonminesco.com/6502interrupts/)

When an interrupt occurs:

1. The CPU finishes the current instruction.
2. Pushes PC and processor status to the stack.
3. Sets the Interrupt Disable (I) flag.
4. Jumps to the appropriate vector.
5. ISR executes and ends with `RTI`, restoring status and PC.

Because the 6502 has only **one IRQ line** and **one NMI line**, multiple interrupt-capable devices share the line. Software must determine which device triggered it.

---

### Memory-Mapped I/O (MMIO) and Peripheral Coordination

The 6502 uses a 64 KB unified address space—no separate I/O space. Peripherals are accessed via memory-mapped registers.

**References:**

- [Western Design Center – 65C02 and 65C816 Programming Manual](https://wdc65xx.com/wdc/documentation/)
- [Retrocomputing StackExchange – 6502 MMIO Discussion](https://retrocomputing.stackexchange.com/questions/12346/late-1970s-and-6502-chip-facilities-for-operating-systems)

Each device provides:

- Status registers for interrupt conditions.
- Enable/mask bits.
- Acknowledge/clear mechanism.

If an interrupt is not acknowledged, the CPU may instantly re-enter the ISR, causing an infinite IRQ loop.

---

### Coordination Between CPU and Peripherals

**Examples:**

- **Cycle stealing:** VIC-II (C64) and ANTIC (Atari) use bus cycles for DMA.
- **Raster interrupts:** Triggered by display scanlines.
- **Timers:** CIA chips on C64 or POKEY on Atari generate timed IRQs.
- **DMA completion:** Signals CPU via IRQ.

**References:**

- [8bitworkshop.com – 6502 CPU Model and Timing](https://8bitworkshop.com/docs/chips/m6502/)
- [Commodore 64 Programmer’s Reference Guide](https://archive.org/details/Commodore_64_Programmers_Reference_Guide_1982_Commodore)

---

## Platform Examples

### Commodore 64

**References:**

- [C64-Wiki: VIC-II Interrupts](https://www.c64-wiki.com/wiki/Raster_interrupt)
- [Developer Rants: C64 Interrupt Examples](https://medium.com/developer-rants/interrupts-on-the-commodore-64-a-very-simple-example-cf1be764715e)

#### A) VIC-II Raster IRQ Example

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

    ; Raster-time effects here

.not_vic:
    pla
    tay
    pla
    tax
    pla
    rti
```

#### B) CIA1 Timer A IRQ Example

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

    ; Periodic work here

.not_ta:
    pla
    tay
    pla
    tax
    pla
    rti
```

---

### Atari 8-bit Series (400/800/XL/XE)

**References:**

- [Atari Archives: ANTIC and Display List Interrupts](https://www.atariarchives.org/dere/chapt05.php)
- [Playermissile.com – DLI Tutorial](https://playermissile.com/dli_tutorial/)
- [Atarimania Technical FAQ: DMA and HALT](https://www.atarimania.com/pgefaq_chapitre.awp?id=14)

#### DLI (Display List Interrupt) Example

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

    lda NMIST               ; acknowledge

    ; per-scanline work here

    pla
    tay
    pla
    tax
    pla
    rti
```

---

### Apple II

**References:**

- [Apple II Reference Manual](https://archive.org/details/appleiireferencemanual)
- [Retrocomputing StackExchange: Apple II Interrupt Design](https://retrocomputing.stackexchange.com/questions/12346/late-1970s-and-6502-chip-facilities-for-operating-systems)
- [AppleFritter: Reset Logic and NMI](https://www.applefritter.com/content/apple-reset-logic-apple-ii-emulation-flaws)

#### Slot IRQ Example

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

    ; Device service code

.not_ours:
    pla
    tay
    pla
    tax
    pla
    rti
```

---

## Comparative Overview

| Feature               | Commodore 64          | Atari 8-bit     | Apple II        |
| --------------------- | --------------------- | --------------- | --------------- |
| **IRQ Line**          | VIC-II, CIA1/2        | POKEY, PIA      | Slot cards      |
| **NMI Line**          | RESTORE key           | ANTIC (DLI/VBI) | RESET/slot      |
| **DMA / Bus Sharing** | VIC-II cycle stealing | ANTIC HALT DMA  | Minimal DMA     |
| **Enable Register**   | `$D01A`, `$DC0D`      | `$D40E`         | Device specific |
| **Status Register**   | `$D019`, `$DC0D`      | `$D40F`         | Device specific |

---

## Practical Takeaways

- Always **acknowledge** interrupts early in the ISR.
- Keep handlers short and deterministic.
- Chain properly with system handlers.
- Poll sources in order of priority.
- Understand DMA/cycle stealing when timing matters.

**Additional References:**

- [Commodore 64 Programmer’s Reference Guide (1982)](https://archive.org/details/Commodore_64_Programmers_Reference_Guide_1982_Commodore)
- [Atari 8-bit Operating System Source Notes](https://www.virtualdub.org/altirra.html)
- [Apple II Technical Notes – Interrupt Handling](https://mirrors.apple2.org.za/Apple%20II%20Documentation%20Project/Books/)

These patterns apply across all 6502-based systems, including your asm465 targets such as the Mega65, Ultimate64, and modern cross-platform environments.

