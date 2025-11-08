//! Bridges MMIO register activity for the `input.joystick` module to a modern
//! controller backend, keeping the shared button/paddle snapshots in sync with
//! Bevy’s gamepad events.

use std::array;
use std::sync::{Arc, Mutex};

use crate::input_mmio::{
    button_bit, AxisSample, ButtonSample, ControllerAxis, ControllerButton, InputOutput,
    InputPadSnapshot, InputSnapshot, ModernControllerPadSnapshot, ModernInputSnapshot,
    CONTROLLER_PAD_COUNT, POT_MAX, POT_MIN, POT_NEUTRAL,
};
use crate::mmio::{HookAction, InputReg, ModuleAdapter, ModuleAdapterEvent, ScatterWriteEvent};

const BUTTON_PRESS_THRESHOLD: f32 = 0.5;
const AXIS_ACTIVE_THRESHOLD: f32 = 0.2;

const BUTTONS: [ControllerButton; 16] = [
    ControllerButton::DPadUp,
    ControllerButton::DPadDown,
    ControllerButton::DPadLeft,
    ControllerButton::DPadRight,
    ControllerButton::South,
    ControllerButton::Start,
    ControllerButton::Select,
    ControllerButton::LeftShoulder,
    ControllerButton::RightShoulder,
    ControllerButton::LeftTrigger,
    ControllerButton::RightTrigger,
    ControllerButton::LeftThumb,
    ControllerButton::RightThumb,
    ControllerButton::East,
    ControllerButton::West,
    ControllerButton::North,
];

const AXES: [ControllerAxis; 2] = [ControllerAxis::LeftStickX, ControllerAxis::LeftStickY];

/// Contract implemented by host backends that mirror MMIO writes and emit live
/// controller telemetry.
pub trait InputBackend: Send + Sync {
    fn update_gamepad(&self, pad: usize, gamepad_id: Option<u32>);
    fn update_button(&self, pad: usize, button: ControllerButton, value: f32);
    fn update_axis(&self, pad: usize, axis: ControllerAxis, value: f32);

    fn write_buttons_lo(&self, pad: usize, value: u8);
    fn write_buttons_hi(&self, pad: usize, value: u8);
    fn write_pot_x(&self, pad: usize, value: u8);
    fn write_pot_y(&self, pad: usize, value: u8);

    fn snapshot(&self) -> InputSnapshot;
    fn handle_hook(&self, _hook: &str, _action: HookAction) {}
}

/// Default backend used by the runtime to map MMIO traffic into the shared
/// [`InputOutput`] snapshot while tracking modern button/axis telemetry.
pub struct InputBackendHandle {
    output: Arc<Mutex<InputOutput>>,
    state: Arc<Mutex<ModernInputState>>,
}

impl InputBackendHandle {
    pub fn new(output: Arc<Mutex<InputOutput>>) -> Self {
        Self {
            output,
            state: Arc::new(Mutex::new(ModernInputState::default())),
        }
    }

    /// Push the latest pad snapshot into the shared MMIO output.
    fn publish(&self, publish: PadPublish) {
        if let Ok(mut output) = self.output.lock() {
            output.set_pad_snapshot(publish.pad, publish.snapshot);
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
        let publish = self
            .state
            .lock()
            .ok()
            .and_then(|mut state| state.update_button(pad, button, value));
        if let Some(publish) = publish {
            self.publish(publish);
        }
    }

    fn update_axis(&self, pad: usize, axis: ControllerAxis, value: f32) {
        let publish = self
            .state
            .lock()
            .ok()
            .and_then(|mut state| state.update_axis(pad, axis, value));
        if let Some(publish) = publish {
            self.publish(publish);
        }
    }

    fn write_buttons_lo(&self, pad: usize, value: u8) {
        let publish = self
            .state
            .lock()
            .ok()
            .and_then(|mut state| state.write_buttons_lo(pad, value));
        if let Some(publish) = publish {
            self.publish(publish);
        }
    }

    fn write_buttons_hi(&self, pad: usize, value: u8) {
        let publish = self
            .state
            .lock()
            .ok()
            .and_then(|mut state| state.write_buttons_hi(pad, value));
        if let Some(publish) = publish {
            self.publish(publish);
        }
    }

    fn write_pot_x(&self, pad: usize, value: u8) {
        let publish = self
            .state
            .lock()
            .ok()
            .and_then(|mut state| state.write_pot_x(pad, value));
        if let Some(publish) = publish {
            self.publish(publish);
        }
    }

    fn write_pot_y(&self, pad: usize, value: u8) {
        let publish = self
            .state
            .lock()
            .ok()
            .and_then(|mut state| state.write_pot_y(pad, value));
        if let Some(publish) = publish {
            self.publish(publish);
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

    fn handle_hook(&self, hook: &str, action: HookAction) {
        if let Ok(mut state) = self.state.lock() {
            state.handle_hook(hook, action);
        }
    }
}

/// Module adapter that listens to MMIO events and forwards them to an
/// [`InputBackend`], keeping the logical pad state coherent.
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
                let pad = write.instance.unwrap_or(0) as usize;
                match write.reg.input() {
                    Some(InputReg::ButtonsLo) => {
                        self.backend.write_buttons_lo(pad, write.module_value)
                    }
                    Some(InputReg::ButtonsHi) => {
                        self.backend.write_buttons_hi(pad, write.module_value)
                    }
                    Some(InputReg::PotX) => self.backend.write_pot_x(pad, write.module_value),
                    Some(InputReg::PotY) => self.backend.write_pot_y(pad, write.module_value),
                    Some(InputReg::Select) | None => {}
                }
            }
            ModuleAdapterEvent::ScatterWrite(write) => self.handle_scatter(write),
            ModuleAdapterEvent::Hook { hook, action } => self.backend.handle_hook(hook, action),
            _ => {}
        }
    }
}

