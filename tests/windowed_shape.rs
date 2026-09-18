//! The other half of `tests/archetype_order.rs`: that file asks whether the
//! windowed build's extra *entities and components* change the sim, this one
//! asks whether its extra **resources and `Update` systems** do.
//!
//! It exists because on one game they did. Sundae Shooter's
//! `launcher::fire_scoop` and `launcher::swap_queue` refused to act while
//! `ui::transition::ScreenFade` was mid-transition — a reasonable-looking
//! "don't shoot during a wipe" gate, and a determinism bug three ways over:
//! the fade lives in `UiPlugin`, which the verifier does not build (so it
//! never blocks anything there); it is ticked from `Update` with the *frame*
//! delta (so how many sim ticks it covers depends on the frame rate); and it
//! is busy for the first ~24 ticks of every run, because entering `Playing`
//! goes through it. A run recorded in the browser came back claiming 195
//! points against 200 re-simulated: the verifier took a shot the player's
//! game had refused.
//!
//! No fixture could have caught that — `--selftest` and the two `.gxr` files
//! all run in bare apps with no fade at all, so the selftest's verdict was
//! byte-for-byte identical before and after the fix — and neither could
//! `archetype_order`, which adds components rather than resources. So:
//! record the selftest script in an app carrying the windowed build's
//! presentation state and its real `Update` systems, then `verify()` it in a
//! bare one, and require the same score, tick count and checksum. That is
//! the production question exactly: the browser records with all of this
//! present and the site re-simulates with none of it.
//!
//! # Adapt this during the port
//!
//! As shipped it covers `ScreenFade`, which every template-derived game has
//! and which is the trap that has actually bitten. Add your game's own
//! `!headless` shape to `record_windowed` below:
//!
//! * every `Update` system `GamePlugin::build` registers only when
//!   `!self.headless` and that could plausibly touch run state (juice,
//!   tweens, recoil, shake — the ones whose names sound harmless);
//! * every resource `UiPlugin`/`AssetsPlugin` own that a sim system might
//!   read. `Option<Res<T>>` in a sim system is a compile fix, not a
//!   determinism fix: the run still forks on whether `T` was there.
//!
//! The rule the test encodes: **a sim system may not read anything
//! `UiPlugin` owns.** `sim::end_run`'s `Option<ResMut<ScreenFade>>` is the
//! one sanctioned touch, because it only *writes* a fade request on the tick
//! the run ends and `sim::RunOver` has already frozen `SimSet` by then.

use bevy::prelude::*;

use gamebient_game::game::replay::recorder::ReplayRecorder;
use gamebient_game::game::replay::selftest::{SELFTEST_SEED, SELFTEST_TICKS, script};
use gamebient_game::game::replay::{Replay, build_headless_app, verify};
use gamebient_game::game::sim::{PendingSeed, RunOver, SimTick};
use gamebient_game::game::states::GameState;
use gamebient_game::ui::transition::ScreenFade;

/// `ui::transition::update_fade` minus the overlay it paints: the fade ticks
/// on the frame delta and drives the state change, exactly as the windowed
/// build does.
fn tick_fade(
    time: Res<Time>,
    mut fade: ResMut<ScreenFade>,
    mut next: ResMut<NextState<GameState>>,
) {
    if let Some(target) = fade.tick(time.delta_secs()) {
        next.set(target);
    }
}

/// The scripted run, recorded in an app shaped like the windowed game:
/// `ScreenFade` present and mid-fade at the start (`boot()` is the state the
/// player's first run really begins in), plus the `Update` systems
/// `GamePlugin` registers only when `!headless`.
///
/// This is where a port adds its own presentation systems — see the module
/// comment.
fn record_windowed() -> Replay {
    let mut app = build_headless_app();
    app.insert_resource(ScreenFade::boot());
    app.add_systems(
        PreUpdate,
        script.before(gamebient_input::input::accumulate_input),
    );
    app.add_systems(Update, tick_fade);
    app.world_mut().resource_mut::<PendingSeed>().0 = Some(SELFTEST_SEED);
    app.update();
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Playing);
    app.update();
    // Bounded exactly like `selftest::record_scripted_run`: a script that
    // ends its own run freezes `SimSet` and leaves `Playing`, and `SimTick`
    // then never reaches `SELFTEST_TICKS`.
    while app.world().resource::<SimTick>().0 < SELFTEST_TICKS {
        if app.world().resource::<RunOver>().0
            || *app.world().resource::<State<GameState>>().get() != GameState::Playing
        {
            break;
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
        .expect("run sealed on OnExit(Playing)")
}

#[test]
fn a_run_recorded_with_the_windowed_presentation_still_verifies_without_it() {
    let replay = record_windowed();
    assert_eq!(
        replay.ticks, SELFTEST_TICKS,
        "the windowed-shaped run should reach the same tick count as the bare one"
    );
    let v = verify(&replay);
    assert!(
        v.matches,
        "a run recorded with ScreenFade and the windowed Update systems present \
         did not reproduce in a bare verifier app. Some sim system is reading \
         presentation state — the fade is the classic one, and it is frame-rate \
         dependent as well as absent headless. See this file's module comment \
         and the port checklist, rule 1.\n{v:?} vs claimed score {} checksum {}",
        replay.score, replay.checksum
    );
    assert_eq!(v.ticks, SELFTEST_TICKS);
}
