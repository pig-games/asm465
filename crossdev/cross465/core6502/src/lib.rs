//! # core6502 — 6502 CPU core for cross465
//!
//! A table–driven 6502 CPU intended to be embedded in a platform **Bus**.
//!
//! ## Highlights
//! - Implements the **official 151 opcodes** (no illegal opcodes yet).
//! - Accurate addressing modes incl. **JMP (indirect) page-wrap bug**.
//! - **Decimal (BCD) mode** for `ADC` / `SBC` following **NMOS 6502** rules:
//!   Overflow (`V`) is computed from the *binary* operation, then BCD fixups
//!   adjust the result and Carry / no-borrow (`C`).
//! - No heap allocation; all I/O goes through the `Bus` trait/object.
//!
use bitflags::bitflags;
use bus::Bus;

bitflags! {
    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    pub struct P: u8 {
        const C = 1<<0; const Z = 1<<1; const I = 1<<2; const D = 1<<3;
        const B = 1<<4; const U = 1<<5; const V = 1<<6; const N = 1<<7;
    }
}

#[derive(Clone, Copy)]
enum AddrMode {
    Imp,
    Acc,
    Imm,
    Zp,
    ZpX,
    ZpY,
    Abs,
    AbsX,
    AbsY,
    Ind,
    IndX,
    IndY,
    Rel,
}

#[derive(Clone, Copy)]
enum Op {
    ADC,
    AND,
    ASL,
    BCC,
    BCS,
    BEQ,
    BIT,
    BMI,
    BNE,
    BPL,
    BRK,
    BVC,
    BVS,
    CLC,
    CLD,
    CLI,
    CLV,
    CMP,
    CPX,
    CPY,
    DEC,
    DEX,
    DEY,
    EOR,
    INC,
    INX,
    INY,
    JMP,
    JSR,
    LDA,
    LDX,
    LDY,
    LSR,
    NOP,
    ORA,
    PHA,
    PHP,
    PLA,
    PLP,
    ROL,
    ROR,
    RTI,
    RTS,
    SBC,
    SEC,
    SED,
    SEI,
    STA,
    STX,
    STY,
    TAX,
    TAY,
    TSX,
    TXA,
    TXS,
    TYA,
    KIL,
}

#[derive(Clone, Copy)]
struct Entry {
    op: Op,
    mode: AddrMode,
    cycles: u8,
    add_page_cycle: bool,
}

/// 6502 CPU state and execution engine.
///
/// Call [`reset`](Cpu::reset) to initialize `PC` from `$FFFC/$FFFD`. Use
/// [`step`](Cpu::step) to execute a single instruction (approx cycles
/// returned), or [`run_for`](Cpu::run_for) to execute for N cycles.
pub struct Cpu {
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub sp: u8,
    pub pc: u16,
    pub p: P,
    pub cycles: u64,
    pub bus: Bus,
}

/// Reason why a bounded CPU run ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunLimit {
    /// The requested cycle budget was exhausted.
    CycleBudget,
    /// Execution encountered a `BRK` instruction.
    Brk,
}

/// Summary of a bounded CPU run, including the number of cycles executed and
/// why the loop ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RunOutcome {
    pub cycles: u64,
    pub limit: RunLimit,
}

impl Cpu {
    /// Construct a CPU bound to the provided [`Bus`].
    ///
    /// The core immediately owns the bus; call [`reset`](Cpu::reset) before
    /// executing instructions so the program counter is initialised from the
    /// reset vector.
    pub fn new(bus: Bus) -> Self {
        Self {
            a: 0,
            x: 0,
            y: 0,
            sp: 0xFD,
            pc: 0,
            p: P::from_bits_truncate(0x24),
            cycles: 0,
            bus,
        }
    }

    /// Reset registers to a known state and load `PC` from `$FFFC/$FFFD`.
    ///
    /// Mirrors typical 6502 power-on defaults: `SP=0xFD`, `P` has the unused
    /// bit set (`U`), others cleared except `Z/N` which follow subsequent loads.
    pub fn reset(&mut self) {
        self.sp = 0xFD;
        self.p = P::from_bits_truncate(0x24);
        self.pc = self.read16(0xFFFC);
    }

    #[inline]
    fn read(&mut self, a: u16) -> u8 {
        self.bus.read(a)
    }
    #[inline]
    fn write(&mut self, a: u16, v: u8) {
        self.bus.write(a, v)
    }
    #[inline]
    fn read16(&mut self, a: u16) -> u16 {
        let lo = self.read(a) as u16;
        let hi = self.read(a.wrapping_add(1)) as u16;
        (hi << 8) | lo
    }

