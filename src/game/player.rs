use bevy::prelude::*;

use super::scoring::{Coin, GameData};
use super::sim::Checksum;

/// The player-controlled entity. Spawned by the level (`content/level.json`)
/// as the `Player` kind, whose `on_spawn` inserts this marker; see
/// `kinds_registration` in `game/mod.rs`.
#[derive(Component)]
pub struct Player;

const PLAYER_SPEED: f32 = 8.0;

/// The player's mesh: the active pack's `player` shape (schema v2) if it
/// sets one, else a cuboid whose edge is the pack's `player` `cuboid`
/// `size`, or 1.0 with no pack (or no `LookPlugin`, as in the `tests/`
/// harnesses). The art child's `Transform` scales it by the Thing's `Size`.
fn player_mesh(look: Option<&crate::look::ActiveLook>) -> Mesh {
    if let Some(crate::look::Slot::Shape(s)) = look.and_then(|l| l.slot("player")) {
        return crate::look::build_mesh(&s.shape);
    }
    let size = look
        .and_then(|l| l.recipe_number("player", "cuboid", "size"))
        .unwrap_or(1.0) as f32;
    Cuboid::new(size, size, size).into()
}

/// Render half of the player: dresses the box art child `spawn_level` gives
/// a [`Player`] Thing with the Look's mesh and a `LookColour`, and rebuilds
/// the mesh on every `LookChanged` so a hot-reloaded pack can change the
/// size or shape too. Registered by `assets::AssetsPlugin` in `Update`,
/// outside `SimSet`; the headless verifier never adds that plugin. It writes
/// only to the art child, never to the sim entity, so the Player's own
/// archetype is the same with or without a pack (D1, docs/look-packs.md;
/// `tests/archetype_order.rs`).
///
/// A child showing a sprite (`content/sprites/Player.png`) is the game's own
/// art: the pack leaves it alone and its `LookColour` is taken off, so the
/// retint never tints the sprite. The material is the kind's shared
/// `ThingArt` material, which `look::apply::retint_material` recolours.
///
/// Same frame as the spawn: `spawn_level` runs in `StateTransition`, before
/// `Update`, so the art child exists here on that frame, these inserts land
/// at the end of `Update`, and `retint_material` (`PostUpdate`) colours the
/// material before the frame renders.
pub fn dress_player(
    mut commands: Commands,
    mut changed: MessageReader<crate::look::LookChanged>,
    meshes: Option<ResMut<Assets<Mesh>>>,
    look: Option<Res<crate::look::ActiveLook>>,
    sprite_art: Option<Res<super::content::sprites::SpriteArt>>,
    players: Query<&Children, With<Player>>,
    art: Query<(&Mesh3d, Has<crate::look::LookColour>), With<super::content::sprites::ArtOf>>,
) {
    let redress = changed.read().count() > 0;
    let Some(mut meshes) = meshes else {
        return;
    };
    let quad = sprite_art.as_ref().map(|a| a.quad.id());
    for children in &players {
        for child in children.iter() {
            let Ok((mesh, tagged)) = art.get(child) else {
                continue;
            };
            if Some(mesh.0.id()) == quad {
                if tagged {
                    commands.entity(child).remove::<crate::look::LookColour>();
                }
                continue;
            }
            if !tagged {
                commands.entity(child).insert((
                    Mesh3d(meshes.add(player_mesh(look.as_deref()))),
                    crate::look::LookColour::new("player"),
                ));
            } else if redress {
                commands
                    .entity(child)
                    .insert(Mesh3d(meshes.add(player_mesh(look.as_deref()))));
            }
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
/// `lives`, the player transform bit-exactly, then the number of coins left,
/// in that order, so the checksum is sensitive to game-over state, movement
/// and which coins were taken.
pub fn checksum_player(
    data: Res<GameData>,
    player: Query<&Transform, With<Player>>,
    coins: Query<(), With<Coin>>,
    mut sum: ResMut<Checksum>,
) {
    sum.fold(u64::from(data.lives));
    if let Ok(tf) = player.single() {
        sum.fold(u64::from(tf.translation.x.to_bits()));
        sum.fold(u64::from(tf.translation.y.to_bits()));
    }
    sum.fold(coins.iter().count() as u64);
}
