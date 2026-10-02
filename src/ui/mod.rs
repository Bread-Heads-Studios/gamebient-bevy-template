use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;

pub mod backdrop;
pub mod card;
pub mod fit;
pub mod fonts;
pub mod how_to_play;
pub mod hud;
pub mod menu;
pub mod studio_logo;
pub mod theme;
pub mod transition;

use crate::game::states::GameState;

/// Title screen, game-over screen, in-run HUD, and cross-state transitions.
pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app
            // Cross-state fade overlay + shared prompt pulse
            .insert_resource(transition::ScreenFade::boot())
            .add_systems(Startup, transition::spawn_fade_overlay)
            .add_systems(Update, (transition::update_fade, transition::pulse_text))
            // Card entrances and the title backdrop's drift (presentation).
            .add_systems(Update, (card::animate_card_intro, backdrop::drift_grid))
            // Studio logo boot screen
            .add_systems(
                OnEnter(GameState::StudioLogo),
                studio_logo::spawn_studio_logo,
            )
            .add_systems(
                Update,
                studio_logo::advance_studio_logo.run_if(in_state(GameState::StudioLogo)),
            )
            .add_systems(
                OnExit(GameState::StudioLogo),
                studio_logo::despawn_studio_logo,
            )
            // How-to-play screen (shown once per session before the first game)
            .init_resource::<how_to_play::SeenHowToPlay>()
            .add_systems(
                OnEnter(GameState::HowToPlay),
                (
                    how_to_play::spawn_how_to_play,
                    how_to_play::mark_seen,
                    how_to_play::sweep_on_enter,
                ),
            )
            .add_systems(
                Update,
                (
                    how_to_play::spin_items,
                    how_to_play::position_labels,
                    how_to_play::how_to_play_input,
                )
                    .run_if(in_state(GameState::HowToPlay)),
            )
            .add_systems(
                OnExit(GameState::HowToPlay),
                how_to_play::despawn_how_to_play,
            )
            // Menu / Game Over
            .add_systems(OnEnter(GameState::Menu), menu::spawn_menu)
            .add_systems(OnExit(GameState::Menu), menu::despawn_menu)
            .add_systems(OnEnter(GameState::GameOver), menu::spawn_game_over)
            .add_systems(OnExit(GameState::GameOver), menu::despawn_menu)
            .add_systems(Update, menu::menu_input)
            // In-run HUD
            .add_systems(OnEnter(GameState::Playing), hud::spawn_hud)
            .add_systems(Update, hud::update_hud.run_if(in_state(GameState::Playing)));
    }

    /// Installs the embedded fonts before the first frame. Not a `Startup`
    /// system: the initial state's `OnEnter` (the studio logo's text) runs
    /// in `StateTransition`, ahead of `PreStartup`. See `fonts::install_fonts`.
    fn finish(&self, app: &mut App) {
        app.world_mut()
            .run_system_once(fonts::install_fonts)
            .expect("install_fonts runs once Assets<Font> exists (TextPlugin)");
    }
}
