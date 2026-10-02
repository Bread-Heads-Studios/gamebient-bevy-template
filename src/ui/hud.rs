use bevy::prelude::*;

use crate::game::GameEntity;
use crate::game::scoring::GameData;
use crate::ui::card::S2;
use crate::ui::fonts::BODY;
use crate::ui::theme::PAPER;

#[derive(Component)]
pub struct ScoreText;

/// Spawns the HUD when entering `Playing`. Marked `GameEntity` so it is cleaned
/// up automatically when the run ends.
pub fn spawn_hud(mut commands: Commands) {
    commands.spawn((
        GameEntity,
        ScoreText,
        Text::new("Score: 0  Lives: 3"),
        // TEMPLATE NOTE: the HUD is set in the body face; theme its colour
        // (and a plate, if the world behind it is busy) in `theme.rs`.
        BODY.font(22.0),
        TextColor(PAPER),
        TextShadow {
            offset: Vec2::new(0.0, 2.0),
            color: Color::srgba(0.0, 0.0, 0.0, 0.7),
        },
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(S2),
            left: Val::Px(S2),
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
