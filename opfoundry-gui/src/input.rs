use std::collections::HashSet;
use std::convert::TryFrom;
use std::sync::Arc;

use bevy::ecs::system::{NonSend, Res, ResMut};
use bevy::input::gamepad::{
    Gamepad, GamepadAxisChangedEvent, GamepadAxisType, GamepadButtonChangedEvent,
    GamepadButtonType, GamepadConnection, GamepadConnectionEvent,
};
use bevy::input::keyboard::KeyCode;
use bevy::prelude::{EventReader, Input, Resource};
use bevy_egui::egui;
use bus::adapters::input::InputBackend;
use bus::input_mmio::{
    AxisSample, ButtonSample, ControllerAxis, ControllerButton, InputSnapshot,
    ModernControllerPadSnapshot, CONTROLLER_PAD_COUNT,
};

use crate::EmulatorState;

const CONTROLLER_PADS: usize = CONTROLLER_PAD_COUNT;

#[derive(Clone, Copy, Default)]
struct PadAssignment {
    gamepad: Option<Gamepad>,
}

#[derive(Resource)]
pub(super) struct ControllerState {
    backend: Option<Arc<dyn InputBackend>>,
    pads: [PadAssignment; CONTROLLER_PADS],
    previous_snapshot: Option<InputSnapshot>,
}

impl ControllerState {
    pub(super) fn new(
        backend: Option<Arc<dyn InputBackend>>,
        snapshot: Option<InputSnapshot>,
    ) -> Self {
        let mut state = Self {
            backend: None,
            pads: [PadAssignment::default(); CONTROLLER_PADS],
            previous_snapshot: snapshot.clone(),
        };
        state.sync_backend(backend, snapshot);
        state
    }

    pub(super) fn has_backend(&self) -> bool {
        self.backend.is_some()
    }

    pub(super) fn sync_backend(
        &mut self,
        backend: Option<Arc<dyn InputBackend>>,
        snapshot: Option<InputSnapshot>,
    ) {
        let changed = match (&self.backend, &backend) {
            (Some(current), Some(next)) => !Arc::ptr_eq(current, next),
            (None, None) => false,
            _ => true,
        };

        if changed {
            if let Some(new_backend) = backend.as_ref() {
                for (index, pad) in self.pads.iter().enumerate() {
                    let id = pad.gamepad.and_then(gamepad_id_u32);
                    new_backend.update_gamepad(index, id);
                }
            }
            self.backend = backend;
        }

        if let Some(snapshot) = snapshot {
            self.previous_snapshot = Some(snapshot);
        } else if self.backend.is_none() {
            self.previous_snapshot = None;
        }
    }

    fn handle_connect(&mut self, gamepad: Gamepad) {
        let _ = self.ensure_pad(gamepad);
    }

    fn handle_disconnect(&mut self, gamepad: Gamepad) {
        if let Some(index) = self.pad_index(gamepad) {
            self.pads[index].gamepad = None;
            if let Some(backend) = &self.backend {
                backend.update_gamepad(index, None);
            }
        }
    }

    fn handle_button(&mut self, gamepad: Gamepad, button: GamepadButtonType, value: f32) {
        let Some(mapped) = map_button(button) else {
            return;
        };
        let Some(index) = self.ensure_pad(gamepad) else {
            return;
        };
        if let Some(backend) = &self.backend {
            backend.update_button(index, mapped, value);
        }
    }

    fn handle_axis(&mut self, gamepad: Gamepad, axis: GamepadAxisType, value: f32) {
        let Some(mapped) = map_axis(axis) else {
            return;
        };
        let Some(index) = self.ensure_pad(gamepad) else {
            return;
        };
        if let Some(backend) = &self.backend {
            backend.update_axis(index, mapped, value);
        }
    }

    pub(super) fn snapshot_pair(&mut self) -> Option<(InputSnapshot, Option<InputSnapshot>)> {
        let backend = self.backend.clone()?;
        let snapshot = backend.snapshot();
        let previous = self.previous_snapshot.replace(snapshot.clone());
        Some((snapshot, previous))
    }

