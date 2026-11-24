//! `run_rules`: the one system that runs a game's rules, in `SimSet` after
//! physics and after the game's own rules; and `begin_run_rules`, which sets
//! a run up on `OnEnter(Playing)` (after `spawn_level`).
//!
//! Determinism (docs/replay-verification.md):
//! - Rules run in file order. A `touch` rule visits pairs in `SpawnOrder`
//!   (each A, then each B), from where things stood when the pass began.
//! - `chance` and `random` places draw from `GameRng`, in that order.
//! - Time is `RunClock` ticks only; timers count ticks.
//! - Spawns, removals and respawns queue and apply at the end of the pass;
//!   counter changes, moves, jumps, sounds, timers and `end_run` apply at
//!   once, in order.
//! - At most `ACTION_CAP` actions run in one tick; the rest wait for the
//!   next tick, first in line (at most `MAX_WAITING` firings wait; more are
//!   dropped, so a rule that fires without end cannot grow memory).
//! - Counter values (in `COUNTERS` order) and running timers (in name
//!   order) fold into the `Checksum` after every pass. A run with no rules
//!   does none of this, so it plays and checksums exactly as before.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use bevy::ecs::system::SystemState;
use bevy::prelude::*;
use rand::RngCore;

use super::model::{Act, Btn, Cond, Edge, Place, Rule, RulesFile, Target, Trigger};
use super::{CounterDef, Outcome, PendingRules, Rules, RulesLog, RunEnd};
use crate::game::audio::SfxEvent;
use crate::game::content::files::fnv1a64;
use crate::game::content::kit::{Kinds, Size, Thing};
use crate::game::content::level::{LevelSource, spawn_thing};
use crate::game::content::model::LEVEL_MAX_THINGS;
use crate::game::replay::RulesRecord;
use crate::game::rule_table;
use crate::game::scoring::GameData;
use crate::game::sim::{self, Checksum, GameRng, RunClock, RunOver, SimTick, SpawnOrder};
use crate::game::states::GameState;
use crate::ui::transition::ScreenFade;

/// The most actions one tick runs.
pub const ACTION_CAP: usize = 4096;
/// The most firings that wait for the next tick.
pub const MAX_WAITING: usize = 4096;
/// The most pairs one `touch` rule visits in a tick.
pub const MAX_PAIRS: usize = 65_536;

/// One rule firing: its actions from `next` on, and the touching pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Firing {
    pub rule: usize,
    pub next: usize,
    pub me: Option<Entity>,
    pub other: Option<Entity>,
}

/// The run's rules and their state, set by [`begin_run_rules`].
#[derive(Resource, Debug, Default)]
pub struct RunRules {
    pub file: RulesFile,
    /// Per rule: whether its `counter` comparison held last tick.
    pub memory: Vec<bool>,
    /// Running timers: ticks left, by name.
    pub timers: BTreeMap<String, u32>,
    /// Where each thing started, by `SpawnOrder`: its kind and place.
    pub starts: BTreeMap<u64, (u16, Vec2)>,
    /// Firings past last tick's cap, run first this tick.
    pub waiting: VecDeque<Firing>,
}

fn counter(name: &str) -> Option<&'static CounterDef> {
    rule_table::COUNTERS.iter().find(|c| c.name == name)
}

fn read(world: &World, name: &str) -> Option<i64> {
    let c = counter(name)?;
    world.get_resource::<GameData>().map(|d| (c.get)(d))
}

/// Changes a counter, clamped to its range.
fn change(world: &mut World, name: &str, f: impl FnOnce(i64) -> i64) {
    let Some(c) = counter(name) else {
        return;
    };
    let Some(mut d) = world.get_resource_mut::<GameData>() else {
        return;
    };
    let v = f((c.get)(&d)).clamp(c.min, c.max);
    (c.set)(&mut d, v);
}

fn overlaps(a: Vec2, a_size: Vec2, b: Vec2, b_size: Vec2) -> bool {
    let d = (a - b).abs();
    let r = (a_size + b_size) * 0.5;
    d.x < r.x && d.y < r.y
}

/// A number in [0, 1) from the run's RNG.
fn unit(world: &mut World) -> f32 {
    world
        .get_resource_mut::<GameRng>()
        .map_or(0.5, |mut r| r.0.next_u32() as f32 / 4_294_967_296.0)
}

