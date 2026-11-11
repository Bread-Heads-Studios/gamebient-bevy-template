use bevy::prelude::*;

/// Loads/creates game assets at startup and dresses sim entities for the
/// screen. The skeleton is procedural and ships no external files; register
/// asset handles here (meshes, materials, audio) as the game grows. See
/// `docs/conventions.md` for the asset-loading pattern.
///
/// Render-side only: `main.rs` adds it, the headless verifier
/// (`replay::build_headless_app`) never does, so a system here may read
/// `ActiveLook` and `LookAssets` and the sim still cannot. `tests/
/// windowed_shape.rs` adds it on top of the headless app to probe that its
/// decorators are sim-safe.
pub struct AssetsPlugin;

impl Plugin for AssetsPlugin {
    fn build(&self, app: &mut App) {
        // `LookPlugin` registers this too; repeated here (idempotent) so the
        // plugin also works in harnesses that add no `LookPlugin`.
        app.add_message::<crate::look::LookChanged>()
            .add_systems(Update, crate::game::player::dress_player);
    }
}
