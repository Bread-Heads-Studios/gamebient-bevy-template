use bevy::prelude::*;

pub mod hud;
pub mod menu;

use crate::game::states::GameState;

/// Title screen, game-over screen, and in-run HUD.
pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Menu), menu::spawn_menu)
            .add_systems(OnExit(GameState::Menu), menu::despawn_menu)
            .add_systems(OnEnter(GameState::GameOver), menu::spawn_game_over)
            .add_systems(OnExit(GameState::GameOver), menu::despawn_menu)
            .add_systems(OnEnter(GameState::Playing), hud::spawn_hud)
            .add_systems(Update, hud::update_hud.run_if(in_state(GameState::Playing)))
            .add_systems(Update, menu::menu_input);
    }
}
