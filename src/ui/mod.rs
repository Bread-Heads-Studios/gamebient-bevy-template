use bevy::prelude::*;

pub mod hud;
pub mod menu;
pub mod studio_logo;
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
            // Studio logo boot screen
            .add_systems(OnEnter(GameState::StudioLogo), studio_logo::spawn_studio_logo)
            .add_systems(
                Update,
                studio_logo::advance_studio_logo.run_if(in_state(GameState::StudioLogo)),
            )
            .add_systems(OnExit(GameState::StudioLogo), studio_logo::despawn_studio_logo)
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
}
