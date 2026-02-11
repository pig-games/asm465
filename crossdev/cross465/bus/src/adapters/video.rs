//! Video/system module adapter that mirrors MMIO activity into a backend while
//! managing raster compare interrupts shared by modern and legacy personalities.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::{Arc, Mutex};

use crate::interrupts::InterruptController;
use crate::mmio::{HookAction, ModuleAdapter, ModuleAdapterEvent, SystemReg};

/// Bit mask used for the raster interrupt source.
pub const RASTER_IRQ_MASK: u32 = 1 << 0;

/// Shared state that tracks the raster compare target and the most recent beam value.
#[derive(Default)]
pub struct RasterIrqState {
    compare: AtomicU16,
    current: AtomicU16,
}

impl RasterIrqState {
    /// Construct a new raster IRQ state block.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn update_atomic(atomic: &AtomicU16, update: impl Fn(u16) -> u16) {
        let _ = atomic.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |old| Some(update(old)));
    }

    /// Program the full 16-bit raster compare value.
    pub fn set_compare(&self, value: u16) {
        self.compare.store(value, Ordering::Relaxed);
    }

    pub fn set_compare_low(&self, value: u8) {
        Self::update_atomic(&self.compare, |old| (old & 0xFF00) | u16::from(value));
    }

    pub fn set_compare_high(&self, value: u8) {
        Self::update_atomic(&self.compare, |old| {
            (u16::from(value) << 8) | (old & 0x00FF)
        });
    }

    pub fn set_compare_high_bit(&self, bit: bool) {
        let high = u8::from(bit);
        self.set_compare_high(high);
    }

    /// Return the currently programmed compare value.
    pub fn compare(&self) -> u16 {
        self.compare.load(Ordering::Relaxed)
    }

    /// Update the present raster line.
    pub fn set_current(&self, value: u16) {
        self.current.store(value, Ordering::Relaxed);
    }

    pub fn set_current_low(&self, value: u8) {
        Self::update_atomic(&self.current, |old| (old & 0xFF00) | u16::from(value));
    }

    pub fn set_current_high(&self, value: u8) {
        Self::update_atomic(&self.current, |old| {
            (u16::from(value) << 8) | (old & 0x00FF)
        });
    }

    /// Return the last recorded raster line.
    pub fn current(&self) -> u16 {
        self.current.load(Ordering::Relaxed)
    }
}

/// Backend interface that consumes system/video register activity.
pub trait VideoBackend: Send + Sync {
    fn write_register(&self, _reg: SystemReg, _cpu_value: u8, _module_value: u8) {}

    fn scatter_write(
        &self,
        _reg: SystemReg,
        _cpu_value: u8,
        _module_value: u8,
        _bit_value: bool,
        _source_bit: u8,
        _target_bit: u8,
    ) {
    }

    fn fanout_write(
        &self,
        _reg: SystemReg,
        _value: u8,
        _source_value: u8,
        _source_instance: Option<u8>,
        _target_instance: Option<u8>,
    ) {
    }

    fn handle_hook(&self, _hook: &str, _action: HookAction) {}
}

/// Adapter that routes system/video MMIO events to a [`VideoBackend`].
/// Module adapter that forwards system/video register traffic to a
/// [`VideoBackend`] and raises raster IRQs when compare conditions are met.
pub struct VideoAdapter {
    backend: Arc<dyn VideoBackend>,
    controller: Arc<InterruptController>,
    raster_irq: Option<Arc<RasterIrqState>>,
}

impl VideoAdapter {
    pub fn new(
        backend: Arc<dyn VideoBackend>,
        controller: Arc<InterruptController>,
        raster_irq: Option<Arc<RasterIrqState>>,
    ) -> Self {
        Self {
            backend,
            controller,
            raster_irq,
        }
    }

    fn evaluate_raster_irq(&self) {
        let Some(state) = &self.raster_irq else {
            return;
        };
        if state.current() == state.compare() {
            self.controller.raise_irq(RASTER_IRQ_MASK);
        }
    }
}

