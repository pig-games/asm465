use std::array;
use std::sync::{Arc, Mutex};

use crate::input_mmio::{
    AxisSample, ButtonSample, ControllerAxis, ControllerButton, InputOutput, InputSnapshot,
    ModernControllerPadSnapshot, ModernInputSnapshot,
};
use crate::mmio::{HookAction, InputReg, ModuleAdapter, ModuleAdapterEvent};

const CONTROLLER_PADS: usize = 2;
const BUTTON_PRESS_THRESHOLD: f32 = 0.5;
const AXIS_ACTIVE_THRESHOLD: f32 = 0.2;
const POT_MIN: u8 = 0;
const POT_MAX: u8 = 255;

const BUTTONS: &[ControllerButton] = &[
    ControllerButton::DPadUp,
    ControllerButton::DPadDown,
    ControllerButton::DPadLeft,
    ControllerButton::DPadRight,
    ControllerButton::South,
    ControllerButton::East,
    ControllerButton::West,
    ControllerButton::North,
    ControllerButton::Start,
    ControllerButton::Select,
    ControllerButton::Mode,
    ControllerButton::LeftThumb,
];

const AXES: &[ControllerAxis] = &[ControllerAxis::LeftStickX, ControllerAxis::LeftStickY];

/// Backend interface that accepts controller state updates.
pub trait InputBackend: Send + Sync {
    fn update_gamepad(&self, pad: usize, gamepad_id: Option<u32>);
    fn update_button(&self, pad: usize, button: ControllerButton, value: f32);
    fn update_axis(&self, pad: usize, axis: ControllerAxis, value: f32);

    fn write_port_a(&self, value: u8);
    fn write_port_b(&self, value: u8);
    fn write_pot_x(&self, value: u8);
    fn write_pot_y(&self, value: u8);

    fn snapshot(&self) -> InputSnapshot;
    fn handle_hook(&self, _hook: &str, _action: HookAction) {}
}

/// Concrete backend that mirrors modern controller state into [`InputOutput`].
pub struct InputBackendHandle {
    output: Arc<Mutex<InputOutput>>,
    state: Arc<Mutex<ModernInputState>>,
}

impl InputBackendHandle {
    pub fn new(state: Arc<Mutex<InputOutput>>) -> Self {
        Self {
            output: state,
            state: Arc::new(Mutex::new(ModernInputState::default())),
        }
    }

    fn write_output(&self, port_a: u8, port_b: u8, pot_x: u8, pot_y: u8) {
        if let Ok(mut output) = self.output.lock() {
            output.set_port_a(port_a);
            output.set_port_b(port_b);
            output.set_pot_x(pot_x);
            output.set_pot_y(pot_y);
        }
    }
}

impl InputBackend for InputBackendHandle {
    fn update_gamepad(&self, pad: usize, gamepad_id: Option<u32>) {
        if let Ok(mut state) = self.state.lock() {
            state.update_gamepad(pad, gamepad_id);
        }
    }

    fn update_button(&self, pad: usize, button: ControllerButton, value: f32) {
        let ports = if let Ok(mut state) = self.state.lock() {
            state.update_button(pad, button, value)
        } else {
            None
        };

        if let Some((port_a, port_b, pot_x, pot_y)) = ports {
            self.write_output(port_a, port_b, pot_x, pot_y);
        }
    }

    fn update_axis(&self, pad: usize, axis: ControllerAxis, value: f32) {
        let ports = if let Ok(mut state) = self.state.lock() {
            state.update_axis(pad, axis, value)
        } else {
            None
        };

        if let Some((port_a, port_b, pot_x, pot_y)) = ports {
            self.write_output(port_a, port_b, pot_x, pot_y);
        }
    }

    fn write_port_a(&self, value: u8) {
        if let Ok(mut output) = self.output.lock() {
            output.set_port_a(value);
        }
    }

    fn write_port_b(&self, value: u8) {
        if let Ok(mut output) = self.output.lock() {
            output.set_port_b(value);
        }
    }

    fn write_pot_x(&self, value: u8) {
        let ports = if let Ok(mut state) = self.state.lock() {
            state.write_primary_pot_x(value)
        } else {
            None
        };

        if let Some((port_a, port_b, pot_x, pot_y)) = ports {
            self.write_output(port_a, port_b, pot_x, pot_y);
        }
    }

