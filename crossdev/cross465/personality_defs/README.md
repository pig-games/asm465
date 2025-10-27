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

Load via CLI (once integrated):

```sh
cross465-runner --personality modern-retro-range
```

## c64-compat-sparse
- Sparse layout inspired by the Commodore 64: display and sprite registers at `$D020..$D047`.
- Demonstrates value builders for active-low joystick inputs (`DC00`).

```toml
[personality]
id = "c64-compat-sparse"
title = "C64-Compatible Sparse Layout"
```

Example usage:

```sh
cross465-runner --personality c64-compat-sparse
```
