//! Time travel on the template (feature `editor`): the `time.*` commands
//! over host messages, seeks that land on the very world a straight run
//! reaches, edits held across a re-simulation, branches, marks and
//! re-entry. The re-simulation budget has its own binary,
//! `tests/seek_budget.rs`. Template only: a game writes its own (Coin
//! Run's `tests/time.rs`), because these lean on the template's kinds and
//! tunables.
#![cfg(feature = "editor")]

use bevy::prelude::*;
use gamebient_game::game::GameEntity;
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
use gamebient_game::game::sim::{Checksum, PendingSeed, RunSeed, SimTick, SpawnOrder};
use gamebient_game::game::states::{GameState, Paused, QuietPause};
use gamebient_game::game::time::TimeTravel;
use gamebient_game::game::time::branch::branch_seek;
use gamebient_game::game::time::seek::{Phase, PlaybackFeeder, Seek};
use gamebient_game::game::tunables::{
    PendingTunes, TunableIndex, TuneChange, TuneLog, Value, read, write,
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
const NO_RULES: &str = r#"{"v":1,"rules":[]}"#;
/// The player in the middle and a coin the script's RIGHT runs through.
const ONE_COIN: &[(&str, f32, f32)] = &[("Player", 0.0, 0.0), ("Coin", 3.0, 0.0)];

fn tick(app: &App) -> u32 {
    app.world().resource::<SimTick>().0
}

/// Plays until tick `t`; panics rather than spin on a paused run.
fn play_to(app: &mut App, t: u32) {
    let budget = t.saturating_sub(tick(app)) + 600;
    for _ in 0..budget {
        if tick(app) >= t {
            return;
        }
        app.update();
    }
    assert!(
        tick(app) >= t,
        "the run never reached tick {t} (at {}, paused: {})",
        tick(app),
        app.world().resource::<Paused>().0
    );
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
    for key in [
        "tick", "of", "paused", "speed", "seeking", "edited", "run", "over",
    ] {
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

#[test]
fn a_branch_drops_everything_after_the_tick_and_plays_on_live() {
    let mut app = on_rules(ADD_1, &[FAR], false);
    play_to(&mut app, 40);
    set_rules(app.world_mut(), ADD_10.as_bytes()).unwrap();
    play_to(&mut app, 59);
    let (id, v) = an_edit(&app);
    app.world_mut()
        .resource_mut::<PendingTunes>()
        .0
        .push((id, v));
    play_to(&mut app, 100);
    command(&mut app, "time.pause", json!({}));
    command(&mut app, "time.seek", json!({ "tick": 30 }));
    settle(&mut app);
    command(&mut app, "time.branch", json!({}));
    settle(&mut app);
    assert_eq!(tick(&app), 30);
    let rec = app
        .world()
        .resource::<TimeTravel>()
        .recording
        .clone()
        .unwrap();
    assert_eq!(rec.ticks, 30);
    assert!(rec.tunes.is_empty(), "the change at 60 is gone");
    assert!(rec.rules.is_empty(), "the swap at 41 is gone");
    assert_eq!(app.world().resource::<ReplayRecorder>().replay.ticks, 30);
    assert!(!app.world().resource::<Paused>().0, "a branch plays on");
    assert!(
        !app.world().contains_resource::<PlaybackFeeder>(),
        "live, not playback"
    );
    assert!(
        app.world().resource::<TimeTravel>().before.is_some(),
        "kept for undo"
    );
    assert!(app.world().resource::<RunEdited>().0, "a branch is an edit");
    assert_eq!(last_answer(&app)["edited"], true);
    app.update();
    assert_eq!(tick(&app), 31);
    assert_eq!(
        app.world().resource::<RulesLog>().0,
        vec![RulesRecord {
            tick: 31,
            bytes: ADD_10.as_bytes().to_vec()
        }],
        "the run goes on under the rules the editor shows"
    );
    assert!(heard(&app, "time.diverged").is_empty());
}

#[test]
fn an_edit_made_while_paused_lands_on_the_tick_after_a_branch() {
    let mut app = scripted();
    play_to(&mut app, 100);
    command(&mut app, "time.pause", json!({}));
    let n = tick(&app);
    let (id, v) = an_edit(&app);
    let path = app
        .world()
        .resource::<TunableIndex>()
        .by_id(id)
        .unwrap()
        .path
        .clone();
    let mut set = serde_json::Map::new();
    set.insert(path, v.to_json());
    command(&mut app, "tunables.set", Json::Object(set));
    command(&mut app, "time.branch", json!({}));
    settle(&mut app);
    assert_eq!(tick(&app), n);
    assert!(
        heard(&app, "time.diverged").is_empty(),
        "nothing up to the tick changed"
    );
    app.update();
    assert_eq!(
        app.world().resource::<TuneLog>().changes,
        vec![TuneChange {
            tick: n + 1,
            id,
            value: v
        }]
    );
}

#[test]
fn a_rules_change_while_paused_does_not_diverge_and_applies_from_the_next_tick() {
    let mut app = on_rules(ADD_1, &[FAR], false);
    play_to(&mut app, 50);
    command(&mut app, "time.pause", json!({}));
    let n = tick(&app);
    let score = app.world().resource::<GameData>().score;
    set_rules(app.world_mut(), ADD_10.as_bytes()).unwrap();
    command(&mut app, "time.branch", json!({}));
    settle(&mut app);
    assert_eq!(tick(&app), n);
    assert_eq!(
        app.world().resource::<GameData>().score,
        score,
        "ticks up to N played as before"
    );
    assert!(heard(&app, "time.diverged").is_empty());
    app.update();
    assert_eq!(app.world().resource::<GameData>().score, score + 10);
}

#[test]
fn a_level_edit_replays_the_earlier_moves_on_it_and_says_they_diverged() {
    let mut app = on_rules(NO_RULES, ONE_COIN, true);
    play_to(&mut app, 150);
    assert_eq!(
        app.world().resource::<GameData>().score,
        1,
        "RIGHT ran through the coin"
    );
    command(&mut app, "time.pause", json!({}));
    let n = tick(&app);
    app.world_mut().resource_mut::<Level>().0.things[1].at = [-8.0, 6.0];
    command(&mut app, "time.branch", json!({}));
    settle(&mut app);
    assert_eq!(tick(&app), n);
    assert_eq!(
        app.world().resource::<GameData>().score,
        0,
        "the coin is out of reach now"
    );
    assert_eq!(heard(&app, "time.diverged"), vec![json!({ "tick": n })]);
    let answer = last_answer(&app);
    assert!(
        answer["marks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["label"] == "Edited the level" && m["tick"] == n),
        "{answer}"
    );
}

#[test]
fn a_plain_branch_changes_nothing_before_the_tick() {
    let mut app = scripted();
    play_to(&mut app, 200);
    command(&mut app, "time.pause", json!({}));
    command(&mut app, "time.seek", json!({ "tick": 150 }));
    settle(&mut app);
    let n = tick(&app);
    assert_eq!(n, 150);
    let want = print(&mut app);
    // A stray run entity: the branch rebuilds the world, so it goes.
    let stray = app.world_mut().spawn(GameEntity).id();
    command(&mut app, "time.branch", json!({}));
    settle(&mut app);
    assert_eq!(tick(&app), n);
    assert_eq!(print(&mut app), want);
    assert!(
        app.world().get_entity(stray).is_err(),
        "the run was rebuilt"
    );
    assert!(heard(&app, "time.diverged").is_empty());
    assert!(!app.world().resource::<Paused>().0, "a branch plays on");
    assert!(!app.world().contains_resource::<PlaybackFeeder>());
    let tt = app.world().resource::<TimeTravel>();
    assert!(tt.before.is_some(), "kept for undo");
    assert_eq!(tt.recording.as_ref().map(|r| r.ticks), Some(150));
    let answer = last_answer(&app);
    assert_eq!(answer["of"], 150, "the recording after 150 is gone");
    assert!(answer["marks"].is_array(), "a branch answers with marks");
}

#[test]
fn undo_puts_the_run_before_the_branch_back() {
    let mut app = scripted();
    play_to(&mut app, 200);
    command(&mut app, "time.pause", json!({}));
    let of = tick(&app);
    let mut original = scripted();
    play_to(&mut original, of);
    let want = print(&mut original);

    command(&mut app, "time.seek", json!({ "tick": 100 }));
    settle(&mut app);
    let (pre, pre_marks) = {
        let tt = app.world().resource::<TimeTravel>();
        (tt.recording.clone().unwrap(), tt.marks.clone())
    };
    assert_eq!(pre.ticks, of);
    command(&mut app, "time.branch", json!({}));
    settle(&mut app);
    assert!(!app.world().resource::<Paused>().0, "a branch plays on");
    play_to(&mut app, 130);
    command(&mut app, "time.pause", json!({}));
    command(
        &mut app,
        "time.seek",
        json!({ "tick": 100, "before_branch": true }),
    );
    settle(&mut app);
    assert_eq!(tick(&app), 100);
    let answer = last_answer(&app);
    assert_eq!(answer["of"], of);
    assert_eq!(answer["tick"], 100);
    assert_eq!(answer["paused"], true, "an undo is a seek: it ends paused");
    assert!(
        answer["marks"].is_array(),
        "the editor gets the run's marks back"
    );
    let tt = app.world().resource::<TimeTravel>();
    assert!(tt.before.is_none());
    assert_eq!(
        tt.recording.as_ref(),
        Some(&pre),
        "the pre-branch recording"
    );
    assert_eq!(tt.marks, pre_marks, "the pre-branch marks");
    command(&mut app, "time.resume", json!({}));
    play_to(&mut app, of);
    assert_eq!(print(&mut app), want, "the pre-branch inputs play back");
}

#[test]
fn a_branch_that_enters_playing_again_keeps_its_timeline_and_held_edits() {
    // `on_run_start` runs on every entry to `Playing`; a re-entering
    // branch (from the game-over screen or the level editor) is the same
    // run and must not get a fresh timeline.
    let mut app = scripted();
    play_to(&mut app, 100);
    command(&mut app, "time.pause", json!({}));
    let n = tick(&app);
    let mut straight = scripted();
    play_to(&mut straight, n);
    let want = print(&mut straight);
    let sum = app.world().resource::<Checksum>().0;
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::GameOver);
    app.update();
    assert!(app.world().resource::<State<GameState>>().get() == &GameState::GameOver);

    let (id, v) = an_edit(&app);
    app.world_mut()
        .resource_mut::<PendingTunes>()
        .0
        .push((id, v.clone()));
    // The game-over screen left the run's seed and a start value behind:
    // `prepare_reentry` must stage the run's own.
    let starts = app.world().resource::<TuneLog>().starts.clone();
    let (sid, sv) = an_edit(&app);
    write_value(app.world_mut(), sid, &sv);
    app.world_mut().resource_mut::<PendingSeed>().0 = Some([7; 32]);
    let seek = branch_seek(app.world_mut(), n, Some(sum), Phase::Reenter, Vec::new())
        .expect("a timeline to branch");
    app.world_mut().insert_resource(seek);
    settle(&mut app);

    assert_eq!(tick(&app), n);
    assert_eq!(
        app.world().resource::<TuneLog>().starts,
        starts,
        "the run's own start values"
    );
    assert_eq!(
        app.world().resource::<RunSeed>().bytes,
        selftest::SELFTEST_SEED,
        "the run's own seed"
    );
    let tt = app.world().resource::<TimeTravel>();
    assert_eq!(tt.run, 1, "the same run, not a fresh one");
    assert!(
        tt.before.is_some(),
        "the run before the branch is kept for undo"
    );
    assert_eq!(tt.recording.as_ref().map(|r| r.ticks), Some(n));
    assert_eq!(print(&mut app), want, "the recording played back to N");
    assert!(heard(&app, "time.diverged").is_empty());
    assert_eq!(
        app.world().resource::<PendingTunes>().0,
        vec![(id, v.clone())],
        "the held edit is queued again"
    );
    assert!(!app.world().resource::<Paused>().0, "a branch plays on");
    assert!(
        app.world().resource::<RunEdited>().0,
        "`BeginRun` cleared it; the re-entry is still an edited run"
    );
    assert_eq!(last_answer(&app)["edited"], true);
    app.update();
    assert_eq!(
        app.world().resource::<TuneLog>().changes,
        vec![TuneChange {
            tick: n + 1,
            id,
            value: v
        }]
    );
}

#[test]
fn a_branched_run_that_enters_playing_again_and_ends_posts_nothing() {
    let mut app = scripted();
    play_to(&mut app, 100);
    command(&mut app, "time.pause", json!({}));
    let n = tick(&app);
    let sum = app.world().resource::<Checksum>().0;
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
    let seek = branch_seek(app.world_mut(), n, Some(sum), Phase::Reenter, Vec::new())
        .expect("a timeline to branch");
    app.world_mut().insert_resource(seek);
    settle(&mut app);
    assert_eq!(tick(&app), n);
    // Play on without touching the timeline again, then end the run.
    play_to(&mut app, n + 60);
    let r = leave(&mut app);
    assert!(r.edited, "sealed edited");
    assert_eq!(app.world().resource::<Heard>().runs, 0, "never submitted");
    assert_eq!(verify_bytes(&r.encode()), Err(Refused::Edited));
}

const LOSE_A_LIFE: &str =
    r#"{"v":1,"rules":[{"id":"ouch","when":{"every":30},"do":[{"add":["lives",-1]}]}]}"#;
const END_AT_120: &str =
    r#"{"v":1,"rules":[{"id":"end","when":{"every":120},"do":[{"end_run":"lose"}]}]}"#;

fn marks(app: &App) -> Vec<Json> {
    heard(app, "time.mark")
}

fn state(app: &App) -> GameState {
    *app.world().resource::<State<GameState>>().get()
}

fn play_until_over(app: &mut App) {
    for _ in 0..1000 {
        if state(app) != GameState::Playing {
            return;
        }
        app.update();
    }
    panic!("the run never ended");
}

#[test]
fn a_lost_life_is_marked_on_its_tick() {
    let mut app = on_rules(LOSE_A_LIFE, &[FAR], false);
    play_to(&mut app, 100);
    let lost = |t: u32| json!({ "tick": t, "kind": "lives", "label": "Lost a life" });
    assert_eq!(marks(&app), vec![lost(30), lost(60), lost(90)]);
}

#[test]
fn the_end_of_the_run_is_marked() {
    let mut app = on_rules(END_AT_120, &[FAR], false);
    play_until_over(&mut app);
    assert_eq!(
        marks(&app),
        vec![json!({ "tick": 120, "kind": "end", "label": "Lost the run" })]
    );
}

#[test]
fn an_applied_change_and_a_rules_swap_are_marked() {
    let mut app = on_rules(ADD_1, &[FAR], false);
    play_to(&mut app, 20);
    let (id, v) = an_edit(&app);
    app.world_mut()
        .resource_mut::<PendingTunes>()
        .0
        .push((id, v));
    play_to(&mut app, 40);
    set_rules(app.world_mut(), ADD_10.as_bytes()).unwrap();
    play_to(&mut app, 50);
    assert_eq!(
        marks(&app),
        vec![
            json!({ "tick": 21, "kind": "edit", "label": "Changed a setting" }),
            json!({ "tick": 41, "kind": "edit", "label": "Changed the rules" }),
        ]
    );
}

#[test]
fn playback_after_a_seek_repeats_no_mark() {
    let mut app = on_rules(LOSE_A_LIFE, &[FAR], false);
    play_to(&mut app, 100);
    command(&mut app, "time.pause", json!({}));
    forget(&mut app);
    command(&mut app, "time.seek", json!({ "tick": 10 }));
    settle(&mut app);
    command(&mut app, "time.resume", json!({}));
    play_to(&mut app, 100);
    assert!(marks(&app).is_empty(), "{:?}", marks(&app));
    assert_eq!(app.world().resource::<TimeTravel>().marks.len(), 3);
}

#[test]
fn the_catalog_offers_time_travel() {
    let app = scripted();
    let j = gamebient_game::game::tunables::protocol::catalog_json(app.world());
    assert_eq!(j["time"], gamebient_game::game::time::catalog());
    assert_eq!(j["time"]["step_max"], 600);
    assert_eq!(j["time"]["update_every"], 30);
    assert_eq!(j["time"]["progress_every"], 2000);
    assert_eq!(j["time"]["speeds"], json!([0.5, 1.0, 2.0]));
}

#[test]
fn a_seek_after_the_run_ended_goes_back_into_it() {
    let mut app = on_rules(END_AT_120, &[FAR], false);
    play_until_over(&mut app);
    assert_eq!(state(&app), GameState::GameOver);
    let run = last_answer(&app)["run"].clone();
    command(&mut app, "time.seek", json!({ "tick": 60 }));
    settle(&mut app);
    assert_eq!(state(&app), GameState::Playing);
    assert_eq!(tick(&app), 60);
    assert!(app.world().resource::<Paused>().0);
    assert_eq!(last_answer(&app)["run"], run, "the same timeline");
    assert!(
        app.world().resource::<RunEdited>().0,
        "a re-entry stays edited"
    );
    assert_eq!(last_answer(&app)["edited"], true);
    // A run that ended: the furthest seek is one tick before its end.
    command(&mut app, "time.seek", json!({ "tick": 500 }));
    settle(&mut app);
    assert_eq!(tick(&app), 119);
    assert_eq!(state(&app), GameState::Playing);
}

#[test]
fn a_level_edit_made_in_the_level_editor_returns_into_a_branch() {
    let mut app = on_rules(NO_RULES, ONE_COIN, true);
    play_to(&mut app, 150);
    command(&mut app, "time.pause", json!({}));
    let n = tick(&app);
    let run = last_answer(&app)["run"].clone();
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Editing);
    app.update();
    assert_eq!(state(&app), GameState::Editing);
    app.world_mut().resource_mut::<Level>().0.things[1].at = [-8.0, 6.0];
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Playing);
    app.update();
    settle(&mut app);
    assert_eq!(state(&app), GameState::Playing);
    assert_eq!(tick(&app), n, "back at the tick the edit was made from");
    assert!(!app.world().resource::<Paused>().0, "and playing on");
    assert_eq!(app.world().resource::<GameData>().score, 0);
    assert_eq!(heard(&app, "time.diverged"), vec![json!({ "tick": n })]);
    assert_eq!(last_answer(&app)["run"], run);
    assert!(
        app.world().resource::<RunEdited>().0,
        "a re-entry stays edited"
    );
}

#[test]
fn restarting_from_the_level_editor_starts_a_fresh_run() {
    let mut app = on_rules(NO_RULES, ONE_COIN, true);
    play_to(&mut app, 60);
    command(&mut app, "time.pause", json!({}));
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Editing);
    app.update();
    command(&mut app, "run.restart", Json::Null);
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(state(&app), GameState::Playing);
    assert_eq!(last_answer(&app)["run"], 2);
    assert!(!app.world().resource::<Paused>().0);
    assert!(tick(&app) < 10);
}

