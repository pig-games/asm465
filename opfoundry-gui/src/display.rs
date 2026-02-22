use std::sync::Arc;

use crate::video_backend::VideoOverlaySignals;
use bevy::ecs::system::NonSend;
use bevy::prelude::*;
use bevy::render::camera::ScalingMode;
use bevy::render::mesh::shape::Quad;
use bevy::render::mesh::Mesh;
use bevy::sprite::{ColorMaterial, MaterialMesh2dBundle, Mesh2dHandle};
use bevy::window::PrimaryWindow;
use bus::display_mmio::DisplaySnapshot;
use bus::interrupts::InterruptController;
use bus::sprite_mmio::{SpriteState, SPRITE_SLOTS};
use bus::{RasterIrqState, RASTER_IRQ_MASK};

use crate::console_ui::mmio_color;
use crate::{DisplaySettings, EmulatorState, VirtualResolution};

pub(crate) const SPRITE_TEXTURE_WIDTH: f32 = 96.0;
pub(crate) const SPRITE_TEXTURE_HEIGHT: f32 = 128.0;
pub(crate) const SPRITE_VIRTUAL_WIDTH: f32 = 40.0;
pub(crate) const SPRITE_VIRTUAL_HEIGHT: f32 =
    SPRITE_VIRTUAL_WIDTH * (SPRITE_TEXTURE_HEIGHT / SPRITE_TEXTURE_WIDTH);
pub(crate) const SPRITE_DEFAULT_MARGIN_X: f32 = SPRITE_VIRTUAL_WIDTH;
pub(crate) const SPRITE_DEFAULT_MARGIN_Y: f32 = SPRITE_VIRTUAL_HEIGHT;
pub(crate) const SPRITE_TEXTURE_PATHS: &[&str] = &[
    "sprites/knight.png",
    "sprites/knight_crimson.png",
    "sprites/knight_glacial.png",
];

#[derive(Resource, Clone)]
pub(crate) struct DisplayPalette {
    pub(crate) border: Color,
    pub(crate) background: Color,
}

impl DisplayPalette {
    pub(crate) fn from_settings(settings: &DisplaySettings) -> Self {
        Self {
            border: settings.border_color,
            background: settings.background_color,
        }
    }

    pub(crate) fn apply_snapshot(
        &mut self,
        snapshot: &DisplaySnapshot,
        defaults: &DisplaySettings,
    ) {
        self.border = mmio_color(snapshot.border_color, defaults.border_color);
        self.background = mmio_color(snapshot.background_color, defaults.background_color);
    }
}

#[derive(Resource, Clone, Copy)]
pub(crate) struct VideoOverlayConfig {
    enabled: bool,
}

impl VideoOverlayConfig {
    pub(crate) fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    pub(crate) fn enabled(self) -> bool {
        self.enabled
    }
}

#[derive(Resource)]
pub(crate) struct RasterDriver {
    state: Arc<RasterIrqState>,
    overlay: Option<Arc<VideoOverlaySignals>>,
    controller: Arc<InterruptController>,
    phase: f32,
}

impl RasterDriver {
    pub(crate) fn new(
        state: Arc<RasterIrqState>,
        overlay: Option<Arc<VideoOverlaySignals>>,
        controller: Arc<InterruptController>,
    ) -> Self {
        Self {
            state,
            overlay,
            controller,
            phase: 0.0,
        }
    }

    pub(crate) fn advance(&mut self, delta: f32, total_lines: u16) {
        const RASTER_REFRESH_HZ: f32 = 60.0;
        let lines = total_lines.max(1);
        let lines_per_second = lines as f32 * RASTER_REFRESH_HZ;
        self.phase = (self.phase + delta * lines_per_second) % lines as f32;
        let line = self.phase.floor() as u16;
        self.state.set_current_low(line as u8);
        self.state.set_current_high((line >> 8) as u8);
        if let Some(overlay) = self.overlay.as_ref() {
            overlay.record_raster(line);
        }
        if self.state.compare() == line {
            self.controller.raise_irq(RASTER_IRQ_MASK);
        }
    }
}

