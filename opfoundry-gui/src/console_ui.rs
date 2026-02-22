use bevy::prelude::Color;
use bevy_egui::egui;
use bus::console_mmio::ConsoleSnapshot;

const CONSOLE_FONT_SIZE: f32 = 16.0;

/// Shared C64 palette (index 0x00-0x0F) as `(R, G, B)` tuples.
const C64_PALETTE: [(u8, u8, u8); 16] = [
    (0x00, 0x00, 0x00), // 0x00 Black
    (0xFF, 0xFF, 0xFF), // 0x01 White
    (0x88, 0x00, 0x00), // 0x02 Red
    (0xAA, 0xFF, 0xEE), // 0x03 Cyan
    (0xCC, 0x44, 0xCC), // 0x04 Magenta
    (0x00, 0xCC, 0x55), // 0x05 Green
    (0x00, 0x00, 0xAA), // 0x06 Blue
    (0xEE, 0xEE, 0x77), // 0x07 Yellow
    (0xDD, 0x88, 0x55), // 0x08 Orange
    (0x66, 0x44, 0x00), // 0x09 Brown
    (0xFF, 0x77, 0x77), // 0x0A Light red
    (0xAA, 0xFF, 0xEE), // 0x0B Light cyan
    (0xFF, 0xAA, 0xFF), // 0x0C Light magenta
    (0xAA, 0xFF, 0xAA), // 0x0D Light green
    (0xAA, 0xCC, 0xFF), // 0x0E Light blue
    (0xCC, 0xCC, 0xCC), // 0x0F Light gray
];

pub(crate) fn mmio_color(value: u8, fallback: Color) -> Color {
    let idx = (value & 0x0F) as usize;
    if idx < C64_PALETTE.len() {
        let (r, g, b) = C64_PALETTE[idx];
        Color::rgb_u8(r, g, b)
    } else {
        fallback
    }
}

pub(crate) fn console_layout_job(snapshot: &ConsoleSnapshot) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    let mut buffer = [0u8; 4];
    let base_font = egui::FontId::monospace(CONSOLE_FONT_SIZE);
    let formats: [egui::text::TextFormat; 16] = std::array::from_fn(|idx| egui::text::TextFormat {
        font_id: base_font.clone(),
        color: palette_color(idx as u8),
        ..Default::default()
    });

    for y in 0..snapshot.height {
        for x in 0..snapshot.width {
            let cell = snapshot.cell(x, y);
            let glyph = cell.ch.encode_utf8(&mut buffer);
            let format = formats[(cell.fg & 0x0F) as usize].clone();
            job.append(glyph, 0.0, format);
        }
        if y + 1 < snapshot.height {
            job.append("\n", 0.0, formats[7].clone());
        }
    }

    job
}

fn palette_color(index: u8) -> egui::Color32 {
    let (r, g, b) = C64_PALETTE[(index & 0x0F) as usize];
    egui::Color32::from_rgb(r, g, b)
}
