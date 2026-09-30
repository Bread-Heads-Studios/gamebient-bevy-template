use bevy::prelude::*;
use gamebient_game::{assets, display, game, ui};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            // Pinned canvas policy at the game's own size; borderless
            // fullscreen on Linux cabinets. See src/display.rs.
            primary_window: Some(display::game_window("Gamebient Game")),
            ..default()
        }))
        // Near-black clear color: visible wherever no geometry/UI covers the
        // viewport (notably the how-to-play showcase background), and in the
        // letterbox bars around the game on native displays.
        .insert_resource(ClearColor(Color::srgb(0.008, 0.012, 0.03)))
        .add_plugins((
            display::DisplayPlugin,
            game::GamePlugin::default(),
            assets::AssetsPlugin,
            ui::UiPlugin,
            // Bezel and marquee around the letterboxed game. Native only;
            // GX_FRAME=off disables it. Never add this to GamePlugin.
            gamebient_game::frame::FramePlugin,
        ))
        .run();
}
