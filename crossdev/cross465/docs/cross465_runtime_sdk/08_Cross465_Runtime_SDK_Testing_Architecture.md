# Cross465 Runtime SDK — Unified Testing Architecture (with Cargo Test Integration and Host-Expect Model)

## Overview
This document specifies the architecture for a **unified testing framework** that spans:
- Inline/standalone 6502 tests (assembled with 64tass),
- Cross465 emulator/integration tests,
- Remote hardware tests (Ultimate64, MEGA65),
- and **first-class integration with `cargo test`**.

It defines the **Runtime Test Stream (RTST)** protocol, the cargo-facing runner, target backends,
and a new **Host-Expect comparison model** that allows the 6502 side to log actuals while Rust (cargo)
performs expectation checks, ideal for modern runtime and personality validation.

---

## 1. Architectural pillars
1. **Single authoring surface:** 6502 assembly (inline-in-Rust or .s files), shared macros.
2. **Target-agnostic result protocol:** RTST record stream with explicit lifecycle (`STATE`, `WPOS`).
3. **Cargo-facing runner contract:** host binary discovers, runs, and prints cargo-style results.
4. **Pluggable targets:** Cross465 emulator, Ultimate64, MEGA65 backends.
5. **Deterministic orchestration:** timeout, seeding, isolation, artifact capture.
6. **Dual comparison modes:** 6502-side asserts and host-side expectations (Host-Expect).

---

## 2. Runtime Test Stream (RTST) Protocol
**Header (at RTST_BASE):**
- `MAGIC="RTST"`, `VERSION=0x01`
- `STATE`: `0=pending`, `1=running`, `2=done`
- `WPOS`: write cursor (u16)
- `TOTAL`, `PASS`, `FAIL` (u16)

**Records (from `RTST_BASE+0x10`):**
| KIND | ID | Description |
|------|----|--------------|
| CASE_START | 0x01 | begin test case |
| OK | 0x02 | case passed |
| FAIL | 0x03 | case failed |
| ASSERT | 0x04 | inline 6502-Assert message |
| MSG | 0x05 | log message |
| END | 0xFF | end of stream |

**End-of-run:** append END, set STATE=2, enter infinite loop to keep memory stable.

**Default bases:**  
C64/U64=$C000 (4K), MEGA65=$40000 (8K), Cross465 mirrors chosen personality.

---

## 3. Cargo Test Integration Architecture
Cargo expects a test binary that can list and run cases, printing pass/fail lines.

### Runner responsibilities
- **Assemble:** invoke 64tass with include paths.
- **Deploy+Start:** push PRG to target (emulator/U64/M65).
- **Poll:** read RTST header and records until STATE=2 or timeout.
- **Render:** cargo-style TTY output and JSON export.
- **Exit:** non-zero exit on any failure.

### Modes
- **Discovery (`--mode list`)**: enumerates CASE_START only.
- **Execution (`--mode run --case <name>`)**: runs one case, emits full RTST.

### Personality/Target
`--personality <id>` controls MMIO layout (``modern-retro``, ``c64-compat``).  
`--target <id>` chooses backend (``cross465``, ``ultimate64``, ``mega65``).

### Output
Per-case lines (`test fqname ... ok|FAILED`), failure grouping, summary.  
`--format json` for CI/Codex.

---

## 4. Target Backends
| Backend | Transport | Notes |
|----------|------------|-------|
| **Cross465** | local CLI/API | fast, default |
| **Ultimate64** | REST API (`run_prg`, `machine:readmem`) | Upload PRG + DMA read |
| **MEGA65** | `m65` CLI / libmegal65 | upload + memory read |

All implement: `assemble()`, `push_and_run()`, `read_mem()`, `reset()`.

Ultimate64 runs talk to the hardware REST API. Configure host/port/polling via
`--ultimate64-host/--ultimate64-port` (runner CLI) or the env vars
`CROSS465_ULTIMATE64_HOST` / `CROSS465_ULTIMATE64_PORT`,
`CROSS465_ULTIMATE64_POLL_DELAY_MS`, `CROSS465_ULTIMATE64_CONNECT_TIMEOUT_MS`,
`CROSS465_ULTIMATE64_READ_TIMEOUT_MS`, and `CROSS465_ULTIMATE64_RETRIES`.

MEGA65 runs shell out to the `m65` CLI. Point the runner at the correct binary
and serial port via `CROSS465_MEGA65_M65_PATH`, `CROSS465_MEGA65_SERIAL`,
`CROSS465_MEGA65_BAUD`, `CROSS465_MEGA65_POLL_DELAY_MS`, and
`CROSS465_MEGA65_RETRIES`.

---

