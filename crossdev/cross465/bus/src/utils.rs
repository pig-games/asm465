//! Shared helpers/utilities for the bus crate.
//!
//! Right now this module provides a minimal PETSCII‑ish mapping that’s “good
//! enough” for printing readable characters to a modern terminal. You can
//! extend this with full PETSCII tables and case/graphics modes later.

use console::Color;
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
        0x00..=0x40 => b as char,
        0x41..=0x5A => (b + 0x20) as char,
        0x61..=0x7A => (b) as char,
        0xc1..=0xdA => (b - 0x80) as char,
        _ => '·',
    }
}

pub fn screen_to_petscii(b: u8) -> u8 {
    match b {
        b'\n' => 0x0D,
        0x00..=0x1F | 0x60..=0x7f => b + 64,
        0x20..=0x3F | 0xe0..=0xfe => b,
        0x40..=0x5F => b + 128,
        0x80..=0x9F => b - 128,
        0xa0..=0xdF => b - 64,
        _ => b, // '.'
    }
}

pub fn cmb_color_to_ansi(c: u8) -> Color {
    match c & 0x0F {
        0 => Color::Black,
        1 => Color::White,
        2 => Color::Red,
        3 => Color::Cyan,
        4 => Color::Magenta,
        5 => Color::Green,
        6 => Color::Blue,
        7 => Color::Yellow,
        _ => Color::White, // unreachable
    }
}