pub(crate) fn drive_raster_counter(
    time: Res<Time>,
    virtual_resolution: Res<SpriteVirtualResolution>,
    mut driver: ResMut<RasterDriver>,
) {
    let lines = virtual_resolution.height().round().clamp(1.0, 1024.0) as u16;
    driver.advance(time.delta_seconds(), lines);
}

#[derive(Component)]
pub(crate) struct SpriteSlot {
    pub(crate) index: usize,
}

#[derive(Component)]
pub(crate) struct ContentBackground;

#[derive(Component)]
pub(crate) struct RasterLine;

#[derive(Resource)]
pub(crate) struct SpriteCatalog {
    pub(crate) handles: Vec<Handle<Image>>,
}

#[derive(Component)]
pub(crate) struct BorderOverlay {
    side: BorderSide,
}

#[derive(Clone, Copy)]
enum BorderSide {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Resource, Clone, Copy)]
pub(crate) struct SpriteVirtualResolution {
    width: f32,
    height: f32,
}

impl SpriteVirtualResolution {
    pub(crate) fn new(resolution: VirtualResolution) -> Self {
        Self {
            width: resolution.width.max(1) as f32,
            height: resolution.height.max(1) as f32,
        }
    }

    pub(crate) fn width(&self) -> f32 {
        self.width
    }

    pub(crate) fn height(&self) -> f32 {
        self.height
    }
}

#[derive(Resource, Clone, Copy)]
pub(crate) struct SpriteViewport {
    width: f32,
    height: f32,
    scale_x: f32,
    scale_y: f32,
    border_x: f32,
    border_y: f32,
    content_width: f32,
    content_height: f32,
}

impl SpriteViewport {
    pub(crate) fn new(window_width: f32, window_height: f32) -> Self {
        Self {
            width: window_width,
            height: window_height,
            scale_x: 1.0,
            scale_y: 1.0,
            border_x: 0.0,
            border_y: 0.0,
            content_width: window_width,
            content_height: window_height,
        }
    }

    fn update_window(&mut self, width: f32, height: f32) {
        self.width = width;
        self.height = height;
    }

    pub(crate) fn window_width(&self) -> f32 {
        self.width.max(1.0)
    }

    pub(crate) fn window_height(&self) -> f32 {
        self.height.max(1.0)
    }

    pub(crate) fn set_content(&mut self, scale_x: f32, scale_y: f32, border_x: f32, border_y: f32) {
        self.scale_x = scale_x;
        self.scale_y = scale_y;
        self.border_x = border_x;
        self.border_y = border_y;
        self.content_width = (self.window_width() - 2.0 * border_x).max(0.0);
        self.content_height = (self.window_height() - 2.0 * border_y).max(0.0);
    }

    pub(crate) fn scale_x(&self) -> f32 {
        self.scale_x
    }

    pub(crate) fn scale_y(&self) -> f32 {
        self.scale_y
    }

    pub(crate) fn border_x(&self) -> f32 {
        self.border_x
    }

    pub(crate) fn border_y(&self) -> f32 {
        self.border_y
    }

    pub(crate) fn content_width(&self) -> f32 {
        self.content_width.max(0.0)
    }

    pub(crate) fn content_height(&self) -> f32 {
        self.content_height.max(0.0)
    }
}

