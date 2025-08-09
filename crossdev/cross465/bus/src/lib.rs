//! cross465 Bus: 64KB RAM + pluggable MMIO devices.
//!
//! This module provides:
//! - A [`Memory`] abstraction for raw 64KB address space.
//! - A [`Bus`] that manages RAM plus memory-mapped I/O (MMIO) devices.
//! - A [`ConsoleMmio`] device that mimics screen/console output via writes to
//!   `$DF00–$DF1F` in the address space.
//!
//! ## Quick usage example
//! ```no_run
//! use bus::Bus;
//!
//! let mut bus = Bus::new();
//! bus.write(0xDF00, b'H'); // prints "H"
//! bus.write(0xDF01, 0);    // prints newline
//! ```

use std::any::Any;
use std::ops::RangeInclusive;

/// Represents the flat 64KB RAM array of the 6502 address space.
pub struct Memory {
    /// Raw memory bytes for the full 64KB space.
    pub data: [u8; 0x10000],
}

impl Memory {
    /// Create a new zero-initialized memory.
    pub fn new() -> Self {
        Self { data: [0; 0x10000] }
    }

    /// Load a contiguous slice of bytes into memory starting at `start`.
    ///
    /// Wraps at 64KB boundary.
    pub fn load(&mut self, start: u16, bytes: &[u8]) {
        let mut a = start as usize;
        for &b in bytes {
            self.data[a & 0xFFFF] = b;
            a += 1;
        }
    }

    /// Read a single byte from `addr`.
    #[inline]
    pub fn read(&self, addr: u16) -> u8 {
        self.data[addr as usize]
    }

    /// Write a single byte to `addr`.
    #[inline]
    pub fn write(&mut self, addr: u16, val: u8) {
        self.data[addr as usize] = val;
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait for memory-mapped I/O devices that can be attached to the [`Bus`].
///
/// Supertraits:
/// - `Any` to allow downcasting for configuration/inspection in tests.
/// - `Send` so we can safely move trait objects between threads if needed.
pub trait MmioDevice: Any + Send {
    /// Read from the device at the given `addr` (within its mapped range).
    fn read(&mut self, addr: u16) -> u8;
    /// Write to the device at the given `addr` (within its mapped range).
    fn write(&mut self, addr: u16, value: u8);
}

impl dyn MmioDevice {
    /// Downcast helper to get a mutable `Any` reference.
    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Simple console device that mirrors to stdout and buffers output.
///
/// ## MMIO window: `$DF00–$DF1F`
/// - `$DF00`: write a byte → prints a character
/// - `$DF01`: write any value → prints newline
/// - `$DF02`: write a byte → prints two hex digits (debug)
pub struct ConsoleMmio {
    /// Accumulates printed output for inspection.
    pub buffer: String,
    /// If `true`, interpret bytes in PETSCII-ish mode; otherwise ASCII-ish.
    pub petscii_mode: bool,
}

impl ConsoleMmio {
    /// Create a new console device.
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
            if (0x20..=0x7E).contains(&b) {
                b as char
            } else if b == 0x0D {
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

    /// Clear the internal output buffer.
    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}

impl MmioDevice for ConsoleMmio {
    fn read(&mut self, addr: u16) -> u8 {
        match addr & 0x001F {
            0x00 | 0x01 | 0x02 => 0, // write-only registers
            _ => 0,
        }
    }

    fn write(&mut self, addr: u16, value: u8) {
        match addr & 0x001F {
            0x00 => self.push_char(value),
            0x01 => self.newline(),
            0x02 => self.push_hex(value),
            _ => {}
        }
    }
}

/// The main 6502 system bus: RAM plus pluggable MMIO devices.
pub struct Bus {
    ram: Memory,
    mmio: Vec<(RangeInclusive<u16>, Box<dyn MmioDevice>)>,
}

impl Bus {
    /// Create a RAM-only bus and map a default [`ConsoleMmio`] at `$DF00–$DF1F`.
    pub fn new() -> Self {
        let mut bus = Self {
            ram: Memory::new(),
            mmio: Vec::new(),
        };
        bus.map_mmio(0xDF00..=0xDF1F, Box::new(ConsoleMmio::new()));
        bus
    }

    /// Map an MMIO device to a specific address range (inclusive).
    pub fn map_mmio(&mut self, range: RangeInclusive<u16>, dev: Box<dyn MmioDevice>) {
        self.mmio.push((range, dev));
    }

    /// Load a contiguous slice into RAM starting at `at`.
    pub fn load(&mut self, at: u16, bytes: &[u8]) {
        self.ram.load(at, bytes);
    }

    /// Set the reset vector (`$FFFC/$FFFD`) to `addr`.
    pub fn set_reset_vector(&mut self, addr: u16) {
        self.write(0xFFFC, (addr & 0xFF) as u8);
        self.write(0xFFFD, (addr >> 8) as u8);
    }

    /// Read a byte from the bus (MMIO devices intercept their ranges).
    pub fn read(&mut self, addr: u16) -> u8 {
        if let Some(dev) = self.find_mmio(addr) {
            return dev.read(addr);
        }
        self.ram.read(addr)
    }

    /// Write a byte to the bus (MMIO devices intercept their ranges).
    pub fn write(&mut self, addr: u16, value: u8) {
        if let Some(dev) = self.find_mmio(addr) {
            dev.write(addr, value);
            return;
        }
        self.ram.write(addr, value);
    }

    /// Optional timing hook (no-op).
    pub fn tick(&mut self, _cycles: u32) {}

    fn find_mmio(&mut self, addr: u16) -> Option<&mut dyn MmioDevice> {
        for (range, dev) in self.mmio.iter_mut() {
            if range.contains(&addr) {
                return Some(dev.as_mut());
            }
        }
        None
    }

    /// Get mutable access to the underlying [`Memory`].
    pub fn mem_mut(&mut self) -> &mut Memory {
        &mut self.ram
    }

    // ===== Convenience helpers for console device =====

    /// Enable/disable PETSCII translation on the default console.
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

    /// Get the current console buffer contents.
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

    /// Clear the console buffer.
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
fn petscii_to_unicode(b: u8) -> char {
    match b {
        0x0D => '\n',
        0x20..=0x5A | 0x61..=0x7A => b as char,
        _ => '·',
    }
}