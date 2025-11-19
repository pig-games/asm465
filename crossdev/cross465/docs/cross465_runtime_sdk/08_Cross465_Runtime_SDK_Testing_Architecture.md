# Cross465 Runtime SDK — Unified Testing Architecture (with Cargo Test Integration and Host-Expect Model)

## Overview
This document specifies the architecture for a **unified testing framework** that spans:
- Inline/standalone 6502 tests (assembled with 64tass),
- Cross465 emulator/integration tests,
- Remote hardware tests (Ultimate64, MEGA65),
- and **first-class integration with `cargo test`**.

It defines the **Runtime Test Stream (RTST)** protocol, the `cross465-test-runner` executable,
target backends, and a new **Host-Expect comparison model** that allows the 6502 side to log actuals
while Rust (via the `cross465_runner` library) performs expectation checks. Invoke
`cross465-test-runner` directly for CLI workflows; `cargo test` links the same logic from the
library so no separate process is needed.

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
`--target <id>` chooses backend (``cross465``, ``asm465``, ``asm465-wasm``, ``ultimate64``, ``mega65``).

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

## 7. Authoring & Workflow Guide

### 7.1 Including the RTST Macros

- Every case must include `native/src/include/test_rtst.h` (aliased as `test_rtst.inc` in the 64tass
  include path). Use `.include "test_rtst.inc"` at the top of the file or inline string. The header
  defines the `RTST_BEGIN/END`, `TEST_CASE_*`, `LOG_*`, and `.ctest.*` helpers shown earlier.
- Declare a `RTST_BASE` symbol (e.g., `$C000` for Cross465/Ultimate64) before
  invoking the macros. This keeps assembler diagnostics friendly.
- When authoring strings, prefer `.null` so the runner can parse names exactly once.

### 7.2 Host-Expect & `asm6502_test!`

- Use `asm6502_test!` inside Rust unit tests to assemble inline snippets or `.s` files. The builder
  mirrors all CLI flags (target, personality, timeout, seed, extra includes) and returns a `Result`
  you can feed to `assert_case_ok`, `expect_eq`, etc.
- Inline tests automatically dump PRG/RTST artifacts under `target/cross465-runner/<target>/<case>`,
  so you can inspect the raw stream.
- For CLI workflows, `cross465-test-runner --mode run --format json --artifacts` provides the same
  data; the JSON mirrors what `asm6502_test!` exposes programmatically.

### 7.3 Fixture Workflow

- Enable fixture checking via `--fixtures` (uses `tests/fixtures/<target>` by default) and refresh
  goldens with `--update-fixtures`. The runner writes one JSON file per case containing `scalars`,
  `hashes`, `memories`, `registers`, and `timings`.
- The same flags are available in `AsmTestBuilder` (`builder.fixtures(...).update_fixtures(true)`).
  Use them whenever you add new ACT_* records so developers get immediate, cargo-friendly diffs.
- Fixture files are regular JSON; review them in PRs just like you would review snapshots.

### 7.4 Backend Configuration Cheat Sheet

- Cross465 (emulator): no extra configuration required. Use `--personality modern-retro` (default) or
  `c64-compat` when developing compatibility suites.
- asm465 (native): point `--target asm465` at a running asm465 desktop build. By default the runner
  talks to `127.0.0.1:7465`, but you can override the endpoint via
  `--asm465-host/--asm465-port` (or `CROSS465_NATIVE_HOST/PORT`). When the host is loopback and the
  workspace is available, the runner will automatically `cargo run --manifest-path crossdev/asm465`
  with `--service-port/--service-host/--max-cycles`. Use `--asm465-max-cycles` (or
  `CROSS465_MAX_CYCLES`) to tweak the runtime budget, use `--asm465-keep-alive` (or
  `CROSS465_ASM465_KEEP_ALIVE=1`) to leave the auto-launched runtime running after your tests finish,
  and inspect logs under `target/cross465-runner/asm465_native.log`. The service monitors the RTST
  stream directly, so `metric cycles=` now reflects the real execution time instead of the hard
  cycle budget.