impl InputAdapter {
    /// Handle scatter updates (bit-level writes) against the input registers.
    fn handle_scatter(&self, write: ScatterWriteEvent) {
        let pad = write.instance.unwrap_or(0) as usize;
        let snapshot = self.backend.snapshot();
        let pad_snapshot = snapshot.pads.get(pad).cloned().unwrap_or_default();
        match write.reg.input() {
            Some(InputReg::ButtonsLo) => {
                let mut value = pad_snapshot.buttons as u8;
                value = update_bit(value, write.target_bit, write.bit_value);
                self.backend.write_buttons_lo(pad, value);
            }
            Some(InputReg::ButtonsHi) => {
                let mut value = (pad_snapshot.buttons >> 8) as u8;
                value = update_bit(value, write.target_bit, write.bit_value);
                self.backend.write_buttons_hi(pad, value);
            }
            _ => {}
        }
    }
}

fn update_bit(mut value: u8, bit: u8, set: bool) -> u8 {
    let mask = 1u8 << bit;
    if set {
        value |= mask;
    } else {
        value &= !mask;
    }
    value
}

/// Aggregated button/axis state for each tracked pad.
#[derive(Default)]
struct ModernInputState {
    pads: [ControllerPadState; CONTROLLER_PAD_COUNT],
}

impl ModernInputState {
    fn update_gamepad(&mut self, pad: usize, gamepad_id: Option<u32>) {
        if let Some(state) = self.pads.get_mut(pad) {
            state.gamepad_id = gamepad_id;
        }
    }

    fn update_button(
        &mut self,
        pad: usize,
        button: ControllerButton,
        value: f32,
    ) -> Option<PadPublish> {
        let state = self.pads.get_mut(pad)?;
        state.set_button_value(button, value);
        Some(PadPublish::new(pad, state.pad_snapshot()))
    }

    fn update_axis(&mut self, pad: usize, axis: ControllerAxis, value: f32) -> Option<PadPublish> {
        let state = self.pads.get_mut(pad)?;
        state.set_axis_value(axis, value);
        Some(PadPublish::new(pad, state.pad_snapshot()))
    }

    fn write_buttons_lo(&mut self, pad: usize, value: u8) -> Option<PadPublish> {
        let state = self.pads.get_mut(pad)?;
        let current = state.digital_mask;
        let new_mask = (current & 0xFF00) | value as u16;
        state.apply_digital_mask(new_mask);
        Some(PadPublish::new(pad, state.pad_snapshot()))
    }

    fn write_buttons_hi(&mut self, pad: usize, value: u8) -> Option<PadPublish> {
        let state = self.pads.get_mut(pad)?;
        let current = state.digital_mask;
        let new_mask = (current & 0x00FF) | ((value as u16) << 8);
        state.apply_digital_mask(new_mask);
        Some(PadPublish::new(pad, state.pad_snapshot()))
    }

