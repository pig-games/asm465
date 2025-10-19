//! cross465 Bus: 64KB RAM + pluggable personalities.
//!
//! This module implements the system bus for a 6502-based virtual machine.
//! It contains:
//!
//! * [`Memory`] — a 64 KB RAM abstraction shared by all MMIO devices.
//! * [`MmioDevice`] — a trait for memory-mapped I/O peripherals.
//! * [`Bus`] — the actual 6502 bus, with RAM and personality-driven MMIO layout.
//! * [`personality`] — descriptors that define which MMIO modules to map and in
//!   which address ranges.
//!
//! ## Personalities & Memory Map
//!
//! By default [`Bus::new`] loads the [`personality::MODERN_RETRO`] descriptor,
//! which maps:
//!
//! ```text
//! $0000–$DFFF   RAM (read/write)
//! $DF00–$DF1F   Console MMIO (text output)
//! $DF20–$DF21   Display MMIO (border/background colours)
//! $DF30–$DF37   Sprite MMIO (sprite slots)
//! $DF38–$FFFB   RAM (read/write)
//! $FFFC–$FFFD   Reset vector
//! $FFFE–$FFFF   NMI vector
//! ```
//!
//! You can construct a bus with a different mapping by calling
//! [`Bus::with_personality`] and supplying a custom descriptor. Each
//! `PersonalityMmio` entry provides a range and a factory for the MMIO module so
//! applications can plug in alternative devices (e.g. different sprite/display
//! implementations) without modifying the bus internals.
//!
//! The [`Bus`] forwards reads/writes in MMIO ranges to their device instead of RAM.

pub mod console_mmio; // expose console device as bus::console_mmio::*
pub mod display_mmio; // expose display device as bus::display_mmio::*
pub mod personality; // personas describing MMIO layouts
pub mod sprite_mmio; // expose sprite device as bus::sprite_mmio::*
pub mod utils; // expose helpers as bus::utils::*

pub use utils::{cmb_color_to_ansi, petscii_to_unicode, screen_to_petscii, unicode_to_screen}; // convenience re-export

use std::any::Any;
use std::ops::RangeInclusive;
use std::sync::{Arc, Mutex};

use console_mmio::ConsoleMmio;
use display_mmio::DisplayMmio;
use personality::{Personality, PersonalityMmioKind, MODERN_RETRO};
use sprite_mmio::SpriteMmio;

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
    pub fn new() -> Self {
        Self { data: [0; 0x10000] }
    }

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
    pub fn read(&self, addr: u16) -> u8 {
        self.data[addr as usize]
    }

    /// Write a byte to memory at `addr`.
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
    /// Downcast helper for `&dyn MmioDevice`.
    #[inline]
    pub fn as_any(&self) -> &dyn Any {
        self
    }

    /// Downcast helper for `&mut dyn MmioDevice`.
    #[inline]
    pub fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
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
    personality: &'static Personality,
    mmio: Vec<MappedDevice>,
}

struct MappedDevice {
    range: RangeInclusive<u16>,
    device: Box<dyn MmioDevice>,
    kind: Option<PersonalityMmioKind>,
}