    fn write_pot_y(&self, value: u8) {
        let ports = if let Ok(mut state) = self.state.lock() {
            state.write_primary_pot_y(value)
        } else {
            None
        };

        if let Some((port_a, port_b, pot_x, pot_y)) = ports {
            self.write_output(port_a, port_b, pot_x, pot_y);
        }
    }

    fn snapshot(&self) -> InputSnapshot {
        let modern = self
            .state
            .lock()
            .map(|state| state.to_modern_snapshot())
            .unwrap_or_default();

        let mut snapshot = self
            .output
            .lock()
            .map(|state| state.snapshot())
            .unwrap_or_default();
        snapshot.modern = modern;
        snapshot
    }
}

/// Adapter that proxies MMIO writes/reads to an [`InputBackend`].
pub struct InputAdapter {
    backend: Arc<dyn InputBackend>,
}

impl InputAdapter {
    pub fn new(backend: Arc<dyn InputBackend>) -> Self {
        Self { backend }
    }
}

impl ModuleAdapter for InputAdapter {
    fn handle_event(&mut self, event: ModuleAdapterEvent<'_>) {
        match event {
            ModuleAdapterEvent::PrimaryWrite(write) => {
                if let Some(reg) = write.reg.input() {
                    match reg {
                        InputReg::PortA => self.backend.write_port_a(write.module_value),
                        InputReg::PortB => self.backend.write_port_b(write.module_value),
                        InputReg::PotX => self.backend.write_pot_x(write.module_value),
                        InputReg::PotY => self.backend.write_pot_y(write.module_value),
                    }
                }
            }
            ModuleAdapterEvent::Hook { hook, action } => {
                self.backend.handle_hook(hook, action);
            }
            _ => {}
        }
    }
}

#[derive(Clone)]
struct ButtonState {
    current_value: f32,
    pressed: bool,
    last_active_value: Option<f32>,
}

impl Default for ButtonState {
    fn default() -> Self {
        Self {
            current_value: 0.0,
            pressed: false,
            last_active_value: None,
        }
    }
}

#[derive(Clone)]
struct AxisState {
    value: f32,
    last_active_value: Option<f32>,
}

impl Default for AxisState {
    fn default() -> Self {
        Self {
            value: 0.0,
            last_active_value: None,
        }
    }
}

#[derive(Clone)]
struct ControllerPadState {
    gamepad_id: Option<u32>,
    buttons: [ButtonState; BUTTONS.len()],
    axes: [AxisState; AXES.len()],
    pot_x: u8,
    pot_y: u8,
}

impl Default for ControllerPadState {
    fn default() -> Self {
        Self {
            gamepad_id: None,
            buttons: array::from_fn(|_| ButtonState::default()),
            axes: array::from_fn(|_| AxisState::default()),
            pot_x: POT_MIN,
            pot_y: POT_MIN,
        }
    }
}

impl ControllerPadState {
    fn set_gamepad(&mut self, gamepad_id: Option<u32>) {
        self.gamepad_id = gamepad_id;
    }

    fn update_button(&mut self, button: ControllerButton, value: f32) {
        if let Some(state) = self.button_state_mut(button) {
            state.current_value = value;
            state.pressed = value > BUTTON_PRESS_THRESHOLD;
            if state.pressed {
                state.last_active_value = Some(value);
            }
        }
    }

    fn update_axis(&mut self, axis: ControllerAxis, value: f32) {
        if let Some(state) = self.axis_state_mut(axis) {
            state.value = value;
            if value.abs() >= AXIS_ACTIVE_THRESHOLD {
                state.last_active_value = Some(value);
            }
        }

        match axis {
            ControllerAxis::LeftStickX => {
                self.pot_x = axis_to_pot(value);
            }
            ControllerAxis::LeftStickY => {
                self.pot_y = axis_to_pot(value);
            }
        }
    }

    fn write_pot_x(&mut self, value: u8) {
        self.pot_x = value;
        if let Some(axis) = self.axis_state_mut(ControllerAxis::LeftStickX) {
            axis.value = pot_to_axis(value);
        }
    }

    fn write_pot_y(&mut self, value: u8) {
        self.pot_y = value;
        if let Some(axis) = self.axis_state_mut(ControllerAxis::LeftStickY) {
            axis.value = pot_to_axis(value);
        }
    }

