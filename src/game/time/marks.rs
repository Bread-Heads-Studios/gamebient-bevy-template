//! Marks: the moments the timeline shows. A counter in `rule_table::MARKS`
//! falling or rising, a change or a rules swap applied, the run ending.
//! Only simulated ticks are seen (this runs in `SimSet`); a mark already
//! in the run's list is never sent again, and nothing is sent while a seek
//! re-simulates.

use bevy::prelude::*;
use gamebient_input::HostEvent;

use super::seek::Seek;
use super::{Mark, TimeTravel, custom};
use crate::game::rule_table::{COUNTERS, MARKS};
use crate::game::rules::{MarkWhen, Outcome, RulesLog, RunEnd};
use crate::game::scoring::GameData;
use crate::game::sim::{RunOver, SimTick};
use crate::game::tunables::TuneApplied;

pub const SETTING_CHANGED: &str = "Changed a setting";
pub const RULES_CHANGED: &str = "Changed the rules";
pub const WON: &str = "Won the run";
pub const LOST: &str = "Lost the run";
pub const ENDED: &str = "The run ended";

/// What the last tick looked like, for telling what changed.
#[derive(Resource, Debug, Default)]
pub struct MarkWatch {
    pub last: Vec<i64>,
    pub rules: usize,
    pub ended: bool,
}

/// Each `MARKS` counter's value, in `MARKS` order.
pub fn counter_values(data: &GameData) -> Vec<i64> {
    MARKS
        .iter()
        .map(|m| {
            COUNTERS
                .iter()
                .find(|c| c.name == m.counter)
                .map_or(0, |c| (c.get)(data))
        })
        .collect()
}

/// The counter marks between two ticks' values.
pub fn counter_marks(tick: u32, last: &[i64], now: &[i64]) -> Vec<Mark> {
    MARKS
        .iter()
        .zip(last.iter().zip(now))
        .filter_map(|(m, (a, b))| {
            let fired = match m.when {
                MarkWhen::Falls => b < a,
                MarkWhen::Rises => b > a,
            };
            fired.then(|| Mark::new(tick, m.counter, m.label))
        })
        .collect()
}

/// In `BeginRun`, after the run's rules are set up: the run starts with
/// nothing changed.
pub fn reset_mark_watch(data: Res<GameData>, log: Res<RulesLog>, mut watch: ResMut<MarkWatch>) {
    *watch = MarkWatch {
        last: counter_values(&data),
        rules: log.0.len(),
        ended: false,
    };
}

/// In `SimSet`, after `record_tick`.
pub fn detect_marks(
    tick: Res<SimTick>,
    data: Res<GameData>,
    log: Res<RulesLog>,
    over: Res<RunOver>,
    end: Res<RunEnd>,
    mut applied: MessageReader<TuneApplied>,
    mut watch: ResMut<MarkWatch>,
    mut tt: ResMut<TimeTravel>,
    seek: Option<Res<Seek>>,
    mut out: MessageWriter<HostEvent>,
) {
    let t = tick.0;
    let now = counter_values(&data);
    let mut found = if watch.last.len() == now.len() {
        counter_marks(t, &watch.last, &now)
    } else {
        Vec::new()
    };
    watch.last = now;
    if applied.read().filter(|a| a.ok && a.tick == Some(t)).count() > 0 {
        found.push(Mark::new(t, "edit", SETTING_CHANGED));
    }
    if log.0.len() > watch.rules {
        watch.rules = log.0.len();
        found.push(Mark::new(t, "edit", RULES_CHANGED));
    }
    if over.0 && !watch.ended {
        watch.ended = true;
        let label = match end.0 {
            Some(Outcome::Win) => WON,
            Some(Outcome::Lose) => LOST,
            None => ENDED,
        };
        found.push(Mark::new(t, "end", label));
    }
    for m in found {
        if tt.marks.contains(&m) {
            continue;
        }
        if seek.is_none() {
            out.write(custom("time.mark", m.to_json()));
        }
        tt.marks.push(m);
    }
}
