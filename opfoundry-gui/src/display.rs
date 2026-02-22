mod sprite;
mod viewport;

pub(crate) use sprite::{
    setup_scene, sprite_world_transform, SpriteCatalog, SpriteSlot, SpriteVirtualResolution,
    SPRITE_DEFAULT_MARGIN_X, SPRITE_DEFAULT_MARGIN_Y, SPRITE_VIRTUAL_HEIGHT, SPRITE_VIRTUAL_WIDTH,
};
pub(crate) use viewport::{
    drive_raster_counter, update_sprite_viewport, update_video_overlay_line, DisplayPalette,
    RasterDriver, SpriteViewport, VideoOverlayConfig,
};

#[cfg(test)]
pub(crate) use sprite::sprite_virtual_size;
#[cfg(test)]
pub(crate) use viewport::compute_viewport_geometry;

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::{Color, Vec2, Vec4};
    use bus::display_mmio::DisplaySnapshot;
    use bus::sprite_mmio::SpriteState;

    use crate::{DisplaySettings, VirtualResolution};

    fn assert_color_eq(a: Color, b: Color) {
        let a = a.as_linear_rgba_f32();
        let b = b.as_linear_rgba_f32();
        for i in 0..4 {
            assert!(
                (a[i] - b[i]).abs() <= 1e-3,
                "component {i} differ: {a:?} vs {b:?}"
            );
        }
    }

    fn sprite(x: f32, y: f32) -> SpriteState {
        let max_value = u16::MAX as f32;
        let clamped_x = x.max(0.0).min(max_value);
        let clamped_y = y.max(0.0).min(max_value);
        SpriteState {
            number: 1,
            anim: 0,
            x: clamped_x.round() as u16,
            y: clamped_y.round() as u16,
            scale_x: 0,
            scale_y: 0,
            enabled: true,
        }
    }

    fn sprite_with_scale(x: f32, y: f32, shift_x: u8, shift_y: u8) -> SpriteState {
        let max_value = u16::MAX as f32;
        let clamped_x = x.max(0.0).min(max_value);
        let clamped_y = y.max(0.0).min(max_value);
        SpriteState {
            number: 1,
            anim: 0,
            x: clamped_x.round() as u16,
            y: clamped_y.round() as u16,
            scale_x: shift_x & 0x0F,
            scale_y: shift_y & 0x0F,
            enabled: true,
        }
    }

    fn virtual_res(width: u32, height: u32) -> SpriteVirtualResolution {
        SpriteVirtualResolution::new(VirtualResolution::new(width, height))
    }

    fn display_no_margins(mmio_max_x: f32, mmio_max_y: f32) -> DisplaySettings {
        DisplaySettings {
            enforce_aspect_ratio: false,
            min_border_x: 0.0,
            min_border_y: 0.0,
            border_color: Color::BLACK,
            background_color: Color::BLACK,
            resize_window_to_aspect: false,
            sprite_margin_left: 0.0,
            sprite_margin_right: 0.0,
            sprite_margin_top: 0.0,
            sprite_margin_bottom: 0.0,
            sprite_mmio_max_x: mmio_max_x,
            sprite_mmio_max_y: mmio_max_y,
            sprite_max_offscreen_width: SPRITE_VIRTUAL_WIDTH,
            sprite_max_offscreen_height: SPRITE_VIRTUAL_HEIGHT,
        }
    }

    fn display_with_margins(
        mmio_max_x: f32,
        mmio_max_y: f32,
        margin: Vec4,
        max_offscreen: Vec2,
    ) -> DisplaySettings {
        DisplaySettings {
            enforce_aspect_ratio: false,
            min_border_x: 0.0,
            min_border_y: 0.0,
            border_color: Color::BLACK,
            background_color: Color::BLACK,
            resize_window_to_aspect: false,
            sprite_margin_left: margin.x.max(0.0),
            sprite_margin_right: margin.y.max(0.0),
            sprite_margin_top: margin.z.max(0.0),
            sprite_margin_bottom: margin.w.max(0.0),
            sprite_mmio_max_x: mmio_max_x,
            sprite_mmio_max_y: mmio_max_y,
            sprite_max_offscreen_width: max_offscreen.x.max(0.0),
            sprite_max_offscreen_height: max_offscreen.y.max(0.0),
        }
    }

    fn approx_equal(a: f32, b: f32, eps: f32) {
        assert!(
            (a - b).abs() <= eps,
            "expected {b}, got {a} (|Δ| = {})",
            (a - b).abs()
        );
    }

    #[test]
    fn sprite_position_aligns_top_left_at_origin() {
        let sprite_virtual = virtual_res(128, 96);
        let mut viewport = SpriteViewport::new(800.0, 600.0);
        let scale_x = viewport.window_width() / sprite_virtual.width();
        let scale_y = viewport.window_height() / sprite_virtual.height();
        viewport.set_content(scale_x, scale_y, 0.0, 0.0);
        let sprite = sprite(0.0, 0.0);
        let display = display_no_margins(128.0, 96.0);

        let (world_pos, world_size) =
            sprite_world_transform(&sprite, &viewport, &sprite_virtual, &display)
                .expect("sprite should be visible");

        let expected_size_x = sprite_virtual_size().x * viewport.scale_x();
        let expected_size_y = sprite_virtual_size().y * viewport.scale_y();
        let expected_x = -viewport.window_width() * 0.5 + expected_size_x * 0.5;
        let expected_y = viewport.window_height() * 0.5 - expected_size_y * 0.5;

        approx_equal(world_pos.x, expected_x, 3.0);
        approx_equal(world_pos.y, expected_y, 3.0);
        approx_equal(world_size.x, expected_size_x, 1e-3);
        approx_equal(world_size.y, expected_size_y, 1e-3);
    }

    #[test]
    fn sprite_position_aligns_bottom_right_at_max() {
        let sprite_virtual = virtual_res(128, 96);
        let mut viewport = SpriteViewport::new(800.0, 600.0);
        let scale_x = viewport.window_width() / sprite_virtual.width();
        let scale_y = viewport.window_height() / sprite_virtual.height();
        viewport.set_content(scale_x, scale_y, 0.0, 0.0);
        let dims = sprite_virtual_size();
        let sprite = sprite(128.0 - dims.x, 96.0 - dims.y);
        let display = display_no_margins(128.0, 96.0);

        let (world_pos, world_size) =
            sprite_world_transform(&sprite, &viewport, &sprite_virtual, &display)
                .expect("sprite should be visible");

        let expected_size_x = sprite_virtual_size().x * viewport.scale_x();
        let expected_size_y = sprite_virtual_size().y * viewport.scale_y();
        let expected_x = viewport.window_width() * 0.5 - expected_size_x * 0.5;
        let expected_y = -viewport.window_height() * 0.5 + expected_size_y * 0.5;

        approx_equal(world_pos.x, expected_x, 3.0);
        approx_equal(world_pos.y, expected_y, 3.0);
        approx_equal(world_size.x, expected_size_x, 1e-3);
        approx_equal(world_size.y, expected_size_y, 1e-3);
    }

    #[test]
    fn sprite_position_centres_at_midpoint() {
        let sprite_virtual = virtual_res(128, 96);
        let mut viewport = SpriteViewport::new(800.0, 600.0);
        let scale_x = viewport.window_width() / sprite_virtual.width();
        let scale_y = viewport.window_height() / sprite_virtual.height();
        viewport.set_content(scale_x, scale_y, 0.0, 0.0);
        let dims = sprite_virtual_size();
        let sprite = sprite((128.0 - dims.x) * 0.5, (96.0 - dims.y) * 0.5);
        let display = display_no_margins(128.0, 96.0);

        let (world_pos, world_size) =
            sprite_world_transform(&sprite, &viewport, &sprite_virtual, &display)
                .expect("sprite should be visible");

        let expected_size_x = sprite_virtual_size().x * viewport.scale_x();
        let expected_size_y = sprite_virtual_size().y * viewport.scale_y();

        approx_equal(world_pos.x, 0.0, 3.0);
        approx_equal(world_pos.y, 0.0, 3.0);
        approx_equal(world_size.x, expected_size_x, 1e-3);
        approx_equal(world_size.y, expected_size_y, 1e-3);
    }

    #[test]
    fn sprite_respects_margins_and_culling() {
        let sprite_virtual = virtual_res(160, 120);
        let mut viewport = SpriteViewport::new(640.0, 480.0);
        let scale_x = viewport.window_width() / sprite_virtual.width();
        let scale_y = viewport.window_height() / sprite_virtual.height();
        viewport.set_content(scale_x, scale_y, 0.0, 0.0);

        let display = display_with_margins(
            65535.0,
            65535.0,
            Vec4::new(40.0, 40.0, 40.0, 40.0),
            Vec2::new(SPRITE_VIRTUAL_WIDTH, SPRITE_VIRTUAL_HEIGHT),
        );

        let on_screen =
            sprite_world_transform(&sprite(0.0, 0.0), &viewport, &sprite_virtual, &display);
        assert!(
            on_screen.is_some(),
            "expected sprite at origin to be visible"
        );

        let culled_display = display_with_margins(
            65535.0,
            65535.0,
            Vec4::new(80.0, 0.0, 80.0, 0.0),
            Vec2::ZERO,
        );
        let culled = sprite_world_transform(
            &sprite(0.0, 0.0),
            &viewport,
            &sprite_virtual,
            &culled_display,
        );
        assert!(
            culled.is_none(),
            "expected sprite outside margins to be culled"
        );
    }

    #[test]
    fn sprite_per_axis_scale_divides_mmio_values() {
        let sprite_virtual = virtual_res(128, 96);
        let mut viewport = SpriteViewport::new(800.0, 600.0);
        let scale_x = viewport.window_width() / sprite_virtual.width();
        let scale_y = viewport.window_height() / sprite_virtual.height();
        viewport.set_content(scale_x, scale_y, 0.0, 0.0);
        let display = display_no_margins(128.0, 96.0);

        let dims = sprite_virtual_size();
        let desired_x = (128.0 - dims.x) * 0.5;
        let desired_y = (96.0 - dims.y) * 0.5;
        let sprite = sprite_with_scale(desired_x * 2.0, desired_y * 2.0, 1, 1);
        let placement =
            sprite_world_transform(&sprite, &viewport, &sprite_virtual, &display).unwrap();

        approx_equal(placement.0.x, 0.0, 1.5);
        approx_equal(placement.0.y, 0.0, 1.5);
    }

    #[test]
    fn viewport_geometry_without_aspect_enforcement() {
        let settings = DisplaySettings {
            enforce_aspect_ratio: false,
            ..DisplaySettings::default()
        };
        let virtual_res = SpriteVirtualResolution::new(VirtualResolution::new(128, 96));
        let (scale_x, scale_y, border_x, border_y) =
            compute_viewport_geometry(800.0, 600.0, &virtual_res, &settings);

        approx_equal(scale_x, 800.0 / 128.0, 1e-6);
        approx_equal(scale_y, 600.0 / 96.0, 1e-6);
        approx_equal(border_x, 0.0, 1e-6);
        approx_equal(border_y, 0.0, 1e-6);
    }

    #[test]
    fn viewport_geometry_with_aspect_enforcement_and_min_border() {
        let settings = DisplaySettings {
            enforce_aspect_ratio: true,
            min_border_x: 10.0,
            min_border_y: 20.0,
            ..DisplaySettings::default()
        };
        let virtual_res = SpriteVirtualResolution::new(VirtualResolution::new(160, 120));
        let (scale_x, scale_y, border_x, border_y) =
            compute_viewport_geometry(800.0, 600.0, &virtual_res, &settings);

        approx_equal(scale_x, scale_y, 1e-6);
        assert!(border_x >= settings.min_border_x - 1e-6);
        assert!(border_y >= settings.min_border_y - 1e-6);
    }

    #[test]
    fn mmio_palette_translation_uses_c64_colours() {
        let defaults = DisplaySettings::default();
        let mut palette = DisplayPalette::from_settings(&defaults);
        let snapshot = DisplaySnapshot {
            border_color: 0x06,
            background_color: 0x0A,
        };
        palette.apply_snapshot(&snapshot, &defaults);
        assert_color_eq(palette.border, Color::rgb_u8(0x00, 0x00, 0xAA));
        assert_color_eq(palette.background, Color::rgb_u8(0xFF, 0x77, 0x77));
    }
}
