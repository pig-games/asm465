//! Shared helpers/utilities for the bus crate.
//!
//! Right now this module provides a minimal PETSCII‑ish mapping that’s “good
//! enough” for printing readable characters to a modern terminal. You can
//! extend this with full PETSCII tables and case/graphics modes later.

use console::Color;

/// Translate a Unicode scalar into the corresponding C64 screen code.
///
/// The mapping mirrors [`screen_to_petscii`]:
///
/// - Printable ASCII characters in the range space (`' '`) through question
///   mark (`'?'`) map directly to their byte value.
/// - Uppercase ASCII letters use their byte value so the PETSCII
///   translation emits the same glyph.
/// - Lowercase ASCII letters are offset into the `$A0–$BF` band so the
///   round-trip through [`screen_to_petscii`] and [`petscii_to_unicode`]
///   preserves their case.
/// - A handful of legacy glyphs (`@`, `[`, `]`, `£`, `↑`, `←`) land on their
///   historical screen codes so host text mirrors terminal output.
/// - Unsupported characters fall back to a plain space to keep the console
///   legible.
#[must_use]
pub fn unicode_to_screen(ch: char) -> u8 {
    match ch {
        '\n' | '\r' => b'\n',
        ' '..='?' => ch as u8,
        '@' => 0x00,
        'A'..='Z' => ch as u8,
        '[' => 0x1B,
        ']' => 0x1D,
        'a'..='z' => (ch as u8) + 0x40,
        '£' => 0x1C,
        '↑' => 0x1E,
        '←' => 0x1F,
        _ => 0x20,
    }
}
/// Minimal PETSCII-ish mapping to Unicode.
///
/// * `$0D` (CR) is mapped to newline for convenience.
/// * Printable ASCII ranges are passed through.
/// * Everything else becomes a middle‑dot placeholder.
///
/// This is intentionally conservative; it keeps unit tests and debug output
/// legible across platforms without dragging in large mapping tables.
#[must_use]
pub fn petscii_to_unicode(b: u8) -> char {
    match b {
        0x0D => '\n',
        0x00..=0x40 | 0x5b | 0x5d | 0x61..=0x7A => b as char,
        0x41..=0x5A => (b + 0x20) as char,
        0xc1..=0xda => (b - 0x80) as char,
        0x5c => 0xa3 as char, // £,
        0x5e => '\u{2191}',   // ↑
        0x5f => '\u{2190}',   // ←
        _ => '=',
    }
}

/// Translate a screen byte (viewer encoding) into PETSCII.
#[must_use]
pub fn screen_to_petscii(b: u8) -> u8 {
    match b {
        b'\n' => 0x0D,
        0x00..=0x1F | 0x60..=0x7f => b + 64,
        0x20..=0x3F | 0xe0..=0xfe => b,
        0x40..=0x5F => b + 128,
        0x80..=0x9F => b - 128,
        0xa0..=0xdf => b - 64,
        _ => b, // '.'
    }
}

/// Convert a C64 colour index into an ANSI colour for terminal output.
#[must_use]
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
        8 => Color::Color256(3),   // Orange
        9 => Color::Color256(88),  // Brown
        10 => Color::Color256(9),  // Light red
        11 => Color::Color256(14), // Light cyan
        12 => Color::Color256(13), // Light magenta
        13 => Color::Color256(10), // Light green
        14 => Color::Color256(33), // Light blue
        15 => Color::Color256(7),  // Light gray
        _ => Color::White,         // unreachable
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unicode_screen_round_trip_basic_text() {
        let sample = "Welcome to the asm465 console viewer!";
        for ch in sample.chars() {
            let screen = unicode_to_screen(ch);
            let round_trip = petscii_to_unicode(screen_to_petscii(screen));
            assert_eq!(round_trip, ch, "character {ch:?} failed to round-trip");
        }
    }
}