impl Bus {
    /// Active personality descriptor backing this bus.
    pub fn personality(&self) -> &'static Personality {
        self.personality
    }

    /// Create a RAM-only bus and map a default [`ConsoleMmio`] at `$DF00–$DF1F`.
    pub fn new() -> Self {
        Self::with_personality(&MODERN_RETRO)
    }

    /// Construct the bus using the specified personality.
    pub fn with_personality(personality: &'static Personality) -> Self {
        let ram = Arc::new(Mutex::new(Memory::new()));
        let mut bus = Self {
            ram: ram.clone(),
            personality,
            mmio: Vec::new(),
        };
        for mapping in personality.mmio {
            let device = (mapping.create)(&ram);
            bus.map_mmio_internal(mapping.range.clone(), device, Some(mapping.kind));
        }
        bus
    }

    /// Map an MMIO device to a specific address range (inclusive).
    pub fn map_mmio(&mut self, range: RangeInclusive<u16>, dev: Box<dyn MmioDevice>) {
        self.map_mmio_internal(range, dev, None);
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
        if let Some(dev) = self.find_mmio(addr) {
            return dev.read(addr);
        }
        let mem = self.ram.lock().unwrap();
        mem.read(addr)
    }

    /// Write a byte to the bus (MMIO devices intercept their ranges).
    pub fn write(&mut self, addr: u16, value: u8) {
        if let Some(dev) = self.find_mmio(addr) {
            dev.write(addr, value);
            return;
        }
        let mut mem = self.ram.lock().unwrap();
        mem.write(addr, value);
    }

    /// Optional timing hook (no-op). Can be overridden to simulate cycles.
    pub fn tick(&mut self, _cycles: u32) {}

    /// Search for an MMIO device covering `addr`.
    pub fn find_mmio(&mut self, addr: u16) -> Option<&mut dyn MmioDevice> {
        for mapped in self.mmio.iter_mut() {
            if mapped.range.contains(&addr) {
                return Some(mapped.device.as_mut());
            }
        }
        None
    }

    /// Mutable access to the underlying RAM (returns a lock guard).
    pub fn mem_mut(&self) -> std::sync::MutexGuard<'_, Memory> {
        self.ram.lock().unwrap()
    }

    // ===== Helpers for tests / host integration =====

    /// Enable/disable PETSCII translation on the default console device.
    pub fn with_console_petscii(mut self, petscii: bool) -> Self {
        for mapped in self.mmio.iter_mut() {
            if mapped.kind == Some(PersonalityMmioKind::Console) {
                if let Some(c) = mapped.device.as_any_mut().downcast_mut::<ConsoleMmio>() {
                    c.petscii_mode = petscii;
                }
            }
        }
        self
    }

    /// Expose the console device's shared output buffer.
    pub fn console_output_handle(&self) -> Option<Arc<Mutex<console_mmio::ConsoleOutput>>> {
        for mapped in self.mmio.iter() {
            if mapped.kind == Some(PersonalityMmioKind::Console) {
                if let Some(c) = mapped.device.as_any().downcast_ref::<ConsoleMmio>() {
                    return Some(c.output());
                }
            }
        }
        None
    }

    /// Expose the display device's shared output buffer (border/background).
    pub fn display_output_handle(&self) -> Option<Arc<Mutex<display_mmio::DisplayOutput>>> {
        for mapped in self.mmio.iter() {
            if mapped.kind == Some(PersonalityMmioKind::Display) {
                if let Some(d) = mapped.device.as_any().downcast_ref::<DisplayMmio>() {
                    return Some(d.output());
                }
            }
        }
        None
    }

    /// Expose the sprite device's shared output buffer.
    pub fn sprite_output_handle(&self) -> Option<Arc<Mutex<sprite_mmio::SpriteOutput>>> {
        for mapped in self.mmio.iter() {
            if mapped.kind == Some(PersonalityMmioKind::Sprite) {
                if let Some(s) = mapped.device.as_any().downcast_ref::<SpriteMmio>() {
                    return Some(s.output());
                }
            }
        }
        None
    }

    /// Read back the console buffer (if present) as a snapshot string.
    pub fn console_buffer(&self) -> Option<String> {
        self.console_output_handle()
            .and_then(|handle| handle.lock().ok().map(|guard| guard.to_plain_string()))
    }

    /// Clear the console buffer (if present).
    pub fn clear_console_buffer(&mut self) {
        for mapped in self.mmio.iter_mut() {
            if mapped.kind == Some(PersonalityMmioKind::Console) {
                if let Some(c) = mapped.device.as_any_mut().downcast_mut::<ConsoleMmio>() {
                    c.clear();
                }
            }
        }
    }
}

impl Bus {
    fn map_mmio_internal(
        &mut self,
        range: RangeInclusive<u16>,
        dev: Box<dyn MmioDevice>,
        kind: Option<PersonalityMmioKind>,
    ) {
        self.mmio.push(MappedDevice {
            range,
            device: dev,
            kind,
        });
    }
}