pub(crate) fn setup_scene(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    palette: Res<DisplayPalette>,
    overlay_config: Res<VideoOverlayConfig>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    window_query: Query<&Window, With<PrimaryWindow>>,
) {
    let mut camera = Camera2dBundle::default();
    camera.projection.scaling_mode = ScalingMode::WindowSize(1.0);
    commands.spawn(camera);

    let window = window_query
        .get_single()
        .expect("primary window not available during setup");
    commands.insert_resource(SpriteViewport::new(window.width(), window.height()));

    let mesh = meshes.add(Mesh::from(Quad::default()));
    let material = materials.add(ColorMaterial::from(palette.background));
    let border_material = materials.add(ColorMaterial::from(palette.border));
    let mut background_transform = Transform::from_xyz(0.0, 0.0, -0.5);
    background_transform.scale = Vec3::new(window.width(), window.height(), 1.0);
    commands.spawn((
        MaterialMesh2dBundle {
            mesh: Mesh2dHandle(mesh),
            material,
            transform: background_transform,
            visibility: Visibility::Visible,
            ..Default::default()
        },
        ContentBackground,
    ));

    let handles: Vec<Handle<Image>> = SPRITE_TEXTURE_PATHS
        .iter()
        .map(|path| asset_server.load(*path))
        .collect();
    let default_texture = handles
        .first()
        .cloned()
        .expect("sprite texture list must not be empty");

    commands.insert_resource(SpriteCatalog {
        handles: handles.clone(),
    });

    for index in 0..SPRITE_SLOTS {
        commands.spawn((
            SpriteBundle {
                texture: default_texture.clone(),
                sprite: Sprite {
                    color: Color::WHITE,
                    custom_size: None,
                    ..Default::default()
                },
                transform: Transform::from_xyz(0.0, 0.0, 1.0 + index as f32 * 0.01),
                visibility: Visibility::Hidden,
                ..Default::default()
            },
            SpriteSlot { index },
        ));
    }

    let overlay_z = 5.0;
    let overlay_mesh = Mesh2dHandle(meshes.add(Mesh::from(Quad::default())));
    for side in [
        BorderSide::Left,
        BorderSide::Right,
        BorderSide::Top,
        BorderSide::Bottom,
    ] {
        commands.spawn((
            MaterialMesh2dBundle {
                mesh: overlay_mesh.clone(),
                material: border_material.clone(),
                transform: Transform::from_xyz(0.0, 0.0, overlay_z),
                visibility: Visibility::Visible,
                ..Default::default()
            },
            BorderOverlay { side },
        ));
    }

    if overlay_config.enabled() {
        let raster_material = materials.add(ColorMaterial::from(Color::rgba(1.0, 0.0, 0.0, 0.6)));
        commands.spawn((
            MaterialMesh2dBundle {
                mesh: overlay_mesh.clone(),
                material: raster_material,
                transform: Transform::from_xyz(0.0, 0.0, 4.5),
                visibility: Visibility::Hidden,
                ..Default::default()
            },
            RasterLine,
        ));
    }
}

pub(crate) fn sprite_virtual_size() -> Vec2 {
    Vec2::new(SPRITE_VIRTUAL_WIDTH, SPRITE_VIRTUAL_HEIGHT)
}

fn sprite_mmio_position(sprite: &SpriteState) -> Vec2 {
    let factor_x = 2f32.powi((sprite.scale_x & 0x0F) as i32).max(1.0);
    let factor_y = 2f32.powi((sprite.scale_y & 0x0F) as i32).max(1.0);
    Vec2::new(sprite.x as f32 / factor_x, sprite.y as f32 / factor_y)
}

