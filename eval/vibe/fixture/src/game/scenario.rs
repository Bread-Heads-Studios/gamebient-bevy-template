//! Headless test harness: set up a little stage, play inputs tick by tick,
//! and look at what happened. The level editor uses it to check that a rule
//! does what the person asked, so **keep its public functions working** when
//! you change the game. Things are found by their `Name`, which `Spawner`
//! sets to the kind's name, so spawn everything through `Spawner`.
//!
//! ```ignore
//! let mut s = Scenario::new();          // floor + Player standing at START
//! s.place("Coin", -260.0, -290.0).start();
//! s.hold(Buttons::RIGHT, 30);           // walk right for half a second
//! assert_eq!(s.count("Coin"), 0);
//! assert_eq!(s.coins(), 1);
//! ```
//!
//! One `tick` is one 60 Hz sim step. Positions are game pixels, origin at
//! the screen centre, y up; the floor's top is [`FLOOR_TOP`].

use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
pub use gamebient_input::Buttons;
use gamebient_input::VirtualInput;

use super::audio::SfxEvent;
use super::level::{Level, Placed};
use super::replay::build_headless_app;
use super::scoring::GameData;
use super::sim::{PendingSeed, RunOver, SimTick, SpawnOrder, seed_bytes};
use super::states::GameState;
use super::things::{Body, Spawner, Thing};

/// The top of [`Scenario::new`]'s floor.
pub const FLOOR_TOP: f32 = -300.0;
/// Where [`Scenario::new`] stands the Player (feet on the floor).
pub const START: Vec2 = Vec2::new(-300.0, -276.0);

#[derive(Resource)]
struct ScenarioInput {
    held: Buttons,
    latch: Buttons,
}

/// Every sound effect the run asked for, with the tick it was asked on.
#[derive(Resource, Default)]
pub struct ScenarioLog {
    pub sounds: Vec<(u32, SfxEvent)>,
}

fn drive_input(mut input: ResMut<ScenarioInput>, mut virt: ResMut<VirtualInput>) {
    virt.set_held(input.held);
    virt.latched |= input.latch;
    input.latch = Buttons::NONE;
}

fn log_sounds(
    tick: Res<SimTick>,
    mut sounds: MessageReader<SfxEvent>,
    mut log: ResMut<ScenarioLog>,
) {
    for s in sounds.read() {
        log.sounds.push((tick.0, *s));
    }
}

pub struct Scenario {
    pub app: App,
    level: Vec<Placed>,
    started: bool,
}

impl Default for Scenario {
    fn default() -> Self {
        Self::new()
    }
}

impl Scenario {
    /// A floor across the whole screen (top at [`FLOOR_TOP`]) and the Player
    /// standing on it at [`START`]. Nothing else.
    pub fn new() -> Self {
        let mut s = Self::empty();
        s.level = vec![
            Placed::sized("Platform", 0.0, FLOOR_TOP - 30.0, 960.0, 60.0),
            Placed::new("Player", START.x, START.y),
        ];
        s
    }

    /// The game's own level (`Level::default()`).
    pub fn default_level() -> Self {
        let mut s = Self::empty();
        s.level = Level::default().0;
        s
    }

    /// Nothing at all: no floor, no Player.
    pub fn empty() -> Self {
        let mut app = build_headless_app();
        app.insert_resource(ScenarioInput {
            held: Buttons::NONE,
            latch: Buttons::NONE,
        })
        .init_resource::<ScenarioLog>()
        .add_systems(
            PreUpdate,
            drive_input.before(gamebient_input::input::accumulate_input),
        )
        .add_systems(Last, log_sounds);
        app.world_mut().resource_mut::<PendingSeed>().0 = Some(seed_bytes(7));
        Self {
            app,
            level: Vec::new(),
            started: false,
        }
    }

    /// Uses `seed` for the run's randomness instead of the default.
    pub fn seed(&mut self, seed: u64) -> &mut Self {
        self.app.world_mut().resource_mut::<PendingSeed>().0 = Some(seed_bytes(seed));
        self
    }

    /// Adds a thing. Before [`start`](Self::start) it becomes part of the
    /// level; after, it is spawned right away.
    pub fn place(&mut self, kind: &str, x: f32, y: f32) -> &mut Self {
        self.place_full(kind, Vec2::new(x, y), None, None)
    }