impl ModuleAdapter for VideoAdapter {
    fn handle_event(&mut self, event: ModuleAdapterEvent<'_>) {
        match event {
            ModuleAdapterEvent::PrimaryWrite(write) => {
                if let Some(reg) = write.reg.system() {
                    match reg {
                        SystemReg::RasterLo => {
                            if let Some(state) = &self.raster_irq {
                                state.set_current_low(write.module_value);
                                self.evaluate_raster_irq();
                            }
                        }
                        SystemReg::RasterCompareLo => {
                            if let Some(state) = &self.raster_irq {
                                state.set_compare_low(write.module_value);
                                self.evaluate_raster_irq();
                            }
                        }
                        SystemReg::RasterCompareHi => {
                            if let Some(state) = &self.raster_irq {
                                state.set_compare_high(write.module_value);
                                self.evaluate_raster_irq();
                            }
                        }
                        _ => {}
                    }
                    self.backend
                        .write_register(reg, write.cpu_value, write.module_value);
                }
            }
            ModuleAdapterEvent::ScatterWrite(write) => {
                if let Some(reg) = write.reg.system() {
                    self.backend.scatter_write(
                        reg,
                        write.cpu_value,
                        write.module_value,
                        write.bit_value,
                        write.source_bit,
                        write.target_bit,
                    );
                    match reg {
                        SystemReg::RasterCompareLo => {
                            if let Some(state) = &self.raster_irq {
                                state.set_compare_low(write.module_value);
                                self.evaluate_raster_irq();
                            }
                        }
                        SystemReg::RasterCompareHi => {
                            if let Some(state) = &self.raster_irq {
                                state.set_compare_high(write.module_value);
                                self.evaluate_raster_irq();
                            }
                        }
                        SystemReg::RasterLo => {
                            if let Some(state) = &self.raster_irq {
                                state.set_current_low(write.module_value);
                                self.evaluate_raster_irq();
                            }
                        }
                        _ => {}
                    }
                }
            }
            ModuleAdapterEvent::FanoutWrite(write) => {
                if let Some(reg) = write.reg.system() {
                    self.backend.fanout_write(
                        reg,
                        write.value,
                        write.source_value,
                        write.source_instance,
                        write.target_instance,
                    );
                }
            }
            ModuleAdapterEvent::Hook { hook, action } => {
                self.backend.handle_hook(hook, action);
            }
        }
    }
}

/// Shared state updated by [`VideoStateBackend`] for inspection and tests.
#[derive(Clone, Debug, Default)]
pub struct VideoState {
    pub registers: HashMap<SystemReg, u8>,
    pub last_hook: Option<(String, HookAction)>,
}

impl VideoState {
    #[must_use]
    pub fn register_value(&self, reg: SystemReg) -> Option<u8> {
        self.registers.get(&reg).copied()
    }
}

/// Backend that records register writes and hook invocations in-memory.
/// Simple backend used in tests to record MMIO writes for later inspection.
pub struct VideoStateBackend {
    state: Arc<Mutex<VideoState>>,
}

impl VideoStateBackend {
    pub fn new(state: Arc<Mutex<VideoState>>) -> Self {
        Self { state }
    }

    #[must_use]
    pub fn snapshot(&self) -> VideoState {
        self.state.lock().unwrap().clone()
    }
}

impl VideoBackend for VideoStateBackend {
    fn write_register(&self, reg: SystemReg, _cpu_value: u8, module_value: u8) {
        let mut state = self.state.lock().unwrap();
        state.registers.insert(reg, module_value);
    }

