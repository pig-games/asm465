//! Decimal (BCD) mode tests for `ADC` and `SBC` on NMOS 6502 semantics.
//!
//! Overflow (`V`) is taken from the binary operation; BCD fixups determine the
//! adjusted result and carry/no-borrow (`C`).

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
        let op = cpu.bus.read(cpu.pc);
        if op == 0x00 {
            cpu.step();
            break;
        }
        cpu.step();
    }
}

#[test]
/// ```asm
/// SED
/// LDA #$15
/// ADC #$27     ; => $42
/// BRK
/// ```
fn adc_bcd_simple() {
    // SED ; LDA #$15 ; ADC #$27 ; BRK  => 0x15 + 0x27 = 0x42 (BCD)
    let code = [0xF8, 0xA9, 0x15, 0x69, 0x27, 0x00];
    let mut cpu = cpu_with_program(&code, 0x8000);
    run_until_brk(&mut cpu, 16);
    assert_eq!(cpu.a, 0x42);
    assert!(!cpu.p.contains(P::Z));
    assert!(!cpu.p.contains(P::N));
    assert!(!cpu.p.contains(P::C));
}

#[test]
/// ```asm
/// SED
/// LDA #$55
/// ADC #$55     ; => $10, C=1
/// BRK
/// ```
fn adc_bcd_with_carry() {
    // 0x55 + 0x55 = 0x110 (BCD 0x10 with carry=1)
    let code = [0xF8, 0xA9, 0x55, 0x69, 0x55, 0x00];
    let mut cpu = cpu_with_program(&code, 0x8000);
    run_until_brk(&mut cpu, 16);
    assert_eq!(cpu.a, 0x10);
    assert!(cpu.p.contains(P::C));
}

#[test]
/// ```asm
/// SED
/// LDA #$19
/// ADC #$01     ; => $20
/// BRK
/// ```
fn adc_bcd_cross_digit() {
    // 0x19 + 0x01 = 0x20 (carry 0)
    let code = [0xF8, 0xA9, 0x19, 0x69, 0x01, 0x00];
    let mut cpu = cpu_with_program(&code, 0x8000);
    run_until_brk(&mut cpu, 16);
    assert_eq!(cpu.a, 0x20);
    assert!(!cpu.p.contains(P::C));
}

#[test]
/// ```asm
/// SED
/// SEC
/// LDA #$50
/// SBC #$25     ; => $25, C=1 (no borrow)
/// BRK
/// ```
fn sbc_bcd_simple_no_borrow() {
    // SEC first, then 0x50 - 0x25 = 0x25 (no borrow => C=1)
    let code = [0xF8, 0x38, 0xA9, 0x50, 0xE9, 0x25, 0x00];
    let mut cpu = cpu_with_program(&code, 0x8000);
    run_until_brk(&mut cpu, 16);
    assert_eq!(cpu.a, 0x25);
    assert!(cpu.p.contains(P::C));
}

#[test]
/// ```asm
/// SED
/// SEC
/// LDA #$30
/// SBC #$45     ; => $85, C=0 (borrow)
/// BRK
/// ```
fn sbc_bcd_with_borrow() {
    // SEC first, then 0x30 - 0x45 = -0x15 => 0x85 with borrow => C=0
    let code = [0xF8, 0x38, 0xA9, 0x30, 0xE9, 0x45, 0x00];
    let mut cpu = cpu_with_program(&code, 0x8000);
    run_until_brk(&mut cpu, 16);
    assert_eq!(cpu.a, 0x85);
    assert!(!cpu.p.contains(P::C));
}

#[test]
/// ```asm
/// SED
/// SEC
/// LDA #$50
/// SBC #$50     ; => $00, Z=1, C=1
/// BRK
/// ```
fn sbc_bcd_carry_in_effect() {
    // SEC to provide borrow-in clear (C=1): 0x50 - 0x50 = 0x00, C=1
    let code = [0xF8, 0x38, 0xA9, 0x50, 0xE9, 0x50, 0x00];
    let mut cpu = cpu_with_program(&code, 0x8000);
    run_until_brk(&mut cpu, 16);
    assert_eq!(cpu.a, 0x00);
    assert!(cpu.p.contains(P::C));
    assert!(cpu.p.contains(P::Z));
}
