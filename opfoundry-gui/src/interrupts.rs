use std::sync::Arc;
use std::time::Duration;

use bevy::ecs::system::{Res, ResMut};
use bevy::input::gamepad::GamepadEvent;
use bevy::input::keyboard::KeyboardInput;
use bevy::input::ButtonState;
use bevy::prelude::{EventReader, Resource, Time, Timer, TimerMode};
use bus::interrupts::{InterruptController, InterruptSnapshot};
use bus::personality::Personality;

#[derive(Resource)]
pub(super) struct InterruptBindings {
    controller: Arc<InterruptController>,
    pub(super) frame_start: Option<u32>,
    pub(super) frame_end: Option<u32>,
    pub(super) timer0: Option<u32>,
    pub(super) keyboard: Option<u32>,
    pub(super) gamepad: Option<u32>,
}

impl InterruptBindings {
    pub(super) fn from_personality(
        controller: Arc<InterruptController>,
        personality: &'static Personality,
    ) -> Option<Self> {
        let mut bindings = Self {
            controller,
            frame_start: None,
            frame_end: None,
            timer0: None,
            keyboard: None,
            gamepad: None,
        };

        for interrupt in personality.interrupts {
            let mask = 1u32 << interrupt.id;
            match interrupt.name {
                "frame_start" => bindings.frame_start = Some(mask),
                "frame_end" => bindings.frame_end = Some(mask),
                "timer0" => bindings.timer0 = Some(mask),
                "keyboard_event" => bindings.keyboard = Some(mask),
                "gamepad_event" => bindings.gamepad = Some(mask),
                _ => {}
            }
        }

        if bindings.has_any() {
            Some(bindings)
        } else {
            None
        }
    }

    fn has_any(&self) -> bool {
        self.frame_start.is_some()
            || self.frame_end.is_some()
            || self.timer0.is_some()
            || self.keyboard.is_some()
            || self.gamepad.is_some()
    }

    pub(super) fn has_timer(&self) -> bool {
        self.timer0.is_some()
    }

    pub(super) fn has_keyboard(&self) -> bool {
        self.keyboard.is_some()
    }

    pub(super) fn has_gamepad(&self) -> bool {
        self.gamepad.is_some()
    }

    pub(super) fn snapshot(&self) -> InterruptSnapshot {
        self.controller.snapshot()
    }

    pub(super) fn raise_frame_start(&self) {
        if let Some(mask) = self.frame_start {
            self.raise_nmi(mask);
        }
    }

    pub(super) fn raise_frame_end(&self) {
        if let Some(mask) = self.frame_end {
            self.raise_irq(mask);
        }
    }

    pub(super) fn raise_timer0(&self) {
        if let Some(mask) = self.timer0 {
            self.raise_irq(mask);
        }
    }

    pub(super) fn raise_keyboard(&self) {
        if let Some(mask) = self.keyboard {
            self.raise_irq(mask);
        }
    }

    pub(super) fn raise_gamepad(&self) {
        if let Some(mask) = self.gamepad {
            self.raise_irq(mask);
        }
    }

    fn raise_irq(&self, mask: u32) {
        if mask != 0 && (self.controller.irq_pending() & mask) == 0 {
            self.controller.raise_irq(mask);
        }
    }

    fn raise_nmi(&self, mask: u32) {
        if mask != 0 && (self.controller.nmi_pending() & mask) == 0 {
            self.controller.raise_nmi(mask);
        }
    }
}

#[derive(Resource)]
pub(super) struct TimerInterruptState {
    timer: Timer,
}

impl TimerInterruptState {
    pub(super) fn new(period: Duration) -> Self {
        Self {
            timer: Timer::new(period, TimerMode::Repeating),
        }
    }
}

pub(super) fn emit_frame_start_interrupt(bindings: Option<Res<InterruptBindings>>) {
    if let Some(bindings) = bindings {
        bindings.raise_frame_start();
    }
}

pub(super) fn emit_frame_end_interrupt(bindings: Option<Res<InterruptBindings>>) {
    if let Some(bindings) = bindings {
        bindings.raise_frame_end();
    }
}

pub(super) fn timer_interrupt_system(
    time: Res<Time>,
    bindings: Option<Res<InterruptBindings>>,
    state: Option<ResMut<TimerInterruptState>>,
) {
    let Some(bindings) = bindings else { return };
    if !bindings.has_timer() {
        return;
    }
    let Some(mut state) = state else { return };
    if state.timer.tick(time.delta()).just_finished() {
        bindings.raise_timer0();
    }
}

pub(super) fn keyboard_interrupt_system(
    bindings: Option<Res<InterruptBindings>>,
    mut events: EventReader<KeyboardInput>,
) {
    let bindings = match bindings {
        Some(bindings) if bindings.has_keyboard() => bindings,
        _ => return,
    };

    for event in events.iter() {
        if matches!(event.state, ButtonState::Pressed | ButtonState::Released) {
            bindings.raise_keyboard();
            break;
        }
    }
}

pub(super) fn gamepad_interrupt_system(
    bindings: Option<Res<InterruptBindings>>,
    mut events: EventReader<GamepadEvent>,
) {
    let bindings = match bindings {
        Some(bindings) if bindings.has_gamepad() => bindings,
        _ => return,
    };

    if events.iter().next().is_some() {
        bindings.raise_gamepad();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bus::personality::MODERN_RETRO;

    #[test]
    fn modern_retro_interrupts_fire_from_host_events() {
        let controller = Arc::new(InterruptController::new());
        controller.set_irq_enable((1 << 1) | (1 << 2) | (1 << 3) | (1 << 4));

        let bindings = InterruptBindings::from_personality(controller.clone(), &MODERN_RETRO)
            .expect("modern-retro bindings");

        bindings.raise_frame_start();
        let snapshot = controller.snapshot();
        assert_eq!(snapshot.nmi_pending & (1 << 0), 1 << 0);
        assert!(snapshot.nmi_line);
        assert!(controller.take_nmi_edge());
        controller.clear_nmi(1 << 0);

        bindings.raise_frame_end();
        let snapshot = controller.snapshot();
        assert_eq!(snapshot.irq_pending & (1 << 1), 1 << 1);
        assert!(snapshot.irq_line);
        controller.clear_irq(1 << 1);

        bindings.raise_timer0();
        let snapshot = controller.snapshot();
        assert_eq!(snapshot.irq_pending & (1 << 2), 1 << 2);
        controller.clear_irq(1 << 2);

        bindings.raise_keyboard();
        let snapshot = controller.snapshot();
        assert_eq!(snapshot.irq_pending & (1 << 3), 1 << 3);
        controller.clear_irq(1 << 3);

        bindings.raise_gamepad();
        let snapshot = controller.snapshot();
        assert_eq!(snapshot.irq_pending & (1 << 4), 1 << 4);
        controller.clear_irq(1 << 4);

        let snapshot = controller.snapshot();
        assert_eq!(snapshot.irq_pending, 0);
        assert_eq!(snapshot.nmi_pending, 0);
        assert!(!snapshot.irq_line);
        assert!(!snapshot.nmi_line);
    }
}
