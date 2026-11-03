//! Coin Run's things: every object in a level is a *thing* of one kind
//! (Player, Platform, Coin, Enemy, Bullet, Goal, Spike).
//!
//! Units are game pixels, with the origin at the centre of the 960 x 720
//! screen and y pointing up. The floor's top is at y = -300.
//!
//! **Spawn every thing through [`Spawner`]** (`spawner.spawn("Coin", pos)`),
//! never with a bare `commands.spawn`. The spawner gives each thing its kind
//! marker, its `Name` (the kind's name, which the level editor and
//! `scenario.rs` find things by), its size, its physics body, a
//! [`SpawnOrder`] stamp (determinism rule 1) and the `GameEntity` cleanup
//! marker, and it dresses it with a mesh in the windowed game only.
//!
//! To add a new kind, add a marker component, a row in [`props`] and an arm
//! in [`Spawner::spawn_sized`].

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use super::GameEntity;
use super::scoring::GameData;
use super::sim::{Checksum, SpawnCounter, SpawnOrder};

/// Every thing carries this, whatever its kind.
#[derive(Component)]
pub struct Thing;

/// The player-controlled thing.
#[derive(Component)]
pub struct Player;
/// Solid ground to stand on.
#[derive(Component)]
pub struct Platform;
#[derive(Component)]
pub struct Coin;
#[derive(Component)]
pub struct Enemy;
#[derive(Component)]
pub struct Bullet;
#[derive(Component)]
pub struct Goal;
#[derive(Component)]
pub struct Spike;

/// A thing's full width and height, in pixels. The collision box is centred
/// on the thing's `Transform`.
#[derive(Component, Clone, Copy, Debug)]
pub struct Size(pub Vec2);

/// Things that move. `physics::step_bodies` applies `vel` (px/s) and, when
/// `gravity` is set, gravity and landing on solid things.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Body {
    pub vel: Vec2,
    pub gravity: bool,
    /// True on a tick the body ended standing on a solid thing.
    pub on_ground: bool,
}

/// Things with a `Body` and `gravity` can't pass through a `Solid`.
#[derive(Component)]
pub struct Solid;

/// Which way the player last moved: `1.0` right, `-1.0` left.
#[derive(Component, Clone, Copy, Debug)]
pub struct Facing(pub f32);

/// Walking speed of the Player, px/s.
pub const PLAYER_SPEED: f32 = 240.0;
/// Speed an Enemy moves at when something makes it move, px/s.
pub const ENEMY_SPEED: f32 = 120.0;
/// Speed a Bullet moves at when something fires it, px/s.
pub const BULLET_SPEED: f32 = 600.0;

/// The per-kind defaults the spawner uses.
#[derive(Clone, Copy, Debug)]
pub struct ThingProps {
    pub size: Vec2,
    pub color: Color,
    pub gravity: bool,
    pub solid: bool,
    /// Whether the thing gets a `Body` at all. Static things don't.
    pub moves: bool,
}

/// The kinds this game knows, and their defaults. `None` for an unknown name.
pub fn props(kind: &str) -> Option<ThingProps> {
    let p = |w: f32, h: f32, color: Color, gravity: bool, solid: bool, moves: bool| ThingProps {
        size: Vec2::new(w, h),
        color,
        gravity,
        solid,
        moves,
    };
    Some(match kind {
        "Player" => p(32.0, 48.0, Color::srgb(0.2, 0.8, 1.0), true, false, true),
        "Platform" => p(
            200.0,
            20.0,
            Color::srgb(0.45, 0.35, 0.3),
            false,
            true,
            false,
        ),
        "Coin" => p(20.0, 20.0, Color::srgb(1.0, 0.85, 0.2), true, false, true),
        "Enemy" => p(36.0, 36.0, Color::srgb(0.9, 0.25, 0.3), true, false, true),
        "Bullet" => p(12.0, 6.0, Color::srgb(1.0, 1.0, 0.9), false, false, true),
        "Goal" => p(40.0, 64.0, Color::srgb(0.3, 0.9, 0.4), false, false, false),
        "Spike" => p(32.0, 16.0, Color::srgb(0.7, 0.7, 0.75), false, false, false),
        _ => return None,
    })
}

