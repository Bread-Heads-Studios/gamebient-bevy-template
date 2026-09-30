//! The bezel sprite and the black backing rectangle under the game. The
//! bezel is tinted by the brightness machine every frame and is never
//! moved, scaled or animated once placed.

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;

use crate::frame::FRAME_LAYER;
use crate::frame::art::FrameArt;
use crate::frame::brightness::BEZEL_IDLE;
use crate::frame::driver::{FrameGeometry, FrameRuntime};
use crate::frame::layout::{PxRect, backing_rect, to_world};

const BEZEL_Z: f32 = 0.0;
const BACKING_Z: f32 = 1.0;

#[derive(Component)]
pub struct BezelSprite;

/// The black rectangle under the game. The ring of it the game does not
/// cover is the gap.
#[derive(Component)]
pub struct GapBacking;

/// A brightness multiplier as a sprite tint. The tint is an sRGB grey, so
/// the result matches the website's CSS `brightness()` filter.
pub fn tint(level: f32) -> Color {
    let level = level.clamp(0.0, 1.0);
    Color::srgb(level, level, level)
}

/// The bezel may never be brighter than its idle level, whatever it is
/// handed. The state machine already guarantees this; the cap is here so
/// the guarantee also holds at the point of drawing.
pub fn bezel_level(level: f32) -> f32 {
    level.min(BEZEL_IDLE)
}

/// Sizes a sprite to `rect` and centres it there, keeping its depth.
pub fn place(
    sprite: &mut Sprite,
    transform: &mut Transform,
    rect: PxRect,
    geometry: &FrameGeometry,
) {
    let (centre, size) = to_world(rect, geometry.window, geometry.scale_factor);
    sprite.custom_size = Some(size);
    transform.translation = centre.extend(transform.translation.z);
}

/// `Startup`, after the art is loaded.
pub fn spawn_bezel(mut commands: Commands, art: Res<FrameArt>) {
    commands.spawn((
        BezelSprite,
        Sprite {
            image: art.bezel.clone(),
            color: tint(BEZEL_IDLE),
            custom_size: Some(Vec2::ZERO),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, BEZEL_Z),
        RenderLayers::layer(FRAME_LAYER),
    ));
    commands.spawn((
        GapBacking,
        Sprite::from_color(Color::BLACK, Vec2::ZERO),
        Transform::from_xyz(0.0, 0.0, BACKING_Z),
        RenderLayers::layer(FRAME_LAYER),
    ));
}

/// Places both sprites when the layout changes.
pub fn layout_bezel(
    geometry: Res<FrameGeometry>,
    mut bezel: Query<(&mut Sprite, &mut Transform), (With<BezelSprite>, Without<GapBacking>)>,
    mut backing: Query<(&mut Sprite, &mut Transform), (With<GapBacking>, Without<BezelSprite>)>,
) {
    if !geometry.is_changed() {
        return;
    }
    for (mut sprite, mut transform) in &mut bezel {
        place(
            &mut sprite,
            &mut transform,
            geometry.layout.bezel,
            &geometry,
        );
    }
    let rect = backing_rect(geometry.game, geometry.layout.insets.gap);
    for (mut sprite, mut transform) in &mut backing {
        place(&mut sprite, &mut transform, rect, &geometry);
    }
}

/// Applies the bezel multiplier. Brightness only: nothing moves.
pub fn paint_bezel(runtime: Res<FrameRuntime>, mut bezel: Query<&mut Sprite, With<BezelSprite>>) {
    let color = tint(bezel_level(runtime.levels.bezel));
    for mut sprite in &mut bezel {
        if sprite.color != color {
            sprite.color = color;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tint_is_a_grey_of_the_given_level() {
        assert_eq!(tint(0.35), Color::srgb(0.35, 0.35, 0.35));
        assert_eq!(tint(1.0), Color::srgb(1.0, 1.0, 1.0));
    }

    #[test]
    fn tint_is_clamped_to_the_unit_range() {
        assert_eq!(tint(1.7), Color::srgb(1.0, 1.0, 1.0));
        assert_eq!(tint(-0.2), Color::srgb(0.0, 0.0, 0.0));
    }

    #[test]
    fn the_bezel_level_is_capped_at_idle_brightness() {
        assert_eq!(bezel_level(0.20), 0.20);
        assert_eq!(bezel_level(0.35), 0.35);
        assert_eq!(bezel_level(0.9), BEZEL_IDLE);
    }

    #[test]
    fn place_sizes_and_centres_a_sprite() {
        let geometry = FrameGeometry {
            window: UVec2::new(1080, 1920),
            scale_factor: 2.0,
            ..FrameGeometry::empty()
        };
        let mut sprite = Sprite::default();
        let mut transform = Transform::from_xyz(9.0, 9.0, 3.0);
        place(
            &mut sprite,
            &mut transform,
            PxRect {
                x: 0,
                y: 0,
                w: 1080,
                h: 360,
            },
            &geometry,
        );
        assert_eq!(sprite.custom_size, Some(Vec2::new(540.0, 180.0)));
        assert_eq!(transform.translation, Vec3::new(0.0, 390.0, 3.0));
    }
}
