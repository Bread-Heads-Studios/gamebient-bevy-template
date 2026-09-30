//! The on-screen marquee of a portrait cabinet: title art, an ambient
//! sweep in attract, a caption (score to beat, final score), and the
//! highlight pulse. Hidden on landscape displays, which have a physical
//! marquee.

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::frame::FRAME_LAYER;
use crate::frame::art::FrameArt;
use crate::frame::bezel::{place, tint};
use crate::frame::driver::{FrameGeometry, FrameRuntime};
use crate::frame::layout::{TITLE_ADVANCE_EM, caption_box, title_box, to_world};

/// Sweep band width as a share of the marquee height.
pub const SWEEP_BAND: f32 = 0.6;
/// Alpha of the pulse colour overlay at the top of the pulse.
pub const PULSE_OVERLAY_ALPHA: f32 = 0.35;
/// Pulse colour when the game gives none: the site's accent cyan.
pub const DEFAULT_PULSE_RGB: [u8; 3] = [0x00, 0xd4, 0xff];

// The fallback title's place and size come from `layout::title_box`: in the
// box to the right of the logo, centred on row 154 of 360.
// The score line's place and size come from `layout::caption_box`: centred
// on row 324 of 360, in the strip the art keeps free of lettering.

const ART_Z: f32 = 2.0;
const TITLE_Z: f32 = 3.0;
const SWEEP_Z: f32 = 4.0;
const PULSE_Z: f32 = 5.0;
const CAPTION_Z: f32 = 6.0;

/// Which part of the marquee an entity is. One component for all five, so
/// the systems below need a single query.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarqueeRole {
    Art,
    Title,
    Sweep,
    Pulse,
    Caption,
}

/// Centre of the sweep band. At 0 it is just off the left edge, at 1 just
/// off the right edge.
pub fn sweep_x(left: f32, width: f32, band: f32, progress: f32) -> f32 {
    left - band / 2.0 + progress * (width + band)
}

/// The colour overlay of a pulse at the given envelope (0..1).
pub fn pulse_color(color: Option<[u8; 3]>, envelope: f32) -> Color {
    let [r, g, b] = color.unwrap_or(DEFAULT_PULSE_RGB);
    let alpha = (envelope.clamp(0.0, 1.0) * PULSE_OVERLAY_ALPHA * 255.0).round() as u8;
    Color::srgba_u8(r, g, b, alpha)
}

/// Marquee text: the site's ivory, scaled by the marquee multiplier.
pub fn text_color(level: f32) -> Color {
    let level = level.clamp(0.0, 1.0);
    Color::srgb(0.91 * level, 0.89 * level, 0.87 * level)
}

/// `Startup`, after the art is loaded. Everything starts hidden;
/// `layout_marquee` shows it once a portrait layout is known.
pub fn spawn_marquee(
    mut commands: Commands,
    art: Res<FrameArt>,
    config: Option<Res<gamebient_input::GxConfig>>,
) {
    let layer = RenderLayers::layer(FRAME_LAYER);
    commands.spawn((
        MarqueeRole::Art,
        Sprite {
            image: art.marquee.clone(),
            custom_size: Some(Vec2::ZERO),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, ART_Z),
        Visibility::Hidden,
        layer.clone(),
    ));
    // The shared art has the logo but no title, so the game's name is drawn
    // in the box beside the logo. A game's own marquee.png carries its title art.
    let title = if art.marquee_is_fallback {
        config.map_or_else(String::new, |c| c.name.to_ascii_uppercase())
    } else {
        String::new()
    };
    commands.spawn((
        MarqueeRole::Title,
        Text2d::new(title),
        TextFont {
            font_size: 1.0,
            ..default()
        },
        TextColor(text_color(1.0)),
        Anchor::CENTER,
        Transform::from_xyz(0.0, 0.0, TITLE_Z),
        Visibility::Hidden,
        layer.clone(),
    ));
    commands.spawn((
        MarqueeRole::Sweep,
        Sprite {
            image: art.sweep.clone(),
            custom_size: Some(Vec2::ZERO),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, SWEEP_Z),
        Visibility::Hidden,
        layer.clone(),
    ));
    commands.spawn((
        MarqueeRole::Pulse,
        Sprite::from_color(Color::NONE, Vec2::ZERO),
        Transform::from_xyz(0.0, 0.0, PULSE_Z),
        Visibility::Hidden,
        layer.clone(),
    ));
    commands.spawn((
        MarqueeRole::Caption,
        Text2d::new(""),
        TextFont {
            font_size: 1.0,
            ..default()
        },
        TextColor(text_color(1.0)),
        Anchor::CENTER,
        Transform::from_xyz(0.0, 0.0, CAPTION_Z),
        Visibility::Hidden,
        layer,
    ));
}

