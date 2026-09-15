//! Drives the fixed-tick input path from a decoded replay. Only the
//! verifier inserts this resource.

use bevy::prelude::*;
use gamebient_input::{Buttons, InputAccumulator, input::dequantize_axis};

use super::{Replay, TickRun};

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
pub fn feed_tick(mut feeder: ResMut<ReplayFeeder>, mut acc: ResMut<InputAccumulator>) {
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
            ticks: 0,
            score: 0,
            checksum: 0,
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
}
