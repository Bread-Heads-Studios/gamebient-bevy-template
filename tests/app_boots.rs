//! The real app boots: the plugin set `main.rs` wires (the windowed
//! `GamePlugin`, `AssetsPlugin`, `UiPlugin`, `DisplayPlugin`, `FramePlugin`
//! and `LookPlugin`), headless, with no OS window and no GPU, under the
//! default look and under every built-in pack.
//!
//! Bevy validates a system's queries and parameters when its schedule is
//! initialised, and a conflicting query (B0001) or a missing resource
//! panics there. The unit tests build systems one at a time, so such a
//! panic in the assembled app otherwise first shows in a browser. This
//! test runs a few frames, initialises **every** schedule (each state's
//! `OnEnter` / `OnExit` included, which the frames never reach), and then
//! runs a few frames of a run.
//!
//! Copy this file into a game unchanged except for the crate name
//! (`gamebient_game` below). It names only modules every template game
//! has. A game whose screens need another engine resource or asset store
//! headless adds it in `app()` and says why.

use bevy::asset::AssetPlugin;
use bevy::ecs::schedule::Schedules;
use bevy::input::InputPlugin;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy::text::TextPlugin;

use gamebient_game::game::GamePlugin;
use gamebient_game::game::states::GameState;
use gamebient_game::look::{self, ActiveLook, LookPlugin, Selection};
use gamebient_game::{assets, display, frame, ui};

/// `main.rs`'s plugin set, minus what needs a window or a GPU: the engine
/// half is `MinimalPlugins` plus the plugins and asset stores the game's
/// render-side code reads (`DefaultPlugins` would bring these with the
/// renderer).
fn app(selection: Selection) -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        StatesPlugin,
        InputPlugin,
        WindowPlugin {
            primary_window: None,
            exit_condition: bevy::window::ExitCondition::DontExit,
            ..default()
        },
        // Assets<Font> for UiPlugin's embedded faces and text layout.
        TextPlugin,
    ));
    // The stores render-side code bakes into (no renderer in a test).
    app.init_asset::<Mesh>()
        .init_asset::<Image>()
        .init_asset::<Shader>()
        .init_asset::<StandardMaterial>()
        .init_asset::<ColorMaterial>()
        .init_asset::<AudioSource>()
        .init_resource::<UiScale>()
        .init_resource::<ClearColor>()
        .insert_resource(selection)
        .add_plugins((
            display::DisplayPlugin,
            LookPlugin {
                contract: &ui::theme::LOOK,
            },
            GamePlugin::default(),
            assets::AssetsPlugin,
            ui::UiPlugin,
            frame::FramePlugin,
        ));
    app
}

/// Initialises every schedule the app holds (Bevy's B0001 check runs
/// here), including the `OnEnter` / `OnExit` schedules no frame reached.
fn initialise_every_schedule(app: &mut App) {
    let mut schedules = app
        .world_mut()
        .remove_resource::<Schedules>()
        .expect("the app has schedules");
    for (label, schedule) in schedules.iter_mut() {
        schedule
            .initialize(app.world_mut())
            .unwrap_or_else(|e| panic!("schedule {label:?} does not initialise: {e}"));
    }
    app.world_mut().insert_resource(schedules);
}

fn boots(selection: Selection) {
    let mut app = app(selection);
    app.finish();
    app.cleanup();
    for _ in 0..4 {
        app.update();
    }
    initialise_every_schedule(&mut app);
    assert!(
        app.world().get_resource::<State<GameState>>().is_some(),
        "the state machine did not start"
    );
    app.world().resource::<ActiveLook>();
    // Into a run: the gameplay schedules and the HUD run for real.
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Playing);
    for _ in 0..8 {
        app.update();
    }
    assert_eq!(
        *app.world().resource::<State<GameState>>().get(),
        GameState::Playing
    );
}

#[test]
fn the_real_app_boots_under_the_default_look() {
    boots(Selection::Default);
}

#[test]
fn the_real_app_boots_under_every_built_in_pack() {
    for id in look::pack::embedded_ids() {
        boots(Selection::Embedded(id.to_string()));
    }
}