/// Writes `v` into tunable `id` directly, as a game-over screen edit would.
fn write_value(world: &mut World, id: u16, v: &Value) {
    let idx = world.resource::<TunableIndex>().clone();
    let entry = idx.by_id(id).expect("a registered tunable");
    assert!(write(world, entry, v), "a value in range");
}

#[test]
fn the_editor_hears_when_the_run_is_over() {
    let mut app = on_rules(END_AT_120, &[FAR], false);
    play_to(&mut app, 60);
    assert_eq!(last_answer(&app)["over"], false, "{}", last_answer(&app));
    play_until_over(&mut app);
    let a = last_answer(&app);
    assert_eq!(a["over"], true, "{a}");
    assert_eq!(a["seeking"], false);
    assert_eq!(a["tick"], 120);
    // Still rewindable: a seek is taken and the run is back in `Playing`.
    command(&mut app, "time.seek", json!({ "tick": 30 }));
    assert_eq!(last_answer(&app)["seeking"], true);
    assert!(last_answer(&app).get("refused").is_none());
    settle(&mut app);
    let a = last_answer(&app);
    assert_eq!(
        (a["over"].clone(), a["tick"].clone()),
        (json!(false), json!(30))
    );
}

#[test]
fn a_refused_command_says_why_and_edits_nothing() {
    let mut app = scripted();
    play_to(&mut app, 40);
    let refused = |app: &mut App, name: &str, data: Json| {
        forget(app);
        command(app, name, data);
        let a = last_answer(app);
        a["refused"]
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| panic!("{name}: {a}"))
    };
    assert_eq!(
        refused(&mut app, "time.speed", json!({ "x": 3 })),
        "The speed is 0.5, 1 or 2."
    );
    assert_eq!(refused(&mut app, "time.branch", json!({})), "Pause first.");
    assert_eq!(
        refused(
            &mut app,
            "time.seek",
            json!({ "tick": 5, "before_branch": true })
        ),
        "There is no branch to undo."
    );
    assert!(!app.world().resource::<RunEdited>().0, "nothing was taken");
    assert_eq!(last_answer(&app)["edited"], false);
    // A taken command carries no `refused`.
    command(&mut app, "time.pause", json!({}));
    assert!(last_answer(&app).get("refused").is_none());
    assert_eq!(last_answer(&app)["edited"], true);
    // During a seek, only another seek is taken.
    send(&mut app, "time.seek", json!({ "tick": 0 }));
    send(&mut app, "time.resume", json!({}));
    forget(&mut app);
    app.update();
    let answers = heard(&app, "time");
    assert!(
        answers
            .iter()
            .any(|a| a["refused"] == "A seek is still running."),
        "{answers:?}"
    );
    settle(&mut app);
    // Outside a run there is nothing to command.
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Menu);
    app.update();
    assert_eq!(
        refused(&mut app, "time.pause", json!({})),
        "No run is playing."
    );
    assert_eq!(
        refused(&mut app, "time.seek", json!({ "tick": 5 })),
        "There is no run to seek in."
    );
}

