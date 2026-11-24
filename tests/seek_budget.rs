//! The seek's re-simulation budget (feature `editor`), from the spec:
//! re-simulating 10800 ticks headless takes under 500 ms native. In its
//! own file so it runs as its own test binary, never beside the rest of
//! the suite (contention there doubled the time).
//!
//! Measured 2026-11-24 on the template, run alone: 12–16 ms in the dev
//! profile. Before `time::seek::run_ticks` put the fixed schedules on the
//! single-threaded executor for a batch it was 544–562 ms (dev) and 533 ms
//! (release): almost all of it the thread pool's per-system dispatch.
#![cfg(feature = "editor")]

use std::time::Instant;

use bevy::prelude::*;
use gamebient_game::game::replay::feeder::ReplayFeeder;
use gamebient_game::game::replay::recorder::ReplayRecorder;
use gamebient_game::game::replay::{build_builtin_app, selftest};
use gamebient_game::game::sim::{PendingSeed, SimTick};
use gamebient_game::game::states::GameState;
use gamebient_game::game::time::seek::{PlaybackFeeder, restart_run, run_ticks};
use gamebient_game::game::time::{TimeTravel, hold};

#[test]
fn re_simulating_10800_ticks_takes_under_500_ms() {
    // 10800 ticks (three minutes) of a run re-simulated headless from tick 0.
    const TICKS: u32 = 10_800;
    let mut app = build_builtin_app();
    app.world_mut().resource_mut::<PendingSeed>().0 = Some(selftest::SELFTEST_SEED);
    app.update();
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Playing);
    app.update();
    let world = app.world_mut();
    let mut rec = world.resource::<ReplayRecorder>().replay.clone();
    rec.runs.clear();
    rec.ticks = 0;
    for t in 0..TICKS {
        // RIGHT and LEFT in turn, as the selftest script plays.
        let held = if (t / 120).is_multiple_of(2) { 8 } else { 4 };
        rec.push_tick(held, 0, 0, 0);
    }
    let from = world
        .resource::<TimeTravel>()
        .start
        .clone()
        .expect("captured");
    hold(world);
    let t0 = Instant::now();
    restart_run(world, &from);
    world.insert_resource(PlaybackFeeder(ReplayFeeder::new(&rec)));
    let ran = run_ticks(world, TICKS);
    let ms = t0.elapsed().as_millis();
    eprintln!("time travel: re-simulated {TICKS} ticks in {ms} ms");
    assert_eq!(ran, TICKS);
    assert_eq!(world.resource::<SimTick>().0, TICKS);
    assert!(
        ms < 500,
        "re-simulating {TICKS} ticks took {ms} ms (budget 500 ms)"
    );
}
