//! Personalities define MMIO layouts and default display state for the bus.
//!
//! A personality bundles which MMIO modules should be mapped where, plus any
//! default configuration that higher-level viewers may want to mirror.

use crate::interrupts::InterruptController;
use crate::{
    console_mmio::ConsoleMmio, display_mmio::DisplayMmio, input_mmio::InputMmio, mmio::Module,
    mmio::ModuleKind, sprite_mmio::SpriteMmio, system_mmio::SystemMmio, Memory,
};
use core::ops::RangeInclusive;
use std::sync::{Arc, Mutex};

type ModuleCreateFn = fn(&Arc<Mutex<Memory>>, &Arc<InterruptController>) -> Box<dyn Module>;

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
    /// Interrupt sources exposed by this personality.
    pub interrupts: &'static [PersonalityInterrupt],
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
    pub create: ModuleCreateFn,
    pub kind: ModuleKind,
}

/// Metadata describing an interrupt source published by a personality.
#[derive(Clone, Copy)]
pub struct PersonalityInterrupt {
    /// Stable numeric identifier for the source (used by MMIO registers).
    pub id: u8,
    /// Human-readable name surfaced in tooling.
    pub name: &'static str,
    /// Which CPU line this source drives.
    pub line: InterruptLine,
    /// Trigger semantics for the source (level or edge).
    pub trigger: InterruptTrigger,
    /// Whether the source is enabled by default when the personality loads.
    pub default_enable: bool,
}

/// Interrupt line driven by a personality interrupt.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InterruptLine {
    Irq,
    Nmi,
}

/// Trigger style for a personality interrupt.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InterruptTrigger {
    Level,
    Edge,
}

/// Built-in personality mirroring the current “modern retro 2D” setup.
pub static MODERN_RETRO: Personality = Personality {
    name: "modern-retro",
    description: "Default cross465 mapping with console, display, and sprite MMIO modules.",
    mmio: &[
        PersonalityMmio {
            range: RangeInclusive::new(0xDF00, 0xDF1F),
            create: |ram, _| Box::new(ConsoleMmio::new(ram.clone())),
            kind: ModuleKind::Console,
        },
        PersonalityMmio {
            range: RangeInclusive::new(0xDF20, 0xDF21),
            create: |_, _| Box::new(DisplayMmio::new()),
            kind: ModuleKind::Display,
        },
        PersonalityMmio {
            range: RangeInclusive::new(0xDF30, 0xDF37),
            create: |_, _| Box::new(SpriteMmio::new()),
            kind: ModuleKind::Sprite,
        },
        PersonalityMmio {
            range: RangeInclusive::new(0xDF40, 0xDF46),
            create: |_, controller| Box::new(SystemMmio::new(controller.clone())),
            kind: ModuleKind::System,
        },
        PersonalityMmio {
            range: RangeInclusive::new(0xDF50, 0xDF53),
            create: |_, _| Box::new(InputMmio::new()),
            kind: ModuleKind::Input,
        },
    ],
    display: DisplayDefaults {
        border_color: 0x00,
        background_color: 0x00,
    },
    interrupts: MODERN_RETRO_INTERRUPTS,
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
            create: |ram, _| Box::new(ConsoleMmio::new(ram.clone())),
            kind: ModuleKind::Console,
        },
        PersonalityMmio {
            range: RangeInclusive::new(0xD020, 0xD021),
            create: |_, _| Box::new(DisplayMmio::new()),
            kind: ModuleKind::Display,
        },
        PersonalityMmio {
            range: RangeInclusive::new(0xD040, 0xD047),
            create: |_, _| Box::new(SpriteMmio::new()),
            kind: ModuleKind::Sprite,
        },
        PersonalityMmio {
            range: RangeInclusive::new(0xD048, 0xD04E),
            create: |_, controller| Box::new(SystemMmio::new(controller.clone())),
            kind: ModuleKind::System,
        },
        PersonalityMmio {
            range: RangeInclusive::new(0xDC00, 0xDC03),
            create: |_, _| Box::new(InputMmio::new()),
            kind: ModuleKind::Input,
        },
    ],
    display: DisplayDefaults {
        border_color: 0x0E,
        background_color: 0x06,
    },
    interrupts: &[],
};

/// Interrupt sources exposed by the modern-retro personality.
const MODERN_RETRO_INTERRUPTS: &[PersonalityInterrupt] = &[
    PersonalityInterrupt {
        id: 0,
        name: "frame_start",
        line: InterruptLine::Nmi,
        trigger: InterruptTrigger::Edge,
        default_enable: true,
    },
    PersonalityInterrupt {
        id: 1,
        name: "frame_end",
        line: InterruptLine::Irq,
        trigger: InterruptTrigger::Level,
        default_enable: false,
    },
    PersonalityInterrupt {
        id: 2,
        name: "timer0",
        line: InterruptLine::Irq,
        trigger: InterruptTrigger::Level,
        default_enable: false,
    },
    PersonalityInterrupt {
        id: 3,
        name: "keyboard_event",
        line: InterruptLine::Irq,
        trigger: InterruptTrigger::Level,
        default_enable: false,
    },
    PersonalityInterrupt {
        id: 4,
        name: "gamepad_event",
        line: InterruptLine::Irq,
        trigger: InterruptTrigger::Level,
        default_enable: false,
    },
];

static PERSONALITIES: &[&Personality] = &[&MODERN_RETRO, &C64_COMPAT];

/// Return the built-in personalities.
#[must_use]
pub fn all() -> &'static [&'static Personality] {
    PERSONALITIES
}

/// Personality used by default when constructing a [`Bus`](crate::Bus).
#[must_use]
pub fn default() -> &'static Personality {
    &MODERN_RETRO
}

/// Look up a personality by `name` (case-sensitive).
#[must_use]
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