#[test]
fn a_step_across_an_update_tick_sends_only_its_paused_answer() {
    let mut app = scripted();
    play_to(&mut app, 50);
    command(&mut app, "time.pause", json!({}));
    let at = tick(&app);
    forget(&mut app);
    command(&mut app, "time.step", json!({ "n": 45 }));
    assert_eq!(tick(&app), at + 45, "the step crossed ticks 60 and 90");
    let answers = heard(&app, "time");
    assert_eq!(answers.len(), 1, "{answers:?}");
    assert_eq!(answers[0]["paused"], true);
    assert_eq!(answers[0]["tick"], at + 45);
}

#[test]
fn a_seek_after_the_run_ended_runs_on_its_own_rules_and_keeps_the_editors() {
    const SCORE_THEN_END: &str = r#"{"v":1,"rules":[{"id":"a","when":{"every":1},"do":[{"add":["score",1]}]},{"id":"end","when":{"every":120},"do":[{"end_run":"lose"}]}]}"#;
    let mut app = on_rules(SCORE_THEN_END, &[FAR], false);
    play_until_over(&mut app);
    set_rules(app.world_mut(), ADD_10.as_bytes()).unwrap();
    let shown = app.world().resource::<Rules>().clone();
    command(&mut app, "time.seek", json!({ "tick": 60 }));
    settle(&mut app);
    assert_eq!(tick(&app), 60);
    assert_eq!(
        app.world().resource::<GameData>().score,
        60,
        "re-simulated on the run's own rules"
    );
    assert_eq!(
        *app.world().resource::<Rules>(),
        shown,
        "the editor's rules are back for what follows"
    );
}

