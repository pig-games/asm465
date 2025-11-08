//! Sprite module adapter that aggregates per-slot MMIO updates before handing
//! them off to a backend renderer.

use std::sync::{Arc, Mutex};

use crate::mmio::{
    FanoutWriteEvent, ModuleAdapter, ModuleAdapterEvent, PrimaryWriteEvent, ScatterWriteEvent,
    SpriteReg,
};
use crate::sprite_mmio::{SpriteOutput, SpriteState, SPRITE_SLOTS};

/// Snapshot of a single sprite slot consumed by sprite backends.
#[derive(Clone, Copy, Debug, Default)]
pub struct SpriteRenderState {
    pub number: u8,
    pub anim: u8,
    pub x: u16,
    pub y: u16,
    pub scale_x: u8,
    pub scale_y: u8,
    pub enabled: bool,
}

/// Backend interface that consumes sprite state updates emitted by the adapter.
pub trait SpriteBackend: Send + Sync {
    fn update_sprite(&self, index: u8, state: SpriteRenderState);
}

/// Backend implementation that mirrors sprite state into the classic [`SpriteOutput`].
pub struct SpriteOutputBackend {
    output: Arc<Mutex<SpriteOutput>>,
}

impl SpriteOutputBackend {
    pub fn new(output: Arc<Mutex<SpriteOutput>>) -> Self {
        Self { output }
    }
}

impl SpriteBackend for SpriteOutputBackend {
    fn update_sprite(&self, index: u8, state: SpriteRenderState) {
        if let Ok(mut guard) = self.output.lock() {
            guard.set_sprite(
                index as usize,
                SpriteState {
                    number: state.number,
                    anim: state.anim,
                    x: state.x,
                    y: state.y,
                    scale_x: state.scale_x,
                    scale_y: state.scale_y,
                    enabled: state.enabled,
                },
            );
        }
    }
}

/// Adapter that bridges sprite MMIO activity to a [`SpriteBackend`].
pub struct SpriteAdapter {
    backend: Arc<dyn SpriteBackend>,
    sprites: Vec<SpriteRenderState>,
    current_select: u8,
}

impl SpriteAdapter {
    pub fn new(backend: Arc<dyn SpriteBackend>) -> Self {
        let mut adapter = Self {
            backend,
            sprites: Vec::new(),
            current_select: 0,
        };
        adapter.ensure_len(SPRITE_SLOTS);
        adapter
    }

    fn resolve_index(&self, instance: Option<u8>) -> Option<u8> {
        instance.or(Some(self.current_select))
    }

    fn ensure_len(&mut self, count: usize) {
        if self.sprites.len() < count {
            self.sprites.resize(count, SpriteRenderState::default());
        }
    }

    fn ensure_state(&mut self, index: u8) -> &mut SpriteRenderState {
        let idx = index as usize;
        self.ensure_len(idx + 1);
        &mut self.sprites[idx]
    }

    fn flush(&self, index: u8) {
        if let Some(state) = self.sprites.get(index as usize) {
            self.backend.update_sprite(index, *state);
        }
    }

    fn handle_primary(&mut self, event: PrimaryWriteEvent) {
        let Some(reg) = event.reg.sprite() else {
            return;
        };

        if reg == SpriteReg::Select {
            self.current_select = event.module_value;
            return;
        }

        let Some(index) = self.resolve_index(event.instance) else {
            return;
        };

        let state = self.ensure_state(index);

        match reg {
            SpriteReg::Number => state.number = event.module_value,
            SpriteReg::Anim => state.anim = event.module_value,
            SpriteReg::XLo => {
                state.x = (state.x & 0xFF00) | event.module_value as u16;
            }
            SpriteReg::XHi => {
                state.x = (state.x & 0x00FF) | ((event.module_value as u16) << 8);
            }
            SpriteReg::YLo => {
                state.y = (state.y & 0xFF00) | event.module_value as u16;
            }
            SpriteReg::YHi => {
                state.y = (state.y & 0x00FF) | ((event.module_value as u16) << 8);
            }
            SpriteReg::Scale => {
                state.scale_x = (event.module_value >> 4) & 0x0F;
                state.scale_y = event.module_value & 0x0F;
            }
            SpriteReg::Enable => {
                state.enabled = (event.module_value & 0x01) != 0;
            }
            SpriteReg::Select => {} // handled above
        }

        self.flush(index);
    }

