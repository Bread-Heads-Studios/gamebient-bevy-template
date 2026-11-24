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
    /// The demo bundle's content cap was reached (src/game/demo.rs). Only
    /// entered when the `demo` feature is on; the card is src/ui/demo_end.rs.
    DemoEnd,
    /// The level editor (editor builds): the level is shown, frozen, under a
    /// free camera. No `SimSet` system runs here. Entered and left only by
    /// the editor's `level.edit` (src/game/tunables/protocol.rs).
    Editing,
}

/// Whether the game is currently paused. Only meaningful in `Playing`.
#[derive(Resource, Default, PartialEq, Eq)]
pub struct Paused(pub bool);

/// A pause the editor's timeline holds (`game::time`): the sim is paused
/// exactly as by [`Paused`], which is set with it, but the pause overlay
/// stays hidden so the frame being inspected shows. Cleared when a run
/// starts, whenever `Paused` is cleared (`game::time`), and by any pause
/// or resume the player or the host makes (`toggle_pause`, `game::host`).
#[derive(Resource, Default, Debug, PartialEq, Eq)]
pub struct QuietPause(pub bool);

/// Run condition: true when the game is NOT paused.
pub fn not_paused(paused: Res<Paused>) -> bool {
    !paused.0
}