    fn pad_index(&self, gamepad: Gamepad) -> Option<usize> {
        self.pads.iter().position(|pad| {
            pad.gamepad
                .map(|candidate| candidate == gamepad)
                .unwrap_or(false)
        })
    }

    fn first_free_pad(&self) -> Option<usize> {
        self.pads.iter().position(|pad| pad.gamepad.is_none())
    }

    fn ensure_pad(&mut self, gamepad: Gamepad) -> Option<usize> {
        if let Some(index) = self.pad_index(gamepad) {
            return Some(index);
        }
        let index = self.first_free_pad()?;
        self.pads[index].gamepad = Some(gamepad);
        if let Some(backend) = &self.backend {
            backend.update_gamepad(index, gamepad_id_u32(gamepad));
        }
        Some(index)
    }

    pub(super) fn pad_gamepad_label(&self, index: usize) -> String {
        self.pads
            .get(index)
            .and_then(|pad| pad.gamepad)
            .and_then(gamepad_id_u32)
            .map(|id| id.to_string())
            .unwrap_or_else(|| "None".to_string())
    }
}

fn gamepad_id_u32(gamepad: Gamepad) -> Option<u32> {
    u32::try_from(gamepad.id).ok()
}

fn map_button(button: GamepadButtonType) -> Option<ControllerButton> {
    match button {
        GamepadButtonType::DPadUp => Some(ControllerButton::DPadUp),
        GamepadButtonType::DPadDown => Some(ControllerButton::DPadDown),
        GamepadButtonType::DPadLeft => Some(ControllerButton::DPadLeft),
        GamepadButtonType::DPadRight => Some(ControllerButton::DPadRight),
        GamepadButtonType::South => Some(ControllerButton::South),
        GamepadButtonType::East => Some(ControllerButton::East),
        GamepadButtonType::West => Some(ControllerButton::West),
        GamepadButtonType::North => Some(ControllerButton::North),
        GamepadButtonType::Start => Some(ControllerButton::Start),
        GamepadButtonType::Select => Some(ControllerButton::Select),
        GamepadButtonType::Mode => Some(ControllerButton::Select),
        GamepadButtonType::LeftThumb => Some(ControllerButton::LeftThumb),
        GamepadButtonType::RightThumb => Some(ControllerButton::RightThumb),
        GamepadButtonType::LeftTrigger | GamepadButtonType::LeftTrigger2 => {
            Some(ControllerButton::LeftTrigger)
        }
        GamepadButtonType::RightTrigger | GamepadButtonType::RightTrigger2 => {
            Some(ControllerButton::RightTrigger)
        }
        GamepadButtonType::C => Some(ControllerButton::LeftShoulder),
        GamepadButtonType::Z => Some(ControllerButton::RightShoulder),
        _ => None,
    }
}

fn map_axis(axis: GamepadAxisType) -> Option<ControllerAxis> {
    match axis {
        GamepadAxisType::LeftStickX => Some(ControllerAxis::LeftStickX),
        GamepadAxisType::LeftStickY => Some(ControllerAxis::LeftStickY),
        _ => None,
    }
}

#[derive(Resource, Default)]
pub(super) struct KeyboardTracker {
    current: HashSet<KeyCode>,
    last: HashSet<KeyCode>,
    previous: Vec<KeyCode>,
}

impl KeyboardTracker {
    fn update_from_input(&mut self, input: &Input<KeyCode>) {
        let pressed: HashSet<KeyCode> = input.get_pressed().copied().collect();

        if pressed.is_empty() {
            self.last.clear();
            self.current.clear();
            return;
        }

        if pressed == self.current {
            return;
        }

        let new_keys: Vec<_> = pressed.difference(&self.current).copied().collect();

        self.last = self.current.clone();
        self.current = pressed;
        self.previous.extend(new_keys);

        if self.previous.len() > 20 {
            let len = self.previous.len();
            self.previous = self.previous[len - 20..].to_vec();
        }
    }

