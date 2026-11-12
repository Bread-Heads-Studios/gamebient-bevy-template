//! Where a tunable change lands: the queue anyone may push to, the one
//! system that drains it (inside `SimSet`, so a value changes between two
//! ticks and never during one), and the log a replay carries.

use bevy::prelude::*;

use crate::game::sim::{Checksum, SimTick};
use crate::game::states::GameState;

use super::hash::{content, fold_value};
use super::index::{TunableIndex, run_starts, write};
use super::value::Value;

/// Changes waiting for the next tick boundary (host layer, feeder, tests).
#[derive(Resource, Default, Debug)]
pub struct PendingTunes(pub Vec<(u16, Value)>);

#[derive(Clone, Debug, PartialEq)]
pub struct TuneChange {
    pub tick: u32,
    pub id: u16,
    pub value: Value,
}

/// Start values and mid-run changes of the current (or last) run.
#[derive(Resource, Default, Clone, Debug, PartialEq)]
pub struct TuneLog {
    pub starts: Vec<(u16, Value)>,
    pub changes: Vec<TuneChange>,
}

/// Reported after a change is applied, or refused (host layer, tests).
#[derive(Message, Debug, Clone, PartialEq)]
pub struct TuneApplied {
    pub id: u16,
    pub value: Value,
    /// The sim tick it applied on; `None` outside a run.
    pub tick: Option<u32>,
    /// False when the write was refused (wrong kind, out of range, unknown
    /// option): nothing changed.
    pub ok: bool,
}

/// Drains [`PendingTunes`] in order. In `SimSet` (right after
/// `sim::sync_run_clock`) during a run; in `Update` outside one.
pub fn apply_pending_tunes(world: &mut World) {
    let pending: Vec<(u16, Value)> = core::mem::take(&mut world.resource_mut::<PendingTunes>().0);
    if pending.is_empty() {
        return;
    }
    let playing = *world.resource::<State<GameState>>().get() == GameState::Playing;
    let idx = world.resource::<TunableIndex>().clone();
    for (id, value) in pending {
        let Some(entry) = idx.by_id(id) else {
            continue;
        };
        let tick = playing.then(|| world.resource::<SimTick>().0);
        if !write(world, entry, &value) {
            world.write_message(TuneApplied {
                id,
                value,
                tick,
                ok: false,
            });
            continue;
        }
        if let Some(tick) = tick {
            // Cosmetic values change the look only: no checksum, no log.
            if !entry.tune.cosmetic {
                fold_value(&mut world.resource_mut::<Checksum>(), id, &value);
                world.resource_mut::<TuneLog>().changes.push(TuneChange {
                    tick,
                    id,
                    value: value.clone(),
                });
            }
        }
        world.write_message(TuneApplied {
            id,
            value,
            tick,
            ok: true,
        });
    }
}

/// `OnEnter(Playing)`, chained after `sim::begin_run`: the run's start
/// values become the log's `starts`, and their hash is the first thing the
/// fresh checksum folds.
pub fn begin_run_tunables(world: &mut World) {
    let starts = run_starts(world);
    let log = TuneLog {
        starts,
        changes: Vec::new(),
    };
    let c = content(&log);
    world.resource_mut::<Checksum>().fold(c);
    *world.resource_mut::<TuneLog>() = log;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::replay::build_headless_app;
    use crate::game::sim::{PendingSeed, seed_bytes};
    use crate::game::tunables::read;

    fn playing() -> App {
        let mut app = build_headless_app();
        app.world_mut().resource_mut::<PendingSeed>().0 = Some(seed_bytes(1));
        app.update();
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.update();
        app
    }

    fn first(app: &App) -> super::super::Entry {
        app.world()
            .resource::<TunableIndex>()
            .entries
            .first()
            .cloned()
            .expect("the template registers at least one tunable (RunTuning)")
    }

    fn bump(v: &Value) -> Value {
        match v {
            Value::Float(f) => Value::Float(f + 1.0),
            Value::Int(i) => Value::Int(i + 1),
            Value::Bool(b) => Value::Bool(!b),
            other => other.clone(),
        }
    }

    #[test]
    fn a_change_lands_on_the_next_tick_and_is_logged() {
        let mut app = playing();
        let entry = first(&app);
        let before = read(app.world(), &entry).unwrap();
        let next = bump(&before);
        app.world_mut()
            .resource_mut::<PendingTunes>()
            .0
            .push((entry.id, next.clone()));
        assert_eq!(
            read(app.world(), &entry),
            Some(before),
            "not applied until the tick runs"
        );
        app.update();
        assert_eq!(read(app.world(), &entry), Some(next.clone()));
        let log = app.world().resource::<TuneLog>();
        let tick = app.world().resource::<SimTick>().0;
        assert_eq!(
            log.changes,
            vec![TuneChange {
                tick,
                id: entry.id,
                value: next
            }]
        );
        assert_eq!(
            log.starts.len(),
            app.world().resource::<TunableIndex>().entries.len()
        );
    }

    #[test]
    fn a_change_moves_the_checksum() {
        let mut a = playing();
        let mut b = playing();
        let entry = first(&a);
        let v = bump(&read(a.world(), &entry).unwrap());
        b.world_mut()
            .resource_mut::<PendingTunes>()
            .0
            .push((entry.id, v));
        for _ in 0..5 {
            a.update();
            b.update();
        }
        assert_ne!(
            a.world().resource::<Checksum>().0,
            b.world().resource::<Checksum>().0
        );
    }

    #[test]
    fn outside_a_run_a_change_applies_at_once_and_becomes_a_start_value() {
        let mut app = build_headless_app();
        app.update();
        let entry = first(&app);
        let v = bump(&read(app.world(), &entry).unwrap());
        app.world_mut()
            .resource_mut::<PendingTunes>()
            .0
            .push((entry.id, v.clone()));
        app.update();
        assert_eq!(read(app.world(), &entry), Some(v.clone()));
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.update();
        let log = app.world().resource::<TuneLog>();
        assert_eq!(log.starts[usize::from(entry.id)].1, v);
        assert!(log.changes.is_empty());
    }
}
