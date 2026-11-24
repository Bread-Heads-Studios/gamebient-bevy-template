//! Drives the fixed-tick input path from a decoded replay: its inputs, its
//! tunable changes and its rules swaps, each on its tick. The verifier
//! inserts [`ReplayFeeder`] and runs [`feed_tick`]; the editor's time
//! travel wraps the same feeder (`time::seek::PlaybackFeeder`) and calls
//! [`queue_rules`] and [`queue_tick`] itself.

use bevy::prelude::*;
use gamebient_input::{Buttons, InputAccumulator, input::dequantize_axis};

use super::{Replay, RulesRecord, TickRun, TuneRecord};
use crate::game::rules::model::check_file;
use crate::game::rules::{PendingRules, RulesSwap, rules_hash, vocab};
use crate::game::sim::SimTick;
use crate::game::tunables::{PendingTunes, TunableIndex, Value};

#[derive(Resource, Debug)]
pub struct ReplayFeeder {
    pub runs: Vec<TickRun>,
    pub run: usize,
    pub left: u16,
    /// Ticks not yet handed out; `done` flips when this reaches zero, i.e.
    /// on the tick that consumes the last record, so the verifier stops
    /// after exactly `replay.ticks` ticks.
    pub remaining: u32,
    pub done: bool,
    /// Mid-run tunable changes, in apply order; `tune_at` is the next one.
    pub tunes: Vec<TuneRecord>,
    pub tune_at: usize,
    /// Mid-run rules swaps, in apply order; `rules_at` is the next one.
    pub rules: Vec<RulesRecord>,
    pub rules_at: usize,
}

impl ReplayFeeder {
    pub fn new(replay: &Replay) -> Self {
        let left = replay.runs.first().map_or(0, |r| r.count);
        Self {
            runs: replay.runs.clone(),
            run: 0,
            left,
            remaining: replay.ticks,
            done: replay.ticks == 0,
            tunes: replay.tunes.clone(),
            tune_at: 0,
            rules: replay.rules.clone(),
            rules_at: 0,
        }
    }

    // Named `next` by the task interface, not `Iterator::next`: it takes no
    // iterator adapters and the resource is never iterated over.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<TickRun> {
        if self.remaining == 0 {
            self.done = true;
            return None;
        }
        while self.run < self.runs.len() && self.left == 0 {
            self.run += 1;
            self.left = self.runs.get(self.run).map_or(0, |r| r.count);
        }
        let Some(r) = self.runs.get(self.run) else {
            self.done = true;
            return None;
        };
        self.left -= 1;
        self.remaining -= 1;
        if self.remaining == 0 {
            self.done = true;
        }
        Some(*r)
    }
}

/// This tick's tunable changes onto `PendingTunes`, and its input into the
/// accumulator. The tick about to be simulated is `SimTick + 1`
/// (`advance_tick` runs first in SimSet); a change recorded on it is queued
/// now and lands in `apply_pending_tunes`, right where the recording game
/// applied it. Past the end, feed nothing (the accumulator is cleared so a
/// stale hold cannot leak). `Buttons::PAUSE` is masked out defensively:
/// current replays never carry it (`ReplayRecorder::push` strips it before
/// it's ever written), but an older file that does must not be able to
/// re-press pause and freeze `SimSet` mid-verify.
pub fn queue_tick(world: &mut World, feeder: &mut ReplayFeeder) {
    let next = world.resource::<SimTick>().0 + 1;
    while let Some(t) = feeder.tunes.get(feeder.tune_at)
        && t.tick <= next
    {
        let value = world
            .resource::<TunableIndex>()
            .by_id(t.id)
            .and_then(|entry| Value::from_bytes(entry.tune.kind, &t.bytes));
        if let Some(v) = value {
            world.resource_mut::<PendingTunes>().0.push((t.id, v));
        }
        feeder.tune_at += 1;
    }
    let record = feeder.next();
    let mut acc = world.resource_mut::<InputAccumulator>();
    match record {
        Some(r) => {
            acc.held = Buttons(u32::from(r.held)).difference(Buttons::PAUSE);
            acc.latched = Buttons(u32::from(r.latched)).difference(Buttons::PAUSE);
            acc.axis = dequantize_axis(r.ax, r.ay);
        }
        None => *acc = InputAccumulator::default(),
    }
}

/// A recorded rules swap due on the tick about to be simulated goes onto
/// `PendingRules`, where `rules::run_rules` takes it at the start of that
/// tick, exactly as it took the editor's `content.set`. Bytes this build's
/// checker refuses are skipped: the checksum then differs and the verdict
/// says so.
pub fn queue_rules(world: &mut World, feeder: &mut ReplayFeeder) {
    let next = world.resource::<SimTick>().0 + 1;
    while let Some(r) = feeder.rules.get(feeder.rules_at)
        && r.tick <= next
    {
        let bytes = r.bytes.clone();
        feeder.rules_at += 1;
        let names = vocab(world);
        if let Ok(file) = check_file(&bytes, &names)
            && let Some(mut pending) = world.get_resource_mut::<PendingRules>()
        {
            let hash = rules_hash(&file, &bytes);
            pending.0 = Some(RulesSwap { file, hash, bytes });
        }
    }
}

