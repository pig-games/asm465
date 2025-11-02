//! Input MMIO device representing joystick/paddle ports.

use std::sync::{Arc, Mutex};

use crate::mmio::{
    InputReg, Module, ModuleDeps, ModuleFactory, ModuleKind, ModuleOptions, RegId, RegisterDesc,
};
use crate::MmioDevice;

/// Logical buttons we surface to modern hosts. These mirror Bevy's [`GamepadButtonType`]
/// naming so downstream tooling can present familiar labels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ControllerButton {
    DPadUp,
    DPadDown,
    DPadLeft,
    DPadRight,
    South,
    East,
    West,
    North,
    Start,
    Select,
    Mode,
    LeftThumb,
}

/// Axes tracked for analog sticks/paddles. Mirrors Bevy's [`GamepadAxisType`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ControllerAxis {
    LeftStickX,
    LeftStickY,
}

/// Snapshot of the instantaneous and last-active state for a controller button.
#[derive(Clone, Copy, Debug, Default)]
pub struct ButtonSample {
    pub pressed: bool,
    pub value: f32,
    pub last_active_value: Option<f32>,
}

/// Snapshot of the instantaneous and last-active state for an analog axis.
#[derive(Clone, Copy, Debug, Default)]
pub struct AxisSample {
    pub value: f32,
    pub last_active_value: Option<f32>,
}

/// Per-pad telemetry exposed to tooling.
#[derive(Clone, Debug, Default)]
pub struct ModernControllerPadSnapshot {
    pub gamepad_id: Option<u32>,
    pub buttons: Vec<(ControllerButton, ButtonSample)>,
    pub axes: Vec<(ControllerAxis, AxisSample)>,
    pub pot_x: u8,
    pub pot_y: u8,
}

/// Combined controller telemetry for all tracked pads.
#[derive(Clone, Debug, Default)]
pub struct ModernInputSnapshot {
    pub pads: Vec<ModernControllerPadSnapshot>,
}

#[derive(Clone, Debug, Default)]
pub struct InputSnapshot {
    pub port_a: u8,
    pub port_b: u8,
    pub pot_x: u8,
    pub pot_y: u8,
    pub modern: ModernInputSnapshot,
}

#[derive(Default)]
pub struct InputState {
    port_a: u8,
    port_b: u8,
    pot_x: u8,
    pot_y: u8,
}

impl InputState {
    fn snapshot(&self) -> InputSnapshot {
        InputSnapshot {
            port_a: self.port_a,
            port_b: self.port_b,
            pot_x: self.pot_x,
            pot_y: self.pot_y,
            modern: ModernInputSnapshot::default(),
        }
    }
}

pub struct InputOutput {
    state: InputState,
}

impl Default for InputOutput {
    fn default() -> Self {
        Self {
            state: InputState {
                port_a: 0xFF,
                port_b: 0xFF,
                pot_x: 0x00,
                pot_y: 0x00,
            },
        }
    }
}

impl InputOutput {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn snapshot(&self) -> InputSnapshot {
        self.state.snapshot()
    }

    pub fn set_port_a(&mut self, value: u8) {
        self.state.port_a = value;
    }

    pub fn set_port_b(&mut self, value: u8) {
        self.state.port_b = value;
    }

    pub fn set_pot_x(&mut self, value: u8) {
        self.state.pot_x = value;
    }

    pub fn set_pot_y(&mut self, value: u8) {
        self.state.pot_y = value;
    }

    fn read(&self, reg: InputReg) -> u8 {
        match reg {
            InputReg::PortA => self.state.port_a,
            InputReg::PortB => self.state.port_b,
            InputReg::PotX => self.state.pot_x,
            InputReg::PotY => self.state.pot_y,
        }
    }

    fn write(&mut self, reg: InputReg, value: u8) {
        match reg {
            InputReg::PortA => self.state.port_a = value,
            InputReg::PortB => self.state.port_b = value,
            InputReg::PotX => self.state.pot_x = value,
            InputReg::PotY => self.state.pot_y = value,
        }
    }
}

pub struct InputMmio {
    state: Arc<Mutex<InputOutput>>,
}

const INPUT_REGS: &[RegisterDesc] = &[
    RegisterDesc::new(
        RegId::Input(InputReg::PortA),
        "PortA",
        1,
        0xFF,
        true,
        true,
        &[],
    ),
    RegisterDesc::new(
        RegId::Input(InputReg::PortB),
        "PortB",
        1,
        0xFF,
        true,
        true,
        &[],
    ),
    RegisterDesc::new(
        RegId::Input(InputReg::PotX),
        "PotX",
        1,
        0x00,
        true,
        true,
        &[],
    ),
    RegisterDesc::new(
        RegId::Input(InputReg::PotY),
        "PotY",
        1,
        0x00,
        true,
        true,
        &[],
    ),
];

impl InputMmio {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(InputOutput::new())),
        }
    }

    pub fn output(&self) -> Arc<Mutex<InputOutput>> {
        Arc::clone(&self.state)
    }

    fn read_reg_locked(&self, reg: InputReg) -> u8 {
        self.state
            .lock()
            .map(|state| state.read(reg))
            .unwrap_or(0xFF)
    }

    fn write_reg_locked(&self, reg: InputReg, value: u8) {
        if let Ok(mut state) = self.state.lock() {
            state.write(reg, value);
        }
    }
}

impl MmioDevice for InputMmio {
    fn read(&mut self, addr: u16) -> u8 {
        match addr & 0x0003 {
            0x00 => self.read_reg_locked(InputReg::PortA),
            0x01 => self.read_reg_locked(InputReg::PortB),
            0x02 => self.read_reg_locked(InputReg::PotX),
            0x03 => self.read_reg_locked(InputReg::PotY),
            _ => 0xFF,
        }
    }

    fn write(&mut self, addr: u16, value: u8) {
        match addr & 0x0003 {
            0x00 => self.write_reg_locked(InputReg::PortA, value),
            0x01 => self.write_reg_locked(InputReg::PortB, value),
            0x02 => self.write_reg_locked(InputReg::PotX, value),
            0x03 => self.write_reg_locked(InputReg::PotY, value),
            _ => {}
        }
    }
}

impl Module for InputMmio {
    fn kind(&self) -> ModuleKind {
        ModuleKind::Input
    }

    fn regs(&self) -> &'static [RegisterDesc] {
        INPUT_REGS
    }

    fn read_reg(&mut self, reg: RegId) -> u8 {
        match reg {
            RegId::Input(reg) => self.read_reg_locked(reg),
            _ => 0xFF,
        }
    }

    fn write_reg(&mut self, reg: RegId, value: u8) {
        if let RegId::Input(reg) = reg {
            self.write_reg_locked(reg, value);
        }
    }
}

pub struct InputModuleFactory;

pub const INPUT_FACTORY: InputModuleFactory = InputModuleFactory;

impl ModuleFactory for InputModuleFactory {
    fn id(&self) -> &'static str {
        "input.joystick"
    }

    fn kind(&self) -> ModuleKind {
        ModuleKind::Input
    }

    fn create(&self, _deps: &ModuleDeps, _options: &ModuleOptions) -> Box<dyn Module> {
        Box::new(InputMmio::new())
    }

    fn regs(&self) -> &'static [RegisterDesc] {
        INPUT_REGS
    }
}
