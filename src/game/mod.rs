use bevy::prelude::*;

pub mod player;
pub mod scoring;
pub mod states;

use states::GameState;

/// Marker for entities spawned during a run; despawned on cleanup.
#[derive(Component)]
pub struct GameEntity;

/// Owns the game state machine, core resources, and gameplay systems.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>()
            .init_resource::<scoring::GameData>()
            .add_message::<scoring::ScoreEvent>()
            .add_systems(Startup, setup_scene)
            .add_systems(OnEnter(GameState::Playing), player::spawn_player)
            .add_systems(
                Update,
                (player::move_player, scoring::handle_score_events)
                    .run_if(in_state(GameState::Playing)),
            )
            .add_systems(OnExit(GameState::Playing), cleanup_game_entities);
    }
}

/// One-time scene setup: a camera and a directional light.
fn setup_scene(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 0.0, 20.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        DirectionalLight::default(),
        Transform::from_xyz(4.0, 8.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

/// Despawns all run entities when leaving `Playing`.
fn cleanup_game_entities(mut commands: Commands, query: Query<Entity, With<GameEntity>>) {
    for e in &query {
        commands.entity(e).despawn();
    }
}
