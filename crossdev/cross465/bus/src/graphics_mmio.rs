//! Graphics MMIO device for sprite and display state.
//!
//! This module exposes sprite registers and basic display colours (border,
//! background) to the guest code.  The Bevy frontend reads a shared snapshot to
//! mirror the sprite array and colours each frame.

use crate::MmioDevice;
use std::sync::{Arc, Mutex};

/// Number of sprite slots available through the MMIO interface.
pub const GRAPHICS_SPRITE_SLOTS: usize = 8;

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
}

/// Immutable snapshot shared with host integrations (Bevy frontend/tests).
#[derive(Clone)]
pub struct GraphicsSnapshot {
    pub sprites: Vec<SpriteState>,
    pub border_color: u8,
    pub background_color: u8,
}

impl GraphicsSnapshot {
    #[inline]
    pub fn sprite(&self, index: usize) -> Option<&SpriteState> {
        self.sprites.get(index)
    }
}

/// Shared state backing the graphics MMIO.
pub struct GraphicsOutput {
    sprites: [SpriteState; GRAPHICS_SPRITE_SLOTS],
    border_color: u8,
    background_color: u8,
}

impl Default for GraphicsOutput {
    fn default() -> Self {
        Self {
            sprites: [SpriteState::default(); GRAPHICS_SPRITE_SLOTS],
            border_color: 0,
            background_color: 0,
        }
    }
}

impl GraphicsOutput {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn snapshot(&self) -> GraphicsSnapshot {
        GraphicsSnapshot {
            sprites: self.sprites.iter().copied().collect(),
            border_color: self.border_color,
            background_color: self.background_color,
        }
    }

    pub fn set_sprite(&mut self, index: usize, sprite: SpriteState) {
        if index < GRAPHICS_SPRITE_SLOTS {
            self.sprites[index] = sprite;
        }
    }

    pub fn set_border_color(&mut self, color: u8) {
        self.border_color = color;
    }

    pub fn set_background_color(&mut self, color: u8) {
        self.background_color = color;
    }
}

/// Graphics MMIO device implementation.
pub struct GraphicsMmio {
    spr_select: u8,
    sprites: [SpriteState; GRAPHICS_SPRITE_SLOTS],
    border_color: u8,
    background_color: u8,
    output: Arc<Mutex<GraphicsOutput>>,
}

impl GraphicsMmio {
    pub fn new() -> Self {
        let output = Arc::new(Mutex::new(GraphicsOutput::new()));
        Self {
            spr_select: 0,
            sprites: [SpriteState::default(); GRAPHICS_SPRITE_SLOTS],
            border_color: 0,
            background_color: 0,
            output,
        }
    }

    pub fn output(&self) -> Arc<Mutex<GraphicsOutput>> {
        Arc::clone(&self.output)
    }

    fn publish_sprite(&self, index: usize) {
        if let Ok(mut output) = self.output.lock() {
            output.set_sprite(index, self.sprites[index]);
        }
    }

    fn publish_colors(&self) {
        if let Ok(mut output) = self.output.lock() {
            output.set_border_color(self.border_color);
            output.set_background_color(self.background_color);
        }
    }

    fn with_selected_sprite<F>(&mut self, f: F)
    where
        F: FnOnce(usize, &mut SpriteState),
    {
        let index = (self.spr_select as usize) % GRAPHICS_SPRITE_SLOTS;
        f(index, &mut self.sprites[index]);
        self.publish_sprite(index);
    }
}

impl MmioDevice for GraphicsMmio {
    fn read(&mut self, addr: u16) -> u8 {
        let slot = (self.spr_select as usize) % GRAPHICS_SPRITE_SLOTS;
        match addr & 0x000F {
            0x00 => self.spr_select,
            0x01 => self.sprites[slot].number,
            0x02 => self.sprites[slot].anim,
            0x03 => (self.sprites[slot].x >> 8) as u8,
            0x04 => (self.sprites[slot].x & 0x00FF) as u8,
            0x05 => (self.sprites[slot].y >> 8) as u8,
            0x06 => (self.sprites[slot].y & 0x00FF) as u8,
            0x07 => self.border_color,
            0x08 => self.background_color,
            0x09 => (self.sprites[slot].scale_x << 4) | (self.sprites[slot].scale_y & 0x0F),
            _ => 0,
        }
    }

