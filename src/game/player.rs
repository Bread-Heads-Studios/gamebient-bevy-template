use bevy::prelude::*;

use super::GameEntity;
use super::scoring::GameData;
use super::sim::Checksum;

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

/// This template's example of the per-game system `sim::checksum_tick`'s
/// module doc calls for: registered right after `sim::checksum_tick` in the
/// `SimSet` chain, folding the key run state `checksum_tick` itself doesn't
/// know about (see docs/replay-verification.md, determinism rule 6). Folds
/// `lives` then the player transform bit-exactly, in that order, so the
/// checksum is sensitive to both game-over state and movement.
pub fn checksum_player(
    data: Res<GameData>,
    player: Query<&Transform, With<Player>>,
    mut sum: ResMut<Checksum>,
) {
    sum.fold(u64::from(data.lives));
    if let Ok(tf) = player.single() {
        sum.fold(u64::from(tf.translation.x.to_bits()));
        sum.fold(u64::from(tf.translation.y.to_bits()));
    }
}
