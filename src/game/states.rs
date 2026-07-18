use bevy::prelude::*;

/// Top-level game flow.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GameState {
    /// "BREAD HEADS STUDIOS PRESENTS" boot screen.
    #[default]
    StudioLogo,
    Menu,
    /// Once-per-session instruction screen (see src/ui/how_to_play.rs).
    HowToPlay,
    Playing,
    GameOver,
}

/// Whether the game is currently paused. Only meaningful in `Playing`.
#[derive(Resource, Default, PartialEq, Eq)]
pub struct Paused(pub bool);

/// Run condition: true when the game is NOT paused.
pub fn not_paused(paused: Res<Paused>) -> bool {
    !paused.0
}
