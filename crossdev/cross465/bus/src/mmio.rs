//! Core MMIO traits and registry infrastructure used by the cross465 bus.
//!
//! This module defines the enums shared by all MMIO modules (`ModuleKind`,
//! `RegId`), the trait objects used to communicate with devices, and the
//! registry/runtime helpers that personalities rely on when wiring modules.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use crate::{interrupts::InterruptController, Memory};
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
    /// Return the lowercase string identifier used in TOML/personality files.
    #[must_use]
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

    /// Parse a module kind from its string representation.
    #[must_use]
    pub fn from_name(kind: &str) -> Option<Self> {
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

impl FromStr for ModuleKind {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_name(s).ok_or(())
    }
}

/// Stable identifier for a personality-visible register.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum RegId {
    Console(ConsoleReg),
    Display(DisplayReg),
    Sprite(SpriteReg),
    Input(InputReg),
    System(SystemReg),
}

impl RegId {
    /// If this register is a console register, return it.
    #[must_use]
    pub fn console(self) -> Option<ConsoleReg> {
        match self {
            RegId::Console(reg) => Some(reg),
            _ => None,
        }
    }

    /// If this register is a display register, return it.
    #[must_use]
    pub fn display(self) -> Option<DisplayReg> {
        match self {
            RegId::Display(reg) => Some(reg),
            _ => None,
        }
    }

    /// If this register is a sprite register, return it.
    #[must_use]
    pub fn sprite(self) -> Option<SpriteReg> {
        match self {
            RegId::Sprite(reg) => Some(reg),
            _ => None,
        }
    }

    /// If this register is an input register, return it.
    #[must_use]
    pub fn input(self) -> Option<InputReg> {
        match self {
            RegId::Input(reg) => Some(reg),
            _ => None,
        }
    }

    /// If this register is a system register, return it.
    #[must_use]
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

/// Input register identifiers (e.g. joystick ports).
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum InputReg {
    Select,
    ButtonsLo,
    ButtonsHi,
    PotX,
    PotY,
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
    RasterCompareLo,
    RasterCompareHi,
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
    #[must_use]
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

    #[must_use]
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
    #[must_use]
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
    /// Create an empty set of backend handles.
    #[must_use]
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    /// Return `true` when no handles have been registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Associate a backend handle with its concrete type.
    pub fn insert<T>(&mut self, handle: Arc<T>)
    where
        T: Any + Send + Sync + 'static,
    {
        let handle: Arc<dyn Any + Send + Sync> = handle;
        self.map.insert(TypeId::of::<T>(), handle);
    }

    /// Convenience builder that returns `Self` for chaining.
    pub fn with_handle<T>(mut self, handle: Arc<T>) -> Self
    where
        T: Any + Send + Sync + 'static,
    {
        self.insert(handle);
        self
    }

    /// Retrieve a handle by type, cloning the `Arc`.
    #[must_use]
    pub fn get<T>(&self) -> Option<Arc<T>>
    where
        T: Any + Send + Sync + 'static,
    {
        self.map
            .get(&TypeId::of::<T>())
            .and_then(|handle| handle.clone().downcast::<T>().ok())
    }

    /// Returns `true` if a handle of the given type is registered.
    #[must_use]
    pub fn contains<T>(&self) -> bool
    where
        T: Any + Send + Sync + 'static,
    {
        self.map.contains_key(&TypeId::of::<T>())
    }
}

/// Shared dependencies handed to module factories upon construction.
/// Shared dependencies handed to module factories upon construction.
#[derive(Clone)]
pub struct ModuleDeps {
    pub ram: Arc<Mutex<Memory>>,
    pub controller: Arc<InterruptController>,
    pub backends: BackendHandles,
}

pub type ModuleOptions = Table;

impl ModuleDeps {
    /// Construct dependency bundle from the supplied shared resources.
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

    /// Retrieve a backend handle of type `T`, if one is registered.
    #[must_use]
    pub fn backend<T>(&self) -> Option<Arc<T>>
    where
        T: Any + Send + Sync + 'static,
    {
        self.backends.get::<T>()
    }
}

/// Runtime contract for MMIO modules in the v2 personality system.
pub trait Module: Any + Send {
    fn kind(&self) -> ModuleKind;
    fn regs(&self) -> &'static [RegisterDesc];

    fn read_reg(&mut self, reg: RegId) -> u8;
    fn write_reg(&mut self, reg: RegId, value: u8);

    fn read(&mut self, _addr: u16) -> u8 {
        0
    }

    fn write(&mut self, _addr: u16, _value: u8) {}

    fn tick(&mut self, _cycles: u32) {}
    fn snapshot(&self) -> ModuleState {
        ModuleState::default()
    }
    fn restore(&mut self, _state: &ModuleState) {}

    fn handle_hook(&mut self, _hook: &str, _action: HookAction) {}
}

impl dyn Module {
    pub fn as_any(&self) -> &dyn Any {
        self
    }

    pub fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Factory for constructing module implementations.
pub trait ModuleFactory: Send + Sync {
    fn id(&self) -> &'static str;
    fn kind(&self) -> ModuleKind;
    fn create(&self, deps: &ModuleDeps, options: &ModuleOptions) -> Box<dyn Module>;
    fn regs(&self) -> &'static [RegisterDesc];
}

/// Registry of available module implementations.
/// Registry of available module implementations. Populated during startup so
/// personalities can lookup factories by identifier.
#[derive(Default)]
pub struct ModuleRegistry {
    builders: Vec<&'static dyn ModuleFactory>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HookAction {
    Read { mask: u8, value: u8 },
    Write { mask: u8, value: u8 },
}

/// CPU-visible register write dispatched to an adapter.
#[derive(Clone, Copy, Debug)]
pub struct PrimaryWriteEvent {
    pub reg: RegId,
    pub cpu_value: u8,
    pub module_value: u8,
    pub instance: Option<u8>,
}

/// Scatter write notification describing a single bit projection.
#[derive(Clone, Copy, Debug)]
pub struct ScatterWriteEvent {
    pub reg: RegId,
    pub cpu_value: u8,
    pub module_value: u8,
    pub bit_value: bool,
    pub source_bit: u8,
    pub target_bit: u8,
    pub instance: Option<u8>,
}

/// Fanout write notification delivered to the receiving module adapter.
#[derive(Clone, Copy, Debug)]
pub struct FanoutWriteEvent {
    pub reg: RegId,
    pub value: u8,
    pub source_value: u8,
    pub source_instance: Option<u8>,
    pub target_instance: Option<u8>,
}

/// Event forwarded to a module adapter.
#[derive(Clone, Copy, Debug)]
pub enum ModuleAdapterEvent<'a> {
    PrimaryWrite(PrimaryWriteEvent),
    ScatterWrite(ScatterWriteEvent),
    FanoutWrite(FanoutWriteEvent),
    Hook { hook: &'a str, action: HookAction },
}

/// Adapter bridge that mirrors MMIO activity into external backends.
/// Adapter hook that observes module activity and mirrors it to external
/// backends (renderers, audio engines, etc.).
pub trait ModuleAdapter: Send {
    fn handle_event(&mut self, event: ModuleAdapterEvent<'_>);
}

impl ModuleRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            builders: Vec::new(),
        }
    }

    pub fn register(&mut self, factory: &'static dyn ModuleFactory) {
        self.builders.push(factory);
    }

    #[must_use]
    pub fn all(&self) -> &[&'static dyn ModuleFactory] {
        &self.builders
    }

    #[must_use]
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
