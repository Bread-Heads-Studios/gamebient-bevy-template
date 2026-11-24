//! Time travel on the template (feature `editor`): the `time.*` commands
//! over host messages, seeks that land on the very world a straight run
//! reaches, edits held across a re-simulation, branches, marks and
//! re-entry. The re-simulation budget has its own binary,
//! `tests/seek_budget.rs`. Template only: a game writes its own (Coin
//! Run's `tests/time.rs`), because these lean on the template's kinds and
//! tunables.
#![cfg(feature = "editor")]

use bevy::prelude::*;
use gamebient_game::game::content::kit::Thing;
use gamebient_game::game::content::level::{Level, LevelSource};
use gamebient_game::game::content::model::{LevelFile, PlacedThing};
use gamebient_game::game::content::saved::reset_to_defaults;
use gamebient_game::game::replay::recorder::{ReplayRecorder, RunEdited};
use gamebient_game::game::replay::{
    EDITED_REFUSAL, Refused, RulesRecord, build_builtin_app, build_headless_app, selftest,
    verify_bytes,
};
use gamebient_game::game::rules::{Rules, RulesLog, set_rules};
use gamebient_game::game::scoring::GameData;
use gamebient_game::game::sim::{Checksum, PendingSeed, SimTick, SpawnOrder};
use gamebient_game::game::states::{GameState, Paused, QuietPause};
use gamebient_game::game::time::TimeTravel;
use gamebient_game::game::time::seek::{PlaybackFeeder, Seek};
use gamebient_game::game::tunables::{
    PendingTunes, TunableIndex, TuneChange, TuneLog, Value, read,
};
use gamebient_input::{Buttons, HostCommand, HostEvent, VirtualInput};
use serde_json::{Value as Json, json};

/// Every custom host message and how many runs were posted.
#[derive(Resource, Default)]
struct Heard {
    custom: Vec<(String, Json)>,
    runs: usize,
}

fn hear(mut events: MessageReader<HostEvent>, mut heard: ResMut<Heard>) {
    for e in events.read() {
        match e {
            HostEvent::Custom { name, data } => heard.custom.push((
                name.clone(),
                serde_json::from_str(data).unwrap_or(Json::Null),
            )),
            HostEvent::Run(_) => heard.runs += 1,
            _ => {}
        }
    }
}

fn listen(app: &mut App) {
    app.init_resource::<Heard>().add_systems(Last, hear);
}

fn start(app: &mut App) {
    app.update();
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Playing);
    app.update();
}

/// The selftest script on built-in content: RIGHT, LEFT, A every second.
fn scripted() -> App {
    let mut app = build_builtin_app();
    listen(&mut app);
    app.add_systems(
        PreUpdate,
        selftest::script.before(gamebient_input::input::accumulate_input),
    );
    app.world_mut().resource_mut::<PendingSeed>().0 = Some(selftest::SELFTEST_SEED);
    start(&mut app);
    app
}

/// A run on `things` under the rules file `rules`, on the content source
/// (so a level edit and a rules swap are live), optionally scripted.
fn on_rules(rules: &str, things: &[(&str, f32, f32)], script: bool) -> App {
    let mut app = build_headless_app();
    reset_to_defaults(app.world_mut());
    listen(&mut app);
    if script {
        app.add_systems(
            PreUpdate,
            selftest::script.before(gamebient_input::input::accumulate_input),
        );
    }
    let level = LevelFile {
        v: 1,
        things: things
            .iter()
            .enumerate()
            .map(|(i, (k, x, y))| PlacedThing {
                id: format!("t{i}"),
                kind: (*k).to_string(),
                at: [*x, *y],
                size: None,
            })
            .collect(),
    };
    app.insert_resource(Level(level))
        .insert_resource(LevelSource::Content);
    set_rules(app.world_mut(), rules.as_bytes()).unwrap();
    app.world_mut().resource_mut::<PendingSeed>().0 = Some(selftest::SELFTEST_SEED);
    start(&mut app);
    app
}

const ADD_1: &str = r#"{"v":1,"rules":[{"id":"a","when":{"every":1},"do":[{"add":["score",1]}]}]}"#;
const ADD_10: &str =
    r#"{"v":1,"rules":[{"id":"a","when":{"every":1},"do":[{"add":["score",10]}]}]}"#;
const FAR: (&str, f32, f32) = ("Player", -9.0, -7.0);

fn tick(app: &App) -> u32 {
    app.world().resource::<SimTick>().0
}

fn play_to(app: &mut App, t: u32) {
    while tick(app) < t {
        app.update();
    }
}

