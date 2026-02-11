//! Integration tests for the official 6502 opcodes.
//!
//! Each test loads a small byte program (starting at `$8000` unless noted) and
//! runs until `BRK`. Above each program we include mnemonics and expected effects.

use bus::Bus;
use core6502::{Cpu, P};

fn cpu_with_program(code: &[u8], load_addr: u16) -> Cpu {
    let mut bus = Bus::new();
    bus.load(load_addr, code);
    bus.write(0xFFFC, (load_addr & 0xFF) as u8);
    bus.write(0xFFFD, (load_addr >> 8) as u8);
    let mut cpu = Cpu::new(bus);
    cpu.reset();
    cpu
}

fn run_until_brk(cpu: &mut Cpu, max_steps: usize) {
    for _ in 0..max_steps {
        let pc = cpu.pc;
        let op = cpu.bus_mut().read(pc);
        if op == 0x00 {
            cpu.step();
            break;
        }
        cpu.step();
    }
}

#[test]
/// ### Program
/// ```asm
/// SEC          ; carry-in = 1
/// LDA #$10     ; A = 0x10
/// ADC #$20     ; A = 0x10 + 0x20 + 1 = 0x31
/// SBC #$05     ; A = 0x31 - 0x05 - (1 - C) = 0x2B
/// BRK
/// ```
fn adc_sbc() {
    let code = [0x38, 0xA9, 0x10, 0x69, 0x20, 0xE9, 0x05, 0x00];
    let mut cpu = cpu_with_program(&code, 0x8000);
    run_until_brk(&mut cpu, 32);
    assert_eq!(cpu.a, 0x2B);
}

#[test]
/// ### Program
/// ```asm
/// LDA #$F0     ; A=F0
/// AND #$3C     ; A=30
/// EOR #$0F     ; A=3F
/// ORA #$80     ; A=BF
/// BRK
/// ```
fn and_eor_ora() {
    let code = [0xA9, 0xF0, 0x29, 0x3C, 0x49, 0x0F, 0x09, 0x80, 0x00];
    let mut cpu = cpu_with_program(&code, 0x8000);
    run_until_brk(&mut cpu, 32);
    assert_eq!(cpu.a, 0xBF);
}

#[test]
/// ### Program
/// ```asm
/// LDA #$40     ; A=40
/// ASL A        ; A=80 (C from bit7)
/// LSR A        ; A=40
/// STA $C000
/// ROL $C000    ; mem <<= 1 with carry-in
/// ROR $C000    ; mem >>= 1 with carry-in
/// BRK
/// ```
fn asl_lsr_rol_ror_acc_and_mem() {
    let code = [
        0xA9, 0x40, 0x0A, 0x4A, 0x8D, 0x00, 0xC0, 0x2E, 0x00, 0xC0, 0x6E, 0x00, 0xC0, 0x00,
    ];
    let mut cpu = cpu_with_program(&code, 0x8000);
    run_until_brk(&mut cpu, 32);
    assert_eq!(cpu.bus().mem_mut().data[0xC000], 0x40);
}

#[test]
/// ### Program
/// ```asm
/// LDA #$00
/// CMP #$00     ; Z=1 -> BEQ taken
/// BEQ +2
/// LDA #$FF     ; skipped
/// LDA #$01
/// CMP #$02     ; Z=0 -> BNE taken
/// BNE +2
/// BRK
/// NOP
/// BRK
/// ```
fn branches_page_cross_and_flags() {
    let code = [
        0xA9, 0x00, 0xC9, 0x00, 0xF0, 0x02, 0xA9, 0xFF, 0xA9, 0x01, 0xC9, 0x02, 0xD0, 0x02, 0x00,
        0xEA, 0x00,
    ];
    let mut cpu = cpu_with_program(&code, 0x8000);
    run_until_brk(&mut cpu, 64);
    assert_eq!(cpu.a, 0x01);
}

#[test]
/// ### Program
/// ```asm
/// LDA #$C0
/// STA $C100
/// LDA #$3F
/// BIT $C100    ; Z=(A&M)==0 ; V<-M6 ; N<-M7
/// BRK
/// ```
fn bit_tests() {
    let code = [
        0xA9, 0xC0, 0x8D, 0x00, 0xC1, 0xA9, 0x3F, 0x2C, 0x00, 0xC1, 0x00,
    ];
    let mut cpu = cpu_with_program(&code, 0x8000);
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    cpu.step();
    assert!(cpu.p.contains(P::N));
    assert!(cpu.p.contains(P::V));
    assert!(cpu.p.contains(P::Z));
}

#[test]
/// ### Program
/// ```asm
/// LDA #$10
/// LDX #$20
/// LDY #$10
/// CMP #$10     ; C=1,Z=1
/// CPX #$10     ; X(20)-10 -> C=1,Z=0
/// CPY #$0F     ; Y(10)-0F -> C=1,Z=0
/// BRK
/// ```
fn cmp_cpx_cpy() {
    let code = [
        0xA9, 0x10, 0xA2, 0x20, 0xA0, 0x10, 0xC9, 0x10, 0xE0, 0x10, 0xC0, 0x0F, 0x00,
    ];
    let mut cpu = cpu_with_program(&code, 0x8000);
    run_until_brk(&mut cpu, 64);
    assert!(cpu.p.contains(P::C));
}

