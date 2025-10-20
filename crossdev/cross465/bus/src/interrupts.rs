//! Thread-safe controller for aggregating IRQ/NMI sources.
//!
//! The controller tracks pending bits for maskable IRQ lines and edge-triggered
//! NMI lines. Host subsystems (display loop, input manager, timers, etc.) can
//! raise or clear sources concurrently, while the CPU core polls the snapshot or
//! edge latch to decide whether to service an interrupt.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// Snapshot of the controller state, useful for debugging or exposing MMIO
/// register reads.
#[derive(Clone, Copy, Debug)]
pub struct InterruptSnapshot {
    pub irq_pending: u32,
    pub irq_enabled: u32,
    pub irq_line: bool,
    pub nmi_pending: u32,
    pub nmi_enabled: u32,
    pub nmi_line: bool,
    pub nmi_edge_latched: bool,
}

/// Shared controller for maskable IRQ and edge-triggered NMI sources.
pub struct InterruptController {
    irq: LevelLine,
    nmi: EdgeLine,
}

impl InterruptController {
    /// Create a new controller with all sources disabled and no pending events.
    pub fn new() -> Self {
        Self {
            irq: LevelLine::new(),
            nmi: EdgeLine::new(),
        }
    }

    /// Enable the supplied IRQ source bits.
    pub fn enable_irq(&self, mask: u32) {
        if mask != 0 {
            self.irq.enable_bits(mask);
        }
    }

    /// Disable the supplied IRQ source bits.
    pub fn disable_irq(&self, mask: u32) {
        if mask != 0 {
            self.irq.disable_bits(mask);
        }
    }

    /// Replace the IRQ enable mask outright.
    pub fn set_irq_enable(&self, mask: u32) {
        self.irq.set_enable(mask);
    }

    /// Raise one or more IRQ sources (level-triggered).
    pub fn raise_irq(&self, mask: u32) {
        if mask != 0 {
            self.irq.raise(mask);
        }
    }

    /// Clear the supplied IRQ pending bits (typically done by the guest).
    pub fn clear_irq(&self, mask: u32) {
        if mask != 0 {
            self.irq.clear(mask);
        }
    }

    /// Current pending IRQ sources.
    pub fn irq_pending(&self) -> u32 {
        self.irq.pending()
    }

    /// Currently enabled IRQ sources.
    pub fn irq_enabled(&self) -> u32 {
        self.irq.enabled()
    }

    /// Whether the IRQ line is asserted (level-triggered).
    pub fn irq_line(&self) -> bool {
        self.irq.line()
    }

    /// Raise one or more NMI sources (edge-triggered).
    pub fn raise_nmi(&self, mask: u32) {
        if mask != 0 {
            self.nmi.raise(mask);
        }
    }

    /// Clear the supplied NMI pending bits (typically done by the guest).
    pub fn clear_nmi(&self, mask: u32) {
        if mask != 0 {
            self.nmi.clear(mask);
        }
    }

    /// Current pending NMI sources.
    pub fn nmi_pending(&self) -> u32 {
        self.nmi.pending()
    }

    /// Whether the NMI line is currently asserted (low).
    pub fn nmi_line(&self) -> bool {
        self.nmi.line()
    }

    /// Consume the latched NMI edge, returning `true` once per falling edge.
    pub fn take_nmi_edge(&self) -> bool {
        self.nmi.take_edge()
    }

    /// Return a snapshot suitable for MMIO register emulation.
    pub fn snapshot(&self) -> InterruptSnapshot {
        InterruptSnapshot {
            irq_pending: self.irq.pending(),
            irq_enabled: self.irq.enabled(),
            irq_line: self.irq.line(),
            nmi_pending: self.nmi.pending(),
            nmi_enabled: u32::MAX,
            nmi_line: self.nmi.line(),
            nmi_edge_latched: self.nmi.edge_latched(),
        }
    }
}

struct LevelLine {
    pending: AtomicU32,
    enabled: AtomicU32,
    line: AtomicBool,
}

impl LevelLine {
    fn new() -> Self {
        Self {
            pending: AtomicU32::new(0),
            enabled: AtomicU32::new(0),
            line: AtomicBool::new(false),
        }
    }

    fn raise(&self, mask: u32) {
        let prev = self.pending.fetch_or(mask, Ordering::SeqCst);
        let new_pending = prev | mask;
        self.update_line(new_pending);
    }

    fn clear(&self, mask: u32) {
        let prev = self.pending.fetch_and(!mask, Ordering::SeqCst);
        let new_pending = prev & !mask;
        self.update_line(new_pending);
    }

    fn enable_bits(&self, mask: u32) {
        let prev = self.enabled.fetch_or(mask, Ordering::SeqCst);
        let _ = prev;
        self.update_line(self.pending.load(Ordering::SeqCst));
    }

    fn disable_bits(&self, mask: u32) {
        let prev = self.enabled.fetch_and(!mask, Ordering::SeqCst);
        let _ = prev;
        self.update_line(self.pending.load(Ordering::SeqCst));
    }

    fn set_enable(&self, value: u32) {
        self.enabled.store(value, Ordering::SeqCst);
        self.update_line(self.pending.load(Ordering::SeqCst));
    }

