//! A scripted headless run used as the determinism test: record it, replay
//! it, the verdicts must agree. Native `cargo test` runs it; the wasm CI
//! job replays the committed fixture it wrote.

use bevy::prelude::*;
use gamebient_input::{Buttons, VirtualInput};

use super::recorder::ReplayRecorder;
use super::{Replay, build_headless_app};
use crate::game::sim::{PendingSeed, RunOver, SimTick};
use crate::game::states::GameState;

pub const SELFTEST_SEED: [u8; 32] = [0x5e; 32];
pub const SELFTEST_TICKS: u32 = 600;

/// Sweeps right for 2 s, left for 2 s, taps A every second. Written before
/// the accumulator folds the virtual source. Nothing here may write score or
/// state directly: only inputs, or the replay could not reproduce it.
///
/// `pub` because `tests/windowed_shape.rs` records this same script in a
/// differently *shaped* app (one carrying `ScreenFade` and the windowed
/// build's `Update` systems). Keep it `pub` when you replace the body with
/// your game's own script, or that probe stops compiling.
pub fn script(tick: Res<SimTick>, mut virt: ResMut<VirtualInput>) {
    let t = tick.0;
    let held = if (t / 120).is_multiple_of(2) {
        Buttons::RIGHT
    } else {
        Buttons::LEFT
    };
    virt.set_held(held);
    if t.is_multiple_of(60) {
        virt.latched |= Buttons::A;
    }
}

/// Records the scripted run and returns the sealed replay.
pub fn record_scripted_run() -> Replay {
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
    // Bounded on purpose. A game whose script ends its own run -- the ball
    // drains, the timer expires -- freezes `SimSet` on that tick
    // (`sim::RunOver`) and leaves `Playing`, and `SimTick` then never
    // reaches `SELFTEST_TICKS`: without these two exits this loop would spin
    // forever, and the native `--selftest` and the CI job that calls it
    // would hang rather than fail.
    while app.world().resource::<SimTick>().0 < SELFTEST_TICKS {
        if app.world().resource::<RunOver>().0
            || *app.world().resource::<State<GameState>>().get() != GameState::Playing
        {
            break;
        }
        app.update();
    }
    let ticks = app.world().resource::<SimTick>().0;
    assert_eq!(
        ticks, SELFTEST_TICKS,
        "scripted run ended early at tick {ticks}; lengthen the script or \
         lower SELFTEST_TICKS (currently {SELFTEST_TICKS})"
    );
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::GameOver);
    app.update();
    app.world()
        .resource::<ReplayRecorder>()
        .last_run()
        .cloned()
        .expect("run sealed on OnExit(Playing)")
}
