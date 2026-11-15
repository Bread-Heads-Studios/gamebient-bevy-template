use bevy::prelude::*;

use super::scoring::{Coin, GameData};
use super::sim::Checksum;

/// The player-controlled entity. Spawned by the level (`content/level.json`)
/// as the `Player` kind, whose `on_spawn` inserts this marker; see
/// `kinds_registration` in `game/mod.rs`.
#[derive(Component)]
pub struct Player;

const PLAYER_SPEED: f32 = 8.0;

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