    fn button_sample(&self, button: ControllerButton) -> ButtonSample {
        let digital_pressed = self
            .button_state(button)
            .map(|state| state.pressed)
            .unwrap_or(false);
        let aggregated_pressed = digital_pressed || self.analog_drives_button(button);

        let (value, last_active) = self
            .button_state(button)
            .map(|state| (state.current_value, state.last_active_value))
            .unwrap_or((0.0, None));

        ButtonSample {
            pressed: aggregated_pressed,
            value,
            last_active_value: last_active,
        }
    }

    fn axis_sample(&self, axis: ControllerAxis) -> AxisSample {
        self.axis_state(axis)
            .map(|state| AxisSample {
                value: state.value,
                last_active_value: state.last_active_value,
            })
            .unwrap_or_default()
    }

    fn button_state(&self, button: ControllerButton) -> Option<&ButtonState> {
        button_index(button).map(|idx| &self.buttons[idx])
    }

    fn button_state_mut(&mut self, button: ControllerButton) -> Option<&mut ButtonState> {
        button_index(button).map(|idx| &mut self.buttons[idx])
    }

    fn axis_state(&self, axis: ControllerAxis) -> Option<&AxisState> {
        axis_index(axis).map(|idx| &self.axes[idx])
    }

    fn axis_state_mut(&mut self, axis: ControllerAxis) -> Option<&mut AxisState> {
        axis_index(axis).map(|idx| &mut self.axes[idx])
    }

    fn is_pressed(&self, button: ControllerButton) -> bool {
        self.button_state(button)
            .map(|state| state.pressed)
            .unwrap_or(false)
            || self.analog_drives_button(button)
    }

    fn analog_drives_button(&self, button: ControllerButton) -> bool {
        match button {
            ControllerButton::DPadLeft => self.pot_x == POT_MIN,
            ControllerButton::DPadRight => self.pot_x == POT_MAX,
            ControllerButton::DPadUp => self.pot_y == POT_MIN,
            ControllerButton::DPadDown => self.pot_y == POT_MAX,
            _ => false,
        }
    }
}

#[derive(Default)]
struct ModernInputState {
    pads: [ControllerPadState; CONTROLLER_PADS],
}

impl ModernInputState {
    fn update_gamepad(&mut self, pad: usize, gamepad_id: Option<u32>) {
        if pad < CONTROLLER_PADS {
            self.pads[pad].set_gamepad(gamepad_id);
        }
    }

    fn update_button(
        &mut self,
        pad: usize,
        button: ControllerButton,
        value: f32,
    ) -> Option<(u8, u8, u8, u8)> {
        if pad >= CONTROLLER_PADS {
            return None;
        }

        self.pads[pad].update_button(button, value);
        Some(self.primary_ports())
    }

    fn update_axis(
        &mut self,
        pad: usize,
        axis: ControllerAxis,
        value: f32,
    ) -> Option<(u8, u8, u8, u8)> {
        if pad >= CONTROLLER_PADS {
            return None;
        }

        self.pads[pad].update_axis(axis, value);
        Some(self.primary_ports())
    }

    fn write_primary_pot_x(&mut self, value: u8) -> Option<(u8, u8, u8, u8)> {
        let idx = self.primary_pad_index();
        self.pads[idx].write_pot_x(value);
        Some(self.primary_ports())
    }

    fn write_primary_pot_y(&mut self, value: u8) -> Option<(u8, u8, u8, u8)> {
        let idx = self.primary_pad_index();
        self.pads[idx].write_pot_y(value);
        Some(self.primary_ports())
    }

    fn primary_ports(&self) -> (u8, u8, u8, u8) {
        let idx = self.primary_pad_index();
        let pad = &self.pads[idx];
        let mut port_a = 0xFF;
        let mut port_b = 0xFF;

        if pad.is_pressed(ControllerButton::DPadUp) {
            port_a &= !(1 << 0);
        }
        if pad.is_pressed(ControllerButton::DPadDown) {
            port_a &= !(1 << 1);
        }
        if pad.is_pressed(ControllerButton::DPadLeft) {
            port_a &= !(1 << 2);
        }
        if pad.is_pressed(ControllerButton::DPadRight) {
            port_a &= !(1 << 3);
        }
        if pad.is_pressed(ControllerButton::South) {
            port_a &= !(1 << 4);
        }

        if pad.is_pressed(ControllerButton::Start) {
            port_b &= !(1 << 0);
        }
        if pad.is_pressed(ControllerButton::Select) {
            port_b &= !(1 << 1);
        }
        if pad.is_pressed(ControllerButton::Mode) {
            port_b &= !(1 << 2);
        }
        if pad.is_pressed(ControllerButton::LeftThumb) {
            port_b &= !(1 << 3);
        }
        if pad.is_pressed(ControllerButton::East) {
            port_b &= !(1 << 4);
        }

        (port_a, port_b, pad.pot_x, pad.pot_y)
    }

