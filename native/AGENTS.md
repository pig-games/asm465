# Repository Guidelines

## Project Structure & Module Organization
Core 64tass sources live in `src/`. Platform-specific boot, layout, and init code stay under `src/platform/<target>/`. Shared macros and constants sit in `src/include/`, while integration routines land in `src/util.s` and `src/screen.s`. Tests reside in `src/tests/`, grouped by feature with optional platform subfolders (`src/tests/mega65/`). Build artifacts land in `build/`; treat it as disposable. Helper scripts, including port waiters and PRG senders, live in `tools/`.

## Build, Test, and Development Commands
Run `make parsertest` or `make screentest` to assemble focused test programs into `build/<target>/`. Use `make unittest` for the full regression suite. Add `TARGET=ultimate64` (or `cross465`) to any `make` call to switch hardware. `make run_utest` builds and launches via the configured runner, while `make ci` iterates the matrix used in automation. Clean intermediates with `make clean`.

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
Place new fixtures beside related code in `src/tests/`, naming files `<feature>test.s`. Prefer assembling through the Makefile targets so debug defines stay consistent (`-D DEBUG_:=true`). Before submitting changes, run `make unittest` and any target-specific variant you touched (e.g., `TARGET=cross465 make parsertest`). For manual verification on devices, use the `run_` wrappers so uploads follow the expected workflow.

## Commit & Pull Request Guidelines
Follow the existing history by writing concise, sentence-case commit subjects (`Fixed the Load PRG functionality on Safari.`). Squash noisy commits before review. Each PR should outline the intent, note the 64tass targets affected, and list the `make` commands you ran. Attach screenshots or serial logs when touching UI-facing or hardware flows, and link issues or tickets where applicable.