/// Mesh and material handles for drawing things. Inserted at `Startup` by
/// `AssetsPlugin`, which the headless verifier and `scenario.rs` don't add,
/// so [`Spawner`] reads it as optional.
#[derive(Resource)]
pub struct ThingArt {
    pub cube: Handle<Mesh>,
    pub materials: Vec<(&'static str, Handle<StandardMaterial>)>,
}

pub fn setup_thing_art(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let kinds = [
        "Player", "Platform", "Coin", "Enemy", "Bullet", "Goal", "Spike",
    ];
    let materials = kinds
        .iter()
        .filter_map(|k| {
            let p = props(k)?;
            Some((
                *k,
                materials.add(StandardMaterial {
                    base_color: p.color,
                    ..default()
                }),
            ))
        })
        .collect();
    commands.insert_resource(ThingArt {
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        materials,
    });
}

/// The one way to spawn a thing. Use it as a system parameter:
///
/// ```ignore
/// fn fire(mut spawner: Spawner, player: Single<&Transform, With<Player>>) {
///     spawner.spawn_moving("Bullet", player.translation.truncate(), Vec2::X * BULLET_SPEED);
/// }
/// ```
#[derive(SystemParam)]
pub struct Spawner<'w, 's> {
    pub commands: Commands<'w, 's>,
    pub counter: ResMut<'w, SpawnCounter>,
    pub art: Option<Res<'w, ThingArt>>,
}

impl Spawner<'_, '_> {
    /// Spawns a `kind` centred on `at` with its default size.
    pub fn spawn(&mut self, kind: &str, at: Vec2) -> Option<Entity> {
        self.spawn_sized(kind, at, None)
    }

    /// Spawns a `kind` already moving at `vel` px/s.
    pub fn spawn_moving(&mut self, kind: &str, at: Vec2, vel: Vec2) -> Option<Entity> {
        let e = self.spawn_sized(kind, at, None)?;
        self.commands
            .entity(e)
            .entry::<Body>()
            .and_modify(move |mut b| b.vel = vel);
        Some(e)
    }

    /// Spawns a `kind` at `at`, with `size` instead of the default when given
    /// (platforms come in many sizes). Unknown kinds spawn nothing.
    pub fn spawn_sized(&mut self, kind: &str, at: Vec2, size: Option<Vec2>) -> Option<Entity> {
        let p = props(kind)?;
        let size = size.unwrap_or(p.size);
        let mut e = self.commands.spawn((
            Thing,
            Name::new(kind.to_string()),
            GameEntity,
            self.counter.stamp(),
            Size(size),
            Transform::from_translation(at.extend(0.0)),
        ));
        if p.moves {
            e.insert(Body {
                gravity: p.gravity,
                ..default()
            });
        }
        if p.solid {
            e.insert(Solid);
        }
        match kind {
            "Player" => e.insert((Player, Facing(1.0))),
            "Platform" => e.insert(Platform),
            "Coin" => e.insert(Coin),
            "Enemy" => e.insert(Enemy),
            "Bullet" => e.insert(Bullet),
            "Goal" => e.insert(Goal),
            "Spike" => e.insert(Spike),
            _ => &mut e,
        };
        let id = e.id();
        if let Some(art) = &self.art
            && let Some((_, mat)) = art.materials.iter().find(|(k, _)| *k == kind)
        {
            // The mesh is a unit cube scaled by a child, so the thing's own
            // Transform stays the sim's (scale 1) and nothing the sim reads
            // depends on whether the game is windowed.
            let (mesh, mat) = (art.cube.clone(), mat.clone());
            self.commands
                .entity(id)
                .insert(Visibility::default())
                .with_child((
                    Mesh3d(mesh),
                    MeshMaterial3d(mat),
                    Transform::from_scale(size.extend(20.0)),
                ));
        }
        Some(id)
    }
}

/// True when the boxes of two things overlap (touching edges don't count).
pub fn overlaps(a_pos: Vec2, a_size: Vec2, b_pos: Vec2, b_size: Vec2) -> bool {
    let d = (a_pos - b_pos).abs();
    let reach = (a_size + b_size) * 0.5;
    d.x < reach.x && d.y < reach.y
}

/// Folds the run state Coin Run adds to the template into the replay
/// checksum (determinism rule 6): lives and coins, then every thing's
/// position in `SpawnOrder`. Registered right after `sim::checksum_tick`.
pub fn checksum_things(
    data: Res<GameData>,
    things: Query<(&SpawnOrder, &Transform), With<Thing>>,
    mut sum: ResMut<Checksum>,
) {
    sum.fold(u64::from(data.lives));
    sum.fold(u64::from(data.coins));
    let mut all: Vec<_> = things.iter().collect();
    all.sort_unstable_by_key(|(order, _)| **order);
    for (order, tf) in all {
        sum.fold(order.0);
        sum.fold(u64::from(tf.translation.x.to_bits()));
        sum.fold(u64::from(tf.translation.y.to_bits()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_has_props() {
        for k in [
            "Player", "Platform", "Coin", "Enemy", "Bullet", "Goal", "Spike",
        ] {
            assert!(props(k).is_some(), "{k}");
        }
        assert!(props("Dragon").is_none());
    }

    #[test]
    fn overlap_needs_area_not_just_touching_edges() {
        let s = Vec2::splat(10.0);
        assert!(overlaps(Vec2::ZERO, s, Vec2::new(9.0, 0.0), s));
        assert!(!overlaps(Vec2::ZERO, s, Vec2::new(10.0, 0.0), s));
    }
}
