# Milestone 5 Planning — Reference Personalities & Tooling

## Current Status
- Legacy hard-coded personalities (`MODERN_RETRO`, `C64_COMPAT`) still live in `bus::personality`; no TOML-driven equivalents exist yet.
- `Bus::from_personality_def` can instantiate personalities from disk, but there is no loader/CLI wiring in the `runner` or `asm465` frontends.
- No curated library of v2 TOML personalities is checked in; value-builder examples exist only in tests.

## Deliverables
1. **Baseline Personality (`modern-retro-range`)**
   - Reproduce the legacy `MODERN_RETRO` contiguous mapping in TOML.
   - Include module options (display dimensions, etc.) mirrored from current defaults.
   - Add a regression harness ensuring `Bus::from_personality_def` with this file matches legacy behaviour (e.g., border/background writes, sprite slots, interrupts).

2. **Vintage Sparse Samples**
   - `c64-compat-sparse`: Active-low joystick inputs via value builders, read-to-ack IRQ register, basic banking example (`active_when`).
   - `atari-compat-sparse`: Demonstrate split trigger registers, float/numeric value builders once implemented.
   - Each personality lives under `crossdev/cross465/personality_defs/` (or similar) with accompanying README snippets.

3. **CLI & Runner Hooks**
   - Extend `crossdev/cross465/runner` (and the `asm465` frontend) with:
     - `--personality <id>` to select a TOML file at launch.
     - `--list-personalities` to enumerate bundled definitions (both legacy and v2).
     - `--list-modules` and optional map viewer stub (textual dump showing active maps/conditions).
   - Provide graceful fallback to legacy personalities when TOML loading fails.

4. **Documentation Updates**
   - Update `docs/personalities_v2_docs` with instructions for selecting personalities via CLI.
   - Include examples of the new TOML personalities and a troubleshooting section.

## Open Questions / Decisions
- Where should bundled TOML files live? (`crossdev/cross465/personality_defs` vs. top-level `resources/`?).
- How should third-party personalities be discovered (environment variable, CLI path, config file)?
- Do we migrate legacy personalities to TOML immediately or keep both paths until after Milestone 5?

## Next Actions
1. Finalise repository layout for TOML personalities and document loader expectations.
2. Author `modern-retro-range.toml` and verify parity with legacy mapping through automated tests. — ✅ created `personality_defs/modern-retro-range.toml`.
3. Implement CLI flags and loader integration, ensuring existing workflows keep working without extra arguments. — ✅ `runner` and `asm465` now accept `--personality` and `--list-personalities` (legacy IDs or TOML path).
4. Backfill sparse/vintage personalities leveraging value builders and condition overlays. — ✅ Added `c64-compat-sparse.toml` with active-low joystick builder (Atari still pending).