    #[inline]
    /// Read a 16-bit little-endian value with the classic **JMP (indirect)**
    /// **page-wrap bug**: when the low byte is at `$xxFF`, the high byte is
    /// read from `$xx00` rather than `$xy00`.
    fn read16_bug(&mut self, a: u16) -> u16 {
        let lo = self.read(a) as u16;
        let hi = self.read((a & 0xFF00) | ((a + 1) & 0x00FF)) as u16;
        (hi << 8) | lo
    }
    #[inline]
    fn push(&mut self, v: u8) {
        let a = 0x0100u16 | self.sp as u16;
        self.write(a, v);
        self.sp = self.sp.wrapping_sub(1);
    }
    #[inline]
    fn pop(&mut self) -> u8 {
        self.sp = self.sp.wrapping_add(1);
        let a = 0x0100u16 | self.sp as u16;
        self.read(a)
    }
    #[inline]
    fn set_zn(&mut self, v: u8) {
        self.p.set(P::Z, v == 0);
        self.p.set(P::N, v & 0x80 != 0);
    }

    /// Compute the effective address for an instruction's addressing mode.
    ///
    /// Returns `(address, page_crossed)`:
    /// - `address` is the resolved 16‑bit effective address (or a placeholder like `0`
    ///   for implied/accumulator where no memory address is used).
    /// - `page_crossed` is `true` when the effective address calculation crosses a
    ///   page boundary (i.e., the high byte changes). Certain indexed modes use this
    ///   to charge an extra cycle.
    ///
    /// # Addressing modes
    ///
    /// | Mode     | Meaning                                                                 | Notes                                                                                      | Page-cross? |
    /// |----------|-------------------------------------------------------------------------|--------------------------------------------------------------------------------------------|-------------|
    /// | `Imp`    | Implied (no operand)                                                    | e.g. `CLC`, `SEI`. No memory address is needed.                                            | `false`     |
    /// | `Acc`    | Accumulator                                                             | e.g. `ASL A`, operand is `A` (not memory).                                                 | `false`     |
    /// | `Imm`    | Immediate                                                               | Operand byte follows the opcode. We return the address of that literal.                    | `false`     |
    /// | `Zp`     | Zero Page                                                               | 8‑bit address; uses `$00xx`.                                                               | `false`     |
    /// | `ZpX`    | Zero Page, X-indexed                                                    | `(zp + X) & 0xFF` (wraps within zero page).                                                | `false`     |
    /// | `ZpY`    | Zero Page, Y-indexed                                                    | `(zp + Y) & 0xFF` (wraps within zero page).                                                | `false`     |
    /// | `Abs`    | Absolute                                                                | 16‑bit address from the stream (little‑endian).                                            | `false`     |
    /// | `AbsX`   | Absolute, X-indexed                                                     | `base + X`; sets page‑cross flag if high byte changes.                                     | maybe       |
    /// | `AbsY`   | Absolute, Y-indexed                                                     | `base + Y`; sets page‑cross flag if high byte changes.                                     | maybe       |
    /// | `Ind`    | Indirect                                                                | Used by `JMP ($addr)`. Reads 16‑bit pointer, then target via `read16_bug` (wrap bug).      | `false`     |
    /// | `IndX`   | Indexed Indirect, X (a.k.a. `(zp,X)`)                                   | Add X to zero‑page pointer byte (wrap), then read 16‑bit target from ZP.                   | `false`     |
    /// | `IndY`   | Indirect Indexed, Y (a.k.a. `(zp),Y`)                                   | Read 16‑bit base from ZP, then add Y; sets page‑cross if high byte changes.                | maybe       |
    /// | `Rel`    | Relative (branches)                                                     | Returns sign‑extended 8‑bit offset; branch code adds it to PC and sets page‑cross there.   | `false`     |
    fn addr(&mut self, mode: AddrMode) -> (u16, bool) {
        use AddrMode::*;
        match mode {
            // Implied / Accumulator: no memory operand.
            Imp | Acc => (0, false),

            // Immediate: return the address of the literal byte that follows the opcode.
            Imm => {
                let a = self.pc;
                self.pc = self.pc.wrapping_add(1);
                (a, false)
            }

            // Zero Page: fetch 8-bit address and use it as $00xx.
            Zp => {
                let a = self.read(self.pc) as u16;
                self.pc = self.pc.wrapping_add(1);
                (a, false)
            }

            // Zero Page,X: add X with wrap in zero page.
            ZpX => {
                let a = self.read(self.pc).wrapping_add(self.x) as u16;
                self.pc = self.pc.wrapping_add(1);
                (a, false)
            }

            // Zero Page,Y: add Y with wrap in zero page (used by a few ops like LDX).
            ZpY => {
                let a = self.read(self.pc).wrapping_add(self.y) as u16;
                self.pc = self.pc.wrapping_add(1);
                (a, false)
            }

            // Absolute: 16-bit address from the instruction stream (little-endian).
            Abs => {
                let a = self.read16(self.pc);
                self.pc = self.pc.wrapping_add(2);
                (a, false)
            }

            // Absolute,X: add X; report page-cross if high byte changes.
            AbsX => {
                let base = self.read16(self.pc);
                self.pc = self.pc.wrapping_add(2);
                let a = base.wrapping_add(self.x as u16);
                (a, (base & 0xFF00) != (a & 0xFF00))
            }

            // Absolute,Y: add Y; report page-cross if high byte changes.
            AbsY => {
                let base = self.read16(self.pc);
                self.pc = self.pc.wrapping_add(2);
                let a = base.wrapping_add(self.y as u16);
                (a, (base & 0xFF00) != (a & 0xFF00))
            }

            // Indirect (JMP only): follow 16-bit pointer, honoring 6502 wraparound bug at $xxFF.
            Ind => {
                let ptr = self.read16(self.pc);
                self.pc = self.pc.wrapping_add(2);
                (self.read16_bug(ptr), false)
            }

            // (zp,X): add X to ZP byte (wrap), then read 16-bit target from zero page.
            IndX => {
                let zp = self.read(self.pc).wrapping_add(self.x);
                self.pc = self.pc.wrapping_add(1);
                let lo = self.read(zp as u16) as u16;
                let hi = self.read(zp.wrapping_add(1) as u16) as u16;
                ((hi << 8) | lo, false)
            }

            // (zp),Y: read 16-bit base from ZP, then add Y; report page-cross if high byte changes.
            IndY => {
                let zp = self.read(self.pc);
                self.pc = self.pc.wrapping_add(1);
                let lo = self.read(zp as u16) as u16;
                let hi = self.read(zp.wrapping_add(1) as u16) as u16;
                let base = (hi << 8) | lo;
                let a = base.wrapping_add(self.y as u16);
                (a, (base & 0xFF00) != (a & 0xFF00))
            }

            // Relative: return sign-extended offset; branch logic later adds it to PC and charges cycles.
            Rel => {
                let off = self.read(self.pc) as i8;
                self.pc = self.pc.wrapping_add(1);
                (off as i16 as u16, false)
            }
        }
    }

