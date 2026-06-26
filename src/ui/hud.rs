use bevy::prelude::*;

use crate::game::GameEntity;
use crate::game::scoring::GameData;

#[derive(Component)]
pub struct ScoreText;

/// Spawns the HUD when entering `Playing`. Marked `GameEntity` so it is cleaned
/// up automatically when the run ends.
pub fn spawn_hud(mut commands: Commands) {
    commands.spawn((
        GameEntity,
        ScoreText,
        Text::new("Score: 0  Lives: 3"),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(16.0),
            left: Val::Px(16.0),
            ..default()
        },
    ));
}

/// Keeps the HUD text in sync with `GameData`.
pub fn update_hud(data: Res<GameData>, mut query: Query<&mut Text, With<ScoreText>>) {
    if !data.is_changed() {
        return;
    }
    for mut text in &mut query {
        text.0 = format!("Score: {}  Lives: {}", data.score, data.lives);
    }
}
