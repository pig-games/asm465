use std::sync::{Arc, Mutex};

use crate::input_mmio::{InputOutput, InputSnapshot};
use crate::mmio::{HookAction, InputReg, ModuleAdapter, ModuleAdapterEvent};

/// Backend interface that accepts controller state updates.
pub trait InputBackend: Send + Sync {
    fn set_port_a(&self, value: u8);
    fn set_port_b(&self, value: u8);
    fn set_pot_x(&self, value: u8) {
        let _ = value;
    }
    fn set_pot_y(&self, value: u8) {
        let _ = value;
    }
    fn snapshot(&self) -> InputSnapshot;
    fn handle_hook(&self, _hook: &str, _action: HookAction) {}
}

/// Concrete backend that mirrors input state into [`InputOutput`].
pub struct InputBackendHandle {
    state: Arc<Mutex<InputOutput>>,
}

impl InputBackendHandle {
    pub fn new(state: Arc<Mutex<InputOutput>>) -> Self {
        Self { state }
    }
}

impl InputBackend for InputBackendHandle {
    fn set_port_a(&self, value: u8) {
        if let Ok(mut state) = self.state.lock() {
            state.set_port_a(value);
        }
    }

    fn set_port_b(&self, value: u8) {
        if let Ok(mut state) = self.state.lock() {
            state.set_port_b(value);
        }
    }

    fn set_pot_x(&self, value: u8) {
        if let Ok(mut state) = self.state.lock() {
            state.set_pot_x(value);
        }
    }

    fn set_pot_y(&self, value: u8) {
        if let Ok(mut state) = self.state.lock() {
            state.set_pot_y(value);
        }
    }

    fn snapshot(&self) -> InputSnapshot {
        self.state
            .lock()
            .map(|state| state.snapshot())
            .unwrap_or_default()
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
                        InputReg::PortA => self.backend.set_port_a(write.module_value),
                        InputReg::PortB => self.backend.set_port_b(write.module_value),
                        InputReg::PotX => self.backend.set_pot_x(write.module_value),
                        InputReg::PotY => self.backend.set_pot_y(write.module_value),
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
        port_a: Mutex<Vec<u8>>,
        port_b: Mutex<Vec<u8>>,
    }

    impl InputBackend for RecordingBackend {
        fn set_port_a(&self, value: u8) {
            self.port_a.lock().unwrap().push(value);
        }

        fn set_port_b(&self, value: u8) {
            self.port_b.lock().unwrap().push(value);
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

        assert_eq!(backend.port_a.lock().unwrap()[..], [0x7F]);
        assert_eq!(backend.port_b.lock().unwrap()[..], [0xFE]);
    }
}