/// `FixedPreUpdate`, `TickInputSet::Feed`: the verifier's feeder, one tick.
pub fn feed_tick(world: &mut World) {
    world.resource_scope(|world, mut feeder: Mut<ReplayFeeder>| {
        queue_rules(world, &mut feeder);
        queue_tick(world, &mut feeder);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::sim::SeedOrigin;

    #[test]
    fn walks_runs_then_is_done() {
        let mut r = Replay {
            build: String::new(),
            tick_hz: 60,
            seed: [0; 32],
            origin: SeedOrigin::Local,
            content: 0,
            ticks: 0,
            score: 0,
            checksum: 0,
            starts: Vec::new(),
            tunes: Vec::new(),
            runs: Vec::new(),
            rules: Vec::new(),
            edited: false,
        };
        r.push_tick(1, 0, 0, 0);
        r.push_tick(1, 0, 0, 0);
        r.push_tick(2, 4, 127, 0);
        let mut f = ReplayFeeder::new(&r);
        assert_eq!(f.next().unwrap().held, 1);
        assert_eq!(f.next().unwrap().held, 1);
        assert!(!f.done);
        assert_eq!(f.next().unwrap().latched, 4);
        assert!(
            f.done,
            "done flips on the tick that consumes the last record"
        );
        assert!(f.next().is_none());
    }

    /// Generic over the game's first tunable (this file is copied into
    /// every game, which has its own `tuning.rs`).
    #[test]
    fn a_recorded_change_is_queued_on_its_tick() {
        use crate::game::replay::build_headless_app;
        use crate::game::sim::SimTick;
        use gamebient_input::TickInputSet;
        let mut app = build_headless_app();
        let entry = app.world().resource::<TunableIndex>().entries[0].clone();
        let mut r = Replay {
            build: String::new(),
            tick_hz: 60,
            seed: [0; 32],
            origin: SeedOrigin::Local,
            content: 0,
            ticks: 0,
            score: 0,
            checksum: 0,
            starts: Vec::new(),
            tunes: vec![TuneRecord {
                tick: 2,
                id: entry.id,
                bytes: entry.default.to_bytes(),
            }],
            runs: Vec::new(),
            rules: Vec::new(),
            edited: false,
        };
        for _ in 0..3 {
            r.push_tick(0, 0, 0, 0);
        }
        app.insert_resource(ReplayFeeder::new(&r))
            .add_systems(FixedPreUpdate, feed_tick.in_set(TickInputSet::Feed));
        app.update();
        // Not yet: the next tick is 1.
        app.world_mut().resource_mut::<SimTick>().0 = 0;
        app.world_mut().run_system_cached(feed_tick).unwrap();
        assert!(app.world().resource::<PendingTunes>().0.is_empty());
        app.world_mut().resource_mut::<SimTick>().0 = 1;
        app.world_mut().run_system_cached(feed_tick).unwrap();
        assert_eq!(
            app.world().resource::<PendingTunes>().0,
            vec![(entry.id, entry.default.clone())]
        );
        assert_eq!(app.world().resource::<ReplayFeeder>().tune_at, 1);
    }

    #[test]
    fn a_recorded_rules_swap_is_queued_on_its_tick() {
        use crate::game::replay::{RulesRecord, build_headless_app};
        use crate::game::rules::PendingRules;
        let mut app = build_headless_app();
        let bytes = br#"{"v":1,"rules":[]}"#.to_vec();
        let mut r = Replay {
            build: String::new(),
            tick_hz: 60,
            seed: [0; 32],
            origin: SeedOrigin::Local,
            content: 0,
            ticks: 0,
            score: 0,
            checksum: 0,
            starts: Vec::new(),
            tunes: Vec::new(),
            runs: Vec::new(),
            rules: vec![RulesRecord {
                tick: 2,
                bytes: bytes.clone(),
            }],
            edited: false,
        };
        for _ in 0..3 {
            r.push_tick(0, 0, 0, 0);
        }
        app.insert_resource(ReplayFeeder::new(&r));
        app.update();
        app.world_mut().resource_mut::<SimTick>().0 = 0;
        app.world_mut().run_system_cached(feed_tick).unwrap();
        assert!(
            app.world().resource::<PendingRules>().0.is_none(),
            "not yet: the next tick is 1"
        );
        app.world_mut().resource_mut::<SimTick>().0 = 1;
        app.world_mut().run_system_cached(feed_tick).unwrap();
        let swap = app
            .world()
            .resource::<PendingRules>()
            .0
            .clone()
            .expect("queued for tick 2");
        assert_eq!(swap.bytes, bytes);
        assert_eq!(swap.hash, 0, "a file with no rules hashes as no rules");
        assert_eq!(app.world().resource::<ReplayFeeder>().rules_at, 1);
    }
}
