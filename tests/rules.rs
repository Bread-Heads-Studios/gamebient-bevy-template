//! The rules interpreter on the template's kinds (Player, Coin): every
//! trigger, condition and action, the cap, clamping, determinism. Template
//! only: a game's own rules are covered by its checks (`checks_list.rs`).

use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use gamebient_game::game::audio::SfxEvent;
use gamebient_game::game::content::kit::{Kinds, Thing};
use gamebient_game::game::content::level::{Level, LevelSource};
use gamebient_game::game::content::model::LEVEL_MAX_THINGS;
use gamebient_game::game::content::model::{LevelFile, PlacedThing};
use gamebient_game::game::content::saved::reset_to_defaults;
use gamebient_game::game::replay::RulesRecord;
use gamebient_game::game::replay::build_headless_app;
use gamebient_game::game::rule_table::COUNTERS;
use gamebient_game::game::rules::model::{MAX_BYTES, canonical_text};
use gamebient_game::game::rules::run::MAX_WAITING;
use gamebient_game::game::rules::run::{RunRules, run_rules};
use gamebient_game::game::rules::{
    Outcome, PendingRules, RulesLog, RunEnd, extreme_rules, set_rules, vocab,
};
use gamebient_game::game::scoring::GameData;
use gamebient_game::game::sim::{Checksum, PendingSeed, RunOver, SimTick, SpawnOrder, seed_bytes};
use gamebient_game::game::states::GameState;
use gamebient_input::{Buttons, VirtualInput};
use serde_json::{Value as Json, json};

#[derive(Resource, Default)]
struct Heard(Vec<SfxEvent>);

fn hear(mut r: MessageReader<SfxEvent>, mut h: ResMut<Heard>) {
    h.0.extend(r.read().copied());
}

/// The player, far from everything.
const FAR: (&str, f32, f32) = ("Player", -9.0, -7.0);

