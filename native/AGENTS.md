# Repository Guidelines

## Project Structure & Module Organization
Core 64tass sources live in `src/`. Platform-specific boot, layout, and init code stay under `src/platform/<target>/`. Shared macros and constants sit in `src/include/`, while integration routines land in `src/util.s` and `src/screen.s`. Tests reside in `src/tests/`, grouped by feature with optional platform subfolders (`src/tests/mega65/`). Build artifacts land in `build/`; treat it as disposable. Helper scripts, including port waiters and PRG senders, live in `tools/`.

## Build, Test, and Development Commands
Run `make parsertest` or `make screentest` to assemble focused test programs into `build/<target>/`. Use `make unittest` for the full regression suite. Add `TARGET=ultimate64` (or `cross465`) to any `make` call to switch hardware. `make run_utest` builds and launches via the configured runner, while `make ci` iterates the matrix used in automation. Clean intermediates with `make clean`.

## Coding Style & Naming Conventions
Write in 64tass syntax with four-space indentation inside blocks. Labels and `.proc` names use lowerCamelCase (`printC`, `asmInit`), while constants and macros stay UpperCamel or ALL_CAPS (`PrtColour`, `.PutC`). Keep comments short with `;` and place them on the line they explain. Shared definitions belong in `.include` headers; avoid duplicating immediate values inline.

## Testing Guidelines
Place new fixtures beside related code in `src/tests/`, naming files `<feature>test.s`. Prefer assembling through the Makefile targets so debug defines stay consistent (`-D DEBUG_:=true`). Before submitting changes, run `make unittest` and any target-specific variant you touched (e.g., `TARGET=cross465 make parsertest`). For manual verification on devices, use the `run_` wrappers so uploads follow the expected workflow.

## Commit & Pull Request Guidelines
Follow the existing history by writing concise, sentence-case commit subjects (`Fixed the Load PRG functionality on Safari.`). Squash noisy commits before review. Each PR should outline the intent, note the 64tass targets affected, and list the `make` commands you ran. Attach screenshots or serial logs when touching UI-facing or hardware flows, and link issues or tickets where applicable.