    fn write(&mut self, addr: u16, value: u8) {
        let slot = (self.spr_select as usize) % GRAPHICS_SPRITE_SLOTS;
        match addr & 0x000F {
            0x00 => {
                self.spr_select = value;
                log::trace!("GraphicsMmio: select sprite slot {value}");
            }
            0x01 => {
                self.with_selected_sprite(|index, sprite| {
                    sprite.number = value;
                    log::trace!("GraphicsMmio: spr_num slot {} => {}", index, sprite.number);
                });
            }
            0x02 => {
                self.with_selected_sprite(|index, sprite| {
                    sprite.anim = value;
                    log::trace!("GraphicsMmio: spr_anim slot {} => {}", index, sprite.anim);
                });
            }
            0x03 => {
                self.with_selected_sprite(|index, sprite| {
                    sprite.x = (sprite.x & 0x00FF) | ((value as u16) << 8);
                    log::trace!("GraphicsMmio: spr_x slot {} => {:04X}", index, sprite.x);
                });
            }
            0x04 => {
                self.with_selected_sprite(|index, sprite| {
                    sprite.x = (sprite.x & 0xFF00) | value as u16;
                    log::trace!("GraphicsMmio: spr_x slot {} => {:04X}", index, sprite.x);
                });
            }
            0x05 => {
                self.with_selected_sprite(|index, sprite| {
                    sprite.y = (sprite.y & 0x00FF) | ((value as u16) << 8);
                    log::trace!("GraphicsMmio: spr_y slot {} => {:04X}", index, sprite.y);
                });
            }
            0x06 => {
                self.with_selected_sprite(|index, sprite| {
                    sprite.y = (sprite.y & 0xFF00) | value as u16;
                    log::trace!("GraphicsMmio: spr_y slot {} => {:04X}", index, sprite.y);
                });
            }
            0x07 => {
                self.border_color = value;
                self.publish_colors();
                log::trace!("GraphicsMmio: border color => {value}");
            }
            0x08 => {
                self.background_color = value;
                self.publish_colors();
                log::trace!("GraphicsMmio: background color => {value}");
            }
            0x09 => {
                let scale_x = (value >> 4) & 0x0F;
                let scale_y = value & 0x0F;
                self.sprites[slot].scale_x = scale_x;
                self.sprites[slot].scale_y = scale_y;
                self.publish_sprite(slot);
                log::trace!(
                    "GraphicsMmio: spr_scale slot {} => x={}, y={}",
                    slot,
                    scale_x,
                    scale_y
                );
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MmioDevice;

    #[test]
    fn writes_update_sprite_state() {
        let mut mmio = GraphicsMmio::new();
        // Select slot 1 and write X/Y + number
        MmioDevice::write(&mut mmio, 0xDF20, 1);
        MmioDevice::write(&mut mmio, 0xDF23, 0x12);
        MmioDevice::write(&mut mmio, 0xDF24, 0x34);
        MmioDevice::write(&mut mmio, 0xDF25, 0x56);
        MmioDevice::write(&mut mmio, 0xDF26, 0x78);
        MmioDevice::write(&mut mmio, 0xDF21, 5);
        MmioDevice::write(&mut mmio, 0xDF29, 0x21);

        let handle = mmio.output();
        let snapshot = handle.lock().unwrap().snapshot();
        let sprite = snapshot.sprite(1).expect("slot 1");
        assert_eq!(sprite.number, 5);
        assert_eq!(sprite.x, 0x1234);
        assert_eq!(sprite.y, 0x5678);
        assert_eq!(sprite.scale_x, 0x2);
        assert_eq!(sprite.scale_y, 0x1);
    }

    #[test]
    fn writes_update_colors() {
        let mut mmio = GraphicsMmio::new();
        MmioDevice::write(&mut mmio, 0xDF27, 0x0F);
        MmioDevice::write(&mut mmio, 0xDF28, 0x02);

        let handle = mmio.output();
        let snapshot = handle.lock().unwrap().snapshot();
        assert_eq!(snapshot.border_color, 0x0F);
        assert_eq!(snapshot.background_color, 0x02);
    }
}