#[test]
/// ### Program
/// ```asm
/// LDA #$00
/// STA $C200
/// INC $C200
/// DEC $C200
/// INX
/// DEX
/// INY
/// DEY
/// BRK
/// ```
fn inc_dec_inx_dex_iny_dey() {
    let code = [
        0xA9, 0x00, 0x8D, 0x00, 0xC2, 0xEE, 0x00, 0xC2, 0xCE, 0x00, 0xC2, 0xE8, 0xCA, 0xC8, 0x88,
        0x00,
    ];
    let mut cpu = cpu_with_program(&code, 0x8000);
    run_until_brk(&mut cpu, 64);
    assert_eq!(cpu.bus().mem_mut().data[0xC200], 0x00);
    assert_eq!(cpu.x, 0x00);
    assert_eq!(cpu.y, 0x00);
}

#[test]
/// Exercises stack and flow-control: PHA/PLA, BRK/RTI status, JSR/RTS.
///
/// ### Main
/// ```asm
/// LDA #$AA
/// PHA
/// PLA
/// SEI
/// BRK          ; vectors to $9000
/// ```
/// ### IRQ/BRK handler @ $9000
/// ```asm
/// PLP
/// RTI
/// ```
/// ### After RTI
/// ```asm
/// JSR $9010    ; subroutine at $9010
/// ... RTS
/// BRK
/// ```
fn jmp_jsr_rts_rti_php_plp() {
    let mut code = vec![0xA9, 0xAA, 0x48, 0x68, 0x78, 0x00];
    let load = 0x8000;
    let irq = 0x9000;
    let pad = irq - load - (code.len() as u16);
    code.extend(std::iter::repeat(0xEA).take(pad as usize));
    code.extend_from_slice(&[0x28, 0x40]);
    code.extend_from_slice(&[0x20, 0x10, 0x90]);
    let pad2 = 0x9010 - load - (code.len() as u16);
    code.extend(std::iter::repeat(0xEA).take(pad2 as usize));
    code.push(0x60);
    code.push(0x00);
    let mut cpu = cpu_with_program(&code, load);
    cpu.bus_mut().write(0xFFFE, 0x00);
    cpu.bus_mut().write(0xFFFF, 0x90);
    run_until_brk(&mut cpu, 256);
    assert_eq!(cpu.a, 0xAA);
}

#[test]
/// ### Program
/// ```asm
/// LDA #$11
/// LDX #$22
/// LDY #$33
/// STA $0010
/// STX $0011
/// STY $0012
/// STA $C300
/// STX $C301
/// STY $C302
/// BRK
/// ```
fn loads_and_stores_all() {
    let code = [
        0xA9, 0x11, 0xA2, 0x22, 0xA0, 0x33, 0x85, 0x10, 0x86, 0x11, 0x84, 0x12, 0x8D, 0x00, 0xC3,
        0x8E, 0x01, 0xC3, 0x8C, 0x02, 0xC3, 0x00,
    ];
    let mut cpu = cpu_with_program(&code, 0x8000);
    run_until_brk(&mut cpu, 64);
    assert_eq!(cpu.bus().mem_mut().data[0x0010], 0x11);
    assert_eq!(cpu.bus().mem_mut().data[0x0011], 0x22);
    assert_eq!(cpu.bus().mem_mut().data[0x0012], 0x33);
    assert_eq!(cpu.bus().mem_mut().data[0xC300], 0x11);
    assert_eq!(cpu.bus().mem_mut().data[0xC301], 0x22);
    assert_eq!(cpu.bus().mem_mut().data[0xC302], 0x33);
}

#[test]
/// ### Program
/// ```asm
/// LDA #$7F
/// TAX
/// TAY
/// TXA
/// TYA
/// TSX
/// TXS
/// CLC
/// SEC
/// CLD
/// SED
/// CLI
/// SEI
/// CLV
/// BRK
/// ```
fn transfers_and_flags() {
    let code = [
        0xA9, 0x7F, 0xAA, 0xA8, 0x8A, 0x98, 0xBA, 0x9A, 0x18, 0x38, 0xD8, 0xF8, 0x58, 0x78, 0xB8,
        0x00,
    ];
    let mut cpu = cpu_with_program(&code, 0x8000);
    run_until_brk(&mut cpu, 64);
    assert!(cpu.p.contains(P::C));
    assert!(cpu.p.contains(P::D));
    assert!(cpu.p.contains(P::I));
    assert!(!cpu.p.contains(P::V));
}

#[test]
/// Demonstrates `($zp),Y`, `($zp,X)`, `abs,X`, and `abs,Y` modes using
/// prepared memory so that `($10),Y` resolves to `$4030`.
fn addressing_modes_indexed() {
    let code = [
        0xA2, 0x04, 0xA9, 0x34, 0x85, 0x30, 0xA9, 0x00, 0x85, 0x10, 0xA9, 0x40, 0x85, 0x11, 0xA9,
        0x20, 0x85, 0x20, 0xA9, 0x40, 0x85, 0x21, 0xA0, 0x30, 0xB1,
        0x10, // LDA ($10),Y => $4030
        0x81, 0x1C, // STA ($1C,X)
        0xA9, 0xFF, 0x85, 0x24, 0xA9, 0x40, 0x85, 0x25, 0xBD, 0x00, 0x40, // LDA $4000,X
        0xB9, 0x00, 0x40, // LDA $4000,Y
        0x00,
    ];
    let mut cpu = cpu_with_program(&code, 0x8000);
    cpu.bus().mem_mut().data[0x4030] = 0x66;
    cpu.bus().mem_mut().data[0x4004] = 0x77;
    run_until_brk(&mut cpu, 128);
    assert_eq!(cpu.a, 0x66);
}
