// Bevy systems idiomatically take many parameters and nested query filters;
// these two lints fire on correct ECS code, so we allow them crate-wide.
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

mod assets;
mod game;
mod ui;

use bevy::prelude::*;
use bevy::window::{PresentMode, WindowResolution};

/// Internal-render scale: shade fewer pixels and let the browser upscale the
/// result. 0.5 quarters the shaded pixel count — good for low-power kiosk
/// hardware (e.g. a Raspberry Pi).
const RENDER_SCALE: f32 = 0.5;

/// Reference height that all UI pixel values are authored against.
const REFERENCE_HEIGHT: f32 = 720.0;

/// Scales all UI proportionally to window height so hardcoded `Val::Px` and
/// `font_size` values look correct on any display.
fn update_ui_scale(windows: Query<&Window>, mut ui_scale: ResMut<UiScale>) {
    let Ok(window) = windows.single() else {
        return;
    };
    let scale = window.resolution.height() / REFERENCE_HEIGHT;
    if (ui_scale.0 - scale).abs() > 0.001 {
        ui_scale.0 = scale;
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Gamebient Game".into(),
                resolution:
                    WindowResolution::new(1280, 720).with_scale_factor_override(RENDER_SCALE),
                canvas: Some("#game".to_string()),
                fit_canvas_to_parent: true,
                present_mode: PresentMode::AutoVsync,
                ..default()
            }),
            ..default()
        }))
        .add_plugins((game::GamePlugin, assets::AssetsPlugin, ui::UiPlugin))
        .add_systems(Update, update_ui_scale)
        .run();
}
