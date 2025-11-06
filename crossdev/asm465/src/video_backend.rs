use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

use bus::adapters::video::{VideoBackend, VideoState};
use bus::mmio::{HookAction, SystemReg};

/// Shared atomics that capture video timing/collision signals for the optional
/// UI overlay widgets.
#[derive(Default)]
pub struct VideoOverlaySignals {
    raster: AtomicU16,
    sprite_collisions: AtomicU8,
    background_collisions: AtomicU8,
    dirty: AtomicBool,
}

impl VideoOverlaySignals {
    /// Construct a new signal block with zeroed state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Store the current raster line and mark the snapshot as dirty.
    pub fn record_raster(&self, value: u16) {
        self.raster.store(value, Ordering::Relaxed);
        self.dirty.store(true, Ordering::Relaxed);
    }

    /// Store the latest sprite collision bits observed by the backend.
    pub fn record_sprite_collisions(&self, value: u8) {
        self.sprite_collisions.store(value, Ordering::Relaxed);
        self.dirty.store(true, Ordering::Relaxed);
    }

    /// Store the latest background collision bits observed by the backend.
    pub fn record_background_collisions(&self, value: u8) {
        self.background_collisions.store(value, Ordering::Relaxed);
        self.dirty.store(true, Ordering::Relaxed);
    }

    /// Produce a snapshot suitable for presenting in the UI.
    pub fn snapshot(&self) -> VideoOverlaySnapshot {
        VideoOverlaySnapshot {
            raster: self.raster.load(Ordering::Relaxed),
            sprite_collisions: self.sprite_collisions.load(Ordering::Relaxed),
            background_collisions: self.background_collisions.load(Ordering::Relaxed),
            //dirty: self.dirty.swap(false, Ordering::Relaxed),
        }
    }
}

/// Plain-data snapshot of the overlay signals read by the UI layer.
#[derive(Clone, Copy)]
pub struct VideoOverlaySnapshot {
    pub raster: u16,
    pub sprite_collisions: u8,
    pub background_collisions: u8,
}

/// Concrete video backend used by the native viewer to cache MMIO state and
/// forward overlay information.
pub struct ModernVideoBackend {
    state: Arc<Mutex<VideoState>>,
    overlay: Arc<VideoOverlaySignals>,
}

impl ModernVideoBackend {
    /// Create a new backend bound to the supplied output handles.
    pub fn new(state: Arc<Mutex<VideoState>>, overlay: Arc<VideoOverlaySignals>) -> Self {
        Self { state, overlay }
    }
}

impl VideoBackend for ModernVideoBackend {
    fn write_register(&self, reg: SystemReg, _cpu_value: u8, module_value: u8) {
        if let Ok(mut state) = self.state.lock() {
            state.registers.insert(reg, module_value);
        }

        match reg {
            SystemReg::RasterLo => {
                self.overlay.record_raster(module_value as u16);
            }
            SystemReg::SpriteCollisions => {
                self.overlay.record_sprite_collisions(module_value);
            }
            SystemReg::BackgroundCollisions => {
                self.overlay.record_background_collisions(module_value);
            }
            _ => {}
        }
    }

    fn scatter_write(
        &self,
        reg: SystemReg,
        cpu_value: u8,
        module_value: u8,
        bit_value: bool,
        source_bit: u8,
        target_bit: u8,
    ) {
        if let Ok(mut state) = self.state.lock() {
            state.registers.insert(reg, module_value);
        }

        // No additional overlay handling for scatter writes yet; extend as needed.
        let _ = (cpu_value, bit_value, source_bit, target_bit);
    }

    fn fanout_write(
        &self,
        reg: SystemReg,
        value: u8,
        source_value: u8,
        source_instance: Option<u8>,
        target_instance: Option<u8>,
    ) {
        if let Ok(mut state) = self.state.lock() {
            state.registers.insert(reg, value);
        }

        let _ = (source_value, source_instance, target_instance);
    }

    fn handle_hook(&self, hook: &str, action: HookAction) {
        if let Ok(mut state) = self.state.lock() {
            state.last_hook = Some((hook.to_string(), action));
        }
    }
}