#[test]
fn a_held_run_quit_to_the_title_starts_fresh_after_the_level_editor() {
    let mut app = on_rules(NO_RULES, ONE_COIN, true);
    play_to(&mut app, 60);
    command(&mut app, "time.pause", json!({}));
    // `pause_quit` works under the timeline's hold: the run goes to the title.
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Menu);
    app.update();
    assert_eq!(state(&app), GameState::Menu);
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Editing);
    app.update();
    assert_eq!(state(&app), GameState::Editing);
    assert_eq!(app.world().resource::<TimeTravel>().edit_return, None);
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Playing);
    app.update();
    settle(&mut app);
    assert_eq!(state(&app), GameState::Playing);
    assert_eq!(last_answer(&app)["run"], 2, "a fresh run");
    assert!(tick(&app) < 10);
    assert!(!app.world().resource::<Paused>().0);
    assert!(!app.world().resource::<RunEdited>().0, "and unedited");
    assert_eq!(last_answer(&app)["edited"], false);
    assert!(heard(&app, "time.diverged").is_empty());
}

#[test]
fn a_restart_sent_with_a_seek_after_the_run_ended_starts_a_fresh_run() {
    let mut app = on_rules(END_AT_120, &[FAR], false);
    play_until_over(&mut app);
    send(&mut app, "time.seek", json!({ "tick": 60 }));
    send(&mut app, "run.restart", Json::Null);
    for _ in 0..3 {
        app.update();
    }
    assert!(!app.world().contains_resource::<Seek>());
    assert_eq!(state(&app), GameState::Playing);
    assert_eq!(last_answer(&app)["run"], 2, "a fresh run, not the seek");
    assert!(tick(&app) < 10);
    assert!(!app.world().resource::<RunEdited>().0);
}