    fn adc_bin(&mut self, v: u8) {
        let a = self.a;
        let c = if self.p.contains(P::C) { 1 } else { 0 };
        let sum = a as u16 + v as u16 + c as u16;
        let res = (sum & 0xFF) as u8;
        let carry = sum > 0xFF;
        let overflow = ((a ^ res) & (v ^ res) & 0x80) != 0;
        self.a = res;
        self.set_zn(self.a);
        self.p.set(P::C, carry);
        self.p.set(P::V, overflow);
    }

    /// **ADC in decimal (BCD) mode**.
    ///
    /// 1) Compute binary sum (for `V`); 2) Apply BCD low/high nibble fixups;
    /// 3) Set `C` if result exceeded 99 (i.e. decimal carry-out).
    fn adc_bcd(&mut self, v: u8) {
        // Do binary add first, then BCD adjust; keep V from binary add (NMOS behavior).
        let a = self.a;
        let c = if self.p.contains(P::C) { 1 } else { 0 };
        let sum = a as u16 + v as u16 + c as u16;
        let mut res = (sum & 0xFF) as u8;
        let mut carry = sum > 0xFF;
        let overflow = ((a ^ res) & (v ^ res) & 0x80) != 0;

        // BCD adjust
        let lo_nibble_sum = (a & 0x0F) + (v & 0x0F) + c as u8;
        if lo_nibble_sum > 9 {
            res = res.wrapping_add(0x06);
        }
        if sum > 0x99 {
            res = res.wrapping_add(0x60);
            carry = true;
        }
        self.a = res;
        self.set_zn(self.a);
        self.p.set(P::C, carry);
        self.p.set(P::V, overflow);
    }
    fn adc(&mut self, v: u8) {
        if self.p.contains(P::D) {
            self.adc_bcd(v);
        } else {
            self.adc_bin(v);
        }
    }
    fn sbc_bin(&mut self, v: u8) {
        // Implement as A + (~v) + C
        let a = self.a;
        let c = if self.p.contains(P::C) { 1 } else { 0 };
        let sum = a as u16 + (!v) as u16 + c as u16;
        let res = (sum & 0xFF) as u8;
        let carry = sum > 0xFF; // Carry set means no borrow
        let overflow = ((a ^ res) & (a ^ v) & 0x80) != 0;
        self.a = res;
        self.set_zn(self.a);
        self.p.set(P::C, carry);
        self.p.set(P::V, overflow);
    }
    /// **SBC in decimal (BCD) mode**.
    ///
    /// Uses binary path to determine `V`, then performs digit-wise subtraction
    /// with borrow across nibbles. `C=1` indicates **no borrow**.
    fn sbc_bcd(&mut self, v: u8) {
        // Binary path for V, then decimal adjust; Carry indicates no borrow.
        let a = self.a;
        let c = if self.p.contains(P::C) { 1 } else { 0 };
        let bin_sum = a as u16 + (!v) as u16 + c as u16;
        let mut res = (bin_sum & 0xFF) as u8;
        let overflow = ((a ^ res) & (a ^ v) & 0x80) != 0;

        // Decimal subtract per BCD: do digit-wise with borrow.
        let mut lo = (a & 0x0F) as i16 - (v & 0x0F) as i16 - (1 - c) as i16;
        let mut hi = (a >> 4) as i16 - (v >> 4) as i16;
        if lo < 0 {
            lo += 10;
            hi -= 1;
        }
        let mut no_borrow = true;
        if hi < 0 {
            hi += 10;
            no_borrow = false;
        }
        res = ((hi as u8) << 4) | ((lo as u8) & 0x0F);

        self.a = res;
        self.set_zn(self.a);
        self.p.set(P::C, no_borrow);
        self.p.set(P::V, overflow);
    }
    fn sbc(&mut self, v: u8) {
        if self.p.contains(P::D) {
            self.sbc_bcd(v);
        } else {
            self.sbc_bin(v);
        }
    }

