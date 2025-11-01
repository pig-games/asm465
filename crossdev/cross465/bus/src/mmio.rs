use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::{interrupts::InterruptController, Memory, MmioDevice};
use toml::value::Table;

/// Logical grouping of a module implementation exposed to the bus.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub enum ModuleKind {
    Console,
    Display,
    Sprite,
    System,
    Input,
    Video,
    Audio,
}

impl ModuleKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ModuleKind::Console => "console",
            ModuleKind::Display => "display",
            ModuleKind::Sprite => "sprite",
            ModuleKind::System => "system",
            ModuleKind::Input => "input",
            ModuleKind::Video => "video",
            ModuleKind::Audio => "audio",
        }
    }

    pub fn from_str(kind: &str) -> Option<Self> {
        match kind {
            "console" => Some(ModuleKind::Console),
            "display" => Some(ModuleKind::Display),
            "sprite" => Some(ModuleKind::Sprite),
            "system" => Some(ModuleKind::System),
            "input" => Some(ModuleKind::Input),
            "video" => Some(ModuleKind::Video),
            "audio" => Some(ModuleKind::Audio),
            _ => None,
        }
    }
}

/// Stable identifier for a personality-visible register.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum RegId {
    Console(ConsoleReg),
    Display(DisplayReg),
    Sprite(SpriteReg),
    System(SystemReg),
}

impl RegId {
    pub fn console(self) -> Option<ConsoleReg> {
        match self {
            RegId::Console(reg) => Some(reg),
            _ => None,
        }
    }

    pub fn display(self) -> Option<DisplayReg> {
        match self {
            RegId::Display(reg) => Some(reg),
            _ => None,
        }
    }

    pub fn sprite(self) -> Option<SpriteReg> {
        match self {
            RegId::Sprite(reg) => Some(reg),
            _ => None,
        }
    }

    pub fn system(self) -> Option<SystemReg> {
        match self {
            RegId::System(reg) => Some(reg),
            _ => None,
        }
    }
}

/// Console register identifiers.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum ConsoleReg {
    WriteChar,
    Newline,
    WriteHex,
    Clear,
    CursorX,
    CursorY,
    CursorApply,
    Foreground,
    Background,
    PointerLo,
    PointerHi,
    PrintBlock,
}

/// Display register identifiers.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum DisplayReg {
    BorderColor,
    BackgroundColor,
}

/// Sprite register identifiers (selected sprite slot context).
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum SpriteReg {
    Select,
    Number,
    Anim,
    XHi,
    XLo,
    YHi,
    YLo,
    Scale,
    Enable,
}

/// System/interrupt controller register identifiers.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum SystemReg {
    IrqPending,
    IrqEnable,
    IrqAck,
    IrqSource,
    NmiPending,
    NmiAck,
    Status,
    RasterLo,
    SpriteCollisions,
    BackgroundCollisions,
}

/// Metadata describing a register’s default value and access semantics.
#[derive(Copy, Clone, Debug)]
pub struct RegisterDesc {
    pub id: RegId,
    pub name: &'static str,
    pub width: u8,
    pub reset: u32,
    pub readable: bool,
    pub writable: bool,
    pub bitfields: &'static [BitField],
}

impl RegisterDesc {
    pub const fn new(
        id: RegId,
        name: &'static str,
        width: u8,
        reset: u32,
        readable: bool,
        writable: bool,
        bitfields: &'static [BitField],
    ) -> Self {
        Self {
            id,
            name,
            width,
            reset,
            readable,
            writable,
            bitfields,
        }
    }

    pub fn matches_name(&self, name: &str) -> bool {
        self.name.eq_ignore_ascii_case(name)
    }
}

/// Optional bitfield metadata for UI/tooling annotations.
#[derive(Copy, Clone, Debug)]
pub struct BitField {
    pub name: &'static str,
    pub lsb: u8,
    pub width: u8,
    pub read_only: bool,
    pub active_low: bool,
}

impl BitField {
    pub const fn new(name: &'static str, lsb: u8, width: u8) -> Self {
        Self {
            name,
            lsb,
            width,
            read_only: false,
            active_low: false,
        }
    }
}