- asm465-wasm: use `--target asm465-wasm` to drive a browser/WebAssembly build through the
  `asm465-server` bridge. The CLI flags `--asm465-bridge-host/--asm465-bridge-port` configure the
  TCP control socket, while `--asm465-ws-host/--asm465-ws-port` control the websocket fan-out. When
  the bridge host is loopback the runner auto-starts `cargo run --manifest-path crossdev/asm465-server`.
  Make sure the wasm viewer is connected to the websocket; if no clients are listening the runner
  reports an `asm465 backend error: no websocket clients connected`.
- Ultimate64: configure `CROSS465_ULTIMATE64_HOST` / `PORT` (or `--ultimate64-host/--ultimate64-port`),
  the REST timeouts (`*_POLL_DELAY_MS`, `*_CONNECT_TIMEOUT_MS`, `*_READ_TIMEOUT_MS`), and optionally
  a `[ci.matrix.ultimate64].remote_failure = "warn"` entry to skip runs when the hardware is offline.
- MEGA65: set `CROSS465_MEGA65_M65_PATH`, `*_SERIAL`, `*_BAUD`, and `*_RETRIES`. Catalog entries can
  supply include paths for MEGA65-specific headers.
- All backends honor catalog-provided include paths (`include`), defines (`define`), extra 64tass args
  (`tass_args`), and the workspace root (`workspace = "../../.."`) so local and CI runs stay in sync.

## 8. Troubleshooting & Isolation

### 8.1 64tass / Include Errors

- “not defined symbol `rtst`” or “can't open file `test_rtst.h`” indicates the header wasn’t found.
  Ensure your catalog entry includes `native/src/include` and `native/src/platform/<target>/include`
  (already present in the sample catalog), or pass additional `--include` flags/`builder.include(...)`.
- If 64tass itself isn’t on PATH, pass `--tass /path/to/64tass` or set `TASS=...` when invoking the
  CLI. The runner prints the assembler stdout/stderr on failure.

### 8.2 Backend Connectivity Issues

- Ultimate64/Mega65 failures throw `ultimate64 backend error: ...` or `mega65 backend error: ...`.
  Use the new `--remote-failure warn` flag (or `[ci.matrix.<target>].remote_failure = "warn"`) to log
  a warning and skip unreachable hardware while you iterate locally. Leave it at `error` in CI.
- asm465-native/asm465-wasm failures surface as `asm465 backend error: …`. Verify the service host/
  port (`--asm465-host/--asm465-port` or `--asm465-bridge-host/--asm465-bridge-port`), and ensure the
  wasm viewer is connected to the bridge. The remote failure policy applies here as well, so you can
  elect to warn instead of failing when the GUI isn’t running.
- Regardless of the policy, the runner always writes artifacts (`program.prg`, `rtst.bin`,
  `meta.json`) so you can inspect partial runs or send the files to someone with hardware access.

### 8.3 Fixture & JSON Diffs

- When fixture checks fail, the runner prints a diff-style message (`fixture mismatch [scalar] ...`)
  and points at the JSON file under `tests/fixtures/...`. Re-run with `--update-fixtures` only after
  verifying the new values are expected.
- The JSON output (`--format json`) mirrors the CLI summary and includes per-case `metrics`, `logs`,
  and actuals. It’s useful for debugging automation before we wire CI.

### 8.4 Isolation Best Practices

- Each RTST case should clean up after itself: disable IRQs when testing critical sections, restore
  zero-page state, and avoid writing outside the RTST buffer unless you log the mutation as ACT_MEM.
- Use deterministic seeds (`--seed` / `AsmTestBuilder::seed`) to make flaky tests reproducible.
- Prefer `.ctest.begin` / `.ctest.end` labels (from `test_rtst.h`) when authoring multi-assert cases;
  they enforce a consistent structure and make aggregated logs easier to read.

## 9. Errors and Comparison Failures

