//! The HUD survives a time-travel seek (feature `editor`). The HUD is a
//! `GameEntity`; a seek despawns every `GameEntity` and runs only the
//! `BeginRun` schedule, so the HUD must be spawned there, not on
//! `OnEnter(Playing)`. This runs the windowed plugin set (`GamePlugin`,
//! `AssetsPlugin`, `UiPlugin`, ...) headless, as `tests/app_boots.rs` does.
#![cfg(feature = "editor")]

use bevy::asset::AssetPlugin;
use bevy::input::InputPlugin;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy::text::TextPlugin;

use gamebient_game::game::GamePlugin;
use gamebient_game::game::sim::SimTick;
use gamebient_game::game::states::GameState;
use gamebient_game::game::time::seek::Seek;
use gamebient_game::look::{LookPlugin, Selection};
use gamebient_game::ui::hud::ScoreText;
use gamebient_game::{assets, display, frame, ui};
use gamebient_input::HostCommand;

fn windowed_app() -> App {
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
        TextPlugin,
    ));
    app.init_asset::<Mesh>()
        .init_asset::<Image>()
        .init_asset::<Shader>()
        .init_asset::<StandardMaterial>()
        .init_asset::<ColorMaterial>()
        .init_asset::<AudioSource>()
        .init_resource::<UiScale>()
        .init_resource::<ClearColor>()
        .insert_resource(Selection::Default)
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
    app.finish();
    app.cleanup();
    app
}

fn huds(app: &mut App) -> usize {
    app.world_mut()
        .query_filtered::<Entity, With<ScoreText>>()
        .iter(app.world())
        .count()
}

fn command(app: &mut App, name: &str, data: serde_json::Value) {
    app.world_mut().write_message(HostCommand::Custom {
        name: name.into(),
        data: data.to_string(),
    });
    app.update();
}

#[test]
fn one_hud_per_run_and_it_survives_a_seek() {
    let mut app = windowed_app();
    for _ in 0..4 {
        app.update();
    }
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Playing);
    for _ in 0..8 {
        app.update();
    }
    assert_eq!(huds(&mut app), 1, "exactly one HUD on entering Playing");

    // Let the run tick (the headless frame clock is fast), then seek back.
    for _ in 0..120 {
        app.update();
    }
    let now = app.world().resource::<SimTick>().0;
    assert!(now > 4, "the run ticked (at {now})");
    command(&mut app, "time.pause", serde_json::json!({}));
    command(&mut app, "time.seek", serde_json::json!({ "tick": 2 }));
    for _ in 0..1000 {
        if !app.world().contains_resource::<Seek>() {
            break;
        }
        app.update();
    }
    assert!(!app.world().contains_resource::<Seek>(), "seek finished");
    app.update();
    assert_eq!(app.world().resource::<SimTick>().0, 2, "landed on tick 2");
    assert_eq!(huds(&mut app), 1, "exactly one HUD after a seek");
}
