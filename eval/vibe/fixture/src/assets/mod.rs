use bevy::prelude::*;

/// Loads/creates game assets at startup. Coin Run is procedural and ships no
/// external files: it builds the mesh and materials things are drawn with
/// (`things::ThingArt`). See `docs/conventions.md` for the asset-loading
/// pattern.
pub struct AssetsPlugin;

impl Plugin for AssetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, crate::game::things::setup_thing_art);
    }
}