/// Places the marquee when the layout changes, or hides it on a landscape
/// display.
pub fn layout_marquee(
    geometry: Res<FrameGeometry>,
    mut parts: Query<(
        &MarqueeRole,
        &mut Transform,
        &mut Visibility,
        Option<&mut Sprite>,
        Option<&mut TextFont>,
        Option<&Text2d>,
    )>,
) {
    if !geometry.is_changed() {
        return;
    }
    let Some(rect) = geometry.layout.marquee else {
        for (_, _, mut visibility, _, _, _) in &mut parts {
            *visibility = Visibility::Hidden;
        }
        return;
    };
    let (centre, size) = to_world(rect, geometry.window, geometry.scale_factor);
    let top = centre.y + size.y / 2.0;
    for (role, mut transform, mut visibility, sprite, font, text) in &mut parts {
        match role {
            MarqueeRole::Art | MarqueeRole::Pulse => {
                if let Some(mut sprite) = sprite {
                    place(&mut sprite, &mut transform, rect, &geometry);
                }
                *visibility = Visibility::Visible;
            }
            MarqueeRole::Sweep => {
                if let Some(mut sprite) = sprite {
                    sprite.custom_size = Some(Vec2::new(size.y * SWEEP_BAND, size.y));
                }
                transform.translation.y = centre.y;
                // `paint_marquee` shows it while a sweep is running.
            }
            MarqueeRole::Title => {
                let chars = text.map_or(0, |t| t.0.chars().count());
                let title = title_box(rect.w, rect.h, chars, TITLE_ADVANCE_EM);
                let scale = geometry.scale_factor;
                let half = geometry.window.as_vec2() / 2.0;
                transform.translation.x = (rect.x as f32 + title.centre_x - half.x) / scale;
                transform.translation.y = (half.y - (rect.y as f32 + title.centre_y)) / scale;
                if let Some(mut font) = font {
                    font.font_size = title.font_size / scale;
                }
                *visibility = Visibility::Visible;
            }
            MarqueeRole::Caption => {
                transform.translation.x = centre.x;
                let (centre_row, font_size) = caption_box(size.x, size.y);
                transform.translation.y = top - centre_row;
                if let Some(mut font) = font {
                    font.font_size = font_size;
                }
                *visibility = Visibility::Visible;
            }
        }
    }
}

/// Every frame: brightness, sweep position, pulse colour and caption text.
pub fn paint_marquee(
    runtime: Res<FrameRuntime>,
    geometry: Res<FrameGeometry>,
    mut parts: Query<(
        &MarqueeRole,
        &mut Transform,
        &mut Visibility,
        Option<&mut Sprite>,
        Option<&mut TextColor>,
        Option<&mut Text2d>,
    )>,
) {
    let Some(rect) = geometry.layout.marquee else {
        return;
    };
    let (centre, size) = to_world(rect, geometry.window, geometry.scale_factor);
    let levels = runtime.levels;
    for (role, mut transform, mut visibility, sprite, color, text) in &mut parts {
        match role {
            MarqueeRole::Art => {
                if let Some(mut sprite) = sprite {
                    sprite.color = tint(levels.marquee);
                }
            }
            MarqueeRole::Title => {
                if let Some(mut color) = color {
                    color.0 = text_color(levels.marquee);
                }
            }
            MarqueeRole::Sweep => match levels.sweep_progress(false) {
                Some(progress) => {
                    transform.translation.x = sweep_x(
                        centre.x - size.x / 2.0,
                        size.x,
                        size.y * SWEEP_BAND,
                        progress,
                    );
                    *visibility = Visibility::Visible;
                }
                None => *visibility = Visibility::Hidden,
            },
            MarqueeRole::Pulse => {
                if let Some(mut sprite) = sprite {
                    sprite.color = match levels.pulse {
                        Some(envelope) => pulse_color(runtime.pulse_color, envelope),
                        None => Color::NONE,
                    };
                }
            }
            MarqueeRole::Caption => {
                if let Some(mut color) = color {
                    color.0 = text_color(levels.marquee);
                }
                if let Some(mut text) = text {
                    let next = runtime.caption().unwrap_or_default();
                    // Compare first: writing the same string would still
                    // mark the text changed and lay it out again.
                    if text.0 != next {
                        text.0 = next;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sweep_starts_and_ends_off_the_marquee() {
        // Marquee from x = -540 to 540, band 216 wide.
        assert_eq!(sweep_x(-540.0, 1080.0, 216.0, 0.0), -648.0);
        assert_eq!(sweep_x(-540.0, 1080.0, 216.0, 1.0), 648.0);
        assert_eq!(sweep_x(-540.0, 1080.0, 216.0, 0.5), 0.0);
    }

    #[test]
    fn the_pulse_uses_the_games_colour_or_the_default() {
        assert_eq!(
            pulse_color(Some([255, 204, 0]), 1.0),
            Color::srgba_u8(255, 204, 0, 89)
        );
        assert_eq!(
            pulse_color(None, 1.0),
            Color::srgba_u8(0x00, 0xd4, 0xff, 89)
        );
        assert_eq!(
            pulse_color(Some([255, 204, 0]), 0.0),
            Color::srgba_u8(255, 204, 0, 0)
        );
    }

    #[test]
    fn text_dims_with_the_marquee() {
        assert_eq!(text_color(1.0), Color::srgb(0.91, 0.89, 0.87));
        assert_eq!(
            text_color(0.5),
            Color::srgb(0.91 * 0.5, 0.89 * 0.5, 0.87 * 0.5)
        );
    }
}
