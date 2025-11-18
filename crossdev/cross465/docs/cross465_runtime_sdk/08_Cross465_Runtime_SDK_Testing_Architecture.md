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
`--personality <id>` controls MMIO layout (``modern-retro``, ``c64-compat``); omit it for targets
with a fixed/built-in layout such as the Ultimate64.  
`--target <id>` chooses backend (``cross465``, ``ultimate64``, ``mega65``).

> **Future expansion:** even “fixed” targets can benefit from the personality catalog once we start
> modeling hardware variants (e.g., JiffyDOS kernels, REU banks, extra SIDs). Those definitions could
> drive both host-side validation (e.g., flagging invalid MMIO addresses) and generated assembler
> includes so 6502 sources fail to assemble when referencing registers that a given personality
> doesn’t expose. Keep target-specific `[ci.matrix.<target>]` sections up to date as those variants
> materialize.

### Output
Per-case lines (`test fqname ... ok|FAILED`), failure grouping, summary.  
`--format json` for CI/Codex.

### Diagnostics & Artifacts
- `--progress-timeout-ms <ms>` (default: 750) aborts runs when a target stops
  advancing the RTST `WPOS` pointer; set to `0` to disable.
- `--transport-retries <n>` retries Ultimate64/MEGA65 memory reads in-place
  before propagating a backend error.
- `--artifacts [DIR]` writes `<dir>/<target>/<case>/{program.prg,rtst.bin,meta.json}`
  on failure (default root: `target/cross465-runner`); add `--keep-success-artifacts`
  to retain passing cases or `--no-artifacts` to disable.
- `--log-metrics` prints per-case lines such as
  `metric case=display::parallax status=passed cycles=523812 rtst_bytes=4096 write_pos=372`
  for CI ingestion.
- `--ci-matrix` iterates over `[ci.matrix.<target>]` entries declared in
  `crossdev/cross465/tests/catalog.toml`, running every target/personality
  combination without having to pass `--target`/`--personality` manually.

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

Every case is executed on a freshly constructed backend so hardware targets
receive an implicit reset between runs.

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

Every `CaseReport` now carries `metrics` (cycles, RTST byte count, header `WPOS`),
which are serialized in JSON output and can be mirrored to stdout via `--log-metrics`.

For convenience, the crate also exposes top-level helpers:

- `assert_case_ok(&result)` / `assert_case_failed(&result)` operate on an `asm6502_test!`
  result or a `CaseReport` (use the `*_named` variants if you need to double-check the case name).
- `expect_eq`, `expect_in`, `expect_hash_eq`, `expect_mem_eq` accept the same inputs, so tests can
  call `expect_eq(&out, "scroll_x", 0x12)?;` directly.
---

## 7. Errors and Comparison Failures

| Error | Detection | Runner response |
|-------|------------|-----------------|
| No MAGIC | header invalid | initialization failure + artifacts saved |
| No progress | static WPOS | timeout (`--progress-timeout-ms`, logs last WPOS) |
| Partial record | short LEN | test failed + `rtst.bin` dump |
| Comparison fail | host diff mismatch | mark FAILED, print expected/actual |
| Target offline | I/O error | retry (`--transport-retries`), fail if persistent |

When artifacts are enabled (default), every failure writes
`target/cross465-runner/<target>/<case>/{program.prg,rtst.bin,meta.json}`. The
metadata captures the stage (`backend`, `rtst`, `case`), status, error string,
cycles, RTST byte count, and the final header `WPOS`.

---

## 8. CI Matrix and Cross‑Target Validation

### 8.1 Matrix Inputs

Targets and personalities form a tree: each target defines zero or more entry points in
`[ci.matrix.<target>]`. A `personalities = []` clause means “run with the target’s built-in
personality”. This data feeds `--ci-matrix`, so automated workflows don’t need to hardcode their
own target/personality cartesian products.

### 8.2 Driving the Matrix from `catalog.toml`

`crossdev/cross465/tests/catalog.toml` stores every case plus optional tags (e.g. `"demo"`,
`"rtst"`). Use the `[ci.matrix.<target>]` tables to declare which personalities should run on
each backend, and an optional top-level `workspace = "<path>"` entry lets the CLI resolve the
project root automatically (paths are relative to the catalog file):

```toml
workspace = "../../.."

[ci.matrix.cross465]
personalities = ["modern-retro", "c64-compat"]
include = [
  "native/src/include",
  "native/src/platform/cross465/include"
]
define = { PLATFORM = "cross465" }
tass_args = ["-Wall"]

[ci.matrix.ultimate64]
personalities = []

[ci.matrix.ultimate64.endpoint]
host = "192.168.0.64"
port = 6510
```

With that in place CI jobs can query the catalog (for filtering) and then rely on
`--ci-matrix` to execute every declared combo sequentially. To build ad-hoc case lists:

> **Note:** Use an empty `personalities = []` list to run the target with its default/built-in
> mapping (no personality override). Any `include`, `define`, `tass_args`, or `[...endpoint]` values
> declared under a `[ci.matrix.<target>]` entry are automatically merged into the runner’s options:
> - `include` &rarr; extra `-I` paths for 64tass (relative to the workspace root).
> - `define` &rarr; `-D KEY:=VALUE` pairs (TOML table syntax keeps them organized).
> - `tass_args` &rarr; additional raw arguments passed to 64tass.
> - `endpoint` &rarr; per-target connection details (currently IP/port for Ultimate64).
>
> With those settings in the catalog, you no longer need to pass `--include`, `--define`, or remote
> host/port flags manually.

```bash
python - <<'PY' > ci-cases.json
import json, tomllib, pathlib
data = tomllib.loads(pathlib.Path("crossdev/cross465/tests/catalog.toml").read_text())
cases = [entry["name"] for entry in data["case"] if "rtst" in entry.get("tags", [])]
print(json.dumps(cases))
PY
```

With the generated JSON you can pass `--case case::name` repeatedly (or rely on the default
“run everything” behavior). For discovery-only jobs use `--mode list --format json` and feed
the output straight into Codex or CI dashboards.

### 8.3 CI Integration

Workflows (GitHub Actions, Buildkite, etc.) can shell out to the runner with `--ci-matrix` so every
declared target/personality combination is exercised automatically. Capture the JSON output or the
artifact directory (`target/cross465-runner/…`) to feed dashboards or log archives—no fixed matrix
snippet required here.

Extend the matrix with `mega65` for hardware labs, and add a weekly job that invokes the
runner with `--fixtures --update-fixtures` to refresh goldens when needed. Because each
run emits JSON (`--format json`) and stores RTST dumps under `target/cross465-runner`,
Codex/CI aggregators can ingest both the structured results and the raw artifacts.

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
    assert_case_ok(&out)?;
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
    assert_case_ok(&out)?;
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

`cross465_runner` now ships both pieces: `AsmTestBuilder`/`run_asm6502_case()` for manual control, and the `asm6502_test!` macro shown above that expands into the builder calls.

The builder/macro accept additional knobs so you can mirror CLI invocations:
- `include = ["path/a", "path/b"]`
- `defines = ["FLAG" => "1", "FOO" => "$c000"]`
- `tass_args = ["--nostart"]` (passed verbatim to 64tass)
-   plus the fixture/update flags described earlier.

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
