//! Sprite MMIO device that exposes sprite register state.

use crate::mmio::{
    BitField, Module, ModuleDeps, ModuleFactory, ModuleKind, ModuleOptions, RegId, RegisterDesc,
    SpriteReg,
};
use std::sync::{Arc, Mutex};

/// Number of sprite slots available through the MMIO interface.
pub const SPRITE_SLOTS: usize = 8;

/// State for a single sprite slot.
#[derive(Clone, Copy, Debug, Default)]
pub struct SpriteState {
    /// Sprite asset identifier; `0` disables the slot.
    pub number: u8,
    /// Optional animation index supplied by the guest.
    pub anim: u8,
    /// Horizontal position in 8.8 fixed-point units (guest space).
    pub x: u16,
    /// Vertical position in 8.8 fixed-point units (guest space).
    pub y: u16,
    /// Power-of-two scaling factors (encoded as shift exponents) for X/Y precision.
    pub scale_x: u8,
    pub scale_y: u8,
    /// Whether the sprite is enabled for rendering.
    pub enabled: bool,
}

/// Immutable snapshot shared with host integrations (Bevy frontend/tests).
#[derive(Clone)]
pub struct SpriteSnapshot {
    pub sprites: Vec<SpriteState>,
}

impl SpriteSnapshot {
    #[inline]
    #[must_use]
    pub fn sprite(&self, index: usize) -> Option<&SpriteState> {
        self.sprites.get(index)
    }
}

/// Shared state backing the sprite MMIO.
pub struct SpriteOutput {
    sprites: [SpriteState; SPRITE_SLOTS],
}

impl Default for SpriteOutput {
    fn default() -> Self {
        Self {
            sprites: [SpriteState::default(); SPRITE_SLOTS],
        }
    }
}

impl SpriteOutput {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn snapshot(&self) -> SpriteSnapshot {
        SpriteSnapshot {
            sprites: self.sprites.to_vec(),
        }
    }

    pub fn set_sprite(&mut self, index: usize, sprite: SpriteState) {
        if index < SPRITE_SLOTS {
            self.sprites[index] = sprite;
        }
    }
}

/// Sprite MMIO device implementation.
pub struct SpriteMmio {
    spr_select: u8,
    sprites: [SpriteState; SPRITE_SLOTS],
    output: Arc<Mutex<SpriteOutput>>,
}

const SPRITE_SCALE_FIELDS: &[BitField] = &[
    BitField::new("scale_y", 0, 4),
    BitField::new("scale_x", 4, 4),
];

const SPRITE_REGS: &[RegisterDesc] = &[
    RegisterDesc::new(
        RegId::Sprite(SpriteReg::Select),
        "Select",
        1,
        0,
        true,
        true,
        &[],
    ),
    RegisterDesc::new(
        RegId::Sprite(SpriteReg::Number),
        "Number",
        1,
        0,
        true,
        true,
        &[],
    ),
    RegisterDesc::new(
        RegId::Sprite(SpriteReg::Anim),
        "Anim",
        1,
        0,
        true,
        true,
        &[],
    ),
    RegisterDesc::new(RegId::Sprite(SpriteReg::XHi), "XHi", 1, 0, true, true, &[]),
    RegisterDesc::new(RegId::Sprite(SpriteReg::XLo), "XLo", 1, 0, true, true, &[]),
    RegisterDesc::new(RegId::Sprite(SpriteReg::YHi), "YHi", 1, 0, true, true, &[]),
    RegisterDesc::new(RegId::Sprite(SpriteReg::YLo), "YLo", 1, 0, true, true, &[]),
    RegisterDesc::new(
        RegId::Sprite(SpriteReg::Scale),
        "Scale",
        1,
        0,
        true,
        true,
        SPRITE_SCALE_FIELDS,
    ),
    RegisterDesc::new(
        RegId::Sprite(SpriteReg::Enable),
        "Enable",
        1,
        0,
        true,
        true,
        &[],
    ),
];

impl Default for SpriteMmio {
    fn default() -> Self {
        Self::new()
    }
}

impl SpriteMmio {
    #[must_use]
    pub fn new() -> Self {
        let output = Arc::new(Mutex::new(SpriteOutput::new()));
        Self {
            spr_select: 0,
            sprites: [SpriteState::default(); SPRITE_SLOTS],
            output,
        }
    }

