//! Personalities define MMIO layouts and default display state for the bus.
//!
//! A personality bundles which MMIO modules should be mapped where, plus any
//! default configuration that higher-level viewers may want to mirror.

use crate::{
    console_mmio::ConsoleMmio, display_mmio::DisplayMmio, sprite_mmio::SpriteMmio, Memory,
    MmioDevice,
};
use core::ops::RangeInclusive;
use std::sync::{Arc, Mutex};

/// Describes the MMIO layout and defaults for a given configuration.
pub struct Personality {
    /// Short identifier for the personality (e.g. `modern-retro`).
    pub name: &'static str,
    /// Human-readable summary that can be surfaced in tooling.
    pub description: &'static str,
    /// MMIO modules that should be mapped when constructing the bus.
    pub mmio: &'static [PersonalityMmio],
    /// Default palette values for the display MMIO (optional metadata).
    pub display: DisplayDefaults,
}

/// Default palette values applied when the display MMIO is initialised.
#[derive(Clone, Copy)]
pub struct DisplayDefaults {
    pub border_color: u8,
    pub background_color: u8,
}

/// Mapping between an address range and a guest-provided MMIO factory.
pub struct PersonalityMmio {
    pub range: RangeInclusive<u16>,
    pub create: fn(&Arc<Mutex<Memory>>) -> Box<dyn MmioDevice>,
    pub kind: PersonalityMmioKind,
}

/// Enumeration of built-in MMIO module kinds.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PersonalityMmioKind {
    Console,
    Display,
    Sprite,
}

/// Built-in personality mirroring the current “modern retro 2D” setup.
pub static MODERN_RETRO: Personality = Personality {
    name: "modern-retro",
    description: "Default cross465 mapping with console, display, and sprite MMIO modules.",
    mmio: &[
        PersonalityMmio {
            range: RangeInclusive::new(0xDF00, 0xDF1F),
            create: |ram| Box::new(ConsoleMmio::new(ram.clone())),
            kind: PersonalityMmioKind::Console,
        },
        PersonalityMmio {
            range: RangeInclusive::new(0xDF20, 0xDF21),
            create: |_| Box::new(DisplayMmio::new()),
            kind: PersonalityMmioKind::Display,
        },
        PersonalityMmio {
            range: RangeInclusive::new(0xDF30, 0xDF37),
            create: |_| Box::new(SpriteMmio::new()),
            kind: PersonalityMmioKind::Sprite,
        },
    ],
    display: DisplayDefaults {
        border_color: 0x00,
        background_color: 0x00,
    },
};