/// Persistent state snapshot for a module implementation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModuleState {
    pub version: u16,
    pub bytes: Vec<u8>,
}

/// Shared handles that allow modules and adapters to reach host backends.
#[derive(Clone, Default)]
pub struct BackendHandles {
    map: HashMap<TypeId, Arc<dyn Any + Send + Sync>>,
}

impl BackendHandles {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn insert<T>(&mut self, handle: Arc<T>)
    where
        T: Any + Send + Sync + 'static,
    {
        let handle: Arc<dyn Any + Send + Sync> = handle;
        self.map.insert(TypeId::of::<T>(), handle);
    }

    pub fn with_handle<T>(mut self, handle: Arc<T>) -> Self
    where
        T: Any + Send + Sync + 'static,
    {
        self.insert(handle);
        self
    }

    pub fn get<T>(&self) -> Option<Arc<T>>
    where
        T: Any + Send + Sync + 'static,
    {
        self.map
            .get(&TypeId::of::<T>())
            .and_then(|handle| handle.clone().downcast::<T>().ok())
    }

    pub fn contains<T>(&self) -> bool
    where
        T: Any + Send + Sync + 'static,
    {
        self.map.contains_key(&TypeId::of::<T>())
    }
}

/// Shared dependencies handed to module factories upon construction.
#[derive(Clone)]
pub struct ModuleDeps {
    pub ram: Arc<Mutex<Memory>>,
    pub controller: Arc<InterruptController>,
    pub backends: BackendHandles,
}

pub type ModuleOptions = Table;

impl ModuleDeps {
    pub fn new(
        ram: Arc<Mutex<Memory>>,
        controller: Arc<InterruptController>,
        backends: BackendHandles,
    ) -> Self {
        Self {
            ram,
            controller,
            backends,
        }
    }

    pub fn backend<T>(&self) -> Option<Arc<T>>
    where
        T: Any + Send + Sync + 'static,
    {
        self.backends.get::<T>()
    }
}

/// Runtime contract for MMIO modules in the v2 personality system.
pub trait Module: MmioDevice {
    fn kind(&self) -> ModuleKind;
    fn regs(&self) -> &'static [RegisterDesc];

    fn read_reg(&mut self, reg: RegId) -> u8;
    fn write_reg(&mut self, reg: RegId, value: u8);

    fn tick(&mut self, _cycles: u32) {}
    fn snapshot(&self) -> ModuleState {
        ModuleState::default()
    }
    fn restore(&mut self, _state: &ModuleState) {}

    fn handle_hook(&mut self, _hook: &str, _action: HookAction) {}
}

/// Factory for constructing module implementations.
pub trait ModuleFactory: Send + Sync {
    fn id(&self) -> &'static str;
    fn kind(&self) -> ModuleKind;
    fn create(&self, deps: &ModuleDeps, options: &ModuleOptions) -> Box<dyn Module>;
    fn regs(&self) -> &'static [RegisterDesc];
}

/// Registry of available module implementations.
#[derive(Default)]
pub struct ModuleRegistry {
    builders: Vec<&'static dyn ModuleFactory>,
}

#[derive(Clone, Copy, Debug)]
pub enum HookAction {
    Read { mask: u8, value: u8 },
    Write { mask: u8, value: u8 },
}

impl ModuleRegistry {
    pub fn new() -> Self {
        Self {
            builders: Vec::new(),
        }
    }

    pub fn register(&mut self, factory: &'static dyn ModuleFactory) {
        self.builders.push(factory);
    }

    pub fn all(&self) -> &[&'static dyn ModuleFactory] {
        &self.builders
    }

    pub fn by_id(&self, id: &str) -> Option<&'static dyn ModuleFactory> {
        self.builders
            .iter()
            .copied()
            .find(|factory| factory.id() == id)
    }

    pub fn by_kind(
        &self,
        kind: ModuleKind,
    ) -> impl Iterator<Item = &'static dyn ModuleFactory> + '_ {
        self.builders
            .iter()
            .copied()
            .filter(move |factory| factory.kind() == kind)
    }
}