pub(crate) fn sprite_world_transform(
    sprite: &SpriteState,
    viewport: &SpriteViewport,
    virtual_resolution: &SpriteVirtualResolution,
    display: &DisplaySettings,
) -> Option<(Vec2, Vec2)> {
    let window_width = viewport.window_width();
    let window_height = viewport.window_height();
    let half_width = window_width * 0.5;
    let half_height = window_height * 0.5;

    let virtual_width = virtual_resolution.width().max(1.0);
    let virtual_height = virtual_resolution.height().max(1.0);
    let margin_left = display.sprite_margin_left.max(0.0);
    let margin_right = display.sprite_margin_right.max(0.0);
    let margin_top = display.sprite_margin_top.max(0.0);
    let margin_bottom = display.sprite_margin_bottom.max(0.0);

    let map_width = virtual_width + margin_left + margin_right;
    let map_height = virtual_height + margin_top + margin_bottom;
    let mmio_max_x = if display.sprite_mmio_max_x > 0.0 {
        display.sprite_mmio_max_x
    } else {
        map_width.max(1.0)
    };
    let mmio_max_y = if display.sprite_mmio_max_y > 0.0 {
        display.sprite_mmio_max_y
    } else {
        map_height.max(1.0)
    };

    let mmio_position = sprite_mmio_position(sprite);
    let clamped_x = mmio_position.x.clamp(0.0, mmio_max_x);
    let clamped_y = mmio_position.y.clamp(0.0, mmio_max_y);

    let normalized_x = (clamped_x / mmio_max_x).clamp(0.0, 1.0);
    let normalized_y = (clamped_y / mmio_max_y).clamp(0.0, 1.0);

    let virtual_x = normalized_x * map_width - margin_left;
    let virtual_y = normalized_y * map_height - margin_top;

    let sprite_virtual = sprite_virtual_size();
    let sprite_virtual_width = sprite_virtual.x;
    let sprite_virtual_height = sprite_virtual.y;

    let max_offscreen_width = display.sprite_max_offscreen_width.max(0.0);
    let max_offscreen_height = display.sprite_max_offscreen_height.max(0.0);

    let left_limit = -max_offscreen_width;
    let right_limit = virtual_width + max_offscreen_width;
    let top_limit = -max_offscreen_height;
    let bottom_limit = virtual_height + max_offscreen_height;

    let sprite_left = virtual_x;
    let sprite_right = virtual_x + sprite_virtual_width;
    let sprite_top = virtual_y;
    let sprite_bottom = virtual_y + sprite_virtual_height;

    if sprite_right < left_limit
        || sprite_left > right_limit
        || sprite_bottom < top_limit
        || sprite_top > bottom_limit
    {
        return None;
    }

    let scale_x = viewport.scale_x();
    let scale_y = viewport.scale_y();

    let sprite_world_width = sprite_virtual_width * scale_x;
    let sprite_world_height = sprite_virtual_height * scale_y;
    let sprite_half_width = sprite_world_width * 0.5;
    let sprite_half_height = sprite_world_height * 0.5;

    let content_left = -half_width + viewport.border_x();
    let content_top = half_height - viewport.border_y();

    let host_left = content_left + virtual_x * scale_x;
    let host_top = content_top - virtual_y * scale_y;

    Some((
        Vec2::new(host_left + sprite_half_width, host_top - sprite_half_height),
        Vec2::new(sprite_world_width, sprite_world_height),
    ))
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn update_sprite_viewport(
    window_query: Query<&Window, With<PrimaryWindow>>,
    mut viewport: ResMut<SpriteViewport>,
    virtual_resolution: Res<SpriteVirtualResolution>,
    display: Res<DisplaySettings>,
    palette: Res<DisplayPalette>,
    mut clear_color: ResMut<ClearColor>,
    mut background: Query<
        (&Handle<ColorMaterial>, &mut Transform),
        (With<ContentBackground>, Without<BorderOverlay>),
    >,
    mut overlays: Query<
        (&BorderOverlay, &Handle<ColorMaterial>, &mut Transform),
        Without<ContentBackground>,
    >,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    if let Ok(window) = window_query.get_single() {
        viewport.update_window(window.width(), window.height());

        let (scale_x, scale_y, computed_border_x, computed_border_y) = compute_viewport_geometry(
            viewport.window_width(),
            viewport.window_height(),
            &virtual_resolution,
            &display,
        );

        viewport.set_content(scale_x, scale_y, computed_border_x, computed_border_y);

        clear_color.0 = palette.border;
        if let Ok((material_handle, mut transform)) = background.get_single_mut() {
            transform.scale = Vec3::new(
                viewport.content_width().max(1.0),
                viewport.content_height().max(1.0),
                1.0,
            );
            if let Some(material) = materials.get_mut(material_handle) {
                material.color = palette.background;
            }
        }

        let half_width = viewport.window_width() * 0.5;
        let half_height = viewport.window_height() * 0.5;
        let border_x = viewport.border_x().max(0.0);
        let border_y = viewport.border_y().max(0.0);

        for (overlay, material_handle, mut transform) in overlays.iter_mut() {
            if let Some(material) = materials.get_mut(material_handle) {
                material.color = palette.border;
            }

            match overlay.side {
                BorderSide::Left => {
                    transform.translation.x = -half_width + border_x * 0.5;
                    transform.translation.y = 0.0;
                    transform.scale = Vec3::new(border_x.max(0.0), viewport.window_height(), 1.0);
                }
                BorderSide::Right => {
                    transform.translation.x = half_width - border_x * 0.5;
                    transform.translation.y = 0.0;
                    transform.scale = Vec3::new(border_x.max(0.0), viewport.window_height(), 1.0);
                }
                BorderSide::Top => {
                    transform.translation.x = 0.0;
                    transform.translation.y = half_height - border_y * 0.5;
                    transform.scale = Vec3::new(viewport.window_width(), border_y.max(0.0), 1.0);
                }
                BorderSide::Bottom => {
                    transform.translation.x = 0.0;
                    transform.translation.y = -half_height + border_y * 0.5;
                    transform.scale = Vec3::new(viewport.window_width(), border_y.max(0.0), 1.0);
                }
            }
        }
    }
}

pub(crate) fn update_video_overlay_line(
    emulator: NonSend<EmulatorState>,
    viewport: Res<SpriteViewport>,
    virtual_resolution: Res<SpriteVirtualResolution>,
    overlay_config: Res<VideoOverlayConfig>,
    mut query: Query<(&mut Transform, &mut Visibility), With<RasterLine>>,
) {
    let Ok((mut transform, mut visibility)) = query.get_single_mut() else {
        return;
    };

    if !overlay_config.enabled() {
        *visibility = Visibility::Hidden;
        return;
    }

    let snapshot = emulator.video_overlay().snapshot();
    let content_height = viewport.content_height();
    let content_width = viewport.content_width();
    if content_height <= 0.0 || content_width <= 0.0 {
        *visibility = Visibility::Hidden;
        return;
    }

    let virtual_height = virtual_resolution.height().max(1.0);
    let raster = snapshot.raster as f32;
    let max_raster = virtual_height.max(1.0);
    let y_virtual = (raster / max_raster).clamp(0.0, 1.0) * virtual_height;
    let scale_y = viewport.scale_y();
    let content_top = viewport.window_height() * 0.5 - viewport.border_y();
    let host_y = content_top - y_virtual * scale_y;

    transform.translation.x = 0.0;
    transform.translation.y = host_y;
    transform.translation.z = 4.5;
    transform.scale.x = content_width.max(1.0);
    transform.scale.y = 2.0;
    *visibility = Visibility::Visible;
}

pub(crate) fn compute_viewport_geometry(
    window_width: f32,
    window_height: f32,
    virtual_resolution: &SpriteVirtualResolution,
    display: &DisplaySettings,
) -> (f32, f32, f32, f32) {
    let min_border_x = display.min_border_x.max(0.0);
    let min_border_y = display.min_border_y.max(0.0);

    if display.enforce_aspect_ratio {
        let inner_width = (window_width - 2.0 * min_border_x).max(1.0);
        let inner_height = (window_height - 2.0 * min_border_y).max(1.0);
        let uniform_scale = (inner_width / virtual_resolution.width())
            .min(inner_height / virtual_resolution.height());
        let mut content_width = virtual_resolution.width() * uniform_scale;
        let mut content_height = virtual_resolution.height() * uniform_scale;
        let mut border_x = (window_width - content_width) * 0.5;
        let mut border_y = (window_height - content_height) * 0.5;

        if border_x < min_border_x || border_y < min_border_y {
            border_x = min_border_x;
            border_y = min_border_y;
            let adjusted_width = (window_width - 2.0 * border_x).max(1.0);
            let adjusted_height = (window_height - 2.0 * border_y).max(1.0);
            let uniform_scale = (adjusted_width / virtual_resolution.width())
                .min(adjusted_height / virtual_resolution.height());
            content_width = virtual_resolution.width() * uniform_scale;
            content_height = virtual_resolution.height() * uniform_scale;
            border_x = (window_width - content_width) * 0.5;
            border_y = (window_height - content_height) * 0.5;
        }

        (
            (window_width - 2.0 * border_x).max(1.0) / virtual_resolution.width(),
            (window_height - 2.0 * border_y).max(1.0) / virtual_resolution.height(),
            border_x,
            border_y,
        )
    } else {
        (
            window_width / virtual_resolution.width(),
            window_height / virtual_resolution.height(),
            0.0,
            0.0,
        )
    }
}