fn send(app: &mut App, name: &str, data: Json) {
    app.world_mut().write_message(HostCommand::Custom {
        name: name.into(),
        data: data.to_string(),
    });
}

/// Sends a command and runs the frame that handles it.
fn command(app: &mut App, name: &str, data: Json) {
    send(app, name, data);
    app.update();
}

/// Runs frames until no seek is in progress.
fn settle(app: &mut App) {
    for _ in 0..1000 {
        if !app.world().contains_resource::<Seek>() {
            return;
        }
        app.update();
    }
    panic!("the seek never finished");
}

fn heard(app: &App, name: &str) -> Vec<Json> {
    app.world()
        .resource::<Heard>()
        .custom
        .iter()
        .filter(|(n, _)| n == name)
        .map(|(_, d)| d.clone())
        .collect()
}

/// The last full `time` answer (progress messages carry no `of`).
fn last_answer(app: &App) -> Json {
    heard(app, "time")
        .into_iter()
        .rev()
        .find(|d| d.get("of").is_some())
        .expect("a time answer")
}

fn forget(app: &mut App) {
    app.world_mut().resource_mut::<Heard>().custom.clear();
}

type Print = (u64, u32, Vec<(u64, u16, u32, u32)>);

/// The checksum, the score and every thing (spawn key, kind, position bits).
fn print(app: &mut App) -> Print {
    let world = app.world_mut();
    let mut q = world.query::<(&SpawnOrder, &Thing, &Transform)>();
    let mut things: Vec<_> = q
        .iter(world)
        .map(|(o, t, tf)| {
            (
                o.0,
                t.kind,
                tf.translation.x.to_bits(),
                tf.translation.y.to_bits(),
            )
        })
        .collect();
    things.sort_unstable();
    (
        world.resource::<Checksum>().0,
        world.resource::<GameData>().score,
        things,
    )
}

/// A non-cosmetic tunable and a value its range takes that differs from now.
fn an_edit(app: &App) -> (u16, Value) {
    let idx = app.world().resource::<TunableIndex>().clone();
    idx.entries
        .iter()
        .filter(|e| !e.tune.cosmetic)
        .find_map(|e| {
            let now = read(app.world(), e)?;
            let next = match &now {
                Value::Bool(b) => Value::Bool(!b),
                Value::Int(i) if ((*i + 1) as f64) <= e.tune.max => Value::Int(i + 1),
                Value::Float(f) if f64::from(f + 0.5) <= e.tune.max => Value::Float(f + 0.5),
                _ => return None,
            };
            Some((e.id, next))
        })
        .expect("the template registers a bumpable non-cosmetic tunable")
}

#[test]
fn a_seek_lands_on_the_same_world_as_playing_straight_there() {
    let mut straight = scripted();
    play_to(&mut straight, 200);
    let want = print(&mut straight);

    let mut app = scripted();
    play_to(&mut app, 300);
    command(&mut app, "time.pause", json!({}));
    command(&mut app, "time.seek", json!({ "tick": 200 }));
    settle(&mut app);
    assert_eq!(tick(&app), 200);
    assert_eq!(print(&mut app), want, "checksum, score and every thing");
    assert!(app.world().resource::<Paused>().0, "a seek ends paused");
    assert!(app.world().resource::<QuietPause>().0, "with no overlay");
}

#[test]
fn resuming_after_a_seek_plays_the_recording_back() {
    let mut app = scripted();
    play_to(&mut app, 300);
    command(&mut app, "time.pause", json!({}));
    let of = tick(&app);
    let mut original = scripted();
    play_to(&mut original, of);
    let want = print(&mut original);

    command(&mut app, "time.seek", json!({ "tick": 120 }));
    settle(&mut app);
    command(&mut app, "time.resume", json!({}));
    play_to(&mut app, of);
    assert_eq!(
        print(&mut app),
        want,
        "the recorded inputs, not the script's"
    );
    app.update();
    assert!(
        !app.world().contains_resource::<PlaybackFeeder>(),
        "past the recording the run is live"
    );
}

#[test]
fn paused_ticks_record_nothing() {
    let mut app = scripted();
    play_to(&mut app, 100);
    command(&mut app, "time.pause", json!({}));
    let at = tick(&app);
    let recorded = app.world().resource::<ReplayRecorder>().replay.ticks;
    for _ in 0..30 {
        app.update();
    }
    assert_eq!(tick(&app), at);
    assert_eq!(
        app.world().resource::<ReplayRecorder>().replay.ticks,
        recorded
    );
}

