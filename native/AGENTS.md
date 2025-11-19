# Repository Guidelines

## Project Structure & Module Organization
Core 64tass sources live in `src/`. Platform-specific boot, layout, and init code stay under `src/platform/<target>/`. Shared macros and constants sit in `src/include/`, while integration routines land in `src/util.s` and `src/screen.s`. Tests reside in `src/tests/`, grouped by feature with optional platform subfolders (`src/tests/mega65/`). Build artifacts land in `build/`; treat it as disposable. Helper scripts, including port waiters and PRG senders, live in `tools/`.

## Build, Test, and Development Commands
Use `make test` to run the RTST catalog (`crossdev/cross465/tests/catalog.toml`) via `cross465-test-runner`. The default backend is the Cross465 emulator; override it with `RTST_TARGET=asm465`, `RTST_TARGET=asm465-wasm`, `RTST_TARGET=ultimate64`, or `RTST_TARGET=mega65` depending on the hardware you have connected. When you target asm465/asm465-wasm, the Makefile automatically rebuilds the native app, bridge, and wasm bundle and spins up the local Python HTTP server (`http://$(WASM_HTTP_HOST):$(WASM_HTTP_PORT)/`). Narrow the run with `CASE="math::add_basic display::parallax_scroll"` (space-separated) or override the MMIO layout with `PERSONALITY=c64-compat`. Pass extra CLI flags straight through to the runner with `RUNNER_ARGS="--artifacts"`. Use `make fixtures` (optionally combining `RUNNER_INCLUDE_TARGETS`/`RUNNER_EXCLUDE_TARGETS` and `FIXTURES=<dir>`) to compare every catalog target against its goldens, or `make update-fixtures` to rebuild them. `make test-ci` (and the alias `make ci`) executes the entire CI matrix from the catalog so you can mirror automation locally; skip flaky targets with `RUNNER_EXCLUDE_TARGETS="asm465-wasm"` or keep a focused list via `RUNNER_INCLUDE_TARGETS="cross465 asm465"`. Clean intermediates with `make clean`.

## Coding Style & Naming Conventions
Write in 64tass syntax with four-space indentation inside blocks. Keep comments short with `;` on the line they describe. Shared definitions belong in `.include` headers; avoid duplicating immediate values inline. Always prefer `.h` headers (e.g., `.include "test_rtst.h"`) and declare strings with `.null` instead of `.byte …,0`. When touching legacy files, fix both the include suffix and the string form in the same pass.

### Naming by construct
- **Constants / symbols:** ALL_CAPS snake case (`PRG_LOAD_ADDR`).
- **Labels & `.proc` names:** lowerCamelCase (`printC`, `asmInit`).
- **Named memory locations/variables:** UpperCamelCase (`PrtColour`).
- **Macros:** lowerCamelCase (`setBgColor`, `logMsg`). Macro invocations always start with `.` (e.g., `.ctest.OK 0, ERR_PTR`).
- **Local labels (inside `.proc`/`.macro`/`.function`):** lower_snake_case and defined without any trailing punctuation (write `wait_loop` on its own line, then the body on the next).

### Macro grammar
- Define macros as `name .macro param1, ...` so the label is explicit (no colon after the macro name).
- Inside the macro body, reference parameters with a leading `\`.
    ```
    setBgColor .macro col
        lda #\col
        sta vic2.SCREENCOL
    .endmacro
    ```
- Group related macro/function sets inside a `.namespace`/`.endnamespace` pair, and repeat the namespace name in a trailing comment on `.endnamespace` (e.g., `graphics .namespace ... .endnamespace ; graphics`).

## Testing Guidelines
Author RTST-enabled cases under `crossdev/cross465/tests/cases/` and update the catalog so `make test` picks them up. Fixtures still live next to the code they exercise (e.g., `src/tests/<feature>/fixtures/`). Run `make test RTST_TARGET=cross465` after touching host logic, and re-run with `RTST_TARGET=asm465`/`asm465-wasm`/`ultimate64` when you change platform-specific paths. Use `make fixtures RUNNER_INCLUDE_TARGETS="cross465 ultimate64"` (or `RUNNER_EXCLUDE_TARGETS=...`) to compare against goldens, `make update-fixtures` to rewrite them (prepend `FIXTURES=path/to/dir` if you need a custom location), and capture artifacts by appending `RUNNER_ARGS="--artifacts target/cross465-runner"` when investigating failures.

## Commit & Pull Request Guidelines
Follow the existing history by writing concise, sentence-case commit subjects (`Fixed the Load PRG functionality on Safari.`). Squash noisy commits before review. Each PR should outline the intent, note the 64tass targets affected, and list the `make` commands you ran. Attach screenshots or serial logs when touching UI-facing or hardware flows, and link issues or tickets where applicable.
