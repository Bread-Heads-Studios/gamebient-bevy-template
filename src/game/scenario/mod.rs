//! Headless test harness: set up a little stage, play inputs tick by tick,
//! and look at what happened. Data checks (`checks_data::run`) and a game's
//! named Rust checks run on it, so **keep its public functions working**
//! when you change the game. Things are found by their kind
//! (`content::kit::Thing`), so spawn everything through the level or the
//! kit (`content::level::spawn_thing`).
//!
//! What is the game's own comes from `rule_table`: the stage (`STAGE`: the
//! player's kind and start, the floor, how far `beside` is), the counters
//! (`COUNTERS`, read and set by name) and `STOP` (a teleport stops the
//! thing). Sounds are `SfxEvent` names.
//!
//! A stage never reads `content/` unless asked: tunables start from the
//! Rust defaults (`saved::reset_to_defaults`), no data rules run unless
//! given ([`Scenario::rules`]), [`Scenario::new`] builds the stage from
//! `STAGE`, [`Scenario::default_level`] runs the built-in `test_level()`
//! (`LevelSource::Test`) and [`Scenario::with_level`] a level it is handed.
//!
//! ```ignore
//! let mut s = Scenario::new();          // STAGE: the floor and the player at START
//! s.place("Coin", START.x + STAGE.beside, START.y).start();
//! s.hold(Buttons::RIGHT, 30);
//! assert_eq!(s.count("Coin"), 0);
//! assert_eq!(s.counter("score"), Some(1));
//! ```
//!
//! One `tick` is one 60 Hz sim step. This folder is copied verbatim into
//! every game (`tools/frame-accept.sh --check-copies`); a game adds its own
//! helpers in its own file with an `impl Scenario` block.

use bevy::prelude::*;
pub use gamebient_input::Buttons;
use gamebient_input::VirtualInput;

use super::audio::SfxEvent;
use super::content::kit::{Kinds, Size, Thing};
use super::content::level::{Level, LevelSource, spawn_thing};
use super::content::model::{LevelFile, PlacedThing};
use super::content::saved::reset_to_defaults;
use super::replay::build_headless_app;
use super::rule_table::{COUNTERS, STAGE, STOP};
use super::rules::model::RulesFile;
use super::rules::{Outcome, Rules, RunEnd};
use super::scoring::GameData;
use super::sim::{PendingSeed, RunOver, SimTick, SpawnOrder, seed_bytes};
use super::states::GameState;
use super::tunables::{PendingTunes, TunableIndex, Value};

/// The floor [`Scenario::new`] lays: a Thing of `kind`, `width` wide and
/// `depth` deep, its top at `StageDef::floor_top`, centred on x = 0.
pub struct FloorDef {
    pub kind: &'static str,
    pub width: f32,
    pub depth: f32,
}

/// A game's stage, in its own world units (`rule_table::STAGE`).
pub struct StageDef {
    /// The kind the player is (one per level).
    pub player: &'static str,
    /// The floor of an empty stage, or none for a game without one.
    pub floor: Option<FloorDef>,
    /// The line Things rest on: a check's `"floor"`.
    pub floor_top: f32,
    /// Where the player stands on an empty stage (feet on the floor).
    pub start: Vec2,
    /// How far a check's `beside` puts a Thing from the player, centre to centre.
    pub beside: f32,
}

/// The top of [`Scenario::new`]'s floor.
pub const FLOOR_TOP: f32 = STAGE.floor_top;
/// Where [`Scenario::new`] stands the player.
pub const START: Vec2 = STAGE.start;

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
    level: Vec<PlacedThing>,
    /// Run on the built-in `test_level()` instead of `level`.
    test_level: bool,
    started: bool,
}

impl Default for Scenario {
    fn default() -> Self {
        Self::new()
    }
}

impl Scenario {
    /// The game's empty stage (`STAGE`): its floor, when it has one, and
    /// the player standing at [`START`]. Nothing else.
    pub fn new() -> Self {
        let mut s = Self::empty();
        if let Some(f) = &STAGE.floor {
            s.level.push(PlacedThing {
                id: "floor".into(),
                kind: f.kind.into(),
                at: [0.0, STAGE.floor_top - f.depth / 2.0],
                size: Some([f.width, f.depth]),
            });
        }
        s.level.push(PlacedThing {
            id: "player".into(),
            kind: STAGE.player.into(),
            at: [START.x, START.y],
            size: None,
        });
        s
    }