| Error | Detection | Runner response |
|-------|------------|-----------------|
| No MAGIC | header invalid | initialization failure + artifacts saved |
| No progress | static WPOS | timeout (`--progress-timeout-ms`, logs last WPOS) |
| Partial record | short LEN | test failed + `rtst.bin` dump |
| Comparison fail | host diff mismatch | mark FAILED, print expected/actual |
| Target offline | I/O error | retry (`--transport-retries`), fail if persistent |

When artifacts are enabled (default), every failure writes `target/cross465-runner/<target>/<case>/{program.prg,rtst.bin,meta.json}`. The metadata captures the stage (`backend`, `rtst`, `case`), status, error string, cycles, RTST byte count, and the final header `WPOS`.

---

## 10. CI Matrix and Cross‑Target Validation

### 8.1 Matrix Inputs

Targets and personalities form a tree: each target defines zero or more entry points in `[ci.matrix.<target>]`. A `personalities = []` clause means “run with the target’s built-in personality”. This data feeds `--ci-matrix`, so automated workflows don’t need to hardcode their own target/personality cartesian products.

### 8.2 Driving the Matrix from `catalog.toml`

`crossdev/cross465/tests/catalog.toml` stores every case plus optional tags (e.g. `"demo"`, `"rtst"`). Use the `[ci.matrix.<target>]` tables to declare which personalities should run on each backend, and an optional top-level `workspace = "<path>"` entry lets the CLI resolve the project root automatically (paths are relative to the catalog file):

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
remote_failure = "error"

[ci.matrix.ultimate64]
personalities = []
include = [
  "native/src/include",
  "native/src/platform/ultimate64/include"
]
tass_args = ["-DREMOTE"]
remote_failure = "warn"

[ci.matrix.ultimate64.define]
FEATURE = "1"

[ci.matrix.ultimate64.endpoint]
host = "192.168.0.64"
port = 6510

[ci.matrix.mega65]
personalities = ["modern-retro"]
include = [
  "native/src/include",
  "native/src/platform/mega65/include"
]
tass_args = ["-Wall"]
```

With that in place CI jobs can query the catalog (for filtering) and then rely on `--ci-matrix` to execute every declared combo sequentially. To build ad-hoc case lists:

> **Note:** Use an empty `personalities = []` list to run the target with its default/built-in
> mapping (no personality override). Any `include`, `define`, `tass_args`, or `[...endpoint]` values
> declared under a `[ci.matrix.<target>]` entry are automatically merged into the runner’s options:
> - `include` &rarr; extra `-I` paths for 64tass (relative to the workspace root).
> - `define` &rarr; `-D KEY:=VALUE` pairs (TOML table syntax keeps them organized).
> - `tass_args` &rarr; additional raw arguments passed to 64tass.
> - `remote_failure` &rarr; how to treat transport errors when the target is unreachable (`"error"` or `"warn"`).
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

### 10.3 CI Integration

Workflows (GitHub Actions, Buildkite, etc.) can shell out to the runner with `--ci-matrix` so every declared target/personality combination is exercised automatically. Capture the JSON output or the artifact directory (`target/cross465-runner/…`) to feed dashboards or log archives. Today this same JSON output and the PRG/RTST artifacts are already useful for local/manual workflows; future automation can consume the same files when we decide to wire them into CI.

Extend the matrix with `mega65` for hardware labs, and add a weekly job that invokes the runner with `--fixtures --update-fixtures` to refresh goldens when needed. Because each run emits JSON (`--format json`) and stores RTST dumps under `target/cross465-runner`, Codex/CI aggregators can ingest both the structured results and the raw artifacts.

---

## 11. Example Flow

```text
[6502 test] --> RTST (ACT_KV: scroll_x=0x123, ACT_HASH: fb_hash=deadbeef)
     ↓
cross465-test-runner (or `cross465_runner` via cargo test) parses RTST
     ↓
compare against expectations
     ↓
print cargo output:
  test display::scroll ... FAILED
    expected fb_hash=8c0b…
    got fb_hash=dead…
```

---

## 12. Summary

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