#[test]
fn step_runs_n_ticks_then_stays_paused() {
    let mut app = scripted();
    play_to(&mut app, 50);
    command(&mut app, "time.pause", json!({}));
    let at = tick(&app);
    command(&mut app, "time.step", json!({}));
    assert_eq!(tick(&app), at + 1, "n defaults to 1");
    command(&mut app, "time.step", json!({ "n": 5 }));
    assert_eq!(tick(&app), at + 6);
    command(&mut app, "time.step", json!({ "n": 5000 }));
    assert_eq!(tick(&app), at + 606, "n is at most 600");
    assert!(app.world().resource::<Paused>().0);
    assert_eq!(last_answer(&app)["tick"], at + 606);
}

#[test]
fn speed_takes_half_one_and_two_only() {
    let mut app = scripted();
    play_to(&mut app, 10);
    command(&mut app, "time.speed", json!({ "x": 3 }));
    assert_eq!(app.world().resource::<TimeTravel>().speed, 1.0);
    command(&mut app, "time.speed", json!({ "x": 2 }));
    assert_eq!(
        app.world().resource::<Time<Virtual>>().relative_speed(),
        2.0
    );
    assert_eq!(last_answer(&app)["speed"], 2.0);
    let at = tick(&app);
    app.update();
    assert_eq!(tick(&app), at + 2, "two ticks a frame");
    command(&mut app, "time.speed", json!({ "x": 0.5 }));
    assert_eq!(app.world().resource::<TimeTravel>().speed, 0.5);
}

#[test]
fn every_command_is_answered_and_play_sends_an_update_every_30_ticks() {
    let mut app = scripted();
    play_to(&mut app, 10);
    command(&mut app, "time.pause", json!({}));
    let a = last_answer(&app);
    for key in ["tick", "of", "paused", "speed", "seeking", "edited", "run"] {
        assert!(a.get(key).is_some(), "{key} in {a}");
    }
    assert_eq!(a["paused"], true);
    assert_eq!(a["seeking"], false);
    assert_eq!(a["edited"], true);
    assert_eq!(a["run"], 1);
    assert_eq!(a["of"], a["tick"]);
    command(&mut app, "time.resume", json!({}));
    forget(&mut app);
    play_to(&mut app, 100);
    let ticks: Vec<u64> = heard(&app, "time")
        .iter()
        .filter_map(|d| d["tick"].as_u64())
        .collect();
    assert_eq!(ticks, vec![30, 60, 90]);
}

#[test]
fn a_long_seek_reports_progress_every_2000_ticks() {
    let mut app = scripted();
    play_to(&mut app, 4500);
    command(&mut app, "time.pause", json!({}));
    forget(&mut app);
    send(&mut app, "time.seek", json!({ "tick": 4400 }));
    app.update();
    settle(&mut app);
    let progress: Vec<u64> = heard(&app, "time")
        .iter()
        .filter(|d| d.get("of").is_none())
        .map(|d| {
            assert_eq!(d["seeking"], true);
            d["tick"].as_u64().unwrap()
        })
        .collect();
    assert_eq!(progress, vec![2000, 4000]);
    assert_eq!(last_answer(&app)["seeking"], false);
    assert_eq!(tick(&app), 4400);
}

#[test]
fn a_seek_after_a_mid_run_rules_swap_reproduces_exactly() {
    let mut straight = on_rules(ADD_1, &[FAR], false);
    play_to(&mut straight, 40);
    set_rules(straight.world_mut(), ADD_10.as_bytes()).unwrap();
    play_to(&mut straight, 70);
    let want = print(&mut straight);

    let mut app = on_rules(ADD_1, &[FAR], false);
    play_to(&mut app, 40);
    set_rules(app.world_mut(), ADD_10.as_bytes()).unwrap();
    play_to(&mut app, 100);
    command(&mut app, "time.pause", json!({}));
    command(&mut app, "time.seek", json!({ "tick": 70 }));
    settle(&mut app);
    assert_eq!(tick(&app), 70);
    assert_eq!(print(&mut app), want);
    assert_eq!(
        app.world().resource::<RulesLog>().0,
        vec![RulesRecord {
            tick: 41,
            bytes: ADD_10.as_bytes().to_vec()
        }]
    );
    assert_eq!(
        app.world().resource::<Rules>().0,
        straight.world().resource::<Rules>().0,
        "the editor's rules are back after the re-simulation"
    );
}