/// A 1–100 roll from the run's RNG.
fn roll(world: &mut World) -> u32 {
    world
        .get_resource_mut::<GameRng>()
        .map_or(100, |mut r| r.0.next_u32() % 100 + 1)
}

fn conditions_hold(world: &mut World, rule: &Rule) -> bool {
    rule.conds.iter().all(|c| match c {
        Cond::Compare(name, op, v) => read(world, name).is_some_and(|x| op.holds(x, *v)),
        Cond::Chance(p) => roll(world) <= *p,
    })
}

/// Where the first placed thing of `kind` started.
fn kind_start(state: &RunRules, kind: u16) -> Option<Vec2> {
    state
        .starts
        .values()
        .find(|(k, _)| *k == kind)
        .map(|(_, p)| *p)
}

/// `OnEnter(Playing)`, chained after `content::level::spawn_level`: the
/// run's rules (the game's `test_rules()` under `LevelSource::Test`, else
/// the content's), fresh edge memory and timers, every thing's start.
pub fn begin_run_rules(world: &mut World) {
    let test = world
        .get_resource::<LevelSource>()
        .is_some_and(|s| *s == LevelSource::Test);
    let file = if test {
        rule_table::test_rules()
    } else {
        world
            .get_resource::<Rules>()
            .map(|r| r.0.clone())
            .unwrap_or_default()
    };
    let mut q = world.query::<(&SpawnOrder, &Thing, &Transform)>();
    let starts = q
        .iter(world)
        .map(|(o, t, tf)| (o.0, (t.kind, tf.translation.truncate())))
        .collect();
    let memory = vec![false; file.rules.len()];
    world.insert_resource(RunRules {
        file,
        memory,
        timers: BTreeMap::new(),
        starts,
        waiting: VecDeque::new(),
    });
    world.insert_resource(PendingRules::default());
    world.insert_resource(RulesLog::default());
    world.insert_resource(RunEnd::default());
}

/// Swaps in a file mid-run. A rule that is unchanged (same id, same
/// content) keeps its edge memory; a changed one starts fresh. Timers whose
/// names are gone are dropped; waiting firings are dropped.
pub fn swap(state: &mut RunRules, file: RulesFile) {
    let memory = file
        .rules
        .iter()
        .map(|r| {
            state
                .file
                .rules
                .iter()
                .position(|o| o == r)
                .and_then(|i| state.memory.get(i).copied())
                .unwrap_or(false)
        })
        .collect();
    let mut names = BTreeSet::new();
    for r in &file.rules {
        if let Trigger::Timer(n) = &r.when {
            names.insert(n.clone());
        }
        for a in &r.acts {
            if let Act::Timer(n, _) = a {
                names.insert(n.clone());
            }
        }
    }
    state.timers.retain(|n, _| names.contains(n));
    state.waiting.clear();
    state.memory = memory;
    state.file = file;
}

/// In `SimSet`: one pass of the run's rules.
pub fn run_rules(world: &mut World) {
    let swapped = world
        .get_resource_mut::<PendingRules>()
        .and_then(|mut p| p.0.take());
    let Some(mut state) = world.remove_resource::<RunRules>() else {
        return;
    };
    if let Some(incoming) = swapped {
        // Logged with the tick it lands on, so a replay (and a seek) can
        // hand the same bytes to the same tick.
        let tick = world.get_resource::<SimTick>().map_or(0, |t| t.0);
        if let Some(mut log) = world.get_resource_mut::<RulesLog>() {
            log.0.push(RulesRecord {
                tick,
                bytes: incoming.bytes,
            });
        }
        swap(&mut state, incoming.file);
        if let Some(mut sum) = world.get_resource_mut::<Checksum>() {
            sum.fold(incoming.hash);
        }
    }
    if !state.file.rules.is_empty() || !state.waiting.is_empty() {
        let mut pass = Pass::new(world);
        pass.run(world, &mut state);
        pass.finish(world, &mut state);
        fold(world, &state);
    }
    world.insert_resource(state);
}

/// Folds every counter and running timer into the checksum.
fn fold(world: &mut World, state: &RunRules) {
    let values: Vec<i64> = match world.get_resource::<GameData>() {
        Some(d) => rule_table::COUNTERS.iter().map(|c| (c.get)(d)).collect(),
        None => Vec::new(),
    };
    let Some(mut sum) = world.get_resource_mut::<Checksum>() else {
        return;
    };
    for v in values {
        sum.fold(v as u64);
    }
    for (name, left) in &state.timers {
        sum.fold(fnv1a64(name.as_bytes()));
        sum.fold(u64::from(*left));
    }
}

