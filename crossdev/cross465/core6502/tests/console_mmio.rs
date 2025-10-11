use bus::Bus;
use core6502::{Cpu, P};

fn cpu_with_program(code: &[u8], load: u16) -> Cpu {
    let mut bus = Bus::new();
    bus.load(load, code);
    bus.set_reset_vector(load);
    let mut cpu = Cpu::new(bus);
    cpu.reset();
    cpu
}

/// Program (mnemonics):
/// ```asm
/// LDA #'H'      ; A=0x48
/// STA $DF00     ; print 'H'
/// LDA #'I'      ; A=0x49
/// STA $DF00     ; print 'I'
/// LDA #$0D      ; CR (mapped to newline in PETSCII helper)
/// STA $DF00
/// BRK
/// ```
#[test]
fn console_mmio_prints_hi() {
    let code = [
        0xA9, 5, // LDA #'H'
        0x8D, 0x07, 0xDF, // STA $DF00
        0x8D, 0x03, 0xDF, // STA $DF03
        0xA9, b'H', // LDA #'H'
        0x8D, 0x00, 0xDF, // STA $DF00
        0xA9, b'I', // LDA #'I'
        0x8D, 0x00, 0xDF, // STA $DF00
        0xA9, 0x0D, // LDA #$0D (CR)
        0x8D, 0x00, 0xDF, // STA $DF00
        0x00, // BRK
    ];

    let mut cpu = cpu_with_program(&code, 0x8000);
    // Run enough steps to hit BRK
    for _ in 0..32 {
        if cpu.bus.read(cpu.pc) == 0x00 {
            cpu.step();
            break;
        }
        cpu.step();
    }
}