## 5. 6502 Test Authoring Model
### Common include
`test_rtst.inc` provides macros for RTST header, records, and END loop.

### Case structure
Each test emits CASE_START, logs results or actuals, and ends cleanly.

### Isolation
Per-case setup/teardown; mask IRQs when needed.

---

## 6. Test Authoring Models: Assert vs Host‑Expect

Two complementary models share the same RTST channel.

| Mode | Who compares | Typical use | Output |
|------|---------------|--------------|---------|
| **6502‑Assert** | 6502 emits OK/FAIL directly | Native asm465 editor, CPU tests | RTST CASE/OK/FAIL/ASSERT |
| **Host‑Expect** | Cargo test compares actuals to expected | Runtime/personality validation, cross‑target diffs | RTST ACT_* records (actuals) |

### 6.1 RTST Actuals Extensions

| Kind | ID | Payload | Purpose |
|------|----|----------|----------|
| `ACT_KV` | 0x10 | key, value | generic key/value pair |
| `ACT_MEM` | 0x11 | addr, len, bytes | small memory dump |
| `ACT_HASH` | 0x12 | key, hash | content fingerprint |
| `ACT_REGS` | 0x13 | CPU registers | diagnostic dump |
| `ACT_TIME` | 0x14 | u32 | timing / cycles |

The protocol remains backward compatible. Older parsers ignore unknown IDs.

### 6.2 Host‑Side Comparison Model

Cargo collects ACT_* records into a map (`key→value`) and performs expectations via Rust asserts or golden fixtures.

- **Exact equals:** `expect_eq("scroll_x", 0x0123)`  
- **Ranges:** `expect_in("fps", 57..=63)`  
- **Hash equality:** `expect_hash_eq("fb_hash", golden!("fb_hash"))`  
- **Cross‑target diff:** compare two runs’ actuals.

Fixtures live under `tests/fixtures/<target>/<suite>.json|hash`.

**Benefits:**
- Smaller RTST payloads.
- Same 6502 binary usable across all targets.
- Allows richer predicates and golden tests on host.

#### Host-Expect Helper APIs

`cross465_runner` now exposes `CaseReport::actuals_view()` with typed helpers and
also groups the ACT_* payloads into `case.actual_groups` (scalars, hashes, memories,
register snapshots, timings) for quick iteration.

```rust
let report = run_cases(&cfg, &filter, &opts)?;
let display = report.cases.iter().find(|c| c.name == "display::parallax_scroll").unwrap();
let expect = display.actuals_view();
expect.expect_eq("scroll_x", 0x0123)?;
expect.expect_in("scroll_y", 0..=0x0010)?;
expect.expect_hash_eq("fb_hash", 0xDEADBEEF)?;
```

Additional accessors return memory dumps (`get_bytes`), register snapshots (`get_regs`),
and timing cycles (`get_cycles`), surfacing descriptive errors when a key is missing or
has the wrong record type.

Fixtures live under `tests/fixtures/<target>/<case>.json`. Use `--fixtures <dir>` to override
the root directory (or just `--fixtures` to use the default `tests/fixtures`) and
`--update-fixtures` to rewrite the golden values for the cases you run.
Each file stores the typed buckets (`scalars`, `hashes`, `memories`, `registers`, `timings`).
---

## 7. Errors and Comparison Failures

| Error | Detection | Runner response |
|-------|------------|-----------------|
| No MAGIC | header invalid | initialization failure |
| No progress | static WPOS | timeout |
| Partial record | short LEN | test failed |
| Comparison fail | host diff mismatch | mark FAILED, print expected/actual |
| Target offline | I/O error | retry (3×), fail if persistent |

---

## 8. CI Matrix and Cross‑Target Validation

- **Axes:** target × personality × OS.  
- **Artifacts:** raw RTST dumps and PRGs on failure.  
- **Cross‑target differential:** run same case on multiple backends and compare ACT_* actuals.  
- **Golden update:** `--update` flag rewrites fixtures.  
- **Codex integration:** JSON schema for ingestion and automated analysis.

---

## 9. Example Flow

```text
[6502 test] --> RTST (ACT_KV: scroll_x=0x123, ACT_HASH: fb_hash=deadbeef)
     ↓
[cargo runner] parses RTST
     ↓
compare against expectations
     ↓
print cargo output:
  test display::scroll ... FAILED
    expected fb_hash=8c0b…
    got fb_hash=dead…
```

---

## 10. Summary

The unified testing architecture now supports **two‑way validation**:
- Pure 6502 asserts for self‑contained tests and in‑editor use.
- Host‑Expect comparisons for high‑level validation across personalities and real hardware.

This design scales from quick assembler self‑tests to full runtime regression suites under `cargo test`.




### 6.3 Inline & External 6502 in Cargo Tests — All Combinations

