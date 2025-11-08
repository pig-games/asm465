# Authoring 6502 Tests — How‑To (Unified Macro‑Based Model)

This document describes how to author tests for the **Cross465 Runtime SDK** using the unified
`asm6502_test!` macro. It covers both **inline** and **external** 6502 assembly, and supports
both **6502‑Assert** (native expected) and **Host‑Expect** (host expected) comparison modes.

All examples use a consistent macro interface that *returns* a `TestRun` result struct (`out`),
allowing assembly, execution, and assertions in a single `#[test]` function.

---

## 🧭 Overview

| Model | Authoring | Comparison | Typical Use |
|--------|------------|-------------|--------------|
| A | Inline | 6502‑Assert | Editor/unit tests; CPU/DMA microtests |
| B | Inline | Host‑Expect | Runtime & personality validation |
| C | External `.s` | 6502‑Assert | Larger integration suites; CPU coverage |
| D | External `.s` | Host‑Expect | Full cross‑target and visual validation |

All models share the same macro and return contract.

---

## 🧱 Macro Contract

### `asm6502_test!`

The macro runs a complete 6502 test and returns a `TestRun` struct.

```rust
let out = asm6502_test!(
    name = "suite::case_name",
    personality = "modern-retro",
    target = "cross465",
    timeout_ms = 2000,
    seed = 0xDEADBEEF,
    asm | asm_path = "..."
);
```

#### Parameters
| Key | Type | Description |
|------|------|-------------|
| `name` | string | Fully qualified test name |
| `personality` | string | MMIO layout (`modern-retro`, `c64-compat`, etc.) |
| `target` | string | Backend (`cross465`, `ultimate64`, `mega65`) |
| `timeout_ms` | int | Timeout in milliseconds |
| `seed` | u64 | Random seed (defaults to 0xDEADBEEF) |
| `asm` | string | Inline assembly source |
| `asm_path` | string | External `.s` file path |

#### Return Type: `TestRun`
| Field | Type | Description |
|--------|------|-------------|
| `cases[name].status` | enum | OK / FAIL (from RTST `CASE_OK/FAIL`) |
| `actuals` | map | key→value/hash/memory (from `LOG_*` macros) |
| `asserts` | list | 6502‑side assertions |
| `logs` | list | RTST `MSG` entries |

### 🧩 Internal Flow — What the Macro Does

The `asm6502_test!` macro is a thin, ergonomic wrapper around the lower-level
runtime function **`run_asm6502_case()`**.

It performs the following steps automatically:

1. Writes the inline snippet (if present) to a temporary `.s` file.
2. Invokes **64tass** with the correct include paths and flags.
3. Uploads the resulting PRG to the selected target (Cross465, Ultimate64, or MEGA65).
4. Waits for the program to complete and reads the RTST buffer.
5. Parses all CASE, ASSERT, and ACT records into a unified `TestRun` struct.
6. Returns that struct as `out` to the calling test.

This means `asm6502_test!` and `run_asm6502_case()` are fully interchangeable:
the macro exists only for syntax comfort and inline assembly support.

---

## ⚙️ Examples

Each example below runs end‑to‑end inside a Rust `#[test]`.

### A) Inline + 6502‑Assert

```rust
#[test]
fn inline_assert_native() -> anyhow::Result<()> {
    let out = asm6502_test!(
        name = "math::add_basic",
        personality = "modern-retro",
        target = "cross465",
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

---

### B) Inline + Host‑Expect

```rust
#[test]
fn inline_host_expect() -> anyhow::Result<()> {
    let out = asm6502_test!(
        name = "display::parallax_scroll",
        personality = "modern-retro",
        target = "cross465",
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

---

### C) External `.s` + 6502‑Assert

```rust
#[test]
fn external_assert_native() -> anyhow::Result<()> {
    let out = asm6502_test!(
        name = "irq::dma_micro",
        personality = "c64-compat",
        target = "ultimate64",
        asm_path = "native/src/tests/irq_dma_micro.s",
        timeout_ms = 5000,
        seed = 0xDEADBEEF
    );
    assert_case_ok(&out, "irq::dma_micro")?;
    Ok(())
}
```

---

### D) External `.s` + Host‑Expect

```rust
#[test]
fn external_host_expect() -> anyhow::Result<()> {
    let out = asm6502_test!(
        name = "display::tilemap_hash",
        personality = "modern-retro",
        target = "mega65",
        asm_path = "native/src/tests/display_tilemap_hash.s",
        timeout_ms = 8000,
        seed = 0xDEADBEEF
    );
    expect_hash_eq(&out, "tilemap_bg0", golden!("modern/display_tilemap_hash.fbhash"))?;
    Ok(())
}
```

---

## 🧩 Best Practices

- Always use `seed = 0xDEADBEEF` in examples for deterministic runs.
- Prefer hashes or short slices for visual comparisons (`LOG_HASH`).
- Disable IRQs for timing‑sensitive tests.
- Cross‑target tests can compare `actuals` maps from different `out` structs.
- For CI, prefer `--format json` and store output under `tests/artifacts/`.

---

## 🔍 Troubleshooting

| Symptom | Likely Cause | Fix |
|----------|--------------|-----|
| No `RTST` MAGIC | Wrong entrypoint | Jump to start of test |
| No progress | Infinite loop before END | Add debug `LOG_MSG` |
| Partial record | Payload > buffer | Split into smaller logs |
| Target stalls | I/O failure | Increase timeout or retry |

---

## 📚 References

- **Architecture:** `08_Cross465_Runtime_SDK_Testing_Architecture.md`
- **Execution Plan:** `S05_Testing_Expansion_Plan_for_Codex.md`
- **Runner Reference:** `README_TestRunner.md`
