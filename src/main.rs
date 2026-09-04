// Bevy systems idiomatically take many parameters and nested query filters;
// these two lints fire on correct ECS code, so we allow them crate-wide.
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

mod assets;
mod game;
mod ui;

use bevy::prelude::*;
use bevy::window::{PresentMode, WindowResolution};

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
                // On web, pin the backbuffer to a fixed 1280×720 with a 1.0
                // scale factor and let CSS scale the fixed-size canvas up to
                // fill the viewport (letterboxed). We must NOT fit the canvas
                // to its parent: winit would then resize the backbuffer to the
                // element's pixel size, shading full-resolution pixels and
                // defeating the low-res render target the Pi 5 depends on.
                #[cfg(target_arch = "wasm32")]
                resolution: WindowResolution::new(1280, 720).with_scale_factor_override(1.0),
                #[cfg(not(target_arch = "wasm32"))]
                resolution: WindowResolution::new(1280, 720),
                canvas: Some("#game".to_string()),
                #[cfg(target_arch = "wasm32")]
                fit_canvas_to_parent: false,
                present_mode: PresentMode::AutoVsync,
                ..default()
            }),
            ..default()
        }))
        // Near-black clear color: visible wherever no geometry/UI covers the
        // viewport (notably the how-to-play showcase background).
        .insert_resource(ClearColor(Color::srgb(0.008, 0.012, 0.03)))
        .add_plugins((game::GamePlugin, assets::AssetsPlugin, ui::UiPlugin))
        .add_systems(Update, update_ui_scale)
        .run();
}