The framework supports both **inline 6502** (embedded in Rust) and **external `.s` files**, and both
**6502‑Assert** and **Host‑Expect** comparison models.

> **API shape:** `asm6502_test!( ... )` **returns** a result struct `out` directly, so a Rust `#[test]`
> can assemble+run **and** assert in one place.

#### A) Inline + 6502‑Assert  *(6502 decides pass/fail)*

```rust
#[test]
fn inline_assert_native() -> anyhow::Result<()> {
    let out = asm6502_test!(
        name = "math::add_basic",
        personality = "`modern-retro`",
        target = "`cross465`",
        timeout_ms = 1500,
        seed = 0xDEADBEEF,
        asm = r#"
            .include "test_rtst.inc"
            RTST_BASE = $C000
            * = $2000
    start:  RTST_BEGIN
            TEST_CASE_BEGIN case_add
            lda #2 : clc : adc #3 : cmp #5
            bne :fail
              TEST_CASE_OK 0,0
              jmp :done
    :fail   TEST_CASE_FAIL 0,0
    :done   RTST_END
    case_add: .asciiz "math::add_basic"
        "#
    );
    assert_case_ok(&out, "math::add_basic")?;
    Ok(())
}
```

#### B) Inline + Host‑Expect  *(host compares actuals)*

```rust
#[test]
fn inline_host_expect() -> anyhow::Result<()> {
    let out = asm6502_test!(
        name = "display::parallax_scroll",
        personality = "`modern-retro`",
        target = "`cross465`",
        timeout_ms = 2000,
        seed = 0xDEADBEEF,
        asm = r#"
            .include "test_rtst.inc"
            RTST_BASE = $C000
            * = $2000
    start:  RTST_BEGIN
            TEST_CASE_BEGIN case_scroll
            lda #$12 : sta $D016
            lda #$03 : sta $D011
            LOG_KV   scroll_x, #$12
            LOG_KV   scroll_y, #$03
            LOG_HASH fb_hash, framebuffer, 256
            RTST_END
    scroll_x:   .asciiz "scroll_x"
    scroll_y:   .asciiz "scroll_y"
    fb_hash:    .asciiz "hash_fb"
    framebuffer:.res 256
    case_scroll:.asciiz "display::parallax_scroll"
        "#
    );
    expect_eq(&out, "scroll_x", 0x12)?;
    expect_eq(&out, "scroll_y", 0x03)?;
    expect_hash_eq(&out, "hash_fb", golden!("modern/display_parallax_scroll.fbhash"))?;
    Ok(())
}
```

#### C) External `.s` + 6502‑Assert

```rust
#[test]
fn external_assert_native() -> anyhow::Result<()> {
    let out = asm6502_test!(
        name = "irq::dma_micro",
        personality = "`c64-compat`",
        target = "`ultimate64`",
        asm_path = "native/src/tests/irq_dma_micro.s",
        timeout_ms = 5000
    );
    assert_case_ok(&out, "irq::dma_micro")?;
    Ok(())
}
```

#### D) External `.s` + Host‑Expect

```rust
#[test]
fn external_host_expect() -> anyhow::Result<()> {
    let out = asm6502_test!(
        name = "display::tilemap_hash",
        personality = "`modern-retro`",
        target = "`mega65`",
        asm_path = "native/src/tests/display_tilemap_hash.s",
        timeout_ms = 8000
    );
    expect_hash_eq(&out, "tilemap_bg0", golden!("modern/display_tilemap_hash.fbhash"))?;
    Ok(())
}
```

#### Internal Flow

The `asm6502_test!` macro delegates to a low-level helper function (e.g. `run_asm6502_case()`) that performs:
1. Write snippet to a temporary file (if inline).
2. Assemble with 64tass using correct include paths.
3. Deploy to target via backend driver.
4. Poll RTST buffer and parse results into a `TestRun` struct.
The macro simply wraps this call to make inline tests ergonomic and uniform with external ones.

**Notes**
- The macro respects CLI flags at runtime (e.g., `--format json`, `--update`, `--target`, `--personality`).
- `seed = 0xDEADBEEF` is used by convention for deterministic examples.
- Return type `out` exposes:
  - per‑case status from RTST OK/FAIL,
  - `actuals` map for Host‑Expect (keys from `LOG_*`),
  - captured `ASSERT`/`MSG` for diagnostics.

---
## Related Docs

- **Architecture:** `08_Cross465_Runtime_SDK_Testing_Architecture.md`
- **Execution Plan:** `S05_Testing_Expansion_Plan_for_Codex.md`
- **Authoring Guide:** `Authoring_6502_Tests_HowTo.md`
- **Runner Reference:** `README_TestRunner.md`