    fn handle_scatter(&mut self, event: ScatterWriteEvent) {
        let Some(reg) = event.reg.sprite() else {
            return;
        };
        let Some(index) = self.resolve_index(event.instance) else {
            return;
        };

        let state = self.ensure_state(index);

        match reg {
            SpriteReg::XLo => {
                let mut low = (state.x & 0x00FF) as u8;
                let mask = 1u8 << event.target_bit.min(7);
                if event.bit_value {
                    low |= mask;
                } else {
                    low &= !mask;
                }
                state.x = (state.x & 0xFF00) | low as u16;
            }
            SpriteReg::XHi => {
                // Treat the register value as the full high byte (typically only bit 0 is used).
                state.x = (state.x & 0x00FF) | ((event.module_value as u16) << 8);
            }
            SpriteReg::Enable => {
                let bit = (event.module_value >> event.source_bit) & 0x01;
                state.enabled = bit != 0;
            }
            SpriteReg::Scale => {
                state.scale_x = (event.module_value >> 4) & 0x0F;
                state.scale_y = event.module_value & 0x0F;
            }
            _ => {}
        }

        self.flush(index);
    }

    fn handle_fanout(&mut self, event: FanoutWriteEvent) {
        let Some(reg) = event.reg.sprite() else {
            return;
        };

        let apply = |state: &mut SpriteRenderState| match reg {
            SpriteReg::Enable => state.enabled = (event.value & 0x01) != 0,
            SpriteReg::Number => state.number = event.value,
            SpriteReg::Anim => state.anim = event.value,
            SpriteReg::XLo => state.x = (state.x & 0xFF00) | event.value as u16,
            SpriteReg::XHi => state.x = (state.x & 0x00FF) | ((event.value as u16) << 8),
            SpriteReg::YLo => state.y = (state.y & 0xFF00) | event.value as u16,
            SpriteReg::YHi => state.y = (state.y & 0x00FF) | ((event.value as u16) << 8),
            SpriteReg::Scale => {
                state.scale_x = (event.value >> 4) & 0x0F;
                state.scale_y = event.value & 0x0F;
            }
            SpriteReg::Select => {}
        };

        match event.target_instance {
            Some(index) => {
                let state = self.ensure_state(index);
                apply(state);
                self.flush(index);
            }
            None => {
                self.ensure_len(SPRITE_SLOTS);
                for idx in 0..self.sprites.len() {
                    let state = &mut self.sprites[idx];
                    apply(state);
                    self.backend.update_sprite(idx as u8, *state);
                }
            }
        }
    }
}

