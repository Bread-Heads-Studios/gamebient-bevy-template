use bevy::prelude::*;

/// Loads/creates game assets at startup. The skeleton is procedural and ships no
/// external files; register asset handles here (meshes, materials, audio) as the
/// game grows. See `docs/conventions.md` for the asset-loading pattern.
pub struct AssetsPlugin;

impl Plugin for AssetsPlugin {
    fn build(&self, _app: &mut App) {
        // Example:
        //   _app.add_systems(Startup, load_audio);
    }
}