/// A run on `things` (kind, x, y) under `rules` (the JSON of the rules
/// list), seeded with `seed`; returns once tick 1 has run.
fn run_with_seed(rules: &str, things: &[(&str, f32, f32)], seed: u64) -> App {
    run_file(&format!(r#"{{"v":1,"rules":{rules}}}"#), things, seed)
}

/// A run on `things` under the whole rules file `text`, seeded with
/// `seed`; returns once tick 1 has run.
fn run_file(text: &str, things: &[(&str, f32, f32)], seed: u64) -> App {
    let mut app = build_headless_app();
    reset_to_defaults(app.world_mut());
    app.init_resource::<Heard>().add_systems(Last, hear);
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
    set_rules(app.world_mut(), text.as_bytes()).unwrap();
    app.world_mut().resource_mut::<PendingSeed>().0 = Some(seed_bytes(seed));
    app.update();
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Playing);
    while app.world().resource::<SimTick>().0 < 1 {
        app.update();
    }
    app
}

fn run(rules: &str, things: &[(&str, f32, f32)]) -> App {
    run_with_seed(rules, things, 3)
}

fn ticks(app: &mut App, n: u32) {
    for _ in 0..n {
        app.update();
    }
}

fn tick(app: &App) -> u32 {
    app.world().resource::<SimTick>().0
}

fn data(app: &App) -> &GameData {
    app.world().resource::<GameData>()
}

/// Where every `kind` is, oldest first.
fn pos(app: &mut App, kind: &str) -> Vec<Vec2> {
    let k = app.world().resource::<Kinds>().index(kind);
    let mut v: Vec<(u64, Vec2)> = app
        .world_mut()
        .query::<(&SpawnOrder, &Thing, &Transform)>()
        .iter(app.world())
        .filter(|(_, t, _)| Some(t.kind) == k)
        .map(|(o, _, tf)| (o.0, tf.translation.truncate()))
        .collect();
    v.sort_by_key(|x| x.0);
    v.into_iter().map(|x| x.1).collect()
}

#[test]
fn start_fires_on_the_first_tick_only() {
    let mut app = run(
        r#"[{"id":"a","when":{"start":true},"do":[{"add":["score",5]}]}]"#,
        &[FAR],
    );
    assert_eq!(tick(&app), 1);
    assert_eq!(data(&app).score, 5);
    ticks(&mut app, 10);
    assert_eq!(data(&app).score, 5);
}

#[test]
fn every_fires_on_each_multiple_of_its_ticks() {
    let mut app = run(
        r#"[{"id":"a","when":{"every":3},"do":[{"add":["score",1]}]}]"#,
        &[FAR],
    );
    ticks(&mut app, 8);
    assert_eq!(tick(&app), 9);
    assert_eq!(data(&app).score, 3);
}

#[test]
fn a_press_fires_once_and_a_hold_every_tick() {
    let mut app = run(
        r#"[{"id":"p","when":{"button":["A","pressed"]},"do":[{"add":["score",1]}]},{"id":"h","when":{"button":["B","held"]},"do":[{"add":["lives",1]}]}]"#,
        &[FAR],
    );
    let lives = data(&app).lives;
    app.world_mut().resource_mut::<VirtualInput>().latched |= Buttons::A;
    ticks(&mut app, 3);
    assert_eq!(data(&app).score, 1);
    app.world_mut()
        .resource_mut::<VirtualInput>()
        .set_held(Buttons::B);
    ticks(&mut app, 4);
    app.world_mut()
        .resource_mut::<VirtualInput>()
        .set_held(Buttons::NONE);
    ticks(&mut app, 2);
    assert_eq!(data(&app).lives, lives + 4);
}

#[test]
fn touch_visits_every_pair_and_binds_self_and_other() {
    // Two coins 0.5 apart touch each other: two pairs, (1, 2) then (2, 1).
    let mut app = run(
        r#"[{"id":"a","when":{"touch":["Coin","Coin"]},"do":[{"add":["score",1]},{"move":["other",0,3]}]}]"#,
        &[FAR, ("Coin", 5.0, 5.0), ("Coin", 5.0, 5.5)],
    );
    assert_eq!(data(&app).score, 2);
    assert_eq!(
        pos(&mut app, "Coin"),
        vec![Vec2::new(5.0, 8.0), Vec2::new(5.0, 8.5)]
    );
    ticks(&mut app, 1);
    assert_eq!(data(&app).score, 4);
}

#[test]
fn spawns_and_removals_apply_at_the_end_of_the_tick() {
    let mut app = run(
        r#"[{"id":"a","when":{"start":true},"do":[{"spawn":["Coin",{"x":4,"y":4}]},{"remove":"Coin"},{"spawn":["Coin","start"]}]}]"#,
        &[FAR, ("Coin", 6.0, 6.0)],
    );
    // The placed coin went; the two spawned were not there yet to remove.
    assert_eq!(
        pos(&mut app, "Coin"),
        vec![Vec2::new(4.0, 4.0), Vec2::new(6.0, 6.0)]
    );
}

#[test]
fn spawns_stop_at_the_level_limit() {
    let mut things = vec![FAR];
    things.extend(std::iter::repeat_n(("Coin", 9.0, 7.0), 1998));
    let mut app = run(
        r#"[{"id":"a","when":{"every":1},"do":[{"spawn":["Coin",{"x":9,"y":7}]},{"spawn":["Coin",{"x":9,"y":7}]}]}]"#,
        &things,
    );
    ticks(&mut app, 2);
    let n = app.world_mut().query::<&Thing>().iter(app.world()).count();
    assert_eq!(n, 2000);
}

#[test]
fn respawn_returns_a_thing_to_where_it_was_placed() {
    let mut app = run(
        r#"[{"id":"m","when":{"start":true},"do":[{"move":["Coin",2,1]}]},{"id":"r","when":{"every":5},"do":[{"respawn":"Coin"}]}]"#,
        &[FAR, ("Coin", 6.0, 6.0)],
    );
    assert_eq!(pos(&mut app, "Coin"), vec![Vec2::new(8.0, 7.0)]);
    ticks(&mut app, 4);
    assert_eq!(pos(&mut app, "Coin"), vec![Vec2::new(6.0, 6.0)]);
}

#[test]
fn a_counter_trigger_fires_once_on_its_edge() {
    let mut app = run(
        r#"[{"id":"e","when":{"counter":["score",">=",3]},"do":[{"add":["lives",1]}]},{"id":"s","when":{"every":1},"do":[{"add":["score",1]}]}]"#,
        &[FAR],
    );
    let lives = data(&app).lives;
    ticks(&mut app, 10);
    assert_eq!(data(&app).score, 11);
    assert_eq!(data(&app).lives, lives + 1);
}

#[test]
fn a_timer_rings_once_after_its_ticks() {
    let mut app = run(
        r#"[{"id":"a","when":{"start":true},"do":[{"timer":["t",5]}]},{"id":"b","when":{"timer":"t"},"do":[{"add":["score",10]}]}]"#,
        &[FAR],
    );
    ticks(&mut app, 4);
    assert_eq!((tick(&app), data(&app).score), (5, 0));
    ticks(&mut app, 1);
    assert_eq!((tick(&app), data(&app).score), (6, 10));
    ticks(&mut app, 10);
    assert_eq!(data(&app).score, 10);
    assert!(app.world().resource::<RunRules>().timers.is_empty());
}

#[test]
fn a_compare_condition_gates_a_firing() {
    let mut app = run(
        r#"[{"id":"a","when":{"every":1},"if":[{"compare":["score","<",5]}],"do":[{"add":["score",1]}]}]"#,
        &[FAR],
    );
    ticks(&mut app, 20);
    assert_eq!(data(&app).score, 5);
}

#[test]
fn chance_draws_from_the_run_seed() {
    let rules =
        r#"[{"id":"a","when":{"every":1},"if":[{"chance":50}],"do":[{"add":["score",1]}]}]"#;
    let mut a = run_with_seed(rules, &[FAR], 11);
    let mut b = run_with_seed(rules, &[FAR], 11);
    ticks(&mut a, 199);
    ticks(&mut b, 199);
    assert_eq!(data(&a).score, data(&b).score);
    assert!((60..140).contains(&data(&a).score), "{}", data(&a).score);
}

#[test]
fn counters_clamp_to_their_range() {
    let app = run(
        r#"[{"id":"a","when":{"start":true},"do":[{"add":["score",-100]},{"set":["lives",1000]}]}]"#,
        &[FAR],
    );
    assert_eq!(data(&app).score, 0);
    assert_eq!(data(&app).lives, 99);
}

/// `CounterDef.set` casts with `as u32`, so a value must be clamped to the
/// counter's range before it is written, or a negative one would wrap.
#[test]
fn add_clamps_before_the_counter_is_written() {
    let start = |n: i64| {
        format!(r#"[{{"id":"a","when":{{"start":true}},"do":[{{"add":["lives",{n}]}}]}}]"#)
    };
    // Past the max.
    assert_eq!(data(&run(&start(1000), &[FAR])).lives, 99);
    // Past the min.
    assert_eq!(data(&run(&start(-1000), &[FAR])).lives, 0);
    // To a negative value: -1 would wrap to u32::MAX, in score and in the
    // high score that follows it.
    let app = run(
        r#"[{"id":"a","when":{"start":true},"do":[{"add":["score",-1]}]}]"#,
        &[FAR],
    );
    assert_eq!(data(&app).score, 0);
    assert!(
        data(&app).high_score < 1_000_000,
        "{}",
        data(&app).high_score
    );
}

#[test]
fn sound_asks_for_the_effect() {
    let app = run(
        r#"[{"id":"a","when":{"start":true},"do":[{"sound":"confirm"}]}]"#,
        &[FAR],
    );
    let heard = &app.world().resource::<Heard>().0;
    assert_eq!(heard.iter().filter(|e| **e == SfxEvent::Confirm).count(), 1);
}

#[test]
fn end_run_freezes_the_sim_and_says_how() {
    let mut app = run(
        r#"[{"id":"a","when":{"every":3},"do":[{"end_run":"win"}]}]"#,
        &[FAR],
    );
    ticks(&mut app, 2);
    assert!(app.world().resource::<RunOver>().0);
    assert_eq!(app.world().resource::<RunEnd>().0, Some(Outcome::Win));
    assert_eq!(tick(&app), 3);
    ticks(&mut app, 3);
    assert_eq!(tick(&app), 3);
    assert_eq!(
        *app.world().resource::<State<GameState>>().get(),
        GameState::GameOver
    );
}

#[test]
fn the_action_cap_carries_the_rest_to_the_next_tick() {
    let mut things = vec![FAR];
    things.extend(std::iter::repeat_n(("Coin", 5.0, 5.0), 70));
    let mut app = run(
        r#"[{"id":"a","when":{"touch":["Coin","Coin"]},"do":[{"add":["score",1]}]}]"#,
        &things,
    );
    // 70 coins on one spot are 70 x 69 = 4830 pairs: 4096 run on tick 1.
    assert_eq!(data(&app).score, 4096);
    assert_eq!(
        app.world().resource::<RunRules>().waiting.len(),
        4830 - 4096
    );
    ticks(&mut app, 1);
    assert_eq!(data(&app).score, 8192);
}

#[test]
fn an_empty_file_folds_nothing() {
    let mut app = run("[]", &[FAR]);
    let before = *app.world().resource::<Checksum>();
    app.world_mut().run_system_once(run_rules).unwrap();
    assert_eq!(*app.world().resource::<Checksum>(), before);
}

#[test]
fn the_same_run_twice_gives_the_same_checksum() {
    let rules = r#"[{"id":"r","when":{"every":7},"if":[{"chance":60}],"do":[{"spawn":["Coin","random"]},{"timer":["t",3]}]},{"id":"t","when":{"timer":"t"},"do":[{"move":["Coin",1,-1]},{"add":["score",2]}]},{"id":"c","when":{"touch":["Coin","Coin"]},"do":[{"remove":"self"}]}]"#;
    let sums: Vec<u64> = (0..2)
        .map(|_| {
            let mut app = run_with_seed(rules, &[FAR, ("Coin", 1.0, 1.0)], 5);
            ticks(&mut app, 299);
            app.world().resource::<Checksum>().0
        })
        .collect();
    assert_eq!(sums[0], sums[1]);
}

#[test]
fn nothing_panics_on_things_that_are_gone() {
    let mut app = run(
        r#"[{"id":"a","when":{"every":1},"do":[{"remove":"Player"},{"respawn":"Player"},{"move":["Player",1,1]},{"spawn":["Player","start"]}]}]"#,
        &[FAR],
    );
    ticks(&mut app, 10);
    assert!(app.world().resource::<PendingRules>().0.is_none());
}

#[test]
fn new_rules_land_on_the_next_tick_of_the_run_in_progress() {
    let mut app = run(
        r#"[{"id":"a","when":{"every":1},"do":[{"add":["score",1]}]}]"#,
        &[FAR],
    );
    ticks(&mut app, 4);
    assert_eq!(data(&app).score, 5);
    let sum = app.world().resource::<Checksum>().0;
    set_rules(
        app.world_mut(),
        br#"{"v":1,"rules":[{"id":"a","when":{"every":1},"do":[{"add":["score",10]}]}]}"#,
    )
    .unwrap();
    assert!(app.world().resource::<PendingRules>().0.is_some());
    assert_eq!(
        app.world().resource::<Checksum>().0,
        sum,
        "nothing folds between ticks"
    );
    ticks(&mut app, 1);
    assert_eq!(data(&app).score, 15);
    assert!(app.world().resource::<PendingRules>().0.is_none());
}

/// `extreme_rules` (64 rules x 8 actions at every limit) with each rule's
/// trigger changed by `when` and each action passed through `act`, so that
/// every rule fires. Conditions are dropped only as needed to stay within
/// the size limit.
fn extreme_firing(when: impl Fn(usize) -> Json, act: impl Fn(&Json) -> Json) -> String {
    let v = vocab(build_headless_app().world());
    let file: Json = serde_json::from_str(&extreme_rules(&v)).unwrap();
    let rules: Vec<Json> = file["rules"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let mut r = r.clone();
            assert!(r["when"].get("timer").is_some(), "{r}");
            r["when"] = when(i);
            let acts: Vec<Json> = r["do"].as_array().unwrap().iter().map(&act).collect();
            r["do"] = Json::Array(acts);
            r
        })
        .collect();
    // A longer trigger can push the file past the size limit: drop
    // conditions, last rule first, until it fits.
    let mut rules = rules;
    let mut i = rules.len();
    let mut text = canonical_text(&rules);
    while text.len() > MAX_BYTES {
        i = if i == 0 { rules.len() - 1 } else { i - 1 };
        if let Some(conds) = rules[i].get_mut("if").and_then(Json::as_array_mut) {
            conds.pop();
        }
        text = canonical_text(&rules);
    }
    text
}

/// Ticks `n` times, checking after each tick that every counter is in its
/// range, the things are within the level limit and the queue is bounded.
fn tick_within_limits(app: &mut App, n: u32) {
    for _ in 0..n {
        app.update();
        let d = data(app);
        for c in COUNTERS {
            let v = (c.get)(d);
            assert!((c.min..=c.max).contains(&v), "{} = {v}", c.name);
        }
        let things = app.world_mut().query::<&Thing>().iter(app.world()).count();
        assert!(things <= LEVEL_MAX_THINGS, "{things} things");
        let waiting = app.world().resource::<RunRules>().waiting.len();
        assert!(waiting <= MAX_WAITING, "{waiting} waiting");
    }
}

/// The spec's "no panics at extreme content" for the interpreter: the
/// limits all at once, every rule firing.
#[test]
fn extreme_rules_all_firing_never_panic() {
    let every = |_| json!({ "every": 1 });
    let keep = |a: &Json| a.clone();
    let crowd: Vec<(&str, f32, f32)> = std::iter::once(FAR)
        .chain(std::iter::repeat_n(("Coin", 5.0, 5.0), 70))
        .collect();

    // As built: `end_run` part-way through the first pass, on things that
    // `remove` has just taken.
    let mut app = run_file(&extreme_firing(every, keep), &crowd, 7);
    tick_within_limits(&mut app, 60);
    assert_eq!(app.world().resource::<RunEnd>().0, Some(Outcome::Lose));
    assert!(app.world().resource::<RunOver>().0);

    // No `end_run` and no `remove`: 60 ticks of 64 spawns a tick from 1990
    // things, so spawns meet the level limit.
    let no_end = |a: &Json| {
        if a.get("end_run").is_some() || a.get("remove").is_some() {
            json!({ "sound": "confirm" })
        } else {
            a.clone()
        }
    };
    let mut full = vec![FAR];
    full.extend(std::iter::repeat_n(("Coin", 9.0, 7.0), 1989));
    let mut app = run_file(&extreme_firing(every, no_end), &full, 7);
    tick_within_limits(&mut app, 60);
    assert!(!app.world().resource::<RunOver>().0);
    let things = app.world_mut().query::<&Thing>().iter(app.world()).count();
    assert_eq!(things, LEVEL_MAX_THINGS);

    // Every rule on a touch in a crowd of 70 coins that stays together
    // (no spawns either): 4830 pairs x 64 rules x 8 actions every tick, far
    // past the action cap, so firings carry and the queue stays full.
    let touch = |_| json!({ "touch": ["Coin", "Coin"] });
    let crowd_only = |a: &Json| {
        if a.get("spawn").is_some() {
            json!({ "sound": "confirm" })
        } else {
            no_end(a)
        }
    };
    let mut app = run_file(&extreme_firing(touch, crowd_only), &crowd, 7);
    assert_eq!(
        app.world().resource::<RunRules>().waiting.len(),
        MAX_WAITING
    );
    tick_within_limits(&mut app, 60);
    assert!(!app.world().resource::<RunOver>().0);
    assert_eq!(
        app.world().resource::<RunRules>().waiting.len(),
        MAX_WAITING
    );
}

#[test]
fn a_swap_is_logged_on_the_tick_it_lands() {
    let mut app = run(
        r#"[{"id":"a","when":{"every":1},"do":[{"add":["score",1]}]}]"#,
        &[FAR],
    );
    ticks(&mut app, 4);
    let text = br#"{"v":1,"rules":[{"id":"a","when":{"every":1},"do":[{"add":["score",10]}]}]}"#;
    set_rules(app.world_mut(), text).unwrap();
    assert!(
        app.world().resource::<RulesLog>().0.is_empty(),
        "nothing lands between ticks"
    );
    ticks(&mut app, 1);
    let tick = app.world().resource::<SimTick>().0;
    assert_eq!(
        app.world().resource::<RulesLog>().0,
        vec![RulesRecord {
            tick,
            bytes: text.to_vec()
        }]
    );
}
