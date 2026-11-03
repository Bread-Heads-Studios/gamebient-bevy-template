//! Coin Run's physics: velocity, gravity and landing on solid things.
//!
//! Runs in `SimSet` after the player's controls and before the game's
//! rules. Order-independent on purpose: each body is resolved only against
//! static `Solid` things, never against other moving bodies, so the order a
//! query yields bodies in can't change the outcome.

use bevy::prelude::*;

use super::things::{Body, Size, Solid};

/// Downward acceleration, px/s².
pub const GRAVITY: f32 = 1500.0;
/// Fastest a body can fall, px/s.
pub const MAX_FALL: f32 = 900.0;

/// Moves every `Body` by its velocity. Bodies with `gravity` fall, can't
/// pass through `Solid` things, and land on them (`on_ground`).
pub fn step_bodies(
    time: Res<Time>,
    mut bodies: Query<(&mut Transform, &mut Body, &Size), Without<Solid>>,
    solids: Query<(&Transform, &Size), With<Solid>>,
) {
    let dt = time.delta_secs();
    let solids: Vec<(Vec2, Vec2)> = solids
        .iter()
        .map(|(tf, size)| (tf.translation.truncate(), size.0))
        .collect();
    for (mut tf, mut body, size) in &mut bodies {
        if !body.gravity {
            tf.translation += (body.vel * dt).extend(0.0);
            continue;
        }
        body.vel.y = (body.vel.y - GRAVITY * dt).max(-MAX_FALL);
        body.on_ground = false;
        let half = size.0 * 0.5;

        // Horizontal: move, then push out of anything solid sideways.
        tf.translation.x += body.vel.x * dt;
        for (s_pos, s_size) in &solids {
            let s_half = *s_size * 0.5;
            let d = tf.translation.truncate() - *s_pos;
            if d.x.abs() < half.x + s_half.x && d.y.abs() < half.y + s_half.y {
                tf.translation.x = if d.x < 0.0 {
                    s_pos.x - s_half.x - half.x
                } else {
                    s_pos.x + s_half.x + half.x
                };
            }
        }

        // Vertical: move, then land on top of (or bump the underside of)
        // anything solid.
        tf.translation.y += body.vel.y * dt;
        for (s_pos, s_size) in &solids {
            let s_half = *s_size * 0.5;
            let d = tf.translation.truncate() - *s_pos;
            if d.x.abs() < half.x + s_half.x && d.y.abs() < half.y + s_half.y {
                if d.y > 0.0 {
                    tf.translation.y = s_pos.y + s_half.y + half.y;
                    body.on_ground = true;
                } else {
                    tf.translation.y = s_pos.y - s_half.y - half.y;
                }
                body.vel.y = 0.0;
            }
        }
    }
}
