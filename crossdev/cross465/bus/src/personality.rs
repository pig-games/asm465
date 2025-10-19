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

/// Scaffold personality that loosely mirrors the C64 MMIO layout. It reuses the
/// existing console/display/sprite devices but maps them to C64-flavoured
/// address ranges so software can target familiar offsets while we prototype a
/// fuller compatibility layer.
pub static C64_COMPAT: Personality = Personality {
    name: "c64-compat",
    description:
        "Prototype mapping that places console/display/sprite MMIO near classic C64 ranges.",
    mmio: &[
        PersonalityMmio {
            range: RangeInclusive::new(0xD000, 0xD01F),
            create: |ram| Box::new(ConsoleMmio::new(ram.clone())),
            kind: PersonalityMmioKind::Console,
        },
        PersonalityMmio {
            range: RangeInclusive::new(0xD020, 0xD021),
            create: |_| Box::new(DisplayMmio::new()),
            kind: PersonalityMmioKind::Display,
        },
        PersonalityMmio {
            range: RangeInclusive::new(0xD040, 0xD047),
            create: |_| Box::new(SpriteMmio::new()),
            kind: PersonalityMmioKind::Sprite,
        },
    ],
    display: DisplayDefaults {
        border_color: 0x0E,
        background_color: 0x06,
    },
};

static PERSONALITIES: &[&Personality] = &[&MODERN_RETRO, &C64_COMPAT];

/// Return the built-in personalities.
pub fn all() -> &'static [&'static Personality] {
    PERSONALITIES
}

/// Personality used by default when constructing a [`Bus`](crate::Bus).
pub fn default() -> &'static Personality {
    &MODERN_RETRO
}

/// Look up a personality by `name` (case-sensitive).
pub fn find(name: &str) -> Option<&'static Personality> {
    PERSONALITIES.iter().copied().find(|p| p.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn personalities_are_discoverable() {
        assert!(find("modern-retro").is_some());
        assert!(find("c64-compat").is_some());
        assert!(find("does-not-exist").is_none());
    }

    #[test]
    fn names_are_unique() {
        let mut seen = HashSet::new();
        for persona in all() {
            assert!(
                seen.insert(persona.name),
                "duplicate personality name: {}",
                persona.name
            );
        }
    }
}