    /// Execute one instruction at `PC` and return the (approximate) cycle count.
    ///
    /// Page-cross penalties are accounted for via the decode table’s
    /// `add_page_cycle` bit.
    pub fn step(&mut self) -> u32 {
        use AddrMode::*;
        use Op::*;
        let opcode = self.read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        let e = &TABLE[opcode as usize];
        let mut extra = 0u32;

        match (e.op, e.mode) {
            (BRK, _) => {
                self.pc = self.pc.wrapping_add(1);
                self.push((self.pc >> 8) as u8);
                self.push((self.pc & 0xFF) as u8);
                self.push(self.p.bits() | P::B.bits() | P::U.bits());
                self.p.insert(P::I);
                self.pc = self.read16(0xFFFE);
            }
            (JSR, Abs) => {
                let target = self.read16(self.pc);
                let ret = self.pc.wrapping_add(1);
                self.push((ret >> 8) as u8);
                self.push((ret & 0xFF) as u8);
                self.pc = target;
            }
            (RTS, _) => {
                let lo = self.pop() as u16;
                let hi = self.pop() as u16;
                self.pc = ((hi << 8) | lo).wrapping_add(1);
            }
            (RTI, _) => {
                let st = self.pop();
                self.p = P::from_bits_truncate((st & !P::B.bits()) | P::U.bits());
                let lo = self.pop() as u16;
                let hi = self.pop() as u16;
                self.pc = (hi << 8) | lo;
            }
            (JMP, Abs) => {
                let a = self.read16(self.pc);
                self.pc = a;
            }
            (JMP, Ind) => {
                let ptr = self.read16(self.pc);
                self.pc = self.read16_bug(ptr);
            }

            (PHA, _) => self.push(self.a),
            (PHP, _) => self.push(self.p.bits() | P::B.bits() | P::U.bits()),
            (PLA, _) => {
                let v = self.pop();
                self.a = v;
                self.set_zn(self.a);
            }
            (PLP, _) => {
                let v = self.pop();
                self.p = P::from_bits_truncate((v & !P::B.bits()) | P::U.bits());
            }

            (CLC, _) => self.p.remove(P::C),
            (CLD, _) => self.p.remove(P::D),
            (CLI, _) => self.p.remove(P::I),
            (CLV, _) => self.p.remove(P::V),
            (SEC, _) => self.p.insert(P::C),
            (SED, _) => self.p.insert(P::D),
            (SEI, _) => self.p.insert(P::I),

            (TAX, _) => {
                self.x = self.a;
                self.set_zn(self.x);
            }
            (TAY, _) => {
                self.y = self.a;
                self.set_zn(self.y);
            }
            (TXA, _) => {
                self.a = self.x;
                self.set_zn(self.a);
            }
            (TYA, _) => {
                self.a = self.y;
                self.set_zn(self.a);
            }
            (TSX, _) => {
                self.x = self.sp;
                self.set_zn(self.x);
            }
            (TXS, _) => {
                self.sp = self.x;
            }

            (NOP, _) => {}

            (LDA, mode) => {
                let (addr, pcross) = self.addr(mode);
                let v = self.read(addr);
                self.a = v;
                self.set_zn(self.a);
                if e.add_page_cycle && pcross {
                    extra += 1;
                }
            }
            (LDX, mode) => {
                let (addr, pcross) = self.addr(mode);
                let v = self.read(addr);
                self.x = v;
                self.set_zn(self.x);
                if e.add_page_cycle && pcross {
                    extra += 1;
                }
            }
            (LDY, mode) => {
                let (addr, pcross) = self.addr(mode);
                let v = self.read(addr);
                self.y = v;
                self.set_zn(self.y);
                if e.add_page_cycle && pcross {
                    extra += 1;
                }
            }

            (STA, mode) => {
                let (addr, _) = self.addr(mode);
                self.write(addr, self.a);
            }
            (STX, mode) => {
                let (addr, _) = self.addr(mode);
                self.write(addr, self.x);
            }
            (STY, mode) => {
                let (addr, _) = self.addr(mode);
                self.write(addr, self.y);
            }

            (ADC, mode) => {
                let (addr, pcross) = self.addr(mode);
                let v = self.read(addr);
                self.adc(v);
                if e.add_page_cycle && pcross {
                    extra += 1;
                }
            }
            (SBC, mode) => {
                let (addr, pcross) = self.addr(mode);
                let v = self.read(addr);
                self.sbc(v);
                if e.add_page_cycle && pcross {
                    extra += 1;
                }
            }

            (AND, mode) => {
                let (addr, pcross) = self.addr(mode);
                let v = self.read(addr);
                self.a &= v;
                self.set_zn(self.a);
                if e.add_page_cycle && pcross {
                    extra += 1;
                }
            }
            (ORA, mode) => {
                let (addr, pcross) = self.addr(mode);
                let v = self.read(addr);
                self.a |= v;
                self.set_zn(self.a);
                if e.add_page_cycle && pcross {
                    extra += 1;
                }
            }
            (EOR, mode) => {
                let (addr, pcross) = self.addr(mode);
                let v = self.read(addr);
                self.a ^= v;
                self.set_zn(self.a);
                if e.add_page_cycle && pcross {
                    extra += 1;
                }
            }

            (BIT, mode) => {
                let (addr, _) = self.addr(mode);
                let v = self.read(addr);
                self.p.set(P::Z, (self.a & v) == 0);
                self.p.set(P::V, (v & 0x40) != 0);
                self.p.set(P::N, (v & 0x80) != 0);
            }

            (CMP, mode) => {
                let (addr, pcross) = self.addr(mode);
                let v = self.read(addr);
                let r = self.a.wrapping_sub(v);
                self.p.set(P::C, self.a >= v);
                self.set_zn(r);
                if e.add_page_cycle && pcross {
                    extra += 1;
                }
            }
            (CPX, mode) => {
                let (addr, _) = self.addr(mode);
                let v = self.read(addr);
                let r = self.x.wrapping_sub(v);
                self.p.set(P::C, self.x >= v);
                self.set_zn(r);
            }
            (CPY, mode) => {
                let (addr, _) = self.addr(mode);
                let v = self.read(addr);
                let r = self.y.wrapping_sub(v);
                self.p.set(P::C, self.y >= v);
                self.set_zn(r);
            }

            (INC, mode) => {
                let (addr, _) = self.addr(mode);
                let v = self.read(addr).wrapping_add(1);
                self.write(addr, v);
                self.set_zn(v);
            }
            (INX, _) => {
                self.x = self.x.wrapping_add(1);
                self.set_zn(self.x);
            }
            (INY, _) => {
                self.y = self.y.wrapping_add(1);
                self.set_zn(self.y);
            }
            (DEC, mode) => {
                let (addr, _) = self.addr(mode);
                let v = self.read(addr).wrapping_sub(1);
                self.write(addr, v);
                self.set_zn(v);
            }
            (DEX, _) => {
                self.x = self.x.wrapping_sub(1);
                self.set_zn(self.x);
            }
            (DEY, _) => {
                self.y = self.y.wrapping_sub(1);
                self.set_zn(self.y);
            }

            (ASL, Acc) => {
                let c = (self.a & 0x80) != 0;
                self.a <<= 1;
                self.p.set(P::C, c);
                self.set_zn(self.a);
            }
            (ASL, mode) => {
                let (addr, _) = self.addr(mode);
                let v = self.read(addr);
                let c = (v & 0x80) != 0;
                let r = v << 1;
                self.write(addr, r);
                self.p.set(P::C, c);
                self.set_zn(r);
            }
            (LSR, Acc) => {
                let c = (self.a & 0x01) != 0;
                self.a >>= 1;
                self.p.set(P::C, c);
                self.set_zn(self.a);
            }
            (LSR, mode) => {
                let (addr, _) = self.addr(mode);
                let v = self.read(addr);
                let c = (v & 0x01) != 0;
                let r = v >> 1;
                self.write(addr, r);
                self.p.set(P::C, c);
                self.set_zn(r);
            }
            (ROL, Acc) => {
                let c = self.p.contains(P::C) as u8;
                let newc = (self.a & 0x80) != 0;
                self.a = (self.a << 1) | c;
                self.p.set(P::C, newc);
                self.set_zn(self.a);
            }
            (ROL, mode) => {
                let (addr, _) = self.addr(mode);
                let v = self.read(addr);
                let c = self.p.contains(P::C) as u8;
                let newc = (v & 0x80) != 0;
                let r = (v << 1) | c;
                self.write(addr, r);
                self.p.set(P::C, newc);
                self.set_zn(r);
            }
            (ROR, Acc) => {
                let c = self.p.contains(P::C) as u8;
                let newc = (self.a & 0x01) != 0;
                self.a = (self.a >> 1) | ((c as u8) << 7);
                self.p.set(P::C, newc);
                self.set_zn(self.a);
            }
            (ROR, mode) => {
                let (addr, _) = self.addr(mode);
                let v = self.read(addr);
                let c = self.p.contains(P::C) as u8;
                let newc = (v & 0x01) != 0;
                let r = (v >> 1) | ((c as u8) << 7);
                self.write(addr, r);
                self.p.set(P::C, newc);
                self.set_zn(r);
            }

            (BCC, Rel) => {
                let off = self.addr(Rel).0 as i8;
                if !self.p.contains(P::C) {
                    let old = self.pc;
                    self.pc = self.pc.wrapping_add(off as i16 as u16);
                    if (old & 0xFF00) != (self.pc & 0xFF00) {
                        extra += 1;
                    }
                    extra += 1;
                }
            }
            (BCS, Rel) => {
                let off = self.addr(Rel).0 as i8;
                if self.p.contains(P::C) {
                    let old = self.pc;
                    self.pc = self.pc.wrapping_add(off as i16 as u16);
                    if (old & 0xFF00) != (self.pc & 0xFF00) {
                        extra += 1;
                    }
                    extra += 1;
                }
            }
            (BEQ, Rel) => {
                let off = self.addr(Rel).0 as i8;
                if self.p.contains(P::Z) {
                    let old = self.pc;
                    self.pc = self.pc.wrapping_add(off as i16 as u16);
                    if (old & 0xFF00) != (self.pc & 0xFF00) {
                        extra += 1;
                    }
                    extra += 1;
                }
            }
            (BMI, Rel) => {
                let off = self.addr(Rel).0 as i8;
                if self.p.contains(P::N) {
                    let old = self.pc;
                    self.pc = self.pc.wrapping_add(off as i16 as u16);
                    if (old & 0xFF00) != (self.pc & 0xFF00) {
                        extra += 1;
                    }
                    extra += 1;
                }
            }
            (BNE, Rel) => {
                let off = self.addr(Rel).0 as i8;
                if !self.p.contains(P::Z) {
                    let old = self.pc;
                    self.pc = self.pc.wrapping_add(off as i16 as u16);
                    if (old & 0xFF00) != (self.pc & 0xFF00) {
                        extra += 1;
                    }
                    extra += 1;
                }
            }
            (BPL, Rel) => {
                let off = self.addr(Rel).0 as i8;
                if !self.p.contains(P::N) {
                    let old = self.pc;
                    self.pc = self.pc.wrapping_add(off as i16 as u16);
                    if (old & 0xFF00) != (self.pc & 0xFF00) {
                        extra += 1;
                    }
                    extra += 1;
                }
            }
            (BVC, Rel) => {
                let off = self.addr(Rel).0 as i8;
                if !self.p.contains(P::V) {
                    let old = self.pc;
                    self.pc = self.pc.wrapping_add(off as i16 as u16);
                    if (old & 0xFF00) != (self.pc & 0xFF00) {
                        extra += 1;
                    }
                    extra += 1;
                }
            }
            (BVS, Rel) => {
                let off = self.addr(Rel).0 as i8;
                if self.p.contains(P::V) {
                    let old = self.pc;
                    self.pc = self.pc.wrapping_add(off as i16 as u16);
                    if (old & 0xFF00) != (self.pc & 0xFF00) {
                        extra += 1;
                    }
                    extra += 1;
                }
            }

            (KIL, _) => { /* jam */ }
            _ => todo!(),
        }

        let cyc = (e.cycles as u32) + extra;
        self.bus.tick(cyc);
        cyc
    }

