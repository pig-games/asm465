//! cross465 Bus: 64KB RAM + pluggable MMIO devices.
//!
//! This module implements the system bus for a 6502-based virtual machine.
//! It contains:
//! 
//! * [`Memory`] — a 64 KB RAM abstraction.
//! * [`MmioDevice`] — a trait for memory-mapped I/O peripherals.
//! * [`Bus`] — the actual 6502 bus, with RAM and pluggable MMIO devices.
//! * Default MMIO mapping for [`console_mmio::ConsoleMmio`] at `$DF00–$DF1F`.
//!
//! ## Memory Map
//!
//! The bus exposes the full 16-bit address space (`$0000`–`$FFFF`):
//!
//! ```text
//! $0000–$DFFF   RAM (read/write)
//! $DF00–$DF1F   Console MMIO (default device)
//! $DF20–$FFFB   RAM (read/write)
//! $FFFC–$FFFD   Reset vector
//! $FFFE–$FFFF   NMI vector
//! ```
//!
//! The [`Bus`] forwards reads/writes in MMIO ranges to their device instead of RAM.
//!
//! ## Default Console MMIO
//!
//! The default console device allows the 6502 core to "print" to the host terminal:
//!
//! - `$DF00`: write a byte → prints a character.
//! - `$DF01`: write any value → prints a newline.
//! - `$DF02`: write a byte → prints two hexadecimal digits.
//!
//! This is useful for quick debugging and smoke tests without a GUI.
//!
//! ## Example
//!
//! ```no_run
//! use bus::{Bus, console_mmio::ConsoleMmio};
//!
//! let mut bus = Bus::new();
//! bus.write(0xDF00, b'H');
//! bus.write(0xDF00, b'i');
//! bus.write(0xDF01, 0); // newline
//! ```

pub mod console_mmio;         // expose console device as bus::console_mmio::*
pub mod utils;                // expose helpers as bus::utils::*

pub use utils::{petscii_to_unicode, screen_to_petscii, cmb_color_to_ansi}; // convenience re-export

use std::any::Any;
use std::ops::RangeInclusive;
use std::sync::{Arc, Mutex};

use console_mmio::ConsoleMmio;

/// Represents the flat 64KB RAM array of the 6502 address space.
///
/// [`Memory`] is a thin wrapper around a fixed `[u8; 65536]` array.
/// It provides basic read/write methods and a helper to load binary blobs.
///
/// It does **not** perform any bounds checking beyond masking the address to 16 bits.
/// All bus-level MMIO logic lives in [`Bus`].
pub struct Memory {
    /// Raw 64 KB addressable storage.
    pub data: [u8; 0x10000],
}

impl Memory {
    /// Create a new zero-initialized memory.
    pub fn new() -> Self { Self { data: [0; 0x10000] } }

    /// Load a contiguous slice of bytes into memory starting at `start`.
    ///
    /// The load wraps around at `$FFFF` if the data slice is longer than the
    /// remaining space in memory.
    pub fn load(&mut self, start: u16, bytes: &[u8]) {
        let mut a = start as usize;
        for &b in bytes {
            self.data[a & 0xFFFF] = b;
            a += 1;
        }
    }

    /// Read a byte from memory at `addr`.
    #[inline]
    pub fn read(&self, addr: u16) -> u8 { self.data[addr as usize] }

    /// Write a byte to memory at `addr`.
    #[inline]
    pub fn write(&mut self, addr: u16, val: u8) { self.data[addr as usize] = val; }
}

impl Default for Memory {
    fn default() -> Self { Self::new() }
}

/// Trait for memory-mapped I/O devices.
///
/// Devices must implement:
/// - `read`: called on bus reads from the device's address range.
/// - `write`: called on bus writes to the device's address range.
///
/// ### Trait bounds
/// - `Any`: so devices can be downcast for inspection/configuration in tests.
/// - `Send`: so devices can be moved across threads if needed in the future.
pub trait MmioDevice: Any + Send {
    /// Read a byte from the device.
    fn read(&mut self, addr: u16) -> u8;

