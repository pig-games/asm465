# Modern UI Interrupt Model

This document captures the desired interrupt model for wiring host-side events
from the modern UI into the `cross465` bus and 6502 core. It extends the work in
`graphics_improvement_plan.md` and builds on the architectural notes in
`cross465/docs/6502_interrupts_overview.md`.

The goals for this phase are:

- mirror common 6502-era patterns (IRQ for maskable sources, NMI for
  frame-critical events);
- provide clear acknowledgement semantics so software can reliably deglitch
  interrupt lines;
- describe how personalities announce available sources and how the host UI
  translates its events into controller signals;
- keep the model future-proof for additional personalities or hardware modules.

---

## Interrupt Lines & Semantics

### IRQ (maskable)
- Level-triggered in the core; delivered whenever the Interrupt Disable flag is
  clear and the controller asserts the line.
- Multiple sources share the same line; the controller latches pending sources
  and raises the line while any enabled source remains pending.
- Software must ACK each source by writing to the acknowledgement register; the
  controller deasserts the line once no enabled pending sources remain.

### NMI (non-maskable)
- Edge-triggered; controller must drop and re-raise the line to deliver the next
  event.
- Sources are latched until the guest ACKs them. If a new NMI arrives while one
  is still pending, the controller coalesces and fires another edge as soon as
  the previous event is acknowledged.
- Personalities can gate individual NMI sources so software can opt out.

---

## Host Event → Interrupt Mapping (ModernRetro Personality)

| Source ID | Event | Line | Trigger | Default Enable | Ack requirement | Notes |
| --------- | ----- | ---- | ------- | ---------------| ----------------| ----- |
| 0 | `frame_start` | NMI | Edge | On | Write bit 0 to `NMI_ACK` after servicing | Raised when the renderer enters vblank; guarantees one NMI per displayed frame. |
| 1 | `frame_end` | IRQ | Level | Off | Write bit 1 to `IRQ_ACK` | Optional hook for games wanting post-present timing. |
| 2 | `timer0` | IRQ | Level | Off | Write bit 2 to `IRQ_ACK` | Backed by a programmable host timer module (separate task). |
| 3 | `keyboard_event` | IRQ | Level | Off | Write bit 3 to `IRQ_ACK` | Fires on key press/release while the window is focused. |
| 4 | `gamepad_event` | IRQ | Level | Off | Write bit 4 to `IRQ_ACK` | Fires on controller button/state changes, including connect/disconnect. |

Additional sources can be added later; IDs are contiguous per personality.

These definitions now live in `MODERN_RETRO_INTERRUPTS` (see
`crossdev/cross465/bus/src/personality.rs`) so tooling can introspect them at
runtime.

---

## Interrupt Controller Surface

The controller sits between host producers (UI, timers) and the 6502 core. It
must be thread-safe so host systems can signal interrupts from the Bevy thread
while the CPU runs on its own worker thread (see follow-up tasks).

When the host requests a bounded CPU run (e.g. after loading a PRG) the worker
now reports whether the batch stopped because it exhausted the cycle budget or
because it executed a `BRK`. The outcome is surfaced via the
`ProgramRunReport`/`RunOutcome` types so debugger tooling can pause on traps and
resume execution.

### Responsibilities
- Keep per-source state: enabled flag, pending bit, trigger mode (edge/level),
  and target line (IRQ/NMI).
- Provide APIs for host code:
  - `raise(source_id)` – mark pending, assert the corresponding line if enabled.
  - `clear(source_id)` – clear the pending flag without touching enable state.
  - `pending_snapshot()` – lightweight check used by tooling/tests.
- Allow the CPU core to query pending masks efficiently (e.g., through atomic
  bitfields) when deciding whether to service an interrupt.

The initial implementation lives in `crossdev/cross465/bus/src/interrupts.rs`.
It exposes `InterruptController` helpers that use atomics so the UI thread and
the CPU worker can concurrently raise/clear sources. Unit tests in the same file
cover the edge/level semantics for NMI/IRQ.

`InterruptBindings` inside `crossdev/asm465/src/lib.rs` wires these sources to
the viewer: frame lifecycle interrupts fire once per Bevy frame, a 60 Hz host
timer raises `timer0`, and keyboard/gamepad events raise the corresponding IRQ
when input is observed on the host side. The Bevy UI exposes an “Interrupts”
inspector window that surfaces the controller snapshot so developers can check
pending/enabled state at runtime.

### Register Map (proposed at `$DF40–$DF47`)