#[test]
fn edits_made_while_paused_survive_a_seek_and_land_on_the_next_tick() {
    let mut app = scripted();
    play_to(&mut app, 200);
    command(&mut app, "time.pause", json!({}));
    let (id, v) = an_edit(&app);
    app.world_mut()
        .resource_mut::<PendingTunes>()
        .0
        .push((id, v.clone()));
    command(&mut app, "time.seek", json!({ "tick": 100 }));
    settle(&mut app);
    assert_eq!(
        app.world().resource::<PendingTunes>().0,
        vec![(id, v.clone())],
        "held aside for the re-simulation, then queued again"
    );
    assert!(app.world().resource::<TuneLog>().changes.is_empty());
    command(&mut app, "time.resume", json!({}));
    app.update();
    assert_eq!(
        app.world().resource::<TuneLog>().changes,
        vec![TuneChange {
            tick: 101,
            id,
            value: v
        }]
    );
}

#[test]
fn an_edited_run_is_flagged_never_posted_and_refused() {
    let leave = |app: &mut App| {
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::GameOver);
        app.update();
        app.world()
            .resource::<ReplayRecorder>()
            .last_run()
            .cloned()
            .expect("sealed")
    };
    let mut plain = scripted();
    play_to(&mut plain, 60);
    let r = leave(&mut plain);
    assert!(!r.edited);
    assert_eq!(plain.world().resource::<Heard>().runs, 1);

    let mut app = scripted();
    play_to(&mut app, 60);
    command(&mut app, "time.pause", json!({}));
    command(&mut app, "time.resume", json!({}));
    play_to(&mut app, 90);
    let r = leave(&mut app);
    assert!(r.edited);
    assert_eq!(app.world().resource::<Heard>().runs, 0, "never submitted");
    assert_eq!(verify_bytes(&r.encode()), Err(Refused::Edited));
    assert_eq!(Refused::Edited.sentence(), EDITED_REFUSAL);
    assert_eq!(EDITED_REFUSAL, "This run was edited in the editor.");
}

#[test]
fn the_run_after_an_edited_run_is_posted() {
    // `RunEdited` is cleared in `BeginRun` (a fresh run), but an in-place
    // restart (a seek) is the same run and stays edited.
    let mut app = scripted();
    play_to(&mut app, 300);
    command(&mut app, "time.pause", json!({}));
    command(&mut app, "time.seek", json!({ "tick": 100 }));
    settle(&mut app);
    assert!(
        app.world().resource::<RunEdited>().0,
        "the seek's restart keeps the run edited"
    );
    assert_eq!(last_answer(&app)["edited"], true);
    command(&mut app, "time.resume", json!({}));
    play_to(&mut app, 150);
    let leave = |app: &mut App| {
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::GameOver);
        app.update();
        app.world()
            .resource::<ReplayRecorder>()
            .last_run()
            .cloned()
            .expect("sealed")
    };
    assert!(leave(&mut app).edited);
    assert_eq!(app.world().resource::<Heard>().runs, 0);

    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Playing);
    app.update();
    assert!(!app.world().resource::<RunEdited>().0, "a fresh run");
    assert_eq!(last_answer(&app)["run"], 2);
    let at = tick(&app);
    play_to(&mut app, at + 60);
    let r = leave(&mut app);
    assert!(!r.edited);
    assert_eq!(
        app.world().resource::<Heard>().runs,
        1,
        "the second run is posted"
    );
}

/// Presses the canon Pause button and runs frames until `Paused` is `want`.
fn press_pause_until(app: &mut App, want: bool) {
    app.world_mut().resource_mut::<VirtualInput>().latched |= Buttons::PAUSE;
    for _ in 0..10 {
        app.update();
        if app.world().resource::<Paused>().0 == want {
            return;
        }
    }
    panic!("the pause press never took");
}

#[test]
fn quiet_pause_clears_whenever_the_player_or_host_pauses() {
    let mut app = scripted();
    play_to(&mut app, 50);
    command(&mut app, "time.pause", json!({}));
    assert!(app.world().resource::<QuietPause>().0);

    // The player's Escape lets go of the timeline's pause...
    press_pause_until(&mut app, false);
    assert!(!app.world().resource::<QuietPause>().0);
    // ...and their next Escape is their own pause, with its overlay.
    press_pause_until(&mut app, true);
    assert!(!app.world().resource::<QuietPause>().0, "the overlay shows");

    // The same with the host's pause and resume.
    press_pause_until(&mut app, false);
    command(&mut app, "time.pause", json!({}));
    assert!(app.world().resource::<QuietPause>().0);
    app.world_mut().write_message(HostCommand::Resume);
    app.update();
    assert!(!app.world().resource::<Paused>().0);
    assert!(!app.world().resource::<QuietPause>().0);
    command(&mut app, "time.pause", json!({}));
    app.world_mut().write_message(HostCommand::Pause);
    app.update();
    assert!(app.world().resource::<Paused>().0);
    assert!(
        !app.world().resource::<QuietPause>().0,
        "a host pause shows the overlay"
    );
}
