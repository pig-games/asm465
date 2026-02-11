//! System/interrupt MMIO device exposing IRQ/NMI enable state and the raster
//! compare registers shared with the video adapter.

use crate::adapters::video::RasterIrqState;
use crate::interrupts::InterruptController;
use crate::mmio::{
    Module, ModuleDeps, ModuleFactory, ModuleKind, ModuleOptions, RegId, RegisterDesc, SystemReg,
};
use std::sync::Arc;

/// MMIO view over the shared interrupt controller, including raster compare
/// registers fed by the video adapter.
pub struct SystemMmio {
    controller: Arc<InterruptController>,
    raster_irq: Option<Arc<RasterIrqState>>,
    raster_current: u16,
    raster_compare: u16,
}

const SYSTEM_REGS: &[RegisterDesc] = &[
    RegisterDesc::new(
        RegId::System(SystemReg::IrqPending),
        "IrqPending",
        1,
        0,
        true,
        false,
        &[],
    ),
    RegisterDesc::new(
        RegId::System(SystemReg::IrqEnable),
        "IrqEnable",
        1,
        0,
        true,
        true,
        &[],
    ),
    RegisterDesc::new(
        RegId::System(SystemReg::IrqAck),
        "IrqAck",
        1,
        0,
        true,
        true,
        &[],
    ),
    RegisterDesc::new(
        RegId::System(SystemReg::IrqSource),
        "IrqSource",
        1,
        0xFF,
        true,
        false,
        &[],
    ),
    RegisterDesc::new(
        RegId::System(SystemReg::NmiPending),
        "NmiPending",
        1,
        0,
        true,
        false,
        &[],
    ),
    RegisterDesc::new(
        RegId::System(SystemReg::NmiAck),
        "NmiAck",
        1,
        0,
        true,
        true,
        &[],
    ),
    RegisterDesc::new(
        RegId::System(SystemReg::Status),
        "Status",
        1,
        0,
        true,
        false,
        &[],
    ),
    RegisterDesc::new(
        RegId::System(SystemReg::RasterLo),
        "RasterLo",
        1,
        0,
        true,
        true,
        &[],
    ),
    RegisterDesc::new(
        RegId::System(SystemReg::RasterCompareLo),
        "RasterCompareLo",
        1,
        0,
        true,
        true,
        &[],
    ),
    RegisterDesc::new(
        RegId::System(SystemReg::RasterCompareHi),
        "RasterCompareHi",
        1,
        0,
        true,
        true,
        &[],
    ),
    RegisterDesc::new(
        RegId::System(SystemReg::SpriteCollisions),
        "SpriteCollisions",
        1,
        0,
        true,
        false,
        &[],
    ),
    RegisterDesc::new(
        RegId::System(SystemReg::BackgroundCollisions),
        "BackgroundCollisions",
        1,
        0,
        true,
        false,
        &[],
    ),
];

impl SystemMmio {
    pub fn new(controller: Arc<InterruptController>) -> Self {
        Self {
            controller,
            raster_irq: None,
            raster_current: 0,
            raster_compare: 0,
        }
    }

    pub fn set_raster_irq_state(&mut self, state: Arc<RasterIrqState>) {
        state.set_current(self.raster_current);
        state.set_compare(self.raster_compare);
        self.raster_irq = Some(state);
    }

    fn irq_pending(&self) -> u8 {
        (self.controller.irq_pending() & 0xFF) as u8
    }

    fn irq_enabled(&self) -> u8 {
        (self.controller.irq_enabled() & 0xFF) as u8
    }

    fn nmi_pending(&self) -> u8 {
        (self.controller.nmi_pending() & 0xFF) as u8
    }
}

impl Module for SystemMmio {
    fn read(&mut self, addr: u16) -> u8 {
        let current = self
            .raster_irq
            .as_ref()
            .map(|state| state.current())
            .unwrap_or(self.raster_current);
        let compare = self
            .raster_irq
            .as_ref()
            .map(|state| state.compare())
            .unwrap_or(self.raster_compare);
        match addr & 0x000F {
            0x00 => self.irq_pending(),
            0x01 => self.irq_enabled(),
            0x02 => self.irq_pending(),
            0x03 => {
                let mask = self.controller.irq_pending() & self.controller.irq_enabled();
                if mask == 0 {
                    0xFF
                } else {
                    mask.trailing_zeros() as u8
                }
            }
            0x04 => self.nmi_pending(),
            0x05 => self.nmi_pending(),
            0x06 => {
                let snapshot = self.controller.snapshot();
                let mut status = 0u8;
                if snapshot.irq_line {
                    status |= 0x01;
                }
                if snapshot.nmi_line {
                    status |= 0x02;
                }
                if snapshot.nmi_edge_latched {
                    status |= 0x04;
                }
                status
            }
            0x07 => (current & 0x00FF) as u8,
            0x08 => (compare & 0x00FF) as u8,
            0x09 => (compare >> 8) as u8,
            0x0A => 0,
            0x0B => 0,
            _ => 0xFF,
        }
    }

    fn write(&mut self, addr: u16, value: u8) {
        let mask = value as u32;
        match addr & 0x000F {
            0x01 => {
                self.controller.set_irq_enable(mask);
            }
            0x02 => {
                if mask != 0 {
                    self.controller.clear_irq(mask);
                }
            }
            0x05 => {
                if mask != 0 {
                    self.controller.clear_nmi(mask);
                }
            }
            0x07 => {
                self.raster_current = (self.raster_current & 0xFF00) | value as u16;
                if let Some(state) = &self.raster_irq {
                    state.set_current_low(value);
                }
            }
            0x08 => {
                self.raster_compare = (self.raster_compare & 0xFF00) | value as u16;
                if let Some(state) = &self.raster_irq {
                    state.set_compare_low(value);
                }
            }
            0x09 => {
                self.raster_compare = ((value as u16) << 8) | (self.raster_compare & 0x00FF);
                if let Some(state) = &self.raster_irq {
                    state.set_compare_high(value);
                }
            }
            _ => {}
        }
    }

