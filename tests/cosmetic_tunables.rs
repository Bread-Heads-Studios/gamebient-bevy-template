//! A cosmetic tunable (`Tune::cosmetic`) changes only how the game looks:
//! it is not a start value, a mid-run change of it is not logged, and it
//! never moves the checksum. D1 of the customization assessment.

use bevy::color::Srgba;
use bevy::prelude::*;
use gamebient_game::game::replay::build_headless_app;
use gamebient_game::game::replay::selftest::SELFTEST_SEED;
use gamebient_game::game::sim::{Checksum, PendingSeed, SimTick};
use gamebient_game::game::states::GameState;
use gamebient_game::game::tunables::{
    self, Group, PendingTunes, TunableIndex, Tune, TuneLog, Value,
};

#[derive(Resource, Reflect, Clone, Debug)]
#[reflect(Resource, @Group::thing("paint", "Paint"))]
struct Paint {
    #[reflect(@Tune::colour("Coin colour").cosmetic())]
    coin: Srgba,
}

impl Default for Paint {
    fn default() -> Self {
        Self {
            coin: Srgba::rgb(1.0, 0.8, 0.1),
        }
    }
}

/// A headless app with `Paint` registered, in `Playing`, and the id of
/// `paint.coin`.
fn playing() -> (App, u16) {
    let mut app = build_headless_app();
    tunables::register::<Paint>(&mut app);
    let id = app
        .world()
        .resource::<TunableIndex>()
        .by_path("paint.coin")
        .expect("paint.coin is registered")
        .id;
    app.world_mut().resource_mut::<PendingSeed>().0 = Some(SELFTEST_SEED);
    app.update();
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Playing);
    app.update();
    (app, id)
}

fn run_to(app: &mut App, tick: u32) {
    while app.world().resource::<SimTick>().0 < tick {
        app.update();
    }
}

#[test]
fn a_cosmetic_tunable_is_not_a_start_value() {
    let (app, id) = playing();
    let log = app.world().resource::<TuneLog>();
    assert!(
        log.starts.iter().all(|(i, _)| *i != id),
        "cosmetic id {id} leaked into starts: {:?}",
        log.starts
    );
    assert!(
        !log.starts.is_empty(),
        "the template's gameplay tunable (run.spare) must still be recorded"
    );
}

#[test]
fn a_cosmetic_change_mid_run_applies_but_leaves_checksum_and_log_alone() {
    let (mut control, _) = playing();
    let (mut changed, id) = playing();
    run_to(&mut control, 10);
    run_to(&mut changed, 10);
    let red = Srgba::rgb(1.0, 0.0, 0.0);
    changed
        .world_mut()
        .resource_mut::<PendingTunes>()
        .0
        .push((id, Value::Colour(red)));
    run_to(&mut control, 30);
    run_to(&mut changed, 30);

    assert_eq!(
        changed.world().resource::<Paint>().coin,
        red,
        "the change applied"
    );
    assert!(
        changed.world().resource::<TuneLog>().changes.is_empty(),
        "a cosmetic change is not replay content"
    );
    assert_eq!(
        changed.world().resource::<SimTick>().0,
        control.world().resource::<SimTick>().0
    );
    assert_eq!(
        changed.world().resource::<Checksum>().0,
        control.world().resource::<Checksum>().0,
        "a cosmetic change must not move the checksum"
    );
}
