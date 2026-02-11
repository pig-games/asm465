# opForge Sprite Demo (C64 + Cross465)

This is a minimal opForge assembly project that animates a single sprite on both C64 and Cross465.

## Build

Requirements:
- `opForge` on your PATH

Commands:
```sh
# C64
opForge -i . -D C64 -l -x

# Cross465
opForge -i . -D CROSS465 -l -x
```

Or use the local Makefile:
```sh
make c64
make cross465
```

## Run

1. Load the generated hex output into your target.
2. The program auto-runs via a BASIC stub (`SYS 2064`).

## Notes

- Sprite data is placed at `$2000` for the C64 path.
- The Cross465 backend uses sprite ID 1 via the Cross465 MMIO sprite interface.
