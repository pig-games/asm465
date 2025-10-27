# Built-in Personalities

Bundled TOML personalities for the cross465 Personalities v2 runtime.

## modern-retro-range
- Mirrors the legacy `MODERN_RETRO` contiguous mapping (`$DF00..=DF46`).
- Modules: `console.text`, `display.basic2d`, `sprite.basic`, `system.interrupts`.
- Demonstrates range-based maps with priority 10.

```toml
[personality]
id = "modern-retro-range"
title = "Modern Retro (Range)"
```

Inspect via CLI:

```sh
cargo run -p asm465 -- --dump-maps modern-retro-range
```

## c64-compat-sparse
- Sparse layout inspired by the Commodore 64: display registers at `$D020/$D021`, per-sprite registers in the `$D100` range.
- Demonstrates value builders for active-low joystick inputs (`DC00`).
- Uses `pre_write_sets`/`pre_read_sets` to select the appropriate sprite slot before each register access.

```toml
[personality]
id = "c64-compat-sparse"
title = "C64-Compatible Sparse Layout"
```

Example usage:

```sh
cargo run -p asm465 -- --dump-maps c64-compat-sparse
```