/// `end_run`: says how the run ended (the first rule to say wins), then
/// ends it the one way the sim ends runs (`sim::end_run`).
fn end(world: &mut World, win: bool) {
    if let Some(mut r) = world.get_resource_mut::<RunEnd>()
        && r.0.is_none()
    {
        r.0 = Some(if win { Outcome::Win } else { Outcome::Lose });
    }
    if world.get_resource::<RunOver>().is_none_or(|o| o.0)
        || !world.contains_resource::<NextState<GameState>>()
    {
        return;
    }
    let mut st: SystemState<(
        ResMut<RunOver>,
        Option<ResMut<ScreenFade>>,
        ResMut<NextState<GameState>>,
    )> = SystemState::new(world);
    let (mut over, fade, mut next) = st.get_mut(world);
    sim::end_run(&mut over, fade, &mut next);
    st.apply(world);
}

/// A thing as the pass found it.
struct Live {
    e: Entity,
    kind: u16,
    at: Vec2,
    size: Vec2,
}

/// One tick's pass: what it read at the start, and what it queued.
struct Pass {
    ticks: u64,
    /// A pressed, A held, B pressed, B held.
    buttons: [bool; 4],
    kinds: Vec<String>,
    /// In `SpawnOrder`.
    things: Vec<Live>,
    removed: Vec<Entity>,
    gone: BTreeSet<Entity>,
    respawns: Vec<Entity>,
    respawned: BTreeSet<Entity>,
    spawns: Vec<(String, Vec2)>,
    rang: BTreeSet<String>,
    budget: usize,
}

impl Pass {
    fn new(world: &mut World) -> Pass {
        let ticks = world.get_resource::<RunClock>().map_or(0, |c| c.ticks);
        let buttons = world
            .get_resource::<gamebient_input::TickInput>()
            .map(|i| {
                [
                    i.primary_just_pressed,
                    i.primary_held,
                    i.secondary_just_pressed,
                    i.secondary_held,
                ]
            })
            .unwrap_or_default();
        let kinds = world
            .get_resource::<Kinds>()
            .map(|k| k.0.iter().map(|(n, _)| n.clone()).collect())
            .unwrap_or_default();
        let mut q = world.query::<(Entity, &SpawnOrder, &Thing, &Transform, &Size)>();
        let mut found: Vec<(u64, Live)> = q
            .iter(world)
            .map(|(e, o, t, tf, s)| {
                (
                    o.0,
                    Live {
                        e,
                        kind: t.kind,
                        at: tf.translation.truncate(),
                        size: s.0,
                    },
                )
            })
            .collect();
        found.sort_unstable_by_key(|(o, _)| *o);
        Pass {
            ticks,
            buttons,
            kinds,
            things: found.into_iter().map(|(_, l)| l).collect(),
            removed: Vec::new(),
            gone: BTreeSet::new(),
            respawns: Vec::new(),
            respawned: BTreeSet::new(),
            spawns: Vec::new(),
            rang: BTreeSet::new(),
            budget: ACTION_CAP,
        }
    }

    fn kind_index(&self, name: &str) -> Option<u16> {
        self.kinds
            .iter()
            .position(|k| k == name)
            .and_then(|i| u16::try_from(i).ok())
    }

    fn run(&mut self, world: &mut World, state: &mut RunRules) {
        for (name, left) in state.timers.iter_mut() {
            *left = left.saturating_sub(1);
            if *left == 0 {
                self.rang.insert(name.clone());
            }
        }
        state.timers.retain(|_, left| *left > 0);
        let carried: Vec<Firing> = state.waiting.drain(..).collect();
        for f in carried {
            self.exec(world, state, f);
        }
        for i in 0..state.file.rules.len() {
            let rule = state.file.rules[i].clone();
            if rule.off {
                continue;
            }
            for (me, other) in self.fire(world, state, i, &rule) {
                if conditions_hold(world, &rule) {
                    self.exec(
                        world,
                        state,
                        Firing {
                            rule: i,
                            next: 0,
                            me,
                            other,
                        },
                    );
                }
            }
        }
    }