    fn handle_hook(&self, hook: &str, action: HookAction) {
        let mut state = self.state.lock().unwrap();
        state.last_hook = Some((hook.to_string(), action));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interrupts::InterruptController;
    use crate::mmio::{
        FanoutWriteEvent, ModuleAdapterEvent, PrimaryWriteEvent, RegId, ScatterWriteEvent,
    };

    #[derive(Default)]
    struct RecordingBackend {
        writes: Mutex<Vec<(SystemReg, u8, u8)>>,
        hooks: Mutex<Vec<(String, HookAction)>>,
    }

    impl RecordingBackend {
        fn new() -> Self {
            Self::default()
        }

        fn writes(&self) -> Vec<(SystemReg, u8, u8)> {
            self.writes.lock().unwrap().clone()
        }

        fn hooks(&self) -> Vec<(String, HookAction)> {
            self.hooks.lock().unwrap().clone()
        }
    }

    impl VideoBackend for RecordingBackend {
        fn write_register(&self, reg: SystemReg, cpu_value: u8, module_value: u8) {
            self.writes
                .lock()
                .unwrap()
                .push((reg, cpu_value, module_value));
        }

        fn handle_hook(&self, hook: &str, action: HookAction) {
            self.hooks.lock().unwrap().push((hook.to_string(), action));
        }
    }

    #[test]
    fn primary_write_forwards_to_backend() {
        let backend = Arc::new(RecordingBackend::new());
        let controller = Arc::new(InterruptController::new());
        let mut adapter = VideoAdapter::new(backend.clone(), controller, None);

        adapter.handle_event(ModuleAdapterEvent::PrimaryWrite(PrimaryWriteEvent {
            reg: RegId::System(SystemReg::IrqEnable),
            cpu_value: 0xFF,
            module_value: 0x7F,
            instance: None,
        }));

        let writes = backend.writes();
        assert_eq!(writes.len(), 1);
        assert_eq!(
            writes[0],
            (SystemReg::IrqEnable, 0xFF, 0x7F),
            "expected backend to receive register update"
        );
    }

    #[test]
    fn hook_events_are_forwarded() {
        let backend = Arc::new(RecordingBackend::new());
        let controller = Arc::new(InterruptController::new());
        let mut adapter = VideoAdapter::new(backend.clone(), controller, None);
        adapter.handle_event(ModuleAdapterEvent::Hook {
            hook: "irq_ack_bits",
            action: HookAction::Read {
                mask: 0x07,
                value: 0x03,
            },
        });

        let hooks = backend.hooks();
        assert_eq!(hooks.len(), 1);
        assert_eq!(
            hooks[0],
            (
                "irq_ack_bits".to_string(),
                HookAction::Read {
                    mask: 0x07,
                    value: 0x03
                }
            ),
            "expected hook to propagate to backend"
        );
    }

    #[test]
    fn scatter_and_fanout_events_call_backend_defaults() {
        struct CounterBackend {
            scatter: Mutex<usize>,
            fanout: Mutex<usize>,
        }

        impl CounterBackend {
            fn new() -> Self {
                Self {
                    scatter: Mutex::new(0),
                    fanout: Mutex::new(0),
                }
            }
        }

        impl VideoBackend for CounterBackend {
            fn scatter_write(
                &self,
                _reg: SystemReg,
                _cpu_value: u8,
                _module_value: u8,
                _bit_value: bool,
                _source_bit: u8,
                _target_bit: u8,
            ) {
                *self.scatter.lock().unwrap() += 1;
            }

            fn fanout_write(
                &self,
                _reg: SystemReg,
                _value: u8,
                _source_value: u8,
                _source_instance: Option<u8>,
                _target_instance: Option<u8>,
            ) {
                *self.fanout.lock().unwrap() += 1;
            }
        }

        let backend = Arc::new(CounterBackend::new());
        let controller = Arc::new(InterruptController::new());
        let mut adapter = VideoAdapter::new(
            backend.clone(),
            controller,
            Some(Arc::new(RasterIrqState::new())),
        );

        adapter.handle_event(ModuleAdapterEvent::ScatterWrite(ScatterWriteEvent {
            reg: RegId::System(SystemReg::RasterLo),
            cpu_value: 0x00,
            module_value: 0x12,
            bit_value: true,
            source_bit: 0,
            target_bit: 0,
            instance: None,
        }));

        adapter.handle_event(ModuleAdapterEvent::FanoutWrite(FanoutWriteEvent {
            reg: RegId::System(SystemReg::SpriteCollisions),
            value: 0xAA,
            source_value: 0xAA,
            source_instance: None,
            target_instance: Some(0),
        }));

        assert_eq!(
            *backend.scatter.lock().unwrap(),
            1,
            "expected scatter callback to increment counter"
        );
        assert_eq!(
            *backend.fanout.lock().unwrap(),
            1,
            "expected fanout callback to increment counter"
        );
    }

    #[test]
    fn state_backend_tracks_writes_and_hooks() {
        let state = Arc::new(Mutex::new(VideoState::default()));
        let backend = Arc::new(VideoStateBackend::new(state.clone()));
        backend.write_register(SystemReg::IrqEnable, 0xFF, 0x12);
        backend.handle_hook(
            "irq_ack_bits",
            HookAction::Read {
                mask: 0x07,
                value: 0x01,
            },
        );

        let snapshot = backend.snapshot();
        assert_eq!(
            snapshot.registers.get(&SystemReg::IrqEnable),
            Some(&0x12),
            "expected state backend to record register value"
        );
        assert!(snapshot.last_hook.is_some(), "expected hook to be recorded");
    }

    #[test]
    fn raster_compare_triggers_irq_when_current_matches() {
        let backend = Arc::new(RecordingBackend::new());
        let controller = Arc::new(InterruptController::new());
        controller.set_irq_enable(RASTER_IRQ_MASK);
        let raster_state = Arc::new(RasterIrqState::new());
        let mut adapter =
            VideoAdapter::new(backend, controller.clone(), Some(raster_state.clone()));

        // Configure compare line and confirm no pending IRQ without a match.
        adapter.handle_event(ModuleAdapterEvent::PrimaryWrite(PrimaryWriteEvent {
            reg: RegId::System(SystemReg::RasterCompareLo),
            cpu_value: 0x50,
            module_value: 0x50,
            instance: None,
        }));

        adapter.handle_event(ModuleAdapterEvent::PrimaryWrite(PrimaryWriteEvent {
            reg: RegId::System(SystemReg::RasterLo),
            cpu_value: 0x10,
            module_value: 0x10,
            instance: None,
        }));

        assert_eq!(
            controller.irq_pending(),
            0,
            "no interrupt expected when raster does not match compare"
        );

        adapter.handle_event(ModuleAdapterEvent::PrimaryWrite(PrimaryWriteEvent {
            reg: RegId::System(SystemReg::RasterLo),
            cpu_value: 0x50,
            module_value: 0x50,
            instance: None,
        }));

        assert_eq!(
            controller.irq_pending(),
            RASTER_IRQ_MASK,
            "raster compare should raise the IRQ source"
        );

        controller.clear_irq(RASTER_IRQ_MASK);

        adapter.handle_event(ModuleAdapterEvent::PrimaryWrite(PrimaryWriteEvent {
            reg: RegId::System(SystemReg::RasterLo),
            cpu_value: 0x51,
            module_value: 0x51,
            instance: None,
        }));
        adapter.handle_event(ModuleAdapterEvent::PrimaryWrite(PrimaryWriteEvent {
            reg: RegId::System(SystemReg::RasterLo),
            cpu_value: 0x50,
            module_value: 0x50,
            instance: None,
        }));

        assert_eq!(
            controller.irq_pending(),
            RASTER_IRQ_MASK,
            "IRQ should re-assert after acknowledge when the raster matches again"
        );
    }
}
