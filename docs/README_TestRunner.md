# Cross465 Test Runner — CLI Reference

This reference describes the command-line interface and ecosystem around the Cross465 test system.

---

## Optional CLI Wrapper — `cross465-test-runner`

The optional **`cross465-test-runner`** CLI exposes the same testing flow as the
`run_asm6502_case()` function used by `asm6502_test!`. It is intended for **CI**, **hardware smoketesting**, and **scripting outside Rust**.

It is **not required** when using `cargo test`.

### Usage Examples

```bash
# List all tests
cross465-test-runner --mode list --target cross465 --personality modern-retro

# Run a specific case and emit JSON
cross465-test-runner --mode run --case display::parallax_scroll \
                     --target cross465 --personality modern-retro --format json
```

### Flags (shared with `run_asm6502_case()` and `asm6502_test!`)

| Flag | Purpose |
|------|----------|
| `--target` | Select backend (`cross465`, `ultimate64`, `mega65`) |
| `--personality` | Select MMIO layout (`modern-retro`, `c64-compat`) |
| `--mode` | Discovery or execution (`list`, `run`) |
| `--case` | Filter specific case name |
| `--timeout` | Override timeout in ms |
| `--seed` | Deterministic RNG seed |
| `--format` | Output format (`text`, `json`) |
| `--update` | Accept new golden baselines (Host-Expect mode) |

### When to Use
- Headless or multi-target CI.
- Rapid hardware validation (`mega65`, `ultimate64`).
- Non-Rust scripting environments.
- Comparing JSON outputs across personalities.

### Internal Flow

The CLI simply forwards all parameters to `run_asm6502_case()`, which handles
assembly, upload, execution, and RTST parsing.

---

## Terminology Quick Reference

| Term | Definition |
|------|-------------|
| **RTST** | Runtime Test Stream — protocol for 6502→host communication |
| **6502-Assert** | 6502 decides pass/fail using `TEST_CASE_OK` / `TEST_CASE_FAIL` |
| **Host-Expect** | Host compares actuals (`LOG_*`) to expectations or golden fixtures |
| **`asm6502_test!`** | Macro wrapping 6502 test execution |
| **`run_asm6502_case()`** | Function performing assembly, execution, and parsing |
| **`TestRun`** | Struct returned by macro, containing case results and actuals |
| **Personality** | MMIO layout (`modern-retro`, `c64-compat`, etc.) |
| **Target** | Backend (`cross465`, `ultimate64`, `mega65`) |