    /// The firings `rule` makes this tick: one with no pair, or one per
    /// touching pair.
    fn fire(
        &self,
        world: &World,
        state: &mut RunRules,
        i: usize,
        rule: &Rule,
    ) -> Vec<(Option<Entity>, Option<Entity>)> {
        let once = vec![(None, None)];
        let yes = match &rule.when {
            Trigger::Start => self.ticks == 1,
            Trigger::Every(n) => {
                *n > 0 && self.ticks > 0 && self.ticks.is_multiple_of(u64::from(*n))
            }
            Trigger::Button(b, e) => match (b, e) {
                (Btn::A, Edge::Pressed) => self.buttons[0],
                (Btn::A, Edge::Held) => self.buttons[1],
                (Btn::B, Edge::Pressed) => self.buttons[2],
                (Btn::B, Edge::Held) => self.buttons[3],
            },
            Trigger::Touch(a, b) => return self.pairs(a, b),
            Trigger::Counter(c, op, v) => {
                let now = read(world, c).is_some_and(|x| op.holds(x, *v));
                let was = state.memory.get(i).copied().unwrap_or(false);
                if let Some(m) = state.memory.get_mut(i) {
                    *m = now;
                }
                now && !was
            }
            Trigger::Timer(n) => self.rang.contains(n),
        };
        if yes { once } else { Vec::new() }
    }

    fn pairs(&self, a: &str, b: &str) -> Vec<(Option<Entity>, Option<Entity>)> {
        let (Some(ka), Some(kb)) = (self.kind_index(a), self.kind_index(b)) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for x in self
            .things
            .iter()
            .filter(|t| t.kind == ka && !self.gone.contains(&t.e))
        {
            for y in self
                .things
                .iter()
                .filter(|t| t.kind == kb && t.e != x.e && !self.gone.contains(&t.e))
            {
                if overlaps(x.at, x.size, y.at, y.size) {
                    out.push((Some(x.e), Some(y.e)));
                    if out.len() >= MAX_PAIRS {
                        return out;
                    }
                }
            }
        }
        out
    }

    fn exec(&mut self, world: &mut World, state: &mut RunRules, f: Firing) {
        // Out of budget: queue the firing (or drop it, queue full) before
        // cloning its rule, which a crowd of touching pairs would otherwise
        // do hundreds of thousands of times a tick.
        if self.budget == 0 {
            let has_acts = state
                .file
                .rules
                .get(f.rule)
                .is_some_and(|r| f.next < r.acts.len());
            if has_acts && state.waiting.len() < MAX_WAITING {
                state.waiting.push_back(f);
            }
            return;
        }
        let Some(rule) = state.file.rules.get(f.rule).cloned() else {
            return;
        };
        for (j, act) in rule.acts.iter().enumerate().skip(f.next) {
            if self.budget == 0 {
                if state.waiting.len() < MAX_WAITING {
                    state.waiting.push_back(Firing { next: j, ..f });
                }
                return;
            }
            self.budget -= 1;
            self.act(world, state, act, &f);
        }
    }

    fn targets(&self, t: &Target, f: &Firing) -> Vec<Entity> {
        let alive = |e: &Entity| !self.gone.contains(e);
        match t {
            Target::Me => f.me.filter(alive).into_iter().collect(),
            Target::Other => f.other.filter(alive).into_iter().collect(),
            Target::Kind(k) => match self.kind_index(k) {
                Some(ki) => self
                    .things
                    .iter()
                    .filter(|l| l.kind == ki && alive(&l.e))
                    .map(|l| l.e)
                    .collect(),
                None => Vec::new(),
            },
        }
    }

    fn place(
        &self,
        world: &mut World,
        state: &RunRules,
        kind: &str,
        place: &Place,
        f: &Firing,
    ) -> Option<Vec2> {
        match place {
            Place::Start => self.kind_index(kind).and_then(|k| kind_start(state, k)),
            Place::Random => {
                let h = rule_table::HALF_SCREEN;
                let x = unit(world);
                let y = unit(world);
                Some(Vec2::new(-h.x + x * 2.0 * h.x, -h.y + y * 2.0 * h.y))
            }
            Place::Me => {
                f.me.and_then(|e| world.get::<Transform>(e))
                    .map(|tf| tf.translation.truncate())
            }
            Place::At(x, y) => Some(Vec2::new(*x, *y)),
        }
    }

