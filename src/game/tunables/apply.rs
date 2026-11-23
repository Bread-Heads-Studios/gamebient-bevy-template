//! Where a tunable change lands: the queue anyone may push to, the one
//! system that drains it (inside `SimSet`, so a value changes between two
//! ticks and never during one), and the log a replay carries.

use bevy::prelude::*;

use crate::game::content::level::{LevelHash, LevelSource, test_level_hash};
use crate::game::rules::{RulesHash, test_rules_hash};
use crate::game::sim::{Checksum, SimTick};
use crate::game::states::GameState;

use super::hash::{content, fold_value, with_rules};
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

/// The level hash, start values and mid-run changes of the current (or
/// last) run.
#[derive(Resource, Default, Clone, Debug, PartialEq)]
pub struct TuneLog {
    /// The run's key: the level hash with the rules hash
    /// (`hash::with_rules`), or the test level's and test rules' under
    /// `LevelSource::Test`; folded first by `hash::content`.
    pub level: u64,
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

/// `OnEnter(Playing)`, chained after `sim::begin_run`: the run's key (the
/// level hash with the rules hash, `hash::with_rules`) and start values
/// become the log's `level` and `starts`, and their hash is the first
/// thing the fresh checksum folds.
pub fn begin_run_tunables(world: &mut World) {
    let starts = run_starts(world);
    let level = match world.get_resource::<LevelSource>() {
        Some(LevelSource::Test) => with_rules(test_level_hash(), test_rules_hash()),
        _ => with_rules(
            world.get_resource::<LevelHash>().map_or(0, |h| h.0),
            world.get_resource::<RulesHash>().map_or(0, |h| h.0),
        ),
    };
    let log = TuneLog {
        level,
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
    use crate::game::replay::{build_builtin_app, build_headless_app};
    use crate::game::sim::{PendingSeed, seed_bytes};
    use crate::game::tunables::index::bumped_in_range;
    use crate::game::tunables::read;

    /// A run on built-in content: the saved values and level in
    /// `content/` never reach these tests.
    fn playing() -> App {
        let mut app = build_builtin_app();
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

    #[test]
    fn a_change_lands_on_the_next_tick_and_is_logged() {
        let mut app = playing();
        let entry = first(&app);
        let before = read(app.world(), &entry).unwrap();
        let next = bumped_in_range(&entry, &before);
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
        let v = bumped_in_range(&entry, &read(a.world(), &entry).unwrap());
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
        let mut app = build_builtin_app();
        app.update();
        let entry = first(&app);
        let v = bumped_in_range(&entry, &read(app.world(), &entry).unwrap());
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

    #[test]
    fn begin_run_copies_the_level_hash_into_the_log() {
        let mut app = build_headless_app();
        // No rules, whatever content/rules.json holds (the editor saves it).
        app.insert_resource(LevelHash(42))
            .insert_resource(RulesHash(0));
        app.world_mut().resource_mut::<PendingSeed>().0 = Some(seed_bytes(1));
        app.update();
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.update();
        assert_eq!(app.world().resource::<TuneLog>().level, 42);
    }

    #[test]
    fn begin_run_uses_zero_without_a_level_hash() {
        let mut app = build_headless_app();
        app.world_mut().remove_resource::<LevelHash>();
        app.world_mut().remove_resource::<RulesHash>();
        app.update();
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.update();
        assert_eq!(app.world().resource::<TuneLog>().level, 0);
    }

    #[test]
    fn begin_run_uses_the_test_hash_under_the_test_source() {
        let mut app = build_builtin_app();
        app.update();
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.update();
        assert_eq!(
            app.world().resource::<TuneLog>().level,
            with_rules(test_level_hash(), test_rules_hash())
        );
    }

    #[test]
    fn begin_run_folds_the_rules_hash_into_the_level_key() {
        use crate::game::rules::set_rules;
        let mut app = build_headless_app();
        set_rules(
            app.world_mut(),
            br#"{"v":1,"rules":[{"id":"a","when":{"start":true},"do":[{"add":["score",1]}]}]}"#,
        )
        .unwrap();
        app.update();
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.update();
        let level = app.world().resource::<LevelHash>().0;
        let rules = app.world().resource::<RulesHash>().0;
        assert_ne!(rules, 0);
        assert_eq!(
            app.world().resource::<TuneLog>().level,
            with_rules(level, rules)
        );
        assert_ne!(app.world().resource::<TuneLog>().level, level);
    }
}
