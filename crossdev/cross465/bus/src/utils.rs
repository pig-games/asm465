//! Shared helpers/utilities for the bus crate.
//!
//! Right now this module provides a minimal PETSCII‑ish mapping that’s “good
//! enough” for printing readable characters to a modern terminal. You can
//! extend this with full PETSCII tables and case/graphics modes later.

/// Minimal PETSCII-ish mapping to Unicode.
///
/// * `$0D` (CR) is mapped to newline for convenience.
/// * Printable ASCII ranges are passed through.
/// * Everything else becomes a middle‑dot placeholder.
///
/// This is intentionally conservative; it keeps unit tests and debug output
/// legible across platforms without dragging in large mapping tables.
pub fn petscii_to_unicode(b: u8) -> char {
    match b {
        0x0D => '\n',
        0x20..=0x5A | 0x61..=0x7A => b as char,
        _ => '·',
    }
}