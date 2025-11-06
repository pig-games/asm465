//! Input MMIO device representing controller state.

use std::sync::{Arc, Mutex};

use crate::mmio::{
    InputReg, Module, ModuleDeps, ModuleFactory, ModuleKind, ModuleOptions, RegId, RegisterDesc,
};
use crate::MmioDevice;

/// Maximum number of pads tracked by the shared controller backend.
pub const CONTROLLER_PAD_COUNT: usize = 4;

/// Neutral and boundary values for the analog potentiometers.
pub const POT_MIN: u8 = 0;
pub const POT_MAX: u8 = 255;
pub const POT_NEUTRAL: u8 = 128;

/// Logical buttons exposed by the modern controller tracker.
///
/// The ordering is stable so personalities can rely on the bit positions in the
/// [`InputPadSnapshot::buttons`] mask (bit = `1 << ControllerButton::index()`).
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
    LeftShoulder,
    RightShoulder,
    LeftThumb,
    RightThumb,
    LeftTrigger,
    RightTrigger,
}

impl ControllerButton {
    /// Bit position inside the 16-bit button mask.
    pub fn index(self) -> u8 {
        match self {
            ControllerButton::DPadUp => 0,
            ControllerButton::DPadDown => 1,
            ControllerButton::DPadLeft => 2,
            ControllerButton::DPadRight => 3,
            ControllerButton::South => 4,
            ControllerButton::East => 5,
            ControllerButton::West => 6,
            ControllerButton::North => 7,
            ControllerButton::Start => 8,
            ControllerButton::Select => 9,
            ControllerButton::LeftShoulder => 10,
            ControllerButton::RightShoulder => 11,
            ControllerButton::LeftThumb => 12,
            ControllerButton::RightThumb => 13,
            ControllerButton::LeftTrigger => 14,
            ControllerButton::RightTrigger => 15,
        }
    }
}

/// Axes tracked for analog sticks/paddles. Mirrors Bevy's [`GamepadAxisType`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ControllerAxis {
    LeftStickX,
    LeftStickY,
}

/// Bit mask helper for a controller button.
#[inline]
pub fn button_bit(button: ControllerButton) -> u16 {
    1u16 << button.index()
}

