# Cross465 Runtime SDK Testing Architecture Implementation Plan

## 1. Shared RTST Protocol Support
- [x] Build the shared `rtst` module in a new `crossdev/cross465/runtime_sdk` crate capturing headers, state lifecycle, record IDs, base addresses, and termination semantics.
- [x] Publish matching 6502 include macros (e.g., `test_rtst.inc`) with `RTST_BEGIN`, `TEST_CASE_*`, `LOG_*`, and END loop helpers wired into the 64tass build setup.
- [x] Add host-side parsers/writers with unit tests covering record sequencing, bounds validation, and unknown ID handling.

## 2. Cargo-Compatible RTST Runner CLI
- [x] Add a `bin/cross465-test-runner` target that accepts mode, case, target, personality, format, timeout, and seed flags.
- [x] Integrate 64tass invocation with configurable include paths, inline snippet handling, and surfaced diagnostics.
- [x] Implement discovery (`list`) and execution (`run`) flows translating RTST streams into cargo-compatible output and optional JSON summaries with correct exit codes.

## 3. Target Backend Trait and Implementations
- [x] Define a `TargetBackend` trait encapsulating assemble/deploy/read/reset operations plus timeout behavior.
- [x] Implement the Cross465 backend leveraging existing emulator APIs for PRG loading and RTST polling.
- [x] Implement the Ultimate64 backend using the REST API (`run_prg` + `machine:readmem`) with configurable host/port and retry logic.
- [x] Implement the MEGA65 backend using the `m65` CLI for upload/execution/memory reads with configurable serial port, binary path, and retry logic.
- [x] Add integration tests or mocks verifying each backend can complete a sample RTST session.

## 4. Host-Expect Aggregation and Helpers
- [x] Extend RTST parsing to collect ACT_* payloads into a typed map covering KV, MEM, HASH, REGS, and TIME variants.
- [x] Provide helper APIs (`expect_eq`, `expect_in`, `expect_hash_eq`, diff utilities) returning rich diagnostics.
- [x] Implement fixture storage under `tests/fixtures/<target>/<suite>.*` with load/save (`--update`) support and hashing utilities.
- [x] Surface detailed mismatch diagnostics for cargo output and JSON traces.

## 5. 6502 Test Authoring Ergonomics
- [x] Implement an `asm6502_test!` macro (or procedural macro) that routes to a `run_asm6502_case` helper supporting inline snippets and file-based assembly.
- [x] Handle temporary files, pass runtime parameters (personality, target, timeout, seed), and parse RTST output into a `TestRun` struct.
- [x] Provide assertion helpers (`assert_case_ok`, `expect_*`) that layer atop the Host-Expect APIs, with documentation examples.

## 6. Robust Error, Timeout, and Artifact Handling
- [x] Detect protocol initialization failures (missing MAGIC/VERSION) and abort with captured RTST buffer artifacts.
- [x] Monitor WPOS progress with configurable timeouts and automatic retries for transient transport failures.
- [x] Validate record lengths, fail tests on malformed streams, and persist raw dumps/PRGs for debugging.
- [x] Reset targets between cases and emit optional metrics/logs suitable for CI dashboards.

## 7. CI Matrix and Codex Export
- [x] Define CI workflows spanning target (`cross465`, `ultimate64`, `mega65`) and personality (`modern-retro`, `c64-compat`) combinations.
- [x] Teach the runner to emit JSON summaries and persist PRG/RTST artifacts so local/manual runs capture the data CI would need.

## 8. Documentation and Onboarding
- [x] Update `docs/cross465_runtime_sdk` with authoring guides covering RTST macros, Host-Expect usage, fixture updates, and backend configuration.
- [x] Provide troubleshooting FAQs for assembler errors, backend connectivity issues, and expectation diffs.
- [x] Highlight isolation practices (IRQ masking, setup/teardown) and pointers to CI outputs for diagnostics.

## 9. Asm465 / WASM Runtime Targets
- [x] Treat the native `asm465` runtime (and its WASM build) as addressable backends for RTST, so tests that exercise graphics/audio via the emulator can run through the same cross465-test-runner pipeline.
- [ ] Replace the Makefile-based unit test harnesses with `cross465-test-runner` once asm465/WASM targets are stable.

## 10. Console MMIO Debug Integration
- [ ] Capture console MMIO debug streams (reads/writes, overlay screenshots) alongside RTST so `cross465-test-runner` can archive them per case.
- [ ] Provide host-side parsers/expectation helpers for the MMIO trace data (diffing overlays, asserting register access patterns).
- [ ] Surface new CLI flags/documentation so developers can enable MMIO capture locally and in CI, keeping artifact formats aligned with the rest of the cross465-test-runner outputs.
