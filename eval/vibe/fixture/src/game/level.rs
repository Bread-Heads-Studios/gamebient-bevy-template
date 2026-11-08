//! The level: which things a run starts with, and where.
//!
//! `Level` is a resource so the level editor (and `scenario.rs`) can swap in
//! another layout before a run starts. `spawn_level` places it on
//! `OnEnter(Playing)`, in list order, which is also the `SpawnOrder` order.

use bevy::prelude::*;

use super::things::Spawner;

/// One thing in the level.
#[derive(Clone, Debug)]
pub struct Placed {
    pub kind: String,
    pub at: Vec2,
    /// `None` keeps the kind's default size.
    pub size: Option<Vec2>,
}

impl Placed {
    pub fn new(kind: &str, x: f32, y: f32) -> Self {
        Self {
            kind: kind.to_string(),
            at: Vec2::new(x, y),
            size: None,
        }
    }

    pub fn sized(kind: &str, x: f32, y: f32, w: f32, h: f32) -> Self {
        Self {
            size: Some(Vec2::new(w, h)),
            ..Self::new(kind, x, y)
        }
    }
}

/// The layout the next run starts with.
#[derive(Resource, Clone, Debug)]
pub struct Level(pub Vec<Placed>);

impl Default for Level {
    /// Coin Run's one level: a floor with a pit near the right, a ledge,
    /// a row of coins, a spike, an enemy past the pit and the goal at the
    /// far right. The player starts at the far left.
    fn default() -> Self {
        Self(vec![
            Placed::sized("Platform", -180.0, -330.0, 600.0, 60.0),
            Placed::sized("Platform", 340.0, -330.0, 280.0, 60.0),
            Placed::sized("Platform", -100.0, -150.0, 200.0, 20.0),
            Placed::new("Coin", -320.0, -290.0),
            Placed::new("Coin", -220.0, -290.0),
            Placed::new("Coin", -120.0, -290.0),
            Placed::new("Coin", -100.0, -130.0),
            Placed::new("Coin", 250.0, -290.0),
            Placed::new("Spike", 0.0, -292.0),
            Placed::new("Enemy", 300.0, -282.0),
            Placed::new("Goal", 440.0, -268.0),
            Placed::new("Player", -400.0, -276.0),
        ])
    }
}

/// Places the level when a run starts.
pub fn spawn_level(level: Res<Level>, mut spawner: Spawner) {
    for p in &level.0 {
        spawner.spawn_sized(&p.kind, p.at, p.size);
    }
}