/// Convert the unified button mask into active-low C64-style port values.
#[inline]
pub fn port_from_buttons(buttons: u16) -> (u8, u8) {
    let mut port_a = 0xFF;
    let mut port_b = 0xFF;

    if buttons & button_bit(ControllerButton::DPadUp) != 0 {
        port_a &= !(1 << 0);
    }
    if buttons & button_bit(ControllerButton::DPadDown) != 0 {
        port_a &= !(1 << 1);
    }
    if buttons & button_bit(ControllerButton::DPadLeft) != 0 {
        port_a &= !(1 << 2);
    }
    if buttons & button_bit(ControllerButton::DPadRight) != 0 {
        port_a &= !(1 << 3);
    }
    if buttons & button_bit(ControllerButton::South) != 0 {
        port_a &= !(1 << 4);
    }

    if buttons & button_bit(ControllerButton::Start) != 0 {
        port_b &= !(1 << 0);
    }
    if buttons & button_bit(ControllerButton::Select) != 0 {
        port_b &= !(1 << 1);
    }
    if buttons & button_bit(ControllerButton::RightShoulder) != 0 {
        port_b &= !(1 << 2);
    }
    if buttons & button_bit(ControllerButton::LeftThumb) != 0 {
        port_b &= !(1 << 3);
    }
    if buttons & button_bit(ControllerButton::East) != 0 {
        port_b &= !(1 << 4);
    }

    (port_a, port_b)
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

/// Binary snapshot returned to consumers that only need register-level state.
#[derive(Clone, Copy, Debug, Default)]
pub struct InputPadSnapshot {
    pub port_a: u8,
    pub port_b: u8,
    pub buttons: u16,
    pub pot_x: u8,
    pub pot_y: u8,
}

/// MMIO snapshot combining binary pad state and modern telemetry.
#[derive(Clone, Debug, Default)]
pub struct InputSnapshot {
    pub pads: Vec<InputPadSnapshot>,
    pub modern: ModernInputSnapshot,
}

#[derive(Clone, Copy, Debug, Default)]
struct InputPadState {
    port_a: u8,
    port_b: u8,
    buttons: u16,
    pot_x: u8,
    pot_y: u8,
}

impl InputPadState {
    fn as_snapshot(&self) -> InputPadSnapshot {
        InputPadSnapshot {
            port_a: self.port_a,
            port_b: self.port_b,
            buttons: self.buttons,
            pot_x: self.pot_x,
            pot_y: self.pot_y,
        }
    }
}

/// Shared controller state exported to the adapter/backend pipeline.
pub struct InputOutput {
    pads: [InputPadState; CONTROLLER_PAD_COUNT],
}

impl Default for InputOutput {
    fn default() -> Self {
        Self {
            pads: [InputPadState {
                port_a: 0xFF,
                port_b: 0xFF,
                buttons: 0,
                pot_x: POT_NEUTRAL,
                pot_y: POT_NEUTRAL,
            }; CONTROLLER_PAD_COUNT],
        }
    }
}

impl InputOutput {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn snapshot(&self) -> InputSnapshot {
        InputSnapshot {
            pads: self.pads.iter().map(InputPadState::as_snapshot).collect(),
            modern: ModernInputSnapshot::default(),
        }
    }

    pub fn set_pad_snapshot(&mut self, pad: usize, snapshot: InputPadSnapshot) {
        if pad < CONTROLLER_PAD_COUNT {
            self.pads[pad] = InputPadState {
                port_a: snapshot.port_a,
                port_b: snapshot.port_b,
                buttons: snapshot.buttons,
                pot_x: snapshot.pot_x,
                pot_y: snapshot.pot_y,
            };
        }
    }

    pub fn set_pad_buttons(&mut self, pad: usize, buttons: u16) {
        if pad < CONTROLLER_PAD_COUNT {
            self.pads[pad].buttons = buttons;
            let (port_a, port_b) = port_from_buttons(buttons);
            self.pads[pad].port_a = port_a;
            self.pads[pad].port_b = port_b;
        }
    }

    pub fn set_port_a(&mut self, value: u8) {
        self.set_pad_port_a(0, value);
    }

    pub fn set_pad_port_a(&mut self, pad: usize, value: u8) {
        if pad < CONTROLLER_PAD_COUNT {
            self.pads[pad].port_a = value;
        }
    }

    pub fn set_port_b(&mut self, value: u8) {
        self.set_pad_port_b(0, value);
    }

    pub fn set_pad_port_b(&mut self, pad: usize, value: u8) {
        if pad < CONTROLLER_PAD_COUNT {
            self.pads[pad].port_b = value;
        }
    }

    pub fn set_pot_x(&mut self, value: u8) {
        self.set_pad_pot_x(0, value);
    }

    pub fn set_pad_pot_x(&mut self, pad: usize, value: u8) {
        if pad < CONTROLLER_PAD_COUNT {
            self.pads[pad].pot_x = value;
        }
    }

    pub fn set_pot_y(&mut self, value: u8) {
        self.set_pad_pot_y(0, value);
    }

    pub fn set_pad_pot_y(&mut self, pad: usize, value: u8) {
        if pad < CONTROLLER_PAD_COUNT {
            self.pads[pad].pot_y = value;
        }
    }

    pub fn pad_snapshot(&self, pad: usize) -> InputPadSnapshot {
        if pad < CONTROLLER_PAD_COUNT {
            self.pads[pad].as_snapshot()
        } else {
            InputPadSnapshot::default()
        }
    }
}

pub struct InputMmio {
    select: u8,
    state: Arc<Mutex<InputOutput>>,
}

const INPUT_REGS: &[RegisterDesc] = &[
    RegisterDesc::new(
        RegId::Input(InputReg::Select),
        "Select",
        1,
        0x00,
        true,
        true,
        &[],
    ),
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
        RegId::Input(InputReg::ButtonsLo),
        "ButtonsLo",
        1,
        0x00,
        true,
        true,
        &[],
    ),
    RegisterDesc::new(
        RegId::Input(InputReg::ButtonsHi),
        "ButtonsHi",
        1,
        0x00,
        true,
        true,
        &[],
    ),
    RegisterDesc::new(
        RegId::Input(InputReg::PotX),
        "PotX",
        1,
        0x7f,
        true,
        true,
        &[],
    ),
    RegisterDesc::new(
        RegId::Input(InputReg::PotY),
        "PotY",
        1,
        0x7f,
        true,
        true,
        &[],
    ),
];

