//! Display MMIO device exposing border/background colours.
//!
//! This module owns the palette registers that control the viewer's border and
//! content background colours.  The sprite device exposes the remaining sprite
//! state so display concerns can evolve independently.

use crate::mmio::{
    DisplayReg, Module, ModuleDeps, ModuleFactory, ModuleKind, ModuleOptions, RegId, RegisterDesc,
};
use crate::MmioDevice;
use std::sync::{Arc, Mutex};

/// Immutable snapshot shared with host integrations (Bevy frontend/tests).
#[derive(Clone, Copy, Debug, Default)]
pub struct DisplaySnapshot {
    pub border_color: u8,
    pub background_color: u8,
}

/// Shared state backing the display MMIO.
pub struct DisplayOutput {
    border_color: u8,
    background_color: u8,
}

impl Default for DisplayOutput {
    fn default() -> Self {
        Self {
            border_color: 0x0E,
            background_color: 0x06,
        }
    }
}

impl DisplayOutput {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn snapshot(&self) -> DisplaySnapshot {
        DisplaySnapshot {
            border_color: self.border_color,
            background_color: self.background_color,
        }
    }

    pub fn set_border_color(&mut self, color: u8) {
        self.border_color = color;
    }

    pub fn set_background_color(&mut self, color: u8) {
        self.background_color = color;
    }
}

/// Display MMIO device implementation.
pub struct DisplayMmio {
    border_color: u8,
    background_color: u8,
    output: Arc<Mutex<DisplayOutput>>,
}

const DISPLAY_REGS: &[RegisterDesc] = &[
    RegisterDesc::new(
        RegId::Display(DisplayReg::BorderColor),
        "BorderColor",
        1,
        0,
        true,
        true,
        &[],
    ),
    RegisterDesc::new(
        RegId::Display(DisplayReg::BackgroundColor),
        "BackgroundColor",
        1,
        0,
        true,
        true,
        &[],
    ),
];

impl DisplayMmio {
    pub fn new() -> Self {
        let output = Arc::new(Mutex::new(DisplayOutput::new()));
        Self {
            border_color: 0,
            background_color: 0,
            output,
        }
    }

    pub fn output(&self) -> Arc<Mutex<DisplayOutput>> {
        Arc::clone(&self.output)
    }

    fn publish(&self) {
        if let Ok(mut output) = self.output.lock() {
            output.set_border_color(self.border_color);
            output.set_background_color(self.background_color);
        }
    }
}

impl MmioDevice for DisplayMmio {
    fn read(&mut self, addr: u16) -> u8 {
        match addr & 0x0001 {
            0x00 => self.border_color,
            0x01 => self.background_color,
            _ => 0,
        }
    }

    fn write(&mut self, addr: u16, value: u8) {
        match addr & 0x0001 {
            0x00 => {
                self.border_color = value;
                self.publish();
                log::trace!("DisplayMmio: border color => {value}");
            }
            0x01 => {
                self.background_color = value;
                self.publish();
                log::trace!("DisplayMmio: background color => {value}");
            }
            _ => {}
        }
    }
}

impl Module for DisplayMmio {
    fn kind(&self) -> ModuleKind {
        ModuleKind::Display
    }

    fn regs(&self) -> &'static [RegisterDesc] {
        DISPLAY_REGS
    }

    fn read_reg(&mut self, reg: RegId) -> u8 {
        match reg {
            RegId::Display(DisplayReg::BorderColor) => self.border_color,
            RegId::Display(DisplayReg::BackgroundColor) => self.background_color,
            _ => 0,
        }
    }

    fn write_reg(&mut self, reg: RegId, value: u8) {
        match reg {
            RegId::Display(DisplayReg::BorderColor) => {
                self.border_color = value;
                self.publish();
            }
            RegId::Display(DisplayReg::BackgroundColor) => {
                self.background_color = value;
                self.publish();
            }
            _ => {}
        }
    }
}

pub struct DisplayModuleFactory;

pub const DISPLAY_FACTORY: DisplayModuleFactory = DisplayModuleFactory;

impl ModuleFactory for DisplayModuleFactory {
    fn id(&self) -> &'static str {
        "display.basic2d"
    }

    fn kind(&self) -> ModuleKind {
        ModuleKind::Display
    }

    fn create(&self, _deps: &ModuleDeps, _options: &ModuleOptions) -> Box<dyn Module> {
        Box::new(DisplayMmio::new())
    }

    fn regs(&self) -> &'static [RegisterDesc] {
        DISPLAY_REGS
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mmio::{Module, ModuleKind, RegId};
    use crate::MmioDevice;

    #[test]
    fn writes_update_display_colors() {
        let mut mmio = DisplayMmio::new();
        MmioDevice::write(&mut mmio, 0xDF20, 0x0E);
        MmioDevice::write(&mut mmio, 0xDF21, 0x05);

        let snapshot = mmio.output().lock().unwrap().snapshot();
        assert_eq!(snapshot.border_color, 0x0E);
        assert_eq!(snapshot.background_color, 0x05);

        mmio.write_reg(RegId::Display(DisplayReg::BorderColor), 0x02);
        assert_eq!(mmio.read_reg(RegId::Display(DisplayReg::BorderColor)), 0x02);
        assert_eq!(mmio.kind(), ModuleKind::Display);
    }
}