    fn current(&self) -> Vec<String> {
        let mut keys: Vec<String> = self.current.iter().map(|key| format!("{key:?}")).collect();
        keys.sort();
        keys
    }

    fn previous(&self) -> String {
        self.previous
            .iter()
            .map(|key| format!("{key:?}"))
            .collect::<Vec<_>>()
            .join("")
    }
}

pub(super) fn update_keyboard_tracker(
    input: Res<Input<KeyCode>>,
    mut tracker: ResMut<KeyboardTracker>,
) {
    tracker.update_from_input(&input);
}

pub(super) fn sync_controller_backend(
    emulator: Option<NonSend<EmulatorState>>,
    mut controller: ResMut<ControllerState>,
) {
    let Some(emulator) = emulator else { return };
    controller.sync_backend(emulator.input_backend(), emulator.input_snapshot());
}

pub(super) fn controller_input_system(
    mut controller: ResMut<ControllerState>,
    mut connection_events: EventReader<GamepadConnectionEvent>,
    mut button_events: EventReader<GamepadButtonChangedEvent>,
    mut axis_events: EventReader<GamepadAxisChangedEvent>,
) {
    if !controller.has_backend() {
        return;
    }

    for event in connection_events.iter() {
        match &event.connection {
            GamepadConnection::Connected(_) => controller.handle_connect(event.gamepad),
            GamepadConnection::Disconnected => controller.handle_disconnect(event.gamepad),
        }
    }

    for event in button_events.iter() {
        controller.handle_button(event.gamepad, event.button_type, event.value);
    }

    for event in axis_events.iter() {
        controller.handle_axis(event.gamepad, event.axis_type, event.value);
    }
}

pub(super) fn render_controller_pad(
    ui: &mut egui::Ui,
    index: usize,
    gamepad_label: &str,
    modern: Option<&ModernControllerPadSnapshot>,
) {
    ui.heading(format!("Controller {index}"));
    ui.label(format!("Gamepad ID: {gamepad_label}"));

    ui.add_space(6.0);
    if let Some(modern) = modern {
        ui.label("Buttons");
        egui::Grid::new(format!("controller_{index}_buttons"))
            .striped(true)
            .show(ui, |grid| {
                grid.label("Button");
                grid.label("Value");
                grid.end_row();
                for (button, sample) in &modern.buttons {
                    grid.label(controller_button_label(*button));
                    grid.label(button_current_text(sample));
                    grid.end_row();
                }
            });

        ui.add_space(6.0);
        ui.label("Axes");
        egui::Grid::new(format!("controller_{index}_axes"))
            .striped(true)
            .show(ui, |grid| {
                grid.label("Axis");
                grid.label("Value");
                grid.end_row();
                for (axis, sample) in &modern.axes {
                    grid.label(controller_axis_label(*axis));
                    grid.label(axis_current_text(sample));
                    grid.end_row();
                }
            });

        ui.add_space(6.0);
        ui.label(format!("Pot X (modern): 0x{:02X}", modern.pot_x));
        ui.label(format!("Pot Y (modern): 0x{:02X}", modern.pot_y));
    } else {
        ui.label("Modern telemetry unavailable.");
    }
}

fn controller_button_label(button: ControllerButton) -> &'static str {
    match button {
        ControllerButton::DPadUp => "D-Pad Up",
        ControllerButton::DPadDown => "D-Pad Down",
        ControllerButton::DPadLeft => "D-Pad Left",
        ControllerButton::DPadRight => "D-Pad Right",
        ControllerButton::South => "South",
        ControllerButton::East => "East",
        ControllerButton::West => "West",
        ControllerButton::North => "North",
        ControllerButton::Start => "Start",
        ControllerButton::Select => "Select",
        ControllerButton::LeftShoulder => "Left Shoulder",
        ControllerButton::RightShoulder => "Right Shoulder",
        ControllerButton::LeftThumb => "Left Thumb",
        ControllerButton::RightThumb => "Right Thumb",
        ControllerButton::LeftTrigger => "Left Trigger",
        ControllerButton::RightTrigger => "Right Trigger",
    }
}

