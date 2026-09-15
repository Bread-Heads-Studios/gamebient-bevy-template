use bevy::prelude::*;

use super::GameEntity;

/// The player-controlled entity.
#[derive(Component)]
pub struct Player;

const PLAYER_SPEED: f32 = 8.0;

/// Spawns a simple procedural player mesh when entering `Playing`. Headless
/// builds (the replay verifier) have no `Assets<Mesh>` / `Assets<StandardMaterial>`
/// resources, so the mesh/material are only inserted when both are present.
pub fn spawn_player(
    mut commands: Commands,
    meshes: Option<ResMut<Assets<Mesh>>>,
    materials: Option<ResMut<Assets<StandardMaterial>>>,
) {
    let entity = commands
        .spawn((Player, GameEntity, Transform::from_xyz(0.0, 0.0, 0.0)))
        .id();
    if let (Some(mut meshes), Some(mut materials)) = (meshes, materials) {
        commands.entity(entity).insert((
            Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(0.2, 0.8, 1.0),
                ..default()
            })),
        ));
    }
}

/// Moves the player using the per-tick [`TickInput`](gamebient_input::TickInput)
/// resource. This is the canonical "add a gameplay system" example: a plain
/// `fn` reading a resource and a `Query`, run in `SimSet` (`FixedUpdate`) by
/// `GamePlugin`. `Res<Time>` inside `FixedUpdate` is the fixed delta.
pub fn move_player(
    time: Res<Time>,
    input: Res<gamebient_input::TickInput>,
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