    fn act(&mut self, world: &mut World, state: &mut RunRules, act: &Act, f: &Firing) {
        match act {
            Act::Add(c, n) => change(world, c, |v| v.saturating_add(*n)),
            Act::Set(c, n) => change(world, c, |_| *n),
            Act::Remove(t) => {
                for e in self.targets(t, f) {
                    if self.gone.insert(e) {
                        self.removed.push(e);
                    }
                }
            }
            Act::Spawn(kind, place) => {
                if self.things.len() + self.spawns.len() < LEVEL_MAX_THINGS
                    && let Some(at) = self.place(world, state, kind, place, f)
                {
                    self.spawns.push((kind.clone(), at));
                }
            }
            Act::Respawn(t) => {
                for e in self.targets(t, f) {
                    if self.respawned.insert(e) {
                        self.respawns.push(e);
                    }
                }
            }
            Act::Jump(t) => {
                if let Some(jump) = rule_table::JUMP {
                    for e in self.targets(t, f) {
                        jump(world, e);
                    }
                }
            }
            Act::Move(t, dx, dy) => {
                for e in self.targets(t, f) {
                    if let Some(mut tf) = world.get_mut::<Transform>(e) {
                        tf.translation.x += dx;
                        tf.translation.y += dy;
                    }
                }
            }
            Act::Sound(s) => {
                if let Some(ev) = SfxEvent::from_name(s) {
                    world.write_message(ev);
                }
            }
            Act::Timer(n, t) => {
                state.timers.insert(n.clone(), *t);
            }
            Act::EndRun(win) => end(world, *win),
        }
    }

    /// The end of the pass: respawns, then removals, then spawns.
    fn finish(&mut self, world: &mut World, state: &mut RunRules) {
        for e in &self.respawns {
            if self.gone.contains(e) {
                continue;
            }
            let order = world.get::<SpawnOrder>(*e).map(|o| o.0);
            let kind = world.get::<Thing>(*e).map(|t| t.kind);
            let at = order
                .and_then(|o| state.starts.get(&o).map(|(_, p)| *p))
                .or_else(|| kind.and_then(|k| kind_start(state, k)));
            if let Some(at) = at
                && let Some(mut tf) = world.get_mut::<Transform>(*e)
            {
                tf.translation.x = at.x;
                tf.translation.y = at.y;
            }
            if let Some(stop) = rule_table::STOP {
                stop(world, *e);
            }
        }
        for e in &self.removed {
            if let Ok(ent) = world.get_entity_mut(*e) {
                ent.despawn();
            }
        }
        for (kind, at) in std::mem::take(&mut self.spawns) {
            if let Some(e) = spawn_thing(world, &kind, at)
                && let (Some(o), Some(t)) = (world.get::<SpawnOrder>(e), world.get::<Thing>(e))
            {
                state.starts.insert(o.0, (t.kind, at));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::rules::model::{Vocab, check_file};

    fn vocab() -> Vocab {
        Vocab {
            triggers: crate::game::rules::model::TRIGGERS
                .iter()
                .map(|s| s.to_string())
                .collect(),
            conditions: vec!["compare".into(), "chance".into()],
            actions: vec!["add".into(), "timer".into()],
            counters: vec!["score".into()],
            kinds: vec![],
            sounds: vec![],
        }
    }

    #[test]
    fn a_swap_keeps_unchanged_rules_and_drops_gone_timers() {
        let old = check_file(br#"{"v":1,"rules":[{"id":"a","when":{"counter":["score",">",1]},"do":[{"timer":["t",9]}]},{"id":"b","when":{"counter":["score",">",2]},"do":[{"timer":["u",9]}]}]}"#, &vocab()).unwrap();
        let new = check_file(br#"{"v":1,"rules":[{"id":"a","when":{"counter":["score",">",1]},"do":[{"timer":["t",9]}]},{"id":"b","when":{"counter":["score",">",3]},"do":[{"add":["score",1]}]}]}"#, &vocab()).unwrap();
        let mut st = RunRules {
            file: old,
            memory: vec![true, true],
            timers: [("t".to_string(), 3), ("u".to_string(), 4)]
                .into_iter()
                .collect(),
            starts: BTreeMap::new(),
            waiting: VecDeque::from([Firing {
                rule: 1,
                next: 0,
                me: None,
                other: None,
            }]),
        };
        swap(&mut st, new.clone());
        assert_eq!(
            st.memory,
            vec![true, false],
            "a keeps its edge, the changed b starts fresh"
        );
        assert_eq!(
            st.timers.keys().cloned().collect::<Vec<_>>(),
            vec!["t".to_string()]
        );
        assert!(st.waiting.is_empty());
        assert_eq!(st.file, new);
    }
}