    fn write_pot_x(&mut self, pad: usize, value: u8) -> Option<PadPublish> {
        let state = self.pads.get_mut(pad)?;
        state.set_pot_x(value);
        Some(PadPublish::new(pad, state.pad_snapshot()))
    }

    fn write_pot_y(&mut self, pad: usize, value: u8) -> Option<PadPublish> {
        let state = self.pads.get_mut(pad)?;
        state.set_pot_y(value);
        Some(PadPublish::new(pad, state.pad_snapshot()))
    }

    fn to_modern_snapshot(&self) -> ModernInputSnapshot {
        let pads = self
            .pads
            .iter()
            .map(ControllerPadState::modern_snapshot)
            .collect();
        ModernInputSnapshot { pads }
    }

    fn handle_hook(&mut self, _hook: &str, _action: HookAction) {
        // Hooks not used yet; kept for future adapter diagnostics.
    }
}

struct PadPublish {
    pad: usize,
    snapshot: InputPadSnapshot,
}

impl PadPublish {
    fn new(pad: usize, snapshot: InputPadSnapshot) -> Self {
        Self { pad, snapshot }
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
/// Live controller state for a single pad, including synthesized button
/// masking and paddle values.
struct ControllerPadState {
    gamepad_id: Option<u32>,
    buttons: [ButtonState; BUTTONS.len()],
    axes: [AxisState; AXES.len()],
    digital_mask: u16,
    pot_x: u8,
    pot_y: u8,
}

impl Default for ControllerPadState {
    fn default() -> Self {
        Self {
            gamepad_id: None,
            buttons: array::from_fn(|_| ButtonState::default()),
            axes: array::from_fn(|_| AxisState::default()),
            digital_mask: 0,
            pot_x: POT_NEUTRAL,
            pot_y: POT_NEUTRAL,
        }
    }
}

impl ControllerPadState {
    fn set_button_value(&mut self, button: ControllerButton, value: f32) {
        if let Some(state) = self.button_state_mut(button) {
            state.current_value = value;
            state.pressed = value > BUTTON_PRESS_THRESHOLD;
            if state.pressed {
                state.last_active_value = Some(value);
                self.digital_mask |= button_bit(button);
            } else {
                self.digital_mask &= !button_bit(button);
            }
        }
    }

    fn set_axis_value(&mut self, axis: ControllerAxis, value: f32) {
        if let Some(state) = self.axis_state_mut(axis) {
            state.value = value;
            if value.abs() >= AXIS_ACTIVE_THRESHOLD {
                state.last_active_value = Some(value);
            }
        }
        match axis {
            ControllerAxis::LeftStickX => self.pot_x = axis_to_pot(value),
            ControllerAxis::LeftStickY => self.pot_y = axis_to_pot(value),
        }
    }

    fn apply_digital_mask(&mut self, mask: u16) {
        self.digital_mask = mask;
        for button in BUTTONS {
            let pressed = (mask & button_bit(button)) != 0;
            if let Some(state) = self.button_state_mut(button) {
                state.pressed = pressed;
                state.current_value = if pressed { 1.0 } else { 0.0 };
                if pressed {
                    state.last_active_value = Some(1.0);
                }
            }
        }
    }

    fn set_pot_x(&mut self, value: u8) {
        self.pot_x = value;
        if let Some(state) = self.axis_state_mut(ControllerAxis::LeftStickX) {
            state.value = pot_to_axis(value);
        }
    }

    fn set_pot_y(&mut self, value: u8) {
        self.pot_y = value;
        if let Some(state) = self.axis_state_mut(ControllerAxis::LeftStickY) {
            state.value = pot_to_axis(value);
        }
    }

    fn pad_snapshot(&self) -> InputPadSnapshot {
        let buttons = self.buttons_mask();
        InputPadSnapshot {
            buttons,
            pot_x: self.pot_x,
            pot_y: self.pot_y,
        }
    }

    fn buttons_mask(&self) -> u16 {
        self.digital_mask | self.analog_mask()
    }

    fn analog_mask(&self) -> u16 {
        let mut mask = 0;
        if self.pot_x == POT_MIN {
            mask |= button_bit(ControllerButton::DPadLeft);
        } else if self.pot_x == POT_MAX {
            mask |= button_bit(ControllerButton::DPadRight);
        }

        if self.pot_y == POT_MIN {
            mask |= button_bit(ControllerButton::DPadUp);
        } else if self.pot_y == POT_MAX {
            mask |= button_bit(ControllerButton::DPadDown);
        }
        mask
    }

    fn button_state_mut(&mut self, button: ControllerButton) -> Option<&mut ButtonState> {
        BUTTONS
            .iter()
            .position(|candidate| *candidate == button)
            .map(|index| &mut self.buttons[index])
    }

    fn button_state(&self, button: ControllerButton) -> Option<&ButtonState> {
        BUTTONS
            .iter()
            .position(|candidate| *candidate == button)
            .map(|index| &self.buttons[index])
    }

    fn axis_state_mut(&mut self, axis: ControllerAxis) -> Option<&mut AxisState> {
        AXES.iter()
            .position(|candidate| *candidate == axis)
            .map(|index| &mut self.axes[index])
    }

    fn axis_state(&self, axis: ControllerAxis) -> Option<&AxisState> {
        AXES.iter()
            .position(|candidate| *candidate == axis)
            .map(|index| &self.axes[index])
    }

    fn modern_snapshot(&self) -> ModernControllerPadSnapshot {
        let buttons = BUTTONS
            .iter()
            .map(|button| {
                let digital = self.button_state(*button).cloned().unwrap_or_default();
                let analog_pressed = (self.analog_mask() & button_bit(*button)) != 0;
                let pressed = digital.pressed || analog_pressed;
                let value = if digital.pressed {
                    digital.current_value
                } else if analog_pressed {
                    1.0
                } else {
                    0.0
                };
                let mut sample = ButtonSample {
                    pressed,
                    value,
                    last_active_value: digital.last_active_value,
                };
                if analog_pressed && sample.last_active_value.is_none() {
                    sample.last_active_value = Some(1.0);
                }
                (*button, sample)
            })
            .collect();

        let axes = AXES
            .iter()
            .map(|axis| {
                let state = self.axis_state(*axis).cloned().unwrap_or_default();
                (
                    *axis,
                    AxisSample {
                        value: state.value,
                        last_active_value: state.last_active_value,
                    },
                )
            })
            .collect();

        ModernControllerPadSnapshot {
            gamepad_id: self.gamepad_id,
            buttons,
            axes,
            pot_x: self.pot_x,
            pot_y: self.pot_y,
        }
    }
}

fn axis_to_pot(value: f32) -> u8 {
    let clamped = value.clamp(-1.0, 1.0);
    ((clamped + 1.0) * 0.5 * POT_MAX as f32).round() as u8
}

fn pot_to_axis(value: u8) -> f32 {
    (value as f32 / POT_MAX as f32) * 2.0 - 1.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mmio::{ModuleAdapterEvent, PrimaryWriteEvent, RegId};

    #[derive(Default)]
    struct RecordingBackend {
        events: Mutex<Vec<(usize, InputReg, u8)>>,
    }

    impl InputBackend for RecordingBackend {
        fn update_gamepad(&self, _pad: usize, _gamepad_id: Option<u32>) {}
        fn update_button(&self, _pad: usize, _button: ControllerButton, _value: f32) {}
        fn update_axis(&self, _pad: usize, _axis: ControllerAxis, _value: f32) {}

        fn write_buttons_lo(&self, pad: usize, value: u8) {
            self.events
                .lock()
                .unwrap()
                .push((pad, InputReg::ButtonsLo, value));
        }

        fn write_buttons_hi(&self, pad: usize, value: u8) {
            self.events
                .lock()
                .unwrap()
                .push((pad, InputReg::ButtonsHi, value));
        }

        fn write_pot_x(&self, pad: usize, value: u8) {
            self.events
                .lock()
                .unwrap()
                .push((pad, InputReg::PotX, value));
        }

        fn write_pot_y(&self, pad: usize, value: u8) {
            self.events
                .lock()
                .unwrap()
                .push((pad, InputReg::PotY, value));
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
            reg: RegId::Input(InputReg::ButtonsLo),
            cpu_value: 0,
            module_value: 0x7F,
            instance: Some(0),
        }));

        adapter.handle_event(ModuleAdapterEvent::PrimaryWrite(PrimaryWriteEvent {
            reg: RegId::Input(InputReg::ButtonsLo),
            cpu_value: 0,
            module_value: 0x55,
            instance: Some(1),
        }));

        let events = backend.events.lock().unwrap().clone();
        assert_eq!(
            events,
            vec![
                (0, InputReg::ButtonsLo, 0x7F),
                (1, InputReg::ButtonsLo, 0x55)
            ]
        );
    }
}
