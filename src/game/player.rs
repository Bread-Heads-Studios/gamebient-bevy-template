use bevy::prelude::*;

use super::GameEntity;
use super::scoring::GameData;
use super::sim::{Checksum, SpawnCounter};

/// The player-controlled entity.
#[derive(Component)]
pub struct Player;

const PLAYER_SPEED: f32 = 8.0;

/// Spawns the player's sim half when entering `Playing`: the marker, the
/// run-scoped `GameEntity`, the spawn stamp and the `Transform`. Nothing
/// render-side and no look read: this runs in the headless verifier too, and
/// a Look Pack never reaches the sim (D1, docs/look-packs.md). The mesh and
/// material come from [`dress_player`], which `AssetsPlugin` registers.
///
/// Every sim entity is stamped with a [`SpawnOrder`](super::sim::SpawnOrder)
/// as it is spawned — the stable key determinism rule 1's sorting clause asks
/// order-sensitive sim systems to sort by. This template has exactly one sim
/// entity, so nothing here has an order to get wrong yet; the stamp is here
/// because a game grows its second sim entity long before anyone remembers
/// to add the key, and `tests/archetype_order.rs` needs a key the sim owns to
/// split archetypes on.
pub fn spawn_player(mut commands: Commands, mut spawn: ResMut<SpawnCounter>) {
    commands.spawn((
        Player,
        GameEntity,
        spawn.stamp(),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));
}

/// The player's cuboid edge: the active pack's `player` `cuboid` `size`, or
/// 1.0 with no pack (or no `LookPlugin`, as in the `tests/` harnesses).
fn player_size(look: Option<&crate::look::ActiveLook>) -> f32 {
    look.and_then(|l| l.recipe_number("player", "cuboid", "size"))
        .unwrap_or(1.0) as f32
}

/// Render half of the player: gives a newly spawned [`Player`] its mesh,
/// material and `LookColour`, and rebuilds the mesh on every `LookChanged`
/// so a hot-reloaded pack can change the size too. Registered by
/// `assets::AssetsPlugin` in `Update`, outside `SimSet`; the headless
/// verifier never adds that plugin, and this writes nothing the sim reads
/// (`tests/archetype_order.rs` mirrors it as its faithful decorator).
///
/// Same frame as the spawn: `spawn_player` runs in `StateTransition`, before
/// `Update`, so this sees `Added<Player>` on that frame, its inserts land at
/// the end of `Update`, and `look::apply::retint_material` (`PostUpdate`)
/// colours the material before the frame renders. No frame shows the player
/// without a mesh.
pub fn dress_player(
    mut commands: Commands,
    mut changed: MessageReader<crate::look::LookChanged>,
    meshes: Option<ResMut<Assets<Mesh>>>,
    materials: Option<ResMut<Assets<StandardMaterial>>>,
    look: Option<Res<crate::look::ActiveLook>>,
    players: Query<(Entity, Ref<Player>)>,
) {
    let redress = changed.read().count() > 0;
    let (Some(mut meshes), Some(mut materials)) = (meshes, materials) else {
        return;
    };
    let size = player_size(look.as_deref());
    for (entity, player) in &players {
        if player.is_added() {
            commands.entity(entity).insert((
                Mesh3d(meshes.add(Cuboid::new(size, size, size))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: crate::ui::theme::PLAYER,
                    ..default()
                })),
                crate::look::LookColour::new("player"),
            ));
        } else if redress {
            commands
                .entity(entity)
                .insert(Mesh3d(meshes.add(Cuboid::new(size, size, size))));
        }
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