    /// Execute instructions until either `max_cycles` have elapsed or a `BRK`
    /// opcode is encountered at the current program counter.
    ///
    /// Returns a [`RunOutcome`] summarising how many cycles were executed and
    /// which condition caused the loop to finish. The total cycle counter stored
    /// on the CPU is updated as well.
    pub fn run_for(&mut self, max_cycles: u64) -> RunOutcome {
        let mut spent = 0u64;
        let mut limit = RunLimit::CycleBudget;
        let start_cycles = self.cycles;
        while spent < max_cycles {
            let opcode = self.bus.read(self.pc);
            let c = self.step() as u64;
            spent += c;
            self.cycles += c;
            if opcode == 0x00 {
                limit = RunLimit::Brk;
                break;
            }
        }
        RunOutcome {
            cycles: self.cycles.saturating_sub(start_cycles),
            limit,
        }
    }
}

// Build the full official 6502 opcode table (undocumented opcodes default to NOP).
const TABLE: [Entry; 256] = build_table();
const fn e(op: Op, mode: AddrMode, cycles: u8, add_page_cycle: bool) -> Entry {
    Entry {
        op,
        mode,
        cycles,
        add_page_cycle,
    }
}

const fn build_table() -> [Entry; 256] {
    use AddrMode::*;
    use Op::*;
    let N = e(NOP, Imp, 2, false);
    let mut t = [N; 256];

    // 0x00-0x0F
    t[0x00] = e(BRK, Imp, 7, false);
    t[0x01] = e(ORA, IndX, 6, false);
    t[0x05] = e(ORA, Zp, 3, false);
    t[0x06] = e(ASL, Zp, 5, false);
    t[0x08] = e(PHP, Imp, 3, false);
    t[0x09] = e(ORA, Imm, 2, false);
    t[0x0A] = e(ASL, Acc, 2, false);
    t[0x0D] = e(ORA, Abs, 4, false);
    t[0x0E] = e(ASL, Abs, 6, false);

    // 0x10-0x1F
    t[0x10] = e(BPL, Rel, 2, false);
    t[0x11] = e(ORA, IndY, 5, true);
    t[0x15] = e(ORA, ZpX, 4, false);
    t[0x16] = e(ASL, ZpX, 6, false);
    t[0x18] = e(CLC, Imp, 2, false);
    t[0x19] = e(ORA, AbsY, 4, true);
    t[0x1D] = e(ORA, AbsX, 4, true);
    t[0x1E] = e(ASL, AbsX, 7, false);

    // 0x20-0x2F
    t[0x20] = e(JSR, Abs, 6, false);
    t[0x21] = e(AND, IndX, 6, false);
    t[0x24] = e(BIT, Zp, 3, false);
    t[0x25] = e(AND, Zp, 3, false);
    t[0x26] = e(ROL, Zp, 5, false);
    t[0x28] = e(PLP, Imp, 4, false);
    t[0x29] = e(AND, Imm, 2, false);
    t[0x2A] = e(ROL, Acc, 2, false);
    t[0x2C] = e(BIT, Abs, 4, false);
    t[0x2D] = e(AND, Abs, 4, false);
    t[0x2E] = e(ROL, Abs, 6, false);

    // 0x30-0x3F
    t[0x30] = e(BMI, Rel, 2, false);
    t[0x31] = e(AND, IndY, 5, true);
    t[0x35] = e(AND, ZpX, 4, false);
    t[0x36] = e(ROL, ZpX, 6, false);
    t[0x38] = e(SEC, Imp, 2, false);
    t[0x39] = e(AND, AbsY, 4, true);
    t[0x3D] = e(AND, AbsX, 4, true);
    t[0x3E] = e(ROL, AbsX, 7, false);

    // 0x40-0x4F
    t[0x40] = e(RTI, Imp, 6, false);
    t[0x41] = e(EOR, IndX, 6, false);
    t[0x45] = e(EOR, Zp, 3, false);
    t[0x46] = e(LSR, Zp, 5, false);
    t[0x48] = e(PHA, Imp, 3, false);
    t[0x49] = e(EOR, Imm, 2, false);
    t[0x4A] = e(LSR, Acc, 2, false);
    t[0x4C] = e(JMP, Abs, 3, false);
    t[0x4D] = e(EOR, Abs, 4, false);
    t[0x4E] = e(LSR, Abs, 6, false);

    // 0x50-0x5F
    t[0x50] = e(BVC, Rel, 2, false);
    t[0x51] = e(EOR, IndY, 5, true);
    t[0x55] = e(EOR, ZpX, 4, false);
    t[0x56] = e(LSR, ZpX, 6, false);
    t[0x58] = e(CLI, Imp, 2, false);
    t[0x59] = e(EOR, AbsY, 4, true);
    t[0x5D] = e(EOR, AbsX, 4, true);
    t[0x5E] = e(LSR, AbsX, 7, false);

    // 0x60-0x6F
    t[0x60] = e(RTS, Imp, 6, false);
    t[0x61] = e(ADC, IndX, 6, false);
    t[0x65] = e(ADC, Zp, 3, false);
    t[0x66] = e(ROR, Zp, 5, false);
    t[0x68] = e(PLA, Imp, 4, false);
    t[0x69] = e(ADC, Imm, 2, false);
    t[0x6A] = e(ROR, Acc, 2, false);
    t[0x6C] = e(JMP, Ind, 5, false);
    t[0x6D] = e(ADC, Abs, 4, false);
    t[0x6E] = e(ROR, Abs, 6, false);

    // 0x70-0x7F
    t[0x70] = e(BVS, Rel, 2, false);
    t[0x71] = e(ADC, IndY, 5, true);
    t[0x75] = e(ADC, ZpX, 4, false);
    t[0x76] = e(ROR, ZpX, 6, false);
    t[0x78] = e(SEI, Imp, 2, false);
    t[0x79] = e(ADC, AbsY, 4, true);
    t[0x7D] = e(ADC, AbsX, 4, true);
    t[0x7E] = e(ROR, AbsX, 7, false);

    // 0x80-0x8F
    t[0x81] = e(STA, IndX, 6, false);
    t[0x84] = e(STY, Zp, 3, false);
    t[0x85] = e(STA, Zp, 3, false);
    t[0x86] = e(STX, Zp, 3, false);
    t[0x88] = e(DEY, Imp, 2, false);
    t[0x8A] = e(TXA, Imp, 2, false);
    t[0x8C] = e(STY, Abs, 4, false);
    t[0x8D] = e(STA, Abs, 4, false);
    t[0x8E] = e(STX, Abs, 4, false);

    // 0x90-0x9F
    t[0x90] = e(BCC, Rel, 2, false);
    t[0x91] = e(STA, IndY, 6, false);
    t[0x94] = e(STY, ZpX, 4, false);
    t[0x95] = e(STA, ZpX, 4, false);
    t[0x96] = e(STX, ZpY, 4, false);
    t[0x98] = e(TYA, Imp, 2, false);
    t[0x99] = e(STA, AbsY, 5, false);
    t[0x9A] = e(TXS, Imp, 2, false);
    t[0x9D] = e(STA, AbsX, 5, false);

    // 0xA0-0xAF
    t[0xA0] = e(LDY, Imm, 2, false);
    t[0xA1] = e(LDA, IndX, 6, false);
    t[0xA2] = e(LDX, Imm, 2, false);
    t[0xA4] = e(LDY, Zp, 3, false);
    t[0xA5] = e(LDA, Zp, 3, false);
    t[0xA6] = e(LDX, Zp, 3, false);
    t[0xA8] = e(TAY, Imp, 2, false);
    t[0xA9] = e(LDA, Imm, 2, false);
    t[0xAA] = e(TAX, Imp, 2, false);
    t[0xAC] = e(LDY, Abs, 4, false);
    t[0xAD] = e(LDA, Abs, 4, false);
    t[0xAE] = e(LDX, Abs, 4, false);

    // 0xB0-0xBF
    t[0xB0] = e(BCS, Rel, 2, false);
    t[0xB1] = e(LDA, IndY, 5, true);
    t[0xB4] = e(LDY, ZpX, 4, false);
    t[0xB5] = e(LDA, ZpX, 4, false);
    t[0xB6] = e(LDX, ZpY, 4, false);
    t[0xB8] = e(CLV, Imp, 2, false);
    t[0xB9] = e(LDA, AbsY, 4, true);
    t[0xBA] = e(TSX, Imp, 2, false);
    t[0xBC] = e(LDY, AbsX, 4, true);
    t[0xBD] = e(LDA, AbsX, 4, true);
    t[0xBE] = e(LDX, AbsY, 4, true);

    // 0xC0-0xCF
    t[0xC0] = e(CPY, Imm, 2, false);
    t[0xC1] = e(CMP, IndX, 6, false);
    t[0xC4] = e(CPY, Zp, 3, false);
    t[0xC5] = e(CMP, Zp, 3, false);
    t[0xC6] = e(DEC, Zp, 5, false);
    t[0xC8] = e(INY, Imp, 2, false);
    t[0xC9] = e(CMP, Imm, 2, false);
    t[0xCA] = e(DEX, Imp, 2, false);
    t[0xCC] = e(CPY, Abs, 4, false);
    t[0xCD] = e(CMP, Abs, 4, false);
    t[0xCE] = e(DEC, Abs, 6, false);

    // 0xD0-0xDF
    t[0xD0] = e(BNE, Rel, 2, false);
    t[0xD1] = e(CMP, IndY, 5, true);
    t[0xD5] = e(CMP, ZpX, 4, false);
    t[0xD6] = e(DEC, ZpX, 6, false);
    t[0xD8] = e(CLD, Imp, 2, false);
    t[0xD9] = e(CMP, AbsY, 4, true);
    t[0xDD] = e(CMP, AbsX, 4, true);
    t[0xDE] = e(DEC, AbsX, 7, false);

    // 0xE0-0xEF
    t[0xE0] = e(CPX, Imm, 2, false);
    t[0xE1] = e(SBC, IndX, 6, false);
    t[0xE4] = e(CPX, Zp, 3, false);
    t[0xE5] = e(SBC, Zp, 3, false);
    t[0xE6] = e(INC, Zp, 5, false);
    t[0xE8] = e(INX, Imp, 2, false);
    t[0xE9] = e(SBC, Imm, 2, false);
    t[0xEA] = e(NOP, Imp, 2, false);
    t[0xEC] = e(CPX, Abs, 4, false);
    t[0xED] = e(SBC, Abs, 4, false);
    t[0xEE] = e(INC, Abs, 6, false);

    // 0xF0-0xFF
    t[0xF0] = e(BEQ, Rel, 2, false);
    t[0xF1] = e(SBC, IndY, 5, true);
    t[0xF5] = e(SBC, ZpX, 4, false);
    t[0xF6] = e(INC, ZpX, 6, false);
    t[0xF8] = e(SED, Imp, 2, false);
    t[0xF9] = e(SBC, AbsY, 4, true);
    t[0xFD] = e(SBC, AbsX, 4, true);
    t[0xFE] = e(INC, AbsX, 7, false);

    t
}
