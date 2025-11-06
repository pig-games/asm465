//! Display adapter bridging MMIO colour writes to a backend.

use std::sync::{Arc, Mutex};

use crate::display_mmio::DisplayOutput;
use crate::mmio::{DisplayReg, HookAction, ModuleAdapter, ModuleAdapterEvent};

/// Backend interface that consumes display colour updates.
pub trait DisplayBackend: Send + Sync {
    fn set_border_color(&self, value: u8);
    fn set_background_color(&self, value: u8);
    fn handle_hook(&self, _hook: &str, _action: HookAction) {}
}

/// Backend that mirrors colours into the legacy [`DisplayOutput`].
pub struct DisplayOutputBackend {
    output: Arc<Mutex<DisplayOutput>>,
}

impl DisplayOutputBackend {
    /// Build a backend that mirrors adapter writes into the legacy snapshot.
    pub fn new(output: Arc<Mutex<DisplayOutput>>) -> Self {
        Self { output }
    }
}

impl DisplayBackend for DisplayOutputBackend {
    /// Update the border colour in the shared snapshot.
    fn set_border_color(&self, value: u8) {
        if let Ok(mut output) = self.output.lock() {
            output.set_border_color(value);
        }
    }

    /// Update the background colour in the shared snapshot.
    fn set_background_color(&self, value: u8) {
        if let Ok(mut output) = self.output.lock() {
            output.set_background_color(value);
        }
    }
}

/// Adapter that forwards MMIO writes to a [`DisplayBackend`].
pub struct DisplayAdapter {
    backend: Arc<dyn DisplayBackend>,
}

impl DisplayAdapter {
    /// Create a new adapter that forwards events to the supplied backend.
    pub fn new(backend: Arc<dyn DisplayBackend>) -> Self {
        Self { backend }
    }
}

impl ModuleAdapter for DisplayAdapter {
    fn handle_event(&mut self, event: ModuleAdapterEvent<'_>) {
        match event {
            ModuleAdapterEvent::PrimaryWrite(write) => {
                if let Some(reg) = write.reg.display() {
                    match reg {
                        DisplayReg::BorderColor => {
                            self.backend.set_border_color(write.module_value);
                        }
                        DisplayReg::BackgroundColor => {
                            self.backend.set_background_color(write.module_value);
                        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mmio::{ModuleAdapterEvent, PrimaryWriteEvent};
    use crate::RegId;
    #[derive(Default)]
    struct RecordingBackend {
        border: Mutex<Vec<u8>>,
        background: Mutex<Vec<u8>>,
    }

    impl DisplayBackend for RecordingBackend {
        fn set_border_color(&self, value: u8) {
            self.border.lock().unwrap().push(value);
        }

        fn set_background_color(&self, value: u8) {
            self.background.lock().unwrap().push(value);
        }
    }

    #[test]
    fn primary_writes_forward_to_backend() {
        let backend = Arc::new(RecordingBackend::default());
        let mut adapter = DisplayAdapter::new(backend.clone());

        adapter.handle_event(ModuleAdapterEvent::PrimaryWrite(PrimaryWriteEvent {
            reg: RegId::Display(DisplayReg::BorderColor),
            cpu_value: 0x11,
            module_value: 0x22,
            instance: None,
        }));

        adapter.handle_event(ModuleAdapterEvent::PrimaryWrite(PrimaryWriteEvent {
            reg: RegId::Display(DisplayReg::BackgroundColor),
            cpu_value: 0x33,
            module_value: 0x44,
            instance: None,
        }));

        assert_eq!(backend.border.lock().unwrap()[..], [0x22]);
        assert_eq!(backend.background.lock().unwrap()[..], [0x44]);
    }
}
