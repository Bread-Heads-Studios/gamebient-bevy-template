//! Drives the fixed-tick input path from a decoded replay. Only the
//! verifier inserts this resource.

use bevy::prelude::*;
use gamebient_input::{Buttons, InputAccumulator, input::dequantize_axis};

use super::{Replay, TickRun, TuneRecord};
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

/// `FixedPreUpdate`, `TickInputSet::Feed`: overwrite the accumulator with
/// this tick's record. Past the end, feed nothing (the accumulator is
/// cleared so a stale hold cannot leak). `Buttons::PAUSE` is masked out
/// defensively: current replays never carry it (`ReplayRecorder::push`
/// strips it before it's ever written), but an older file that does must
/// not be able to re-press pause and freeze `SimSet` mid-verify.
pub fn feed_tick(
    mut feeder: ResMut<ReplayFeeder>,
    mut acc: ResMut<InputAccumulator>,
    tick: Res<SimTick>,
    idx: Res<TunableIndex>,
    mut pending: ResMut<PendingTunes>,
) {
    // The tick about to be simulated is `SimTick + 1` (`advance_tick` runs
    // first in SimSet); a change recorded on it is queued now and lands in
    // `apply_pending_tunes`, right where the recording game applied it.
    let next = tick.0 + 1;
    while let Some(t) = feeder.tunes.get(feeder.tune_at)
        && t.tick <= next
    {
        if let Some(entry) = idx.by_id(t.id)
            && let Some(v) = Value::from_bytes(entry.tune.kind, &t.bytes)
        {
            pending.0.push((t.id, v));
        }
        feeder.tune_at += 1;
    }
    match feeder.next() {
        Some(r) => {
            acc.held = Buttons(u32::from(r.held)).difference(Buttons::PAUSE);
            acc.latched = Buttons(u32::from(r.latched)).difference(Buttons::PAUSE);
            acc.axis = dequantize_axis(r.ax, r.ay);
        }
        None => *acc = InputAccumulator::default(),
    }
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

    #[test]
    fn a_recorded_change_is_queued_on_its_tick() {
        use crate::game::replay::build_headless_app;
        use crate::game::sim::SimTick;
        use gamebient_input::TickInputSet;
        let mut app = build_headless_app();
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
                id: 0,
                bytes: Value::Int(3).to_bytes(),
            }],
            runs: Vec::new(),
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
            vec![(0, Value::Int(3))]
        );
        assert_eq!(app.world().resource::<ReplayFeeder>().tune_at, 1);
    }
}
