use bevy::prelude::*;

use crate::game::states::GameState;

#[derive(Component)]
pub struct MenuRoot;

/// Spawns a full-screen centered message.
fn spawn_centered(commands: &mut Commands, message: &str) {
    commands
        .spawn((
            MenuRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
        ))
        .with_children(|p| {
            p.spawn(Text::new(message.to_string()));
        });
}

pub fn spawn_menu(mut commands: Commands) {
    spawn_centered(&mut commands, "GAMEBIENT GAME\nPress ENTER to start");
}

pub fn spawn_game_over(mut commands: Commands) {
    spawn_centered(&mut commands, "GAME OVER\nPress ENTER to restart");
}

pub fn despawn_menu(mut commands: Commands, query: Query<Entity, With<MenuRoot>>) {
    for e in &query {
        commands.entity(e).despawn();
    }
}

/// ENTER advances `Menu -> Playing` and `GameOver -> Playing`.
pub fn menu_input(
    input: Res<ButtonInput<KeyCode>>,
    state: Res<State<GameState>>,
    mut next: ResMut<NextState<GameState>>,
) {
    if input.just_pressed(KeyCode::Enter)
        && matches!(state.get(), GameState::Menu | GameState::GameOver)
    {
        next.set(GameState::Playing);
    }
}