    #[must_use]
    pub fn output(&self) -> Arc<Mutex<SpriteOutput>> {
        Arc::clone(&self.output)
    }

    fn publish_sprite(&self, index: usize) {
        if let Ok(mut output) = self.output.lock() {
            output.set_sprite(index, self.sprites[index]);
        }
    }

    fn with_selected_sprite<F>(&mut self, f: F)
    where
        F: FnOnce(usize, &mut SpriteState),
    {
        let index = (self.spr_select as usize) % SPRITE_SLOTS;
        f(index, &mut self.sprites[index]);
        self.publish_sprite(index);
    }
}

impl Module for SpriteMmio {
    fn read(&mut self, addr: u16) -> u8 {
        let slot = (self.spr_select as usize) % SPRITE_SLOTS;
        match addr & 0x000F {
            0x00 => self.spr_select,
            0x01 => self.sprites[slot].number,
            0x02 => self.sprites[slot].anim,
            0x03 => (self.sprites[slot].x >> 8) as u8,
            0x04 => (self.sprites[slot].x & 0x00FF) as u8,
            0x05 => (self.sprites[slot].y >> 8) as u8,
            0x06 => (self.sprites[slot].y & 0x00FF) as u8,
            0x07 => (self.sprites[slot].scale_x << 4) | (self.sprites[slot].scale_y & 0x0F),
            0x08 => u8::from(self.sprites[slot].enabled),
            _ => 0,
        }
    }

    fn write(&mut self, addr: u16, value: u8) {
        let slot = (self.spr_select as usize) % SPRITE_SLOTS;
        match addr & 0x000F {
            0x00 => {
                self.spr_select = value;
                log::trace!("SpriteMmio: select sprite slot {value}");
            }
            0x01 => {
                self.with_selected_sprite(|index, sprite| {
                    sprite.number = value;
                    log::trace!("SpriteMmio: spr_num slot {} => {}", index, sprite.number);
                });
            }
            0x02 => {
                self.with_selected_sprite(|index, sprite| {
                    sprite.anim = value;
                    log::trace!("SpriteMmio: spr_anim slot {} => {}", index, sprite.anim);
                });
            }
            0x03 => {
                self.with_selected_sprite(|index, sprite| {
                    sprite.x = (sprite.x & 0x00FF) | (u16::from(value) << 8);
                    log::trace!("SpriteMmio: spr_x slot {} => {:04X}", index, sprite.x);
                });
            }
            0x04 => {
                self.with_selected_sprite(|index, sprite| {
                    sprite.x = (sprite.x & 0xFF00) | u16::from(value);
                    log::trace!("SpriteMmio: spr_x slot {} => {:04X}", index, sprite.x);
                });
            }
            0x05 => {
                self.with_selected_sprite(|index, sprite| {
                    sprite.y = (sprite.y & 0x00FF) | (u16::from(value) << 8);
                    log::trace!("SpriteMmio: spr_y slot {} => {:04X}", index, sprite.y);
                });
            }
            0x06 => {
                self.with_selected_sprite(|index, sprite| {
                    sprite.y = (sprite.y & 0xFF00) | u16::from(value);
                    log::trace!("SpriteMmio: spr_y slot {} => {:04X}", index, sprite.y);
                });
            }
            0x07 => {
                let scale_x = (value >> 4) & 0x0F;
                let scale_y = value & 0x0F;
                self.sprites[slot].scale_x = scale_x;
                self.sprites[slot].scale_y = scale_y;
                self.publish_sprite(slot);
                log::trace!("SpriteMmio: spr_scale slot {slot} => x={scale_x}, y={scale_y}");
            }
            0x08 => {
                let enabled = value & 1 != 0;
                self.with_selected_sprite(|index, sprite| {
                    sprite.enabled = enabled;
                    log::trace!("SpriteMmio: spr_enable slot {index} => {enabled}");
                });
            }
            _ => {}
        }
    }

    fn kind(&self) -> ModuleKind {
        ModuleKind::Sprite
    }