    fn primary_pad_index(&self) -> usize {
        self.pads
            .iter()
            .position(|pad| pad.gamepad_id.is_some())
            .unwrap_or(0)
    }

    fn to_modern_snapshot(&self) -> ModernInputSnapshot {
        let pads = self
            .pads
            .iter()
            .map(|pad| ModernControllerPadSnapshot {
                gamepad_id: pad.gamepad_id,
                buttons: BUTTONS
                    .iter()
                    .map(|button| (*button, pad.button_sample(*button)))
                    .collect(),
                axes: AXES
                    .iter()
                    .map(|axis| (*axis, pad.axis_sample(*axis)))
                    .collect(),
                pot_x: pad.pot_x,
                pot_y: pad.pot_y,
            })
            .collect();

        ModernInputSnapshot { pads }
    }
}

fn button_index(button: ControllerButton) -> Option<usize> {
    BUTTONS.iter().position(|candidate| *candidate == button)
}

fn axis_index(axis: ControllerAxis) -> Option<usize> {
    AXES.iter().position(|candidate| *candidate == axis)
}

fn axis_to_pot(value: f32) -> u8 {
    let clamped = value.clamp(-1.0, 1.0);
    ((clamped + 1.0) * 0.5 * 255.0).round() as u8
}

fn pot_to_axis(value: u8) -> f32 {
    (value as f32 / 255.0) * 2.0 - 1.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mmio::{ModuleAdapterEvent, PrimaryWriteEvent};
    use crate::RegId;
    use std::sync::Mutex;

    #[derive(Default)]
    struct RecordingBackend {
        port_a: Mutex<Vec<u8>>,
        port_b: Mutex<Vec<u8>>,
        pot_x: Mutex<Vec<u8>>,
        pot_y: Mutex<Vec<u8>>,
    }

    impl InputBackend for RecordingBackend {
        fn update_gamepad(&self, _pad: usize, _gamepad_id: Option<u32>) {}

        fn update_button(&self, _pad: usize, _button: ControllerButton, _value: f32) {}

        fn update_axis(&self, _pad: usize, _axis: ControllerAxis, _value: f32) {}

        fn write_port_a(&self, value: u8) {
            self.port_a.lock().unwrap().push(value);
        }

        fn write_port_b(&self, value: u8) {
            self.port_b.lock().unwrap().push(value);
        }

        fn write_pot_x(&self, value: u8) {
            self.pot_x.lock().unwrap().push(value);
        }

        fn write_pot_y(&self, value: u8) {
            self.pot_y.lock().unwrap().push(value);
        }

        fn snapshot(&self) -> InputSnapshot {
            InputSnapshot::default()
        }
    }

    #[test]
    fn primary_writes_forward_to_backend() {
        let backend = Arc::new(RecordingBackend::default());
        let mut adapter = InputAdapter::new(backend.clone());

        adapter.handle_event(ModuleAdapterEvent::PrimaryWrite(PrimaryWriteEvent {
            reg: RegId::Input(InputReg::PortA),
            cpu_value: 0,
            module_value: 0x7F,
            instance: None,
        }));

        adapter.handle_event(ModuleAdapterEvent::PrimaryWrite(PrimaryWriteEvent {
            reg: RegId::Input(InputReg::PortB),
            cpu_value: 0,
            module_value: 0xFE,
            instance: None,
        }));

        adapter.handle_event(ModuleAdapterEvent::PrimaryWrite(PrimaryWriteEvent {
            reg: RegId::Input(InputReg::PotX),
            cpu_value: 0,
            module_value: 0x12,
            instance: None,
        }));

        adapter.handle_event(ModuleAdapterEvent::PrimaryWrite(PrimaryWriteEvent {
            reg: RegId::Input(InputReg::PotY),
            cpu_value: 0,
            module_value: 0x34,
            instance: None,
        }));

        assert_eq!(backend.port_a.lock().unwrap()[..], [0x7F]);
        assert_eq!(backend.port_b.lock().unwrap()[..], [0xFE]);
        assert_eq!(backend.pot_x.lock().unwrap()[..], [0x12]);
        assert_eq!(backend.pot_y.lock().unwrap()[..], [0x34]);
    }
}
