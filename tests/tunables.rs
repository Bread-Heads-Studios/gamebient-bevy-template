//! A tunable change made mid-run is replay content: the verifier reproduces
//! it, and the same inputs under a different value do not verify.

use bevy::prelude::*;
use gamebient_game::game::replay::recorder::ReplayRecorder;
use gamebient_game::game::replay::selftest::{SELFTEST_SEED, script};
use gamebient_game::game::replay::{Replay, build_headless_app, verify};
use gamebient_game::game::sim::{PendingSeed, SimTick};
use gamebient_game::game::states::GameState;
use gamebient_game::game::tunables::{PendingTunes, TunableIndex, Value, read};

const TICKS: u32 = 300;
const CHANGE_AT: u32 = 120;

/// Records the selftest script and, on tick `change_at`, queues `value`
/// for the first tunable. `None` records a plain run.
fn record(change_at: Option<u32>, value: Value) -> Replay {
    let mut app = build_headless_app();
    app.add_systems(
        PreUpdate,
        script.before(gamebient_input::input::accumulate_input),
    );
    app.world_mut().resource_mut::<PendingSeed>().0 = Some(SELFTEST_SEED);
    app.update();
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Playing);
    app.update();
    let id = app.world().resource::<TunableIndex>().entries[0].id;
    while app.world().resource::<SimTick>().0 < TICKS {
        if Some(app.world().resource::<SimTick>().0) == change_at {
            app.world_mut()
                .resource_mut::<PendingTunes>()
                .0
                .push((id, value.clone()));
        }
        app.update();
    }
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::GameOver);
    app.update();
    app.world()
        .resource::<ReplayRecorder>()
        .last_run()
        .cloned()
        .unwrap()
}

fn bumped(v: &Value) -> Value {
    match v {
        Value::Int(i) => Value::Int(i + 1),
        Value::Float(f) => Value::Float(f + 1.0),
        Value::Bool(b) => Value::Bool(!b),
        other => other.clone(),
    }
}

fn first_default() -> Value {
    let app = build_headless_app();
    let e = app.world().resource::<TunableIndex>().entries[0].clone();
    read(app.world(), &e).unwrap()
}

#[test]
fn a_mid_run_change_replays_identically() {
    let v = bumped(&first_default());
    let replay = record(Some(CHANGE_AT), v);
    assert_eq!(replay.tunes.len(), 1, "one change recorded");
    assert_eq!(
        replay.tunes[0].tick,
        CHANGE_AT + 1,
        "applied on the tick after it was queued"
    );
    let verdict = verify(&replay);
    assert!(
        verdict.matches,
        "{verdict:?} vs {} / {} / {}",
        replay.score, replay.checksum, replay.content
    );
    assert_eq!(verdict.content, replay.content);
}

#[test]
fn the_same_inputs_under_another_value_do_not_verify() {
    let plain = record(None, first_default());
    let changed = record(Some(CHANGE_AT), bumped(&first_default()));
    assert_ne!(plain.content, changed.content);
    assert_ne!(plain.checksum, changed.checksum);
    // Lie: claim the changed run's result with the plain run's tune log.
    let mut forged = changed.clone();
    forged.tunes.clear();
    forged.content = plain.content;
    let v = verify(&forged);
    assert!(
        !v.matches,
        "a replay stripped of its change must not verify: {v:?}"
    );
}

#[test]
fn different_start_values_are_different_content() {
    let mut app = build_headless_app();
    app.update();
    let e = app.world().resource::<TunableIndex>().entries[0].clone();
    let v = bumped(&read(app.world(), &e).unwrap());
    app.world_mut()
        .resource_mut::<PendingTunes>()
        .0
        .push((e.id, v));
    app.update();
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Playing);
    app.update();
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::GameOver);
    app.update();
    let tweaked = app
        .world()
        .resource::<ReplayRecorder>()
        .last_run()
        .cloned()
        .unwrap();
    let plain = record(None, first_default());
    assert_ne!(tweaked.content, plain.content);
    assert_ne!(tweaked.starts, plain.starts);
    // And a tweaked start replays on its own merits.
    assert!(verify(&tweaked).matches);
}
