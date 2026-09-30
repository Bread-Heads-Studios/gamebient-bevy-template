//! The display layer's row of the question `tests/windowed_shape.rs` asks:
//! does state the windowed build has, and the verifier does not, change a
//! run?
//!
//! `src/display.rs` writes `Camera.viewport`, `UiScale` and `GameViewport`
//! every frame, and what it writes depends on the panel the game is on: a
//! portrait TV under a marquee, a landscape TV, a phone. The verifier has no
//! window at all. So: record the selftest script with `DisplayPlugin`
//! letterboxing a real `Window` and `Camera`, on two differently shaped
//! panels, and require the replay to be identical to the bare recording and
//! to verify in a bare app.
//!
//! This lives in its own file because `tools/rollout-replay.sh` copies
//! `tests/windowed_shape.rs` into games that do not have `src/display.rs`.

use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};

use gamebient_game::display::{DisplayPlugin, FrameInsets, GameViewport};
use gamebient_game::game::replay::recorder::ReplayRecorder;
use gamebient_game::game::replay::selftest::{
    SELFTEST_SEED, SELFTEST_TICKS, record_scripted_run, script,
};
use gamebient_game::game::replay::{Replay, build_headless_app, verify};
use gamebient_game::game::sim::{PendingSeed, RunOver, SimTick};
use gamebient_game::game::states::GameState;

/// `selftest::record_scripted_run`, in an app that also carries the display
/// layer with something to act on.
fn record_under_display(panel: UVec2, insets: FrameInsets) -> Replay {
    let mut app = build_headless_app();
    app.add_plugins(DisplayPlugin);
    app.insert_resource(insets);
    app.world_mut().spawn((
        Window {
            resolution: WindowResolution::new(panel.x, panel.y),
            ..default()
        },
        PrimaryWindow,
    ));
    let camera = app.world_mut().spawn(Camera::default()).id();
    app.add_systems(
        PreUpdate,
        script.before(gamebient_input::input::accumulate_input),
    );
    app.world_mut().resource_mut::<PendingSeed>().0 = Some(SELFTEST_SEED);
    app.update();

    // The probe is only a probe if the display layer did its work.
    let viewport = *app.world().resource::<GameViewport>();
    assert_ne!(
        viewport.size, panel,
        "the game filled the panel, so nothing was letterboxed"
    );
    assert_eq!(
        app.world()
            .get::<Camera>(camera)
            .and_then(|c| c.viewport.as_ref())
            .map(|v| v.physical_size),
        Some(viewport.size),
        "the camera did not receive the game viewport"
    );

    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Playing);
    app.update();
    while app.world().resource::<SimTick>().0 < SELFTEST_TICKS {
        if app.world().resource::<RunOver>().0
            || *app.world().resource::<State<GameState>>().get() != GameState::Playing
        {
            break;
        }
        app.update();
    }
    assert_eq!(app.world().resource::<SimTick>().0, SELFTEST_TICKS);
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
fn a_run_recorded_under_a_letterboxed_display_still_verifies_without_it() {
    let replay = record_under_display(
        UVec2::new(1080, 1920),
        FrameInsets {
            reserve_top: 360,
            gap: 8,
        },
    );
    let v = verify(&replay);
    assert!(
        v.matches,
        "{v:?} vs claimed score {} checksum {}",
        replay.score, replay.checksum
    );
    assert_eq!(v.ticks, SELFTEST_TICKS);
}

#[test]
fn the_shape_of_the_panel_does_not_change_the_run() {
    let bare = record_scripted_run();
    let portrait = record_under_display(
        UVec2::new(1080, 1920),
        FrameInsets {
            reserve_top: 360,
            gap: 8,
        },
    );
    // 16:9 landscape: no sanctioned ratio fills it, so this letterboxes too.
    let landscape = record_under_display(UVec2::new(1920, 1080), FrameInsets::default());
    assert_eq!(portrait, bare, "a portrait panel changed the run");
    assert_eq!(landscape, bare, "a landscape panel changed the run");
}
