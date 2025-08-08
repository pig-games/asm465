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

#[test]
fn lda_and_sta() {
    let code = [0xA9, 0x42, 0x8D, 0x23, 0xC1, 0x00];
    let mut cpu = cpu_with_program(&code, 0x0800);
    cpu.step();
    assert_eq!(cpu.a, 0x42);
    cpu.step();
    assert_eq!(cpu.bus.mem_mut().data[0xC123], 0x42);
}