| Address | Name | Access | Description |
| ------- | ---- | ------ | ----------- |
| `$DF40` | `IRQ_PENDING` | R | Bitmask of pending IRQ sources (masked by enable when the line is asserted). |
| `$DF41` | `IRQ_ENABLE` | R/W | Bitmask selecting which IRQ sources may raise the line. |
| `$DF42` | `IRQ_ACK` | W | Writing a bit clears the corresponding pending IRQ source. Reads return `IRQ_PENDING`. |
| `$DF43` | `IRQ_SOURCE` | R | Highest-priority pending IRQ source (lowest set bit), or `0xFF` if none. |
| `$DF44` | `NMI_PENDING` | R | Bitmask of pending NMI sources. |
| `$DF45` | `NMI_ACK` | W | Writing a bit clears the corresponding pending NMI source and drops the NMI line. Reads return `NMI_PENDING`. |
| `$DF46` | `CTRL_STATUS` | R | Debug/status bitfield (e.g., latched line state, overflow counters); reserved bits read as zero. |

This register block is implemented by the `system_mmio` device (`crossdev/cross465/bus/src/system_mmio.rs`), which fronts the shared `InterruptController`.

All registers are personality-agnostic; unused bits read as zero and ignore
writes. Personalities describe which IDs are meaningful.

### CPU ↔ Controller Handshake
1. Host producer raises a source.
2. Controller sets the pending bit and, if enabled, asserts the relevant line.
3. CPU core notices the asserted line:
   - For IRQ: core checks `IRQ_PENDING & IRQ_ENABLE` to decide which handler to
     run. Priority defaults to lowest-numbered bit.
   - For NMI: the controller provides an edge immediately; the core can inspect
     `NMI_PENDING` during the handler to determine the active source.
4. Guest handler acknowledges by writing to `IRQ_ACK`/`NMI_ACK`.
5. Controller clears the bit; if no pending enabled IRQ sources remain, the IRQ
   line is deasserted. For NMI, clearing drops the line so the next edge can
   fire.

If software fails to acknowledge, the line remains asserted (IRQ) or blocked
from generating new edges (NMI), mirroring real 6502-era hardware behaviour.

---

## Personality Integration

### Descriptor Extensions
- `Personality` gains an `interrupts: &'static [PersonalityInterrupt]` slice.
- Each entry declares:
  - `id: u8` – stable source identifier (0–7 recommended).
  - `name: &'static str` – surfaced in tooling/UI.
  - `line: InterruptLine` – `Irq` or `Nmi`.
  - `trigger: InterruptTrigger` – `Level` or `Edge`.
  - `default_enable: bool` – initial value for the enable bit.
  - Optional metadata (priority override, documentation pointer).
- Personalities are responsible for wiring host producers to the controller at
  construction time. For ModernRetro this includes handing display/timer/input
  handles to the interrupt subsystem.

The base descriptors now expose these fields via the
`PersonalityInterrupt`, `InterruptLine`, and `InterruptTrigger` types defined in
`crossdev/cross465/bus/src/personality.rs`. Existing personalities provide an
empty slice until their sources are wired in later steps.

### Host Wiring Strategy
- Display loop publishes frame lifecycle callbacks; the personality registers
  them to raise `frame_start` (NMI) and `frame_end` (IRQ).
- Timer module exposes programmable intervals (period registers + enable bits
  via separate MMIO) and calls `raise(timer0)` upon expiry.
- Input manager maps keyboard/controller state changes to `keyboard_event` and
  `gamepad_event` sources and writes any event metadata into device-specific
  MMIO buffers before raising the IRQ.
- Personalities may include additional sources (e.g., DMA completion) by adding
  new IDs and mapping them to MMIO registers.

### Tooling & Documentation
- Bus docs should list interrupt sources alongside MMIO ranges so developers can
  configure handlers without scanning code.
- Viewer UI can expose a diagnostics panel that reads `IRQ_PENDING`/`NMI_PENDING`
  for debugging.

---

## Outstanding Considerations (covered in later steps)
- CPU worker thread must poll the controller at instruction cadence and honour
  pause/throttle hooks.
- Timer module design (reload values, scaling relative to host time) will be
  specified during implementation.
- Input MMIO layout (key matrix vs. FIFO) needs a follow-up doc; the interrupt
  model assumes changes are latched somewhere guest-visible before raising the
  IRQ.

With this model documented we can proceed to implement the controller, thread
architecture, and ModernRetro wiring in subsequent steps.
