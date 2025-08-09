//! Console MMIO device for the cross465 Bus.
//!
//! This device lets 6502 code “print” to the host terminal by writing to an
//! MMIO window. It also keeps a buffer so tests can assert the printed output.
//!
//! ## Address window: `$DF00–$DF1F`
//! - `$DF00`: write a byte → prints a character
//! - `$DF01`: write any value → prints newline
//! - `$DF02`: write a byte → prints two hex digits (debug)
//!
//! The device is mapped by default by [`Bus::new`](crate::Bus::new). You can
//! enable a PETSCII‑ish translation mode by calling
//! [`Bus::with_console_petscii`](crate::Bus::with_console_petscii).

use crate::MmioDevice;
use crate::petscii_to_unicode;

/// Console MMIO device for host-side text output.
///
/// The console mirrors to `stdout` **and** stores everything in an internal
/// `buffer` so you can assert on it in tests via
/// [`Bus::console_buffer`](crate::Bus::console_buffer).
pub struct ConsoleMmio {
    /// Accumulates printed output for inspection (tests, tooling, etc.).
    pub buffer: String,
    /// If `true`, interpret bytes using a PETSCII‑ish mapping; otherwise a
    /// simple ASCII‑ish pass‑through is used.
    pub petscii_mode: bool,
}

impl ConsoleMmio {
    /// Create a new console device with an empty buffer and ASCII‑ish mode.
    pub fn new() -> Self {
        Self { buffer: String::new(), petscii_mode: false }
    }

    /// Print a single character byte according to the current mode.
    fn push_char(&mut self, b: u8) {
        let ch = if self.petscii_mode {
            petscii_to_unicode(b)
        } else {
            if (0x20..=0x7E).contains(&b) {
                b as char
            } else if b == 0x0D {
                // Treat CR as newline for convenience.
                '\n'
            } else {
                // Placeholder for non‑printables/high bytes in ASCII-ish mode.
                '·'
            }
        };
        print!("{ch}");
        self.buffer.push(ch);
    }

    /// Print a newline (also pushes '\n' to the buffer).
    fn newline(&mut self) {
        println!();
        self.buffer.push('\n');
    }

    /// Print a byte as two hexadecimal digits (debugging helper).
    fn push_hex(&mut self, b: u8) {
        let s = format!("{b:02X}");
        print!("{s}");
        self.buffer.push_str(&s);
    }

    /// Clear the internal output buffer (handy for test setup/teardown).
    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}

impl MmioDevice for ConsoleMmio {
    /// Returns 0 for all addresses; the registers are write‑only in this device.
    fn read(&mut self, addr: u16) -> u8 {
        match addr & 0x001F {
            0x00 | 0x01 | 0x02 => 0, // write-only registers
            _ => 0,
        }
    }

    /// Dispatch writes to the appropriate “register”.
    fn write(&mut self, addr: u16, value: u8) {
        match addr & 0x001F {
            0x00 => self.push_char(value),
            0x01 => self.newline(),
            0x02 => self.push_hex(value),
            _ => { /* reserved for future features (cursor, color, clear, etc.) */ }
        }
    }
}