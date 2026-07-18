use bevy::prelude::*;

use super::GameEntity;

/// The player-controlled entity.
#[derive(Component)]
pub struct Player;

const PLAYER_SPEED: f32 = 8.0;

/// Spawns a simple procedural player mesh when entering `Playing`.
pub fn spawn_player(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Player,
        GameEntity,
        Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.2, 0.8, 1.0),
            ..default()
        })),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));
}

/// Moves the player using the canonical [`GameInput`] resource populated by
/// `collect_input`. This is the canonical "add a gameplay system" example: a
/// plain `fn` reading a resource and a `Query`, gated to the `Playing` state
/// by `GamePlugin`.
pub fn move_player(
    time: Res<Time>,
    input: Res<crate::game::input::GameInput>,
    mut query: Query<&mut Transform, With<Player>>,
) {
    let Ok(mut tf) = query.single_mut() else {
        return;
    };
    let dir = Vec2::new(input.move_x, input.move_y);
    let delta = dir.normalize_or_zero() * PLAYER_SPEED * time.delta_secs();
    tf.translation.x += delta.x;
    tf.translation.y += delta.y;
}