impl ModuleAdapter for SpriteAdapter {
    fn handle_event(&mut self, event: ModuleAdapterEvent<'_>) {
        match event {
            ModuleAdapterEvent::PrimaryWrite(write) => self.handle_primary(write),
            ModuleAdapterEvent::ScatterWrite(write) => self.handle_scatter(write),
            ModuleAdapterEvent::FanoutWrite(write) => self.handle_fanout(write),
            ModuleAdapterEvent::Hook { .. } => {
                // Sprite adapter does not need to react to hooks yet.
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mmio::{
        FanoutWriteEvent, ModuleAdapterEvent, PrimaryWriteEvent, RegId, ScatterWriteEvent,
    };

    #[derive(Default)]
    struct RecordingBackend {
        updates: Mutex<Vec<(u8, SpriteRenderState)>>,
    }

    impl RecordingBackend {
        fn new() -> Self {
            Self {
                updates: Mutex::new(Vec::new()),
            }
        }

        fn updates(&self) -> Vec<(u8, SpriteRenderState)> {
            self.updates.lock().unwrap().clone()
        }
    }

    impl SpriteBackend for RecordingBackend {
        fn update_sprite(&self, index: u8, state: SpriteRenderState) {
            self.updates.lock().unwrap().push((index, state));
        }
    }

    #[test]
    fn primary_events_update_sprite_state() {
        let backend = Arc::new(RecordingBackend::new());
        let mut adapter = SpriteAdapter::new(backend.clone());
        adapter.handle_event(ModuleAdapterEvent::PrimaryWrite(PrimaryWriteEvent {
            reg: RegId::Sprite(SpriteReg::XLo),
            cpu_value: 0x34,
            module_value: 0x34,
            instance: Some(2),
        }));
        adapter.handle_event(ModuleAdapterEvent::PrimaryWrite(PrimaryWriteEvent {
            reg: RegId::Sprite(SpriteReg::XHi),
            cpu_value: 0x01,
            module_value: 0x01,
            instance: Some(2),
        }));
        adapter.handle_event(ModuleAdapterEvent::PrimaryWrite(PrimaryWriteEvent {
            reg: RegId::Sprite(SpriteReg::Enable),
            cpu_value: 0x01,
            module_value: 0x01,
            instance: Some(2),
        }));

        let updates = backend.updates();
        assert!(
            updates
                .iter()
                .any(|(idx, state)| *idx == 2 && state.x == 0x0134 && state.enabled),
            "expected sprite 2 to be updated with combined X coordinate and enabled flag"
        );
    }

    #[test]
    fn scatter_events_toggle_high_bits() {
        let backend = Arc::new(RecordingBackend::new());
        let mut adapter = SpriteAdapter::new(backend.clone());

        adapter.handle_event(ModuleAdapterEvent::ScatterWrite(ScatterWriteEvent {
            reg: RegId::Sprite(SpriteReg::XHi),
            cpu_value: 0x01,
            module_value: 0x01,
            bit_value: true,
            source_bit: 0,
            target_bit: 0,
            instance: Some(1),
        }));

        let updates = backend.updates();
        assert!(
            updates
                .iter()
                .any(|(idx, state)| *idx == 1 && state.x == 0x0100),
            "expected scatter write to set high bit"
        );
    }

    #[test]
    fn fanout_event_updates_specific_instance() {
        let backend = Arc::new(RecordingBackend::new());
        let mut adapter = SpriteAdapter::new(backend.clone());

        adapter.handle_event(ModuleAdapterEvent::FanoutWrite(FanoutWriteEvent {
            reg: RegId::Sprite(SpriteReg::Enable),
            value: 1,
            source_value: 0xFF,
            source_instance: None,
            target_instance: Some(0),
        }));

        let updates = backend.updates();
        assert!(
            updates
                .iter()
                .any(|(idx, state)| *idx == 0 && state.enabled),
            "expected fanout to enable sprite 0"
        );
    }

    #[test]
    fn fanout_broadcast_initializes_missing_states() {
        let backend = Arc::new(RecordingBackend::new());
        let mut adapter = SpriteAdapter::new(backend.clone());
        adapter.sprites.clear();

        adapter.handle_event(ModuleAdapterEvent::FanoutWrite(FanoutWriteEvent {
            reg: RegId::Sprite(SpriteReg::Enable),
            value: 1,
            source_value: 0xFF,
            source_instance: None,
            target_instance: None,
        }));

        let updates = backend.updates();
        assert!(
            !updates.is_empty(),
            "fanout broadcast should update sprites even before direct writes"
        );
        assert!(
            updates.iter().any(|(_, state)| state.enabled),
            "expected at least one sprite to be enabled via broadcast"
        );
    }
}
