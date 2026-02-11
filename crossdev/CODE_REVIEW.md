# crossdev Code Review — Remaining Issues

> Generated 2026-02-11.  The original review contained 20 findings; 12 have
> been resolved.  This document captures only the **8 open items** so a future
> agent session can pick up where we left off.

---

## Severity guide

| Level  | Meaning |
|--------|---------|
| Medium | Measurable maintenance cost or latent bug risk |
| Low    | Hygiene / long-term quality improvements |

---

## Medium severity

### 1. MmioDevice / Module trait duplication

| | |
|-|-|
| **Files** | `cross465/bus/src/lib.rs` (line ~951), `cross465/bus/src/mmio.rs` (line ~331) |
| **Impact** | Two parallel dispatch paths for the same concept |

`MmioDevice` (legacy) defines `read(&mut self, addr) -> u8` and
`write(&mut self, addr, value)`.  `Module` (v2 personalities) is a supertrait
of `MmioDevice` that adds `kind()`, register-level access, `tick()`,
`snapshot()`/`restore()`, etc.

The legacy `MappedDevice` in `Bus` stores `Box<dyn MmioDevice>`, while v2
modules are wrapped in `ModuleInstance` around `Box<dyn Module>`.  Every new
MMIO device must decide which trait to implement, and consumers of the legacy
path can't benefit from register introspection.

**Suggested fix:** Merge `MmioDevice` into `Module` (give `Module` default
no-op `read`/`write`) and update `MappedDevice` to hold `Box<dyn Module>`.
This unifies the two personality code paths and is a prerequisite for
eliminating Issue 5 below.

---

### 2. Cpu::bus is a public field

| | |
|-|-|
| **File** | `cross465/core6502/src/lib.rs` (line ~123) |
| **Impact** | Broken encapsulation; external code bypasses CPU cycle accounting |

```rust
pub struct Cpu {
    pub bus: Bus,  // ← should not be pub
    // ...
}
```

External code (the runner, asm465 crate) reaches directly into `cpu.bus` for
reads/writes, bypassing any CPU-level invariants (cycle tracking, interrupt
checks).

**Suggested fix:** Make `bus` private (or `pub(crate)`) and expose accessor
methods: `bus(&self) -> &Bus` / `bus_mut(&mut self) -> &mut Bus`.  Update the
~3 external call sites in the runner and asm465 crate.

---

### 3. `write_console_line` hardcodes MMIO addresses

| | |
|-|-|
| **File** | `asm465/src/lib.rs` (line ~2604) |
| **Impact** | Silently breaks if personality changes console mapping |

```rust
pub(crate) fn write_console_line(bus: &mut Bus, line: &str) {
    for ch in line.chars() {
        let screen_code = unicode_to_screen(ch);
        if screen_code == b'\n' {
            bus.write(0xDF01, 0);       // magic number
        } else {
            bus.write(0xDF00, screen_code); // magic number
        }
    }
    bus.write(0xDF01, 0);
}
```

`0xDF00`/`0xDF01` must match the `ConsoleMmio` mapping in the personality
definition (`0xDF00..=0xDF1F` in `bus/src/personality.rs` line ~90).

**Suggested fix:** Export the console base address as constants from the bus
crate (`bus::CONSOLE_CHAR_ADDR`, `bus::CONSOLE_COMMIT_ADDR`), or add a
`Bus::write_console(ch)` helper that routes through the personality's console
module.

---

### 4. RTST polling loop duplication

| | |
|-|-|
| **File** | `cross465/cross465-runner/src/executor.rs` (3 locations ≈ lines 118–180, 340–400, 600–650) |
| **Impact** | Protocol or bug fixes must be applied 3× |

The RTST header-polling + progress-tracking + timeout state machine is
copy-pasted across `Cross465Backend`, `Ultimate64Backend`, and
`Mega65Backend`.  Each backend re-implements:

```
parse header → check init → track progress → check terminal → sleep
```

**Suggested fix:** Extract a generic `RtstPoller` or `poll_rtst_loop()`
function that accepts a closure for "read header bytes" and a config struct
for timeouts/intervals.  Each backend provides only its transport-specific
read logic.

---

## Low severity

### 5. PersonalityMmioKind duplicates ModuleKind