    fn kind(&self) -> ModuleKind {
        ModuleKind::System
    }

    fn regs(&self) -> &'static [RegisterDesc] {
        SYSTEM_REGS
    }

    fn read_reg(&mut self, reg: RegId) -> u8 {
        let current = self
            .raster_irq
            .as_ref()
            .map(|state| state.current())
            .unwrap_or(self.raster_current);
        let compare = self
            .raster_irq
            .as_ref()
            .map(|state| state.compare())
            .unwrap_or(self.raster_compare);
        match reg {
            RegId::System(SystemReg::IrqPending) => self.irq_pending(),
            RegId::System(SystemReg::IrqEnable) => self.irq_enabled(),
            RegId::System(SystemReg::IrqAck) => self.irq_pending(),
            RegId::System(SystemReg::IrqSource) => {
                let mask = self.controller.irq_pending() & self.controller.irq_enabled();
                if mask == 0 {
                    0xFF
                } else {
                    mask.trailing_zeros() as u8
                }
            }
            RegId::System(SystemReg::NmiPending) => self.nmi_pending(),
            RegId::System(SystemReg::NmiAck) => self.nmi_pending(),
            RegId::System(SystemReg::Status) => {
                let snapshot = self.controller.snapshot();
                let mut status = 0u8;
                if snapshot.irq_line {
                    status |= 0x01;
                }
                if snapshot.nmi_line {
                    status |= 0x02;
                }
                if snapshot.nmi_edge_latched {
                    status |= 0x04;
                }
                status
            }
            RegId::System(SystemReg::RasterLo) => (current & 0x00FF) as u8,
            RegId::System(SystemReg::RasterCompareLo) => (compare & 0x00FF) as u8,
            RegId::System(SystemReg::RasterCompareHi) => (compare >> 8) as u8,
            RegId::System(SystemReg::SpriteCollisions)
            | RegId::System(SystemReg::BackgroundCollisions) => 0,
            _ => 0xFF,
        }
    }

    fn write_reg(&mut self, reg: RegId, value: u8) {
        let mask = value as u32;
        match reg {
            RegId::System(SystemReg::IrqEnable) => {
                self.controller.set_irq_enable(mask);
            }
            RegId::System(SystemReg::IrqAck) => {
                if mask != 0 {
                    self.controller.clear_irq(mask);
                }
            }
            RegId::System(SystemReg::NmiAck) => {
                if mask != 0 {
                    self.controller.clear_nmi(mask);
                }
            }
            RegId::System(SystemReg::RasterLo) => {
                self.raster_current = (self.raster_current & 0xFF00) | value as u16;
                if let Some(state) = &self.raster_irq {
                    state.set_current_low(value);
                }
            }
            RegId::System(SystemReg::RasterCompareLo) => {
                self.raster_compare = (self.raster_compare & 0xFF00) | value as u16;
                if let Some(state) = &self.raster_irq {
                    state.set_compare_low(value);
                }
            }
            RegId::System(SystemReg::RasterCompareHi) => {
                self.raster_compare = ((value as u16) << 8) | (self.raster_compare & 0x00FF);
                if let Some(state) = &self.raster_irq {
                    state.set_compare_high(value);
                }
            }
            _ => {}
        }
    }
}

pub struct SystemModuleFactory;

pub const SYSTEM_FACTORY: SystemModuleFactory = SystemModuleFactory;

impl ModuleFactory for SystemModuleFactory {
    fn id(&self) -> &'static str {
        "system.interrupts"
    }

    fn kind(&self) -> ModuleKind {
        ModuleKind::System
    }

    fn create(&self, deps: &ModuleDeps, _options: &ModuleOptions) -> Box<dyn Module> {
        Box::new(SystemMmio::new(deps.controller.clone()))
    }

    fn regs(&self) -> &'static [RegisterDesc] {
        SYSTEM_REGS
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mmio::{Module, ModuleKind, RegId};

    #[test]
    fn read_write_registers() {
        let controller = Arc::new(InterruptController::new());
        controller.set_irq_enable(0);
        let mut mmio = SystemMmio::new(controller.clone());

        // Enable IRQ/NMI bits via register writes.
        mmio.write(0xDF41, 0b0000_0011);
        assert_eq!(controller.irq_enabled(), 0b11);

        // Raise lines through the controller and check the mirrors.
        controller.raise_irq(0b0000_0010);
        controller.raise_nmi(0b0000_0001);
        assert_eq!(mmio.read(0xDF40), 0b10);
        assert_eq!(mmio.read(0xDF42), 0b10);
        assert_eq!(mmio.read(0xDF44), 0b1);
        assert_eq!(mmio.read(0xDF45), 0b1);

        // IRQ_SOURCE should report the lowest enabled pending bit.
        assert_eq!(mmio.read(0xDF43), 1);

        // Ack the interrupts via MMIO.
        mmio.write(0xDF42, 0b0000_0010);
        mmio.write(0xDF45, 0b0000_0001);
        assert_eq!(controller.irq_pending(), 0);
        assert_eq!(controller.nmi_pending(), 0);

        assert_eq!(mmio.kind(), ModuleKind::System);
        mmio.write_reg(RegId::System(SystemReg::IrqEnable), 0b0000_0100);
        assert_eq!(
            mmio.read_reg(RegId::System(SystemReg::IrqEnable)),
            0b0000_0100
        );
    }
}