    /// [`place`](Self::place) with a size other than the kind's default.
    pub fn place_sized(&mut self, kind: &str, x: f32, y: f32, w: f32, h: f32) -> &mut Self {
        self.place_full(kind, Vec2::new(x, y), Some(Vec2::new(w, h)), None)
    }

    /// [`place`](Self::place) already moving at (`vx`, `vy`) px/s. Only
    /// kinds that move (have a `Body`) keep the velocity. Placed after the
    /// run has started.
    pub fn place_moving(&mut self, kind: &str, x: f32, y: f32, vx: f32, vy: f32) -> &mut Self {
        if !self.started {
            self.start();
        }
        self.place_full(kind, Vec2::new(x, y), None, Some(Vec2::new(vx, vy)))
    }

    fn place_full(
        &mut self,
        kind: &str,
        at: Vec2,
        size: Option<Vec2>,
        vel: Option<Vec2>,
    ) -> &mut Self {
        if !self.started {
            self.level.push(Placed {
                kind: kind.to_string(),
                at,
                size,
            });
            return self;
        }
        let kind = kind.to_string();
        self.app
            .world_mut()
            .run_system_once(move |mut sp: Spawner| match vel {
                Some(v) => {
                    sp.spawn_moving(&kind, at, v);
                }
                None => {
                    sp.spawn_sized(&kind, at, size);
                }
            })
            .expect("spawning a thing");
        self
    }

    /// Starts the run: the level is placed and the first tick runs.
    pub fn start(&mut self) -> &mut Self {
        if self.started {
            return self;
        }
        self.started = true;
        self.app
            .insert_resource(Level(std::mem::take(&mut self.level)));
        self.app.update();
        self.app
            .world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        self.app.update();
        self
    }

    /// Runs `n` ticks with no buttons held.
    pub fn tick(&mut self, n: u32) -> &mut Self {
        self.start();
        for _ in 0..n {
            self.app.update();
        }
        self
    }

    /// Holds `buttons` (e.g. `Buttons::RIGHT`) for `n` ticks, then lets go.
    pub fn hold(&mut self, buttons: Buttons, n: u32) -> &mut Self {
        self.start();
        self.app.world_mut().resource_mut::<ScenarioInput>().held = buttons;
        self.tick(n);
        self.app.world_mut().resource_mut::<ScenarioInput>().held = Buttons::NONE;
        self
    }

    /// Holds `buttons` until `done` is true or `max` ticks pass, then lets
    /// go. Returns whether `done` became true.
    pub fn hold_until(
        &mut self,
        buttons: Buttons,
        max: u32,
        mut done: impl FnMut(&mut Scenario) -> bool,
    ) -> bool {
        self.start();
        self.app.world_mut().resource_mut::<ScenarioInput>().held = buttons;
        let mut hit = false;
        for _ in 0..max {
            if done(self) {
                hit = true;
                break;
            }
            self.app.update();
        }
        hit = hit || done(self);
        self.app.world_mut().resource_mut::<ScenarioInput>().held = Buttons::NONE;
        hit
    }

    /// Taps `buttons` (pressed for one tick), and runs that tick.
    pub fn press(&mut self, buttons: Buttons) -> &mut Self {
        self.start();
        self.app.world_mut().resource_mut::<ScenarioInput>().latch = buttons;
        self.tick(1)
    }

    fn things(&mut self, kind: &str) -> Vec<(Entity, SpawnOrder, Vec2)> {
        let world = self.app.world_mut();
        let mut q = world.query_filtered::<(Entity, &Name, &SpawnOrder, &Transform), With<Thing>>();
        let mut v: Vec<_> = q
            .iter(world)
            .filter(|(_, n, _, _)| n.as_str() == kind)
            .map(|(e, _, o, tf)| (e, *o, tf.translation.truncate()))
            .collect();
        v.sort_unstable_by_key(|(_, o, _)| *o);
        v
    }

    /// How many things of `kind` exist.
    pub fn count(&mut self, kind: &str) -> usize {
        self.things(kind).len()
    }

    /// Where every `kind` is, oldest first.
    pub fn positions(&mut self, kind: &str) -> Vec<Vec2> {
        self.things(kind).into_iter().map(|(_, _, p)| p).collect()
    }

