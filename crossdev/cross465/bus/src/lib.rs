//! cross465 Bus: 64KB RAM + pluggable MMIO devices.
//!
//! - Adds a console MMIO window at `$DF00–$DF1F` so 6502 code can “print” by
//!   `STA $DF00` (char) or strobe `$DF01` (newline), `$DF02` (hex debug).

use std::any::Any;
use std::ops::RangeInclusive;
pub struct Memory {
    pub data: [u8; 0x10000],
}

impl Memory {
    pub fn new() -> Self {
        Self { data: [0; 0x10000] }
    }
    pub fn load(&mut self, start: u16, bytes: &[u8]) {
        let mut a = start as usize;
        for &b in bytes {
            self.data[a & 0xFFFF] = b;
            a += 1;
        }
    }

    pub fn read(&self, addr: u16) -> u8 {
        self.data[addr as usize]
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        self.data[addr as usize] = val;
    }
}

/// MMIO device: host‑side peripherals live behind address ranges on the bus.
///
/// Supertraits:
/// - `Any` to allow downcasting for configuration/inspection in tests.
/// - `Send` so we can move trait objects safely if we ever go multi‑threaded.
pub trait MmioDevice: Any + Send {
    fn read(&mut self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, value: u8);
}

/// Inherent helper on the trait object for downcasting.
impl dyn MmioDevice {
    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Simple console device that mirrors to stdout and also buffers what was printed.
///
/// MMIO window: `$DF00–$DF1F`
/// - `$DF00`: write a byte → prints a character
/// - `$DF01`: write any value → prints newline
/// - `$DF02`: write a byte → prints two hex digits (debug)
pub struct ConsoleMmio {
    pub buffer: String,
    pub petscii_mode: bool,
}

impl ConsoleMmio {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            petscii_mode: false,
        }
    }

    fn push_char(&mut self, b: u8) {
        let ch = if self.petscii_mode {
            petscii_to_unicode(b)
        } else {
            // crude "ASCII-ish" pass-through; map control/high to placeholder
            if (0x20..=0x7E).contains(&b) {
                b as char
            } else if b == 0x0D {
                // treat CR as newline for convenience
                '\n'
            } else {
                '·'
            }
        };
        print!("{ch}");
        self.buffer.push(ch);
    }

    fn newline(&mut self) {
        println!();
        self.buffer.push('\n');
    }

    fn push_hex(&mut self, b: u8) {
        let s = format!("{b:02X}");
        print!("{s}");
        self.buffer.push_str(&s);
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}

impl MmioDevice for ConsoleMmio {
    fn read(&mut self, addr: u16) -> u8 {
        match addr & 0x001F {
            0x00 => 0, // write-only data
            0x01 => 0, // write-only newline strobe
            0x02 => 0, // write-only hex debug
            _ => 0,
        }
    }

    fn write(&mut self, addr: u16, value: u8) {
        match addr & 0x001F {
            0x00 => self.push_char(value),
            0x01 => self.newline(),
            0x02 => self.push_hex(value),
            _ => { /* reserved */ }
        }
    }
}

/// 64KB flat bus with a list of MMIO windows.
pub struct Bus {
    ram: Memory,
    mmio: Vec<(RangeInclusive<u16>, Box<dyn MmioDevice>)>,
}

impl Bus {
    /// Create a RAM‑only bus and map a default console at `$DF00–$DF1F`.
    pub fn new() -> Self {
        let mut bus = Self {
            ram: Memory::new(),
            mmio: Vec::new(),
        };
        bus.map_mmio(0xDF00..=0xDF1F, Box::new(ConsoleMmio::new()));
        bus
    }

    /// Map an MMIO device to an address range (inclusive).
    pub fn map_mmio(&mut self, range: RangeInclusive<u16>, dev: Box<dyn MmioDevice>) {
        self.mmio.push((range, dev));
    }

    /// Write a contiguous blob into RAM starting at `at`.
    pub fn load(&mut self, at: u16, bytes: &[u8]) {
        self.ram.load(at, bytes);
    }

    /// Set the reset vector (`$FFFC/$FFFD`) to `addr`.
    pub fn set_reset_vector(&mut self, addr: u16) {
        self.write(0xFFFC, (addr & 0xFF) as u8);
        self.write(0xFFFD, (addr >> 8) as u8);
    }

    /// Read a byte from the bus (MMIO windows intercept).
    pub fn read(&mut self, addr: u16) -> u8 {
        if let Some(dev) = self.find_mmio(addr) {
            return dev.read(addr);
        }
        self.ram.read(addr)
    }

    /// Write a byte to the bus (MMIO windows intercept).
    pub fn write(&mut self, addr: u16, value: u8) {
        if let Some(dev) = self.find_mmio(addr) {
            dev.write(addr, value);
            return;
        }
        self.ram.write(addr, value);
    }

    /// Optional timing hook (no‑op). Keeps compatibility with older cores/tests.
    pub fn tick(&mut self, _cycles: u32) {}

    fn find_mmio(&mut self, addr: u16) -> Option<&mut dyn MmioDevice> {
        for (range, dev) in self.mmio.iter_mut() {
            if range.contains(&addr) {
                return Some(dev.as_mut());
            }
        }
        None
    }

    pub fn mem_mut(&mut self) -> &mut Memory {
        &mut self.ram
    }

    // ===== Helpers for tests / host integration =====

    /// Enable/disable PETSCII‑ish translation on the default console device.
    pub fn with_console_petscii(mut self, petscii: bool) -> Self {
        for (range, dev) in self.mmio.iter_mut() {
            if *range == (0xDF00..=0xDF1F) {
                if let Some(c) = dev.as_any_mut().downcast_mut::<ConsoleMmio>() {
                    c.petscii_mode = petscii;
                }
            }
        }
        self
    }

    /// Snapshot the current console buffer contents (if the console is mapped).
    pub fn console_buffer(&mut self) -> Option<String> {
        for (range, dev) in self.mmio.iter_mut() {
            if *range == (0xDF00..=0xDF1F) {
                if let Some(c) = dev.as_any_mut().downcast_mut::<ConsoleMmio>() {
                    return Some(c.buffer.clone());
                }
            }
        }
        None
    }

    /// Clear the console buffer (useful in tests).
    pub fn clear_console_buffer(&mut self) {
        for (range, dev) in self.mmio.iter_mut() {
            if *range == (0xDF00..=0xDF1F) {
                if let Some(c) = dev.as_any_mut().downcast_mut::<ConsoleMmio>() {
                    c.clear();
                }
            }
        }
    }
}

// Minimal PETSCII-ish mapper (expand as needed)
// Treats CR ($0D) as newline; otherwise passes ASCII-ish range.
fn petscii_to_unicode(b: u8) -> char {
    match b {
        0x0D => '\n',
        0x20..=0x5A | 0x61..=0x7A => b as char,
        _ => '·',
    }
}