    /// Write a byte to the device.
    fn write(&mut self, addr: u16, value: u8);
}

impl dyn MmioDevice {
    /// Downcast helper for `&mut dyn MmioDevice`.
    #[inline]
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}

/// The main 6502 system bus: RAM plus pluggable MMIO devices.
///
/// The bus owns:
/// - A [`Memory`] instance for the 64 KB RAM.
/// - A list of `(address_range, device)` pairs for MMIO mapping.
///
/// Reads/writes to addresses inside a mapped MMIO range are delegated to that device.
/// Everything else goes to RAM.
pub struct Bus {
    ram: Arc<Mutex<Memory>>,
    mmio: Vec<(RangeInclusive<u16>, Box<dyn MmioDevice>)>,
}

impl Bus {
    /// Create a RAM-only bus and map a default [`ConsoleMmio`] at `$DF00–$DF1F`.
    pub fn new() -> Self {
        let ram = Arc::new(Mutex::new(Memory::new()));
        let mut bus = Self { ram: ram.clone(), mmio: Vec::new() };
        bus.map_mmio(0xDF00..=0xDF1F, Box::new(ConsoleMmio::new(ram.clone())));
        bus
    }

    /// Map an MMIO device to a specific address range (inclusive).
    pub fn map_mmio(&mut self, range: RangeInclusive<u16>, dev: Box<dyn MmioDevice>) {
        self.mmio.push((range, dev));
    }

    /// Load a contiguous slice into RAM starting at `at`.
    pub fn load(&mut self, at: u16, bytes: &[u8]) { 
        let mut mem = self.ram.lock().unwrap();
        mem.load(at, bytes);
    }

    /// Set the reset vector (`$FFFC/$FFFD`) to `addr`.
    pub fn set_reset_vector(&mut self, addr: u16) {
        self.write(0xFFFC, (addr & 0xFF) as u8);
        self.write(0xFFFD, (addr >> 8) as u8);
    }

    /// Read a byte from the bus (MMIO devices intercept their ranges).
    pub fn read(&mut self, addr: u16) -> u8 {
        if let Some(dev) = self.find_mmio(addr) { return dev.read(addr); }
        let mem = self.ram.lock().unwrap();
        mem.read(addr)
    }

    /// Write a byte to the bus (MMIO devices intercept their ranges).
    pub fn write(&mut self, addr: u16, value: u8) {
        if let Some(dev) = self.find_mmio(addr) { dev.write(addr, value); return; }
        let mut mem = self.ram.lock().unwrap();
        mem.write(addr, value);
    }

    /// Optional timing hook (no-op). Can be overridden to simulate cycles.
    pub fn tick(&mut self, _cycles: u32) {}

    /// Search for an MMIO device covering `addr`.
    pub fn find_mmio(&mut self, addr: u16) -> Option<&mut dyn MmioDevice> {
        for (range, dev) in self.mmio.iter_mut() {
            if range.contains(&addr) { return Some(dev.as_mut()); }
        }
        None
    }

    /// Mutable access to the underlying RAM (returns a lock guard).
    pub fn mem_mut(&self) -> std::sync::MutexGuard<'_, Memory> { self.ram.lock().unwrap() }

    // ===== Helpers for tests / host integration =====

    /// Enable/disable PETSCII translation on the default console device.
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

    /// Read back the console buffer (if present).
    pub fn console_buffer(&mut self) -> Option<String> {
        for (range, dev) in self.mmio.iter_mut() {
            if *range == (0xDF00..=0xDF1F) {
                if let Some(c) = dev.as_any_mut().downcast_mut::<ConsoleMmio>() {
                    // ConsoleMmio currently doesn't keep a separate buffer string;
                    // return None to indicate "no buffer available".
                    return None;
                }
            }
        }
        None
    }

    /// Clear the console buffer (if present).
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