use bus::Bus;
use core6502::{Cpu, RunLimit};

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

#[test]
fn run_for_executes_brk_instruction() {
    let mut cpu = cpu_with_program(&[0x00], 0x0200);
    cpu.bus.write(0xFFFE, 0x00);
    cpu.bus.write(0xFFFF, 0x40); // BRK should vector to $4000.

    let outcome = cpu.run_for(10);

    assert_eq!(outcome.limit, RunLimit::Brk);
    assert_eq!(outcome.cycles, 7); // BRK consumes 7 cycles.
    assert_eq!(cpu.pc, 0x4000);
    assert_eq!(cpu.sp, 0xFA);
    let mem = cpu.bus.mem_mut();
    assert_eq!(mem.data[0x01FD], 0x02); // PC high byte pushed first.
    assert_eq!(mem.data[0x01FC], 0x02); // PC low byte pushed second.
    assert_eq!(mem.data[0x01FB], 0x34); // Status with B and U flags set.
}