    fn pending(&self) -> u32 {
        self.pending.load(Ordering::SeqCst)
    }

    fn enabled(&self) -> u32 {
        self.enabled.load(Ordering::SeqCst)
    }

    fn line(&self) -> bool {
        self.line.load(Ordering::SeqCst)
    }

    fn update_line(&self, pending: u32) {
        let enabled = self.enabled.load(Ordering::SeqCst);
        let asserted = (pending & enabled) != 0;
        self.line.store(asserted, Ordering::SeqCst);
    }
}

struct EdgeLine {
    pending: AtomicU32,
    line: AtomicBool,
    edge: AtomicBool,
}

impl EdgeLine {
    fn new() -> Self {
        Self {
            pending: AtomicU32::new(0),
            line: AtomicBool::new(false),
            edge: AtomicBool::new(false),
        }
    }

    fn raise(&self, mask: u32) {
        let prev = self.pending.fetch_or(mask, Ordering::SeqCst);
        let new_pending = prev | mask;
        self.update_line(new_pending);
    }

    fn clear(&self, mask: u32) {
        let prev = self.pending.fetch_and(!mask, Ordering::SeqCst);
        let new_pending = prev & !mask;
        self.update_line(new_pending);
    }

    fn pending(&self) -> u32 {
        self.pending.load(Ordering::SeqCst)
    }

    fn line(&self) -> bool {
        self.line.load(Ordering::SeqCst)
    }

    fn edge_latched(&self) -> bool {
        self.edge.load(Ordering::SeqCst)
    }

    fn take_edge(&self) -> bool {
        self.edge.swap(false, Ordering::SeqCst)
    }

    fn update_line(&self, pending: u32) {
        let should_assert = pending != 0;
        let was_asserted = self.line.swap(should_assert, Ordering::SeqCst);
        if should_assert && !was_asserted {
            self.edge.store(true, Ordering::SeqCst);
        } else if !should_assert && was_asserted {
            self.edge.store(false, Ordering::SeqCst);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE0: u32 = 1 << 0;
    const SOURCE1: u32 = 1 << 1;

    #[test]
    fn irq_line_tracks_pending_and_enable() {
        let ctrl = InterruptController::new();
        assert!(!ctrl.irq_line());

        // Pending without enable should not assert the line.
        ctrl.raise_irq(SOURCE0);
        assert_eq!(ctrl.irq_pending(), SOURCE0);
        assert!(!ctrl.irq_line());

        // Enabling the source should assert the line.
        ctrl.enable_irq(SOURCE0);
        assert!(ctrl.irq_line());
        assert_eq!(ctrl.snapshot().irq_pending, SOURCE0);

        // Clearing the IRQ should deassert.
        ctrl.clear_irq(SOURCE0);
        assert_eq!(ctrl.irq_pending(), 0);
        assert!(!ctrl.irq_line());
    }

    #[test]
    fn irq_disable_clears_line() {
        let ctrl = InterruptController::new();
        ctrl.enable_irq(SOURCE0 | SOURCE1);
        ctrl.raise_irq(SOURCE0 | SOURCE1);
        assert!(ctrl.irq_line());

        ctrl.disable_irq(SOURCE0 | SOURCE1);
        assert!(!ctrl.irq_line());
        assert_eq!(ctrl.irq_pending(), SOURCE0 | SOURCE1);
    }

    #[test]
    fn nmi_edge_latches_once_per_assertion() {
        let ctrl = InterruptController::new();

        ctrl.raise_nmi(SOURCE0);
        assert!(ctrl.nmi_line());
        assert!(ctrl.take_nmi_edge());
        assert!(!ctrl.take_nmi_edge(), "edge should clear after consumption");

        // Additional raises while the line is asserted should not create new edges.
        ctrl.raise_nmi(SOURCE0);
        assert!(!ctrl.take_nmi_edge());

        ctrl.clear_nmi(SOURCE0);
        assert!(!ctrl.nmi_line());
        assert!(!ctrl.take_nmi_edge());
    }

    #[test]
    fn nmi_reasserts_after_clear() {
        let ctrl = InterruptController::new();

        ctrl.raise_nmi(SOURCE0);
        assert!(ctrl.take_nmi_edge());
        ctrl.clear_nmi(SOURCE0);
        assert!(!ctrl.nmi_line());

        ctrl.raise_nmi(SOURCE0);
        assert!(ctrl.nmi_line());
        assert!(ctrl.take_nmi_edge(), "clearing the line should allow a new edge");
    }

    #[test]
    fn enable_nmi_mid_stream_asserts_line() {
        let ctrl = InterruptController::new();
        ctrl.raise_nmi(SOURCE0);
        assert!(ctrl.nmi_line());
        assert!(ctrl.take_nmi_edge());

        // Raising again while pending should not generate a new edge until cleared.
        ctrl.raise_nmi(SOURCE0);
        assert!(ctrl.nmi_line());
        assert!(!ctrl.take_nmi_edge());

        ctrl.clear_nmi(SOURCE0);
        assert!(!ctrl.nmi_line());

        ctrl.raise_nmi(SOURCE0);
        assert!(ctrl.nmi_line());
        assert!(ctrl.take_nmi_edge());
    }
}