    /// Where the oldest `kind` is.
    pub fn pos(&mut self, kind: &str) -> Option<Vec2> {
        self.positions(kind).first().copied()
    }

    /// The velocity of the oldest `kind` that moves.
    pub fn velocity(&mut self, kind: &str) -> Option<Vec2> {
        let ids: Vec<Entity> = self.things(kind).into_iter().map(|(e, _, _)| e).collect();
        let world = self.app.world();
        ids.into_iter()
            .find_map(|e| world.get::<Body>(e).map(|b| b.vel))
    }

    /// Moves the oldest `kind` to (`x`, `y`) and stops it.
    pub fn teleport(&mut self, kind: &str, x: f32, y: f32) -> &mut Self {
        self.start();
        if let Some((e, _, _)) = self.things(kind).into_iter().next() {
            let world = self.app.world_mut();
            if let Some(mut tf) = world.get_mut::<Transform>(e) {
                tf.translation.x = x;
                tf.translation.y = y;
            }
            if let Some(mut b) = world.get_mut::<Body>(e) {
                b.vel = Vec2::ZERO;
            }
        }
        self
    }

    /// Removes every `kind` from the world at once.
    pub fn remove_all(&mut self, kind: &str) -> &mut Self {
        self.start();
        for (e, _, _) in self.things(kind) {
            self.app.world_mut().despawn(e);
        }
        self
    }

    fn data(&self) -> &GameData {
        self.app.world().resource::<GameData>()
    }

    pub fn score(&self) -> u32 {
        self.data().score
    }

    pub fn lives(&self) -> u32 {
        self.data().lives
    }

    pub fn coins(&self) -> u32 {
        self.data().coins
    }

    pub fn set_lives(&mut self, lives: u32) -> &mut Self {
        self.start();
        self.app.world_mut().resource_mut::<GameData>().lives = lives;
        self
    }

    /// Ticks since the run began.
    pub fn now(&self) -> u32 {
        self.app.world().resource::<SimTick>().0
    }

    /// True once the run has ended (won or lost): the sim froze, the game
    /// left `Playing`, or it is about to.
    pub fn run_over(&self) -> bool {
        let w = self.app.world();
        w.resource::<RunOver>().0
            || *w.resource::<State<GameState>>().get() != GameState::Playing
            || !matches!(*w.resource::<NextState<GameState>>(), NextState::Unchanged)
    }

    /// Every sound effect asked for so far, with its tick.
    pub fn sounds(&self) -> Vec<(u32, SfxEvent)> {
        self.app.world().resource::<ScenarioLog>().sounds.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_player_stands_on_the_floor_and_walks() {
        let mut s = Scenario::new();
        s.start().tick(30);
        let p = s.pos("Player").unwrap();
        assert!((p.y - START.y).abs() < 0.5, "{p}");
        s.hold(Buttons::RIGHT, 30);
        let q = s.pos("Player").unwrap();
        assert!((q.x - p.x - 120.0).abs() < 5.0, "{p} -> {q}");
    }

    #[test]
    fn coins_fall_and_land_and_nothing_happens_on_touch() {
        let mut s = Scenario::new();
        s.place("Coin", -260.0, -200.0).start().tick(60);
        let c = s.pos("Coin").unwrap();
        assert!((c.y - (FLOOR_TOP + 10.0)).abs() < 0.5, "{c}");
        s.hold(Buttons::RIGHT, 30);
        assert_eq!(s.count("Coin"), 1, "the fixture has no rules yet");
        assert_eq!(s.coins(), 0);
    }

    #[test]
    fn moving_things_move_and_bullets_ignore_gravity() {
        let mut s = Scenario::new();
        s.place_moving("Bullet", 0.0, 0.0, 600.0, 0.0).tick(10);
        let b = s.pos("Bullet").unwrap();
        assert!((b.x - 100.0).abs() < 1.0 && b.y == 0.0, "{b}");
    }

    #[test]
    fn the_default_level_plays_without_ending() {
        let mut s = Scenario::default_level();
        s.hold(Buttons::RIGHT, 120).hold(Buttons::LEFT, 120);
        assert!(!s.run_over());
        assert_eq!(s.count("Coin"), 5);
    }
}
