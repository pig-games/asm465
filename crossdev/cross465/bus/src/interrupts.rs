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

impl Default for InterruptController {
    fn default() -> Self {
        Self::new()
    }
}

impl InterruptController {
    /// Create a new controller with all sources disabled and no pending events.
    #[must_use]
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
}

impl LevelLine {
    fn new() -> Self {
        Self {
            pending: AtomicU32::new(0),
            enabled: AtomicU32::new(0),
        }
    }

    fn raise(&self, mask: u32) {
        self.pending.fetch_or(mask, Ordering::AcqRel);
    }

    fn clear(&self, mask: u32) {
        self.pending.fetch_and(!mask, Ordering::AcqRel);
    }

    fn enable_bits(&self, mask: u32) {
        self.enabled.fetch_or(mask, Ordering::AcqRel);
    }

    fn disable_bits(&self, mask: u32) {
        self.enabled.fetch_and(!mask, Ordering::AcqRel);
    }

    fn set_enable(&self, value: u32) {
        self.enabled.store(value, Ordering::Release);
    }

    fn pending(&self) -> u32 {
        self.pending.load(Ordering::Acquire)
    }

    fn enabled(&self) -> u32 {
        self.enabled.load(Ordering::Acquire)
    }

    /// Compute the line state from both atomics on every read rather than
    /// caching in a separate `AtomicBool`.  This eliminates a TOCTOU race
    /// where concurrent modifications to `pending` and `enabled` could leave
    /// a stale cached `line` value.
    fn line(&self) -> bool {
        let p = self.pending.load(Ordering::Acquire);
        let e = self.enabled.load(Ordering::Acquire);
        (p & e) != 0
    }
}

struct EdgeLine {
    pending: AtomicU32,
    edge: AtomicBool,
}

impl EdgeLine {
    fn new() -> Self {
        Self {
            pending: AtomicU32::new(0),
            edge: AtomicBool::new(false),
        }
    }

    /// Raise one or more NMI sources.  The return value of `fetch_or` gives
    /// the pending word *before* the bits were set, so the edge decision is
    /// based on the atomically-correct previous state.
    fn raise(&self, mask: u32) {
        let prev = self.pending.fetch_or(mask, Ordering::AcqRel);
        // Line transitions from deasserted to asserted → latch the edge.
        if prev == 0 {
            self.edge.store(true, Ordering::Release);
        }
    }

    /// Clear NMI source bits.  If pending drops to zero the line deasserts;
    /// an unserviced edge latch is cleared (matching original semantics).
    fn clear(&self, mask: u32) {
        let prev = self.pending.fetch_and(!mask, Ordering::AcqRel);
        let new_pending = prev & !mask;
        if prev != 0 && new_pending == 0 {
            self.edge.store(false, Ordering::Release);
        }
    }

    fn pending(&self) -> u32 {
        self.pending.load(Ordering::Acquire)
    }

    /// Compute lazily — no cached `AtomicBool` to go stale.
    fn line(&self) -> bool {
        self.pending.load(Ordering::Acquire) != 0
    }

    fn edge_latched(&self) -> bool {
        self.edge.load(Ordering::Acquire)
    }

    fn take_edge(&self) -> bool {
        self.edge.swap(false, Ordering::AcqRel)
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
        assert!(
            ctrl.take_nmi_edge(),
            "clearing the line should allow a new edge"
        );
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
