# 6502 System Architecture: CPU, MMIO, and Interrupts
[→ Platform Examples](Platform_Examples.md) | [→ Sprite Collision and Bevy](Sprite_Collision_and_Bevy.md) | [→ Controller Integration](Controller_Integration.md)

This document explains the overall 6502 architecture focusing on how the CPU and peripherals coordinate through memory-mapped I/O and the interrupt system.

---

## CPU Interrupt Architecture

The MOS 6502 has two main hardware interrupt inputs and one reset line:
- **IRQ (Interrupt Request)** – Maskable, active low.
- **NMI (Non-Maskable Interrupt)** – Edge-triggered, non-maskable.
- **RESET** – System reset.

### Interrupt Vectors
| Interrupt | Address | Description |
|------------|----------|--------------|
| NMI | `$FFFA/FFFB` | Non-maskable interrupt vector |
| RESET | `$FFFC/FFFD` | Reset vector |
| IRQ/BRK | `$FFFE/FFFF` | Maskable interrupt and BRK vector |

When an interrupt occurs:
1. The CPU finishes the current instruction.  
2. Pushes PC and status register to the stack.  
3. Sets the I-flag (disables IRQ).  
4. Fetches the vector address.  
5. Executes ISR, ending with `RTI`.

Because the 6502 only has one IRQ and one NMI line, multiple peripherals must share them.  
Software identifies which device triggered the interrupt via MMIO status registers.

**References**
- [MOS Technology 6502 Programmer’s Manual](https://archive.org/details/MOS6502ProgrammersManual)  
- [Wilson Minesco – 6502 Interrupts Explained](https://wilsonminesco.com/6502interrupts/)

---

## Memory-Mapped I/O (MMIO)

6502-based computers use a unified 64 KB address space. Devices are mapped into memory; the CPU communicates by reading/writing those addresses.

Each peripheral typically provides:
- Status and enable registers for interrupts.
- Control and data registers for I/O.
- Acknowledge/clear bits to reset interrupt state.

If an interrupt is not cleared, the CPU re-enters the ISR immediately, causing an endless IRQ loop.

**References**
- [Western Design Center – 65C02 Programming Manual](https://wdc65xx.com/wdc/documentation/)  
- [Retrocomputing StackExchange – 6502 MMIO Discussion](https://retrocomputing.stackexchange.com/questions/12346/late-1970s-and-6502-chip-facilities-for-operating-systems)

---

## Coordination Between CPU and Peripherals

### Typical Coordination
- **Cycle stealing:** VIC-II (C64) and ANTIC (Atari) borrow CPU cycles for DMA.  
- **Raster interrupts:** triggered on specific scanlines for timed effects.  
- **Timers:** CIA (C64) or POKEY (Atari) chips issue periodic IRQs.  
- **DMA completion:** signals the CPU via IRQ.

**References**
- [8bitworkshop – 6502 CPU Timing](https://8bitworkshop.com/docs/chips/m6502/)  
- [Commodore 64 Programmer’s Reference Guide (1982)](https://archive.org/details/Commodore_64_Programmers_Reference_Guide_1982_Commodore)

---

## Practical Takeaways
- Acknowledge interrupts early.  
- Keep ISRs short and deterministic.  
- Chain handlers correctly.  
- Poll MMIO sources by priority.  
- Be aware of DMA contention when timing matters.