    /// The game's own level: the built-in `test_level()`. Things
    /// [`place`](Self::place)d before the start are not added to it.
    pub fn default_level() -> Self {
        let mut s = Self::empty();
        s.test_level = true;
        s
    }

    /// A stage on `file`'s things (a data check's `"level": "current"`).
    pub fn with_level(file: &LevelFile) -> Self {
        let mut s = Self::empty();
        s.level = file.things.clone();
        s
    }

    /// Nothing at all: no floor, no player.
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
        // Built-in test content only: never the saved tunables.
        reset_to_defaults(app.world_mut());
        // No data rules unless a stage asks for them (`rules`).
        app.insert_resource(Rules(RulesFile::default()));
        Self {
            app,
            level: Vec::new(),
            test_level: false,
            started: false,
        }
    }

    /// Sets a tunable (by path, e.g. `"ammo.start"`) before the run starts;
    /// it applies at [`start`](Self::start). Unknown paths are ignored.
    pub fn tune(&mut self, path: &str, value: Value) -> &mut Self {
        let id = self
            .app
            .world()
            .resource::<TunableIndex>()
            .by_path(path)
            .map(|e| e.id);
        if let Some(id) = id {
            self.app
                .world_mut()
                .resource_mut::<PendingTunes>()
                .0
                .push((id, value));
        }
        self
    }

    /// Uses `seed` for the run's randomness instead of the default (7).
    pub fn seed(&mut self, seed: u64) -> &mut Self {
        self.app.world_mut().resource_mut::<PendingSeed>().0 = Some(seed_bytes(seed));
        self
    }

    /// Runs the stage under `file`. Call before [`start`](Self::start).
    pub fn rules(&mut self, file: RulesFile) -> &mut Self {
        self.app.insert_resource(Rules(file));
        self
    }

    /// How the rules ended the run, if they did.
    pub fn outcome(&self) -> Option<Outcome> {
        self.app.world().resource::<RunEnd>().0
    }

    /// Adds a thing with its kind's size. Before [`start`](Self::start) it
    /// becomes part of the level; after, it is spawned right away.
    pub fn place(&mut self, kind: &str, x: f32, y: f32) -> &mut Self {
        self.place_full(kind, Vec2::new(x, y), None)
    }

    /// [`place`](Self::place) with a size other than the kind's default.
    pub fn place_sized(&mut self, kind: &str, x: f32, y: f32, w: f32, h: f32) -> &mut Self {
        self.place_full(kind, Vec2::new(x, y), Some(Vec2::new(w, h)))
    }

    fn place_full(&mut self, kind: &str, at: Vec2, size: Option<Vec2>) -> &mut Self {
        if !self.started {
            let id = format!("placed-{}", self.level.len() + 1);
            self.level.push(PlacedThing {
                id,
                kind: kind.to_string(),
                at: at.into(),
                size: size.map(Into::into),
            });
            return self;
        }
        let world = self.app.world_mut();
        if let Some(e) = spawn_thing(world, kind, at)
            && let Some(size) = size
            && let Some(mut s) = world.get_mut::<Size>(e)
        {
            s.0 = size;
        }
        self
    }

    /// Moves the first `kind` to (`x`, `y`): in the level before the start,
    /// else with [`teleport`](Self::teleport). False when there is none.
    pub fn put(&mut self, kind: &str, x: f32, y: f32) -> bool {
        if !self.started {
            return match self.level.iter_mut().find(|t| t.kind == kind) {
                Some(t) => {
                    t.at = [x, y];
                    true
                }
                None => false,
            };
        }
        if self.entities(kind).is_empty() {
            return false;
        }
        self.teleport(kind, x, y);
        true
    }

    /// Where the first `kind` is: in the level before the start, else in the world.
    pub fn placed_pos(&mut self, kind: &str) -> Option<Vec2> {
        if !self.started {
            return self
                .level
                .iter()
                .find(|t| t.kind == kind)
                .map(|t| Vec2::from(t.at));
        }
        self.pos(kind)
    }

    /// The default size of `kind`, if the game has it.
    pub fn size_of(&self, kind: &str) -> Option<Vec2> {
        let kinds = self.app.world().resource::<Kinds>();
        kinds.props(kinds.index(kind)?).map(|p| p.size)
    }

    /// Starts the run: the level is placed and the first tick runs.
    pub fn start(&mut self) -> &mut Self {
        if self.started {
            return self;
        }
        self.started = true;
        if self.test_level {
            self.app.insert_resource(LevelSource::Test);
        } else {
            self.app.insert_resource(LevelSource::Content);
            self.app.insert_resource(Level(LevelFile {
                v: 1,
                things: std::mem::take(&mut self.level),
            }));
        }
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

    /// Runs one tick with `held` held, then lets go.
    pub fn step(&mut self, held: Buttons) -> &mut Self {
        self.start();
        self.app.world_mut().resource_mut::<ScenarioInput>().held = held;
        self.app.update();
        self.app.world_mut().resource_mut::<ScenarioInput>().held = Buttons::NONE;
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
        let Some(k) = world.resource::<Kinds>().index(kind) else {
            return Vec::new();
        };
        let mut q = world.query::<(Entity, &Thing, &SpawnOrder, &Transform)>();
        let mut v: Vec<_> = q
            .iter(world)
            .filter(|(_, t, _, _)| t.kind == k)
            .map(|(e, _, o, tf)| (e, *o, tf.translation.truncate()))
            .collect();
        v.sort_unstable_by_key(|(_, o, _)| *o);
        v
    }

    /// Every `kind` in the world, oldest first.
    pub fn entities(&mut self, kind: &str) -> Vec<Entity> {
        self.things(kind).into_iter().map(|(e, _, _)| e).collect()
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

    /// Moves the oldest `kind` to (`x`, `y`) and stops it (`rule_table::STOP`).
    pub fn teleport(&mut self, kind: &str, x: f32, y: f32) -> &mut Self {
        self.start();
        if let Some(e) = self.entities(kind).first().copied() {
            let world = self.app.world_mut();
            if let Some(mut tf) = world.get_mut::<Transform>(e) {
                tf.translation.x = x;
                tf.translation.y = y;
            }
            if let Some(stop) = STOP {
                stop(world, e);
            }
        }
        self
    }

    /// Removes every `kind` from the world at once.
    pub fn remove_all(&mut self, kind: &str) -> &mut Self {
        self.start();
        for e in self.entities(kind) {
            self.app.world_mut().despawn(e);
        }
        self
    }

    /// A counter's value by its `rule_table::COUNTERS` name; `None` for a
    /// name the game lacks.
    pub fn counter(&self, name: &str) -> Option<i64> {
        let c = COUNTERS.iter().find(|c| c.name == name)?;
        Some((c.get)(self.app.world().resource::<GameData>()))
    }

    /// Sets a counter by name, clamped to its range. Starts the run first.
    /// A name the game lacks changes nothing.
    pub fn set_counter(&mut self, name: &str, v: i64) -> &mut Self {
        self.start();
        if let Some(c) = COUNTERS.iter().find(|c| c.name == name) {
            let mut data = self.app.world_mut().resource_mut::<GameData>();
            (c.set)(&mut data, v.clamp(c.min, c.max));
        }
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

    /// Whether the sound effect called `name` (`SfxEvent::name`) played.
    pub fn heard(&self, name: &str) -> bool {
        self.app
            .world()
            .resource::<ScenarioLog>()
            .sounds
            .iter()
            .any(|(_, e)| e.name() == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Game-agnostic: the stage places its player (and floor) and nothing
    /// else, and the player stays put with nothing held.
    #[test]
    fn the_empty_stage_holds_only_the_player_and_its_floor() {
        let mut s = Scenario::new();
        s.start().tick(10);
        assert_eq!(s.count(STAGE.player), 1);
        let p = s.pos(STAGE.player).unwrap();
        assert!((p - START).length() < 0.5, "{p}");
        if let Some(f) = &STAGE.floor {
            assert_eq!(s.count(f.kind), 1);
        }
    }

    #[test]
    fn an_unknown_kind_is_never_found_or_spawned() {
        let mut s = Scenario::new();
        s.start();
        assert_eq!(s.count("NoSuchKind"), 0);
        s.place("NoSuchKind", 0.0, 0.0);
        assert_eq!(s.count("NoSuchKind"), 0);
        assert!(!s.put("NoSuchKind", 0.0, 0.0));
    }

    #[test]
    fn every_counter_reads_back_what_was_set() {
        let mut s = Scenario::new();
        for c in COUNTERS {
            s.set_counter(c.name, c.min);
            assert_eq!(s.counter(c.name), Some(c.min), "{}", c.name);
        }
    }
}
