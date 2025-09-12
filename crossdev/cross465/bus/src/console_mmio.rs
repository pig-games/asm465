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
use crate::{petscii_to_unicode, screen_to_petscii, cmb_color_to_ansi};
use console::Term;
use std::io::Write;
use console::Color;
use console::style;
use crate::Memory;
use std::sync::{Arc, Mutex};

/// Console MMIO device for host-side text output.
pub struct ConsoleMmio {
    ram: Arc<Mutex<Memory>>,
    /// Accumulates printed output for inspection (tests, tooling, etc.).
    pub term: Term,
    /// If `true`, interpret bytes using a PETSCII‑ish mapping; otherwise a
    /// simple ASCII‑ish pass‑through is used.
    pub petscii_mode: bool,
    pub x: u8,
    pub y: u8,
    pub color: u8,
    pub bg_color: u8,
    pub lptr: u8,
    pub hptr: u8,
    pub plength: u8
}

impl ConsoleMmio {
    /// Create a new console device with an empty buffer and ASCII‑ish mode.
    pub fn new(ram: Arc<Mutex<Memory>>) -> Self {
        let term = Term::stdout();
        term.style().force_styling(true);
        Self { ram, term: term, petscii_mode: true, x:0, y:0, color:7, bg_color:0, lptr:0, hptr:0, plength:0 }
    }

    /// Print a single character byte according to the current mode.
    fn push_char(&mut self, b: u8) {
        let ch = if self.petscii_mode {
            petscii_to_unicode(screen_to_petscii(b))
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
        write!(&self.term, "{}", &format!("{}", style(ch).fg(cmb_color_to_ansi(self.color)).bg(cmb_color_to_ansi(self.bg_color)))).unwrap();
    }

    /// Print a newline (also pushes '\n' to the buffer).
    fn newline(&mut self) {
        self.term.write_line("").unwrap();
    }

    /// Print a byte as two hexadecimal digits (debugging helper).
    fn push_hex(&mut self, b: u8) {
        let s = format!("{b:02X}");
        self.term.write(&s.as_bytes()).unwrap();
    }

    /// Clear the internal output buffer (handy for test setup/teardown).
    pub fn clear(&mut self) {
        self.term.clear_screen().unwrap();
    }

    pub fn set_x(&mut self, b:u8) {
        self.x = b;
    }

    pub fn set_y(&mut self, b:u8) {
        self.y = b;
    }

    pub fn set_location(&mut self) {
        self.term.move_cursor_to(self.x.into(), self.y.into()).unwrap();
    }

    pub fn set_color(&mut self, b:u8) {
        self.color = b;
    }

    pub fn set_bg_color(&mut self, b:u8) {
        self.bg_color = b;
    }

    pub fn set_lptr(&mut self, b:u8) {
        self.lptr = b;
    }

    pub fn set_hptr(&mut self, b:u8) {
        self.hptr = b;
    }

    /// Read a NUL-terminated string from RAM at addr (u16) and print it.
    pub fn print(&mut self, high: u8) {
        // Compose 16-bit pointer from high/low registers
        self.set_hptr(high);
        let mut addr = (((self.hptr as u16) << 8) | (self.lptr as u16)) as u16;
        let mut length = addr;
        loop {
            // limit RAM lock scope so we don't hold an immutable borrow across
            // the mutable self.push_char(...) call
            let b = {
                let mem = self.ram.lock().unwrap();
                mem.read(addr)
            };
            if b == 0 {
                break;
            }
            match petscii_to_unicode(screen_to_petscii(b)) {
                '!' => {
                    addr = addr.wrapping_add(1);
                    let c = {
                        let mem = self.ram.lock().unwrap();
                        mem.read(addr)
                    };
                    match petscii_to_unicode(screen_to_petscii(c)) {
                        'n' => self.newline(),
                        _ => self.push_char(c)
                    }
                }
                _ => self.push_char(b)
            }
            addr = addr.wrapping_add(1);
        }
        self.set_lptr((addr & 0x00FF) as u8);
        self.set_hptr((addr >> 8) as u8);
        self.plength = ((addr - length) as u8);
        //println!("length: {}", self.plength);
    }
}

impl MmioDevice for ConsoleMmio {
    /// Returns 0 for all addresses; the registers are write‑only in this device.
    fn read(&mut self, addr: u16) -> u8 {
        let val = match addr & 0x001F {
            0x00 | 0x01 | 0x02 => 0, // write-only registers
            0x04 => self.x,
            0x05 => self.y,
            0x07 => self.color,
            0x08 => self.bg_color,
            0x09 => self.lptr,
            0x0a => self.hptr,
            0x0b => self.plength,
            _ => 0,
        };
        //println!("ConsoleMmio: read {:#06x} => {}", addr, val);
        val
    }

    /// Dispatch writes to the appropriate “register”.
    fn write(&mut self, addr: u16, value: u8) {
        match addr & 0x001F {
            0x00 => self.push_char(value),
            0x01 => self.newline(),
            0x02 => self.push_hex(value),
            0x03 => self.clear(),
            0x04 => self.set_x(value),
            0x05 => self.set_y(value),
            0x06 => self.set_location(),
            0x07 => self.set_color(value),
            0x08 => self.set_bg_color(value),
            0x09 => self.set_lptr(value),
            0x0a => self.set_hptr(value),
            0x0b => self.print(value),  // only requires previous call to set_lptr(val), the passed value is the high ptr for the text to be printed.
            _ => { /* reserved for future features (cursor, color, clear, etc.) */ }
        }
    }
}