impl InputMmio {
    pub fn new() -> Self {
        Self {
            select: 0,
            state: Arc::new(Mutex::new(InputOutput::new())),
        }
    }

    pub fn output(&self) -> Arc<Mutex<InputOutput>> {
        Arc::clone(&self.state)
    }

    fn select_index(&self) -> usize {
        (self.select as usize) % CONTROLLER_PAD_COUNT
    }

    fn read_reg_locked(&self, reg: InputReg) -> u8 {
        match reg {
            InputReg::Select => self.select,
            _ => self
                .state
                .lock()
                .map(|state| match reg {
                    InputReg::Select => self.select,
                    InputReg::PortA => state.pad_snapshot(self.select_index()).port_a,
                    InputReg::PortB => state.pad_snapshot(self.select_index()).port_b,
                    InputReg::ButtonsLo => state.pad_snapshot(self.select_index()).buttons as u8,
                    InputReg::ButtonsHi => {
                        (state.pad_snapshot(self.select_index()).buttons >> 8) as u8
                    }
                    InputReg::PotX => state.pad_snapshot(self.select_index()).pot_x,
                    InputReg::PotY => state.pad_snapshot(self.select_index()).pot_y,
                })
                .unwrap_or(0xFF),
        }
    }

    fn write_reg_locked(&mut self, reg: InputReg, value: u8) {
        match reg {
            InputReg::Select => {
                self.select = value;
            }
            _ => {
                if let Ok(mut state) = self.state.lock() {
                    let pad = self.select_index();
                    match reg {
                        InputReg::Select => {}
                        InputReg::PortA => state.set_pad_port_a(pad, value),
                        InputReg::PortB => state.set_pad_port_b(pad, value),
                        InputReg::ButtonsLo => {
                            let snapshot = state.pad_snapshot(pad);
                            let combined = (snapshot.buttons & 0xFF00) | value as u16;
                            state.set_pad_buttons(pad, combined);
                        }
                        InputReg::ButtonsHi => {
                            let snapshot = state.pad_snapshot(pad);
                            let combined = (snapshot.buttons & 0x00FF) | ((value as u16) << 8);
                            state.set_pad_buttons(pad, combined);
                        }
                        InputReg::PotX => state.set_pad_pot_x(pad, value),
                        InputReg::PotY => state.set_pad_pot_y(pad, value),
                    }
                }
            }
        }
    }
}

impl MmioDevice for InputMmio {
    fn read(&mut self, addr: u16) -> u8 {
        match addr & 0x0007 {
            0x0000 => self.read_reg_locked(InputReg::PortA),
            0x0001 => self.read_reg_locked(InputReg::PortB),
            0x0002 => self.read_reg_locked(InputReg::PotX),
            0x0003 => self.read_reg_locked(InputReg::PotY),
            0x0004 => self.read_reg_locked(InputReg::ButtonsLo),
            0x0005 => self.read_reg_locked(InputReg::ButtonsHi),
            0x0006 => self.read_reg_locked(InputReg::Select),
            _ => 0xFF,
        }
    }

    fn write(&mut self, addr: u16, value: u8) {
        match addr & 0x0007 {
            0x0000 => self.write_reg_locked(InputReg::PortA, value),
            0x0001 => self.write_reg_locked(InputReg::PortB, value),
            0x0002 => self.write_reg_locked(InputReg::PotX, value),
            0x0003 => self.write_reg_locked(InputReg::PotY, value),
            0x0004 => self.write_reg_locked(InputReg::ButtonsLo, value),
            0x0005 => self.write_reg_locked(InputReg::ButtonsHi, value),
            0x0006 => self.write_reg_locked(InputReg::Select, value),
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
