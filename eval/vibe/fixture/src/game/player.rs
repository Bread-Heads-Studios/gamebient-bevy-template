use bevy::prelude::*;

// The Player marker lives with the other kinds in `things.rs`; re-exported
// here so `player::Player` keeps resolving (autopilot, tests).
pub use super::things::Player;
use super::things::{Body, Facing, PLAYER_SPEED};

/// How fast a jump leaves the ground, px/s. Coin Run has no jump yet: a rule
/// that adds one should use this.
pub const JUMP_STRENGTH: f32 = 600.0;
/// Extra jumps allowed in the air (1 = double jump). Unused until there is a
/// jump.
pub const AIR_JUMPS: u32 = 0;

/// Walks the player left and right from the per-tick
/// [`TickInput`](gamebient_input::TickInput) (arrows, WASD, D-pad or stick),
/// and remembers which way it faces. Gravity and landing are
/// `physics::step_bodies`'s job, which runs next.
pub fn control_player(
    input: Res<gamebient_input::TickInput>,
    mut query: Query<(&mut Body, &mut Facing), With<Player>>,
) {
    for (mut body, mut facing) in &mut query {
        body.vel.x = input.move_x.clamp(-1.0, 1.0) * PLAYER_SPEED;
        if input.move_x != 0.0 {
            facing.0 = input.move_x.signum();
        }
    }
}
