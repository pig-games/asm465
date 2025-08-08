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
fn adc_sbc() {
    let code = [0x38, 0xA9, 0x10, 0x69, 0x20, 0xE9, 0x05, 0x00];
    let mut cpu = cpu_with_program(&code, 0x8000);
    run_until_brk(&mut cpu, 32);
    assert_eq!(cpu.a, 0x2B);
}

#[test]
fn and_eor_ora() {
    let code = [0xA9, 0xF0, 0x29, 0x3C, 0x49, 0x0F, 0x09, 0x80, 0x00];
    let mut cpu = cpu_with_program(&code, 0x8000);
    run_until_brk(&mut cpu, 32);
    assert_eq!(cpu.a, 0xBF);
}

#[test]
fn asl_lsr_rol_ror_acc_and_mem() {
    let code = [
        0xA9, 0x40, 0x0A, 0x4A, 0x8D, 0x00, 0xC0, 0x2E, 0x00, 0xC0, 0x6E, 0x00, 0xC0, 0x00,
    ];
    let mut cpu = cpu_with_program(&code, 0x8000);
    run_until_brk(&mut cpu, 32);
    assert_eq!(cpu.bus.mem_mut().data[0xC000], 0x40);
}

#[test]
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
fn cmp_cpx_cpy() {
    let code = [
        0xA9, 0x10, 0xA2, 0x20, 0xA0, 0x10, 0xC9, 0x10, 0xE0, 0x10, 0xC0, 0x0F, 0x00,
    ];
    let mut cpu = cpu_with_program(&code, 0x8000);
    run_until_brk(&mut cpu, 64);
    assert!(cpu.p.contains(P::C));
}

#[test]
fn inc_dec_inx_dex_iny_dey() {
    let code = [
        0xA9, 0x00, 0x8D, 0x00, 0xC2, 0xEE, 0x00, 0xC2, 0xCE, 0x00, 0xC2, 0xE8, 0xCA, 0xC8, 0x88,
        0x00,
    ];
    let mut cpu = cpu_with_program(&code, 0x8000);
    run_until_brk(&mut cpu, 64);
    assert_eq!(cpu.bus.mem_mut().data[0xC200], 0x00);
    assert_eq!(cpu.x, 0x00);
    assert_eq!(cpu.y, 0x00);
}

#[test]
fn jmp_jsr_rts_rti_php_plp() {
    let mut code = vec![0xA9,0xAA, 0x48, 0x68, 0x78, 0x00];
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
    cpu.bus.write(0xFFFE, 0x00);
    cpu.bus.write(0xFFFF, 0x90);
    run_until_brk(&mut cpu, 256);
    assert_eq!(cpu.a, 0xAA);
}

#[test]
fn loads_and_stores_all() {
    let code = [
        0xA9, 0x11, 0xA2, 0x22, 0xA0, 0x33, 0x85, 0x10, 0x86, 0x11, 0x84, 0x12, 0x8D, 0x00, 0xC3,
        0x8E, 0x01, 0xC3, 0x8C, 0x02, 0xC3, 0x00,
    ];
    let mut cpu = cpu_with_program(&code, 0x8000);
    run_until_brk(&mut cpu, 64);
    assert_eq!(cpu.bus.mem_mut().data[0x0010], 0x11);
    assert_eq!(cpu.bus.mem_mut().data[0x0011], 0x22);
    assert_eq!(cpu.bus.mem_mut().data[0x0012], 0x33);
    assert_eq!(cpu.bus.mem_mut().data[0xC300], 0x11);
    assert_eq!(cpu.bus.mem_mut().data[0xC301], 0x22);
    assert_eq!(cpu.bus.mem_mut().data[0xC302], 0x33);
}

#[test]
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
    cpu.bus.mem_mut().data[0x4030] = 0x66;
    cpu.bus.mem_mut().data[0x4004] = 0x77;
    run_until_brk(&mut cpu, 128);
    assert_eq!(cpu.a, 0x66);
}