| | |
|-|-|
| **Files** | `cross465/bus/src/personality.rs` (line ~73), `cross465/bus/src/mmio.rs` (line ~16) |

```rust
// personality.rs                     // mmio.rs
enum PersonalityMmioKind {            enum ModuleKind {
    Console,                              Console,
    Display,                              Display,
    Sprite,                               Sprite,
    System,                               System,
    Input,                                Input,
}                                         Video,   // extra
                                          Audio,   // extra
                                      }
```

`PersonalityMmioKind` is an exact subset (5 of 7 variants) of `ModuleKind`.
Both classify the same concept.

**Suggested fix:** Delete `PersonalityMmioKind`; use `ModuleKind` everywhere.
This pairs naturally with Issue 1 (trait unification).

---

### 6. SeqCst atomics in interrupt controller

| | |
|-|-|
| **File** | `cross465/bus/src/interrupts.rs` (lines ~143–229) |
| **Impact** | Unnecessary memory fences on ARM/WASM (no-op on x86) |

There are 17 `Ordering::SeqCst` usages across `LevelLine` and `EdgeLine`.
The actual requirements are weaker:

| Operation | Sufficient ordering |
|-----------|-------------------|
| `fetch_or`, `fetch_and`, `swap` | `AcqRel` |
| plain `store` | `Release` |
| plain `load` | `Acquire` |

**Suggested fix:** Replace orderings as above.  Optionally verify with the
`loom` crate if concurrency testing is added.  Lowest priority since x86
perf-identical.

---

### 7. No crate-level lint configuration

| | |
|-|-|
| **Files** | `cross465/Cargo.toml`, all member `Cargo.toml` files, all `lib.rs` roots |

None of the crate roots or the workspace `Cargo.toml` set any lint policy.
`dead_code`, `unused_imports`, `missing_docs`, `unsafe_code`, etc. all use
compiler defaults.  There's no CI-enforceable baseline.

**Suggested fix:** Add to the workspace `Cargo.toml`:

```toml
[workspace.lints.rust]
unsafe_code = "deny"
unused_must_use = "deny"
missing_debug_implementations = "warn"

[workspace.lints.clippy]
all = { level = "warn", priority = -1 }
pedantic = { level = "warn", priority = -1 }
```

And in each member `Cargo.toml`:

```toml
[lints]
workspace = true
```

---

### 8. `std::env::set_var` in test runner (unsound)

| | |
|-|-|
| **File** | `cross465/cross465-runner/src/bin/cross465-test-runner.rs` (lines ~64–91, ~573–596) |
| **Impact** | UB risk in multi-threaded context; compile error on Rust ≥ 1.83 |

18 calls to `std::env::set_var` across two locations pass CLI/CI config
values through environment variables.  Since Rust 1.66 this is documented as
unsound in multi-threaded programs; since Rust 1.83 it requires `unsafe`.

```rust
// Line ~64: CLI override block
if let Some(host) = &cli.ultimate64_host {
    std::env::set_var("CROSS465_ULTIMATE64_HOST", host);
}
// ... 17 more similar calls
```

**Suggested fix:** Replace env-var-based config passing with a proper
`EndpointConfig` struct populated from CLI args and CI matrix entries, then
threaded through to backends.  Each backend's `*Config::from_env()` method
would read env vars once at startup (before threads) or read the config struct
directly.

---

## Summary table

| # | Severity | Title | Files |
|---|----------|-------|-------|
| 1 | Medium | MmioDevice / Module trait duplication | `bus/src/lib.rs`, `bus/src/mmio.rs` |
| 2 | Medium | Cpu::bus public field | `core6502/src/lib.rs` |
| 3 | Medium | `write_console_line` hardcoded addrs | `asm465/src/lib.rs` |
| 4 | Medium | RTST polling loop duplication | `cross465-runner/src/executor.rs` |
| 5 | Low | PersonalityMmioKind ≈ ModuleKind | `bus/src/personality.rs`, `bus/src/mmio.rs` |
| 6 | Low | SeqCst atomics overkill | `bus/src/interrupts.rs` |
| 7 | Low | No workspace lint config | `Cargo.toml` (workspace + members) |
| 8 | Low | `set_var` unsound in runner | `cross465-runner/src/bin/cross465-test-runner.rs` |

Issues 1 and 5 are related (trait + enum unification) and should be tackled together.