fn controller_axis_label(axis: ControllerAxis) -> &'static str {
    match axis {
        ControllerAxis::LeftStickX => "Left Stick X",
        ControllerAxis::LeftStickY => "Left Stick Y",
    }
}

fn button_current_text(sample: &ButtonSample) -> String {
    let label = if sample.pressed {
        "Pressed"
    } else {
        "Released"
    };
    if sample.value.abs() > f32::EPSILON {
        format!("{label} ({:.2})", sample.value)
    } else {
        label.to_string()
    }
}

fn axis_current_text(sample: &AxisSample) -> String {
    format!("{:.2}", sample.value)
}

pub(super) fn render_keyboard_section(ui: &mut egui::Ui, tracker: &KeyboardTracker) {
    let current = tracker.current();
    let last = tracker.previous();

    let current_text = if current.is_empty() {
        "None".to_string()
    } else {
        current.join(", ")
    };

    let last_text = if last.is_empty() {
        "None".to_string()
    } else {
        last
    };

    ui.horizontal(|ui| {
        ui.label("Current:");
        ui.monospace(current_text.as_str());
    });
    ui.horizontal(|ui| {
        ui.label("Last:");
        ui.monospace(last_text.as_str());
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct TestBackend {
        gamepad_updates: Mutex<Vec<(usize, Option<u32>)>>,
    }

    impl TestBackend {
        fn new() -> Self {
            Self {
                gamepad_updates: Mutex::new(Vec::new()),
            }
        }

        fn updates(&self) -> Vec<(usize, Option<u32>)> {
            self.gamepad_updates
                .lock()
                .expect("test backend updates lock")
                .clone()
        }
    }

    impl InputBackend for TestBackend {
        fn update_gamepad(&self, pad: usize, gamepad_id: Option<u32>) {
            self.gamepad_updates
                .lock()
                .expect("test backend update lock")
                .push((pad, gamepad_id));
        }

        fn update_button(&self, _pad: usize, _button: ControllerButton, _value: f32) {}

        fn update_axis(&self, _pad: usize, _axis: ControllerAxis, _value: f32) {}

        fn write_buttons_lo(&self, _pad: usize, _value: u8) {}

        fn write_buttons_hi(&self, _pad: usize, _value: u8) {}

        fn write_pot_x(&self, _pad: usize, _value: u8) {}

        fn write_pot_y(&self, _pad: usize, _value: u8) {}

        fn snapshot(&self) -> InputSnapshot {
            InputSnapshot::default()
        }
    }

    #[test]
    fn sync_backend_replays_pad_assignments() {
        let mut state = ControllerState::new(None, None);
        let backend = Arc::new(TestBackend::new());

        state.sync_backend(Some(backend.clone()), None);

        let updates = backend.updates();
        assert_eq!(updates.len(), CONTROLLER_PADS);
        assert!(updates.iter().all(|(_, id)| id.is_none()));
    }

    #[test]
    fn sync_backend_clears_previous_snapshot_when_detached() {
        let mut state = ControllerState::new(None, Some(InputSnapshot::default()));

        state.sync_backend(None, None);

        assert!(state.previous_snapshot.is_none());
    }

    #[test]
    fn keyboard_tracker_tracks_and_deduplicates_pressed_keys() {
        let mut tracker = KeyboardTracker::default();
        let mut input = Input::<KeyCode>::default();

        input.press(KeyCode::A);
        tracker.update_from_input(&input);

        assert_eq!(tracker.current(), vec!["A".to_string()]);
        assert_eq!(tracker.previous(), "A");

        tracker.update_from_input(&input);
        assert_eq!(tracker.previous(), "A");

        input.release(KeyCode::A);
        tracker.update_from_input(&input);
        assert!(tracker.current().is_empty());
    }
}