    fn regs(&self) -> &'static [RegisterDesc] {
        SPRITE_REGS
    }

    fn read_reg(&mut self, reg: RegId) -> u8 {
        let slot = (self.spr_select as usize) % SPRITE_SLOTS;
        match reg {
            RegId::Sprite(SpriteReg::Select) => self.spr_select,
            RegId::Sprite(SpriteReg::Number) => self.sprites[slot].number,
            RegId::Sprite(SpriteReg::Anim) => self.sprites[slot].anim,
            RegId::Sprite(SpriteReg::XHi) => (self.sprites[slot].x >> 8) as u8,
            RegId::Sprite(SpriteReg::XLo) => (self.sprites[slot].x & 0x00FF) as u8,
            RegId::Sprite(SpriteReg::YHi) => (self.sprites[slot].y >> 8) as u8,
            RegId::Sprite(SpriteReg::YLo) => (self.sprites[slot].y & 0x00FF) as u8,
            RegId::Sprite(SpriteReg::Scale) => {
                (self.sprites[slot].scale_x << 4) | (self.sprites[slot].scale_y & 0x0F)
            }
            RegId::Sprite(SpriteReg::Enable) => u8::from(self.sprites[slot].enabled),
            _ => 0,
        }
    }

    fn write_reg(&mut self, reg: RegId, value: u8) {
        let slot = (self.spr_select as usize) % SPRITE_SLOTS;
        match reg {
            RegId::Sprite(SpriteReg::Select) => {
                self.spr_select = value;
                log::trace!("SpriteMmio: select sprite slot {value}");
            }
            RegId::Sprite(SpriteReg::Number) => {
                self.with_selected_sprite(|index, sprite| {
                    sprite.number = value;
                    log::trace!("SpriteMmio: spr_num slot {} => {}", index, sprite.number);
                });
            }
            RegId::Sprite(SpriteReg::Anim) => {
                self.with_selected_sprite(|index, sprite| {
                    sprite.anim = value;
                    log::trace!("SpriteMmio: spr_anim slot {} => {}", index, sprite.anim);
                });
            }
            RegId::Sprite(SpriteReg::XHi) => {
                self.with_selected_sprite(|index, sprite| {
                    sprite.x = (sprite.x & 0x00FF) | (u16::from(value) << 8);
                    log::trace!("SpriteMmio: spr_x slot {} => {:04X}", index, sprite.x);
                });
            }
            RegId::Sprite(SpriteReg::XLo) => {
                self.with_selected_sprite(|index, sprite| {
                    sprite.x = (sprite.x & 0xFF00) | u16::from(value);
                    log::trace!("SpriteMmio: spr_x slot {} => {:04X}", index, sprite.x);
                });
            }
            RegId::Sprite(SpriteReg::YHi) => {
                self.with_selected_sprite(|index, sprite| {
                    sprite.y = (sprite.y & 0x00FF) | (u16::from(value) << 8);
                    log::trace!("SpriteMmio: spr_y slot {} => {:04X}", index, sprite.y);
                });
            }
            RegId::Sprite(SpriteReg::YLo) => {
                self.with_selected_sprite(|index, sprite| {
                    sprite.y = (sprite.y & 0xFF00) | u16::from(value);
                    log::trace!("SpriteMmio: spr_y slot {} => {:04X}", index, sprite.y);
                });
            }
            RegId::Sprite(SpriteReg::Scale) => {
                let scale_x = (value >> 4) & 0x0F;
                let scale_y = value & 0x0F;
                self.sprites[slot].scale_x = scale_x;
                self.sprites[slot].scale_y = scale_y;
                self.publish_sprite(slot);
                log::trace!("SpriteMmio: spr_scale slot {slot} => x={scale_x}, y={scale_y}");
            }
            RegId::Sprite(SpriteReg::Enable) => {
                let enabled = value & 1 != 0;
                self.with_selected_sprite(|index, sprite| {
                    sprite.enabled = enabled;
                    log::trace!("SpriteMmio: spr_enable slot {index} => {enabled}");
                });
            }
            _ => {}
        }
    }
}

pub struct SpriteModuleFactory;

pub const SPRITE_FACTORY: SpriteModuleFactory = SpriteModuleFactory;

impl ModuleFactory for SpriteModuleFactory {
    fn id(&self) -> &'static str {
        "sprite.basic"
    }

    fn kind(&self) -> ModuleKind {
        ModuleKind::Sprite
    }

    fn create(&self, _deps: &ModuleDeps, _options: &ModuleOptions) -> Box<dyn Module> {
        Box::new(SpriteMmio::new())
    }

    fn regs(&self) -> &'static [RegisterDesc] {
        SPRITE_REGS
    }
}
