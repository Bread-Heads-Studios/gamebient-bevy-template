//! The data-check interpreter (`game::checks_data::run`) on the template's
//! kinds (Player, Coin), counters (score, lives) and sounds: every step,
//! every expectation, `until`, `ever`, the failure words and the seed.
//! Template only: a game's own checks are covered by its own tests.

use gamebient_game::game::checks_data::model::{DataCheck, check_one};
use gamebient_game::game::checks_data::run::{CheckContext, NO_PLAYER, SEED, context, run_check};
use gamebient_game::game::checks_data::vocab;
use gamebient_game::game::content::model::{LevelFile, PlacedThing};
use gamebient_game::game::replay::{build_builtin_app, build_headless_app};
use gamebient_game::game::rules::model::check_file as check_rules;
use gamebient_game::game::rules::vocab as rules_vocab;
use gamebient_game::game::scenario::Scenario;

/// The built-in content's context under the rules list `rules` (JSON).
fn ctx(rules: &str) -> CheckContext {
    let app = build_builtin_app();
    let mut c = context(app.world());
    let text = format!(r#"{{"v":1,"rules":{rules}}}"#);
    c.rules = check_rules(text.as_bytes(), &rules_vocab(app.world())).unwrap();
    c
}

/// One check, checked against the template's names.
fn check(json: &str) -> DataCheck {
    let app = build_headless_app();
    check_one(&serde_json::from_str(json).unwrap(), &vocab(app.world())).unwrap()
}

fn run(json: &str, rules: &str) -> Result<(), String> {
    run_check(&check(json), &ctx(rules))
}

const COIN_RIGHT: &str = r#"{"place":[{"kind":"Coin","beside":"right"}]}"#;

#[test]
fn hold_walks_into_a_coin_beside_the_player() {
    let c = format!(
        r#"{{"id":"a","source":"s","given":{COIN_RIGHT},"do":[{{"hold":["right"],"ticks":30}}],"expect":[{{"change":["score","=",1]}},{{"count":["Coin","=",0]}}]}}"#
    );
    assert_eq!(run(&c, "[]"), Ok(()));
}

#[test]
fn press_taps_a_button_rule() {
    let rules = r#"[{"id":"a","when":{"button":["A","pressed"]},"do":[{"add":["score",5]}]}]"#;
    let c = r#"{"id":"a","source":"s","do":[{"press":"A"}],"expect":[{"change":["score","=",5]}]}"#;
    assert_eq!(run(c, rules), Ok(()));
}

#[test]
fn wait_lets_time_pass() {
    let rules = r#"[{"id":"a","when":{"every":30},"do":[{"add":["score",1]}]}]"#;
    let c = r#"{"id":"a","source":"s","do":[{"wait":90}],"expect":[{"counter":["score",">=",2]}]}"#;
    assert_eq!(run(c, rules), Ok(()));
}

#[test]
fn until_stops_as_soon_as_it_holds() {
    let rules = r#"[{"id":"a","when":{"every":10},"do":[{"add":["score",1]}]}]"#;
    let c = r#"{"id":"a","source":"s","do":[{"until":{"counter":["score",">=",3]},"within":600}],"expect":[{"counter":["score","=",3]}]}"#;
    assert_eq!(run(c, rules), Ok(()));
}

#[test]
fn until_times_out_in_seconds() {
    let c = r#"{"id":"a","source":"s","do":[{"until":{"counter":["score",">=",1]},"within":30}],"expect":[{"outcome":"playing"}]}"#;
    assert_eq!(
        run(c, "[]"),
        Err("Score is at least 1 didn't happen within 0.5 s".into())
    );
    let c = r#"{"id":"a","source":"s","do":[{"until":{"change":["lives","=",-1]},"within":120,"hold":["right"]}],"expect":[{"outcome":"playing"}]}"#;
    assert_eq!(
        run(c, "[]"),
        Err("Lives going down by 1 didn't happen within 2 s".into())
    );
}

#[test]
fn until_says_when_the_run_ended_first() {
    let rules = r#"[{"id":"a","when":{"every":30},"do":[{"end_run":"lose"}]}]"#;
    let c = r#"{"id":"a","source":"s","do":[{"until":{"outcome":"win"},"within":60}],"expect":[{"outcome":"win"}]}"#;
    assert_eq!(
        run(c, rules),
        Err("Winning didn't happen: the run was lost first".into())
    );
}

#[test]
fn steps_after_the_run_ended_are_skipped() {
    let rules = r#"[{"id":"a","when":{"start":true},"do":[{"end_run":"lose"}]}]"#;
    let c = r#"{"id":"a","source":"s","do":[{"until":{"outcome":"win"},"within":60}],"expect":[{"outcome":"win"}]}"#;
    assert_eq!(run(c, rules), Err("didn't win: the run was lost".into()));
}

#[test]
fn ever_holds_at_some_tick_while_the_end_differs() {
    let c = format!(
        r#"{{"id":"a","source":"s","given":{COIN_RIGHT},"do":[{{"hold":["right"],"ticks":30}}],"expect":[{{"count":["Coin","=",1],"ever":true}},{{"count":["Coin","=",0]}}]}}"#
    );
    assert_eq!(run(&c, "[]"), Ok(()));
    let c = format!(
        r#"{{"id":"a","source":"s","given":{COIN_RIGHT},"do":[{{"hold":["right"],"ticks":30}}],"expect":[{{"counter":["score",">=",5],"ever":true}}]}}"#
    );
    assert_eq!(
        run(&c, "[]"),
        Err("score was never at least 5 (it ended at 1)".into())
    );
}

#[test]
fn sounds_are_heard_by_name() {
    let rules = r#"[{"id":"a","when":{"start":true},"do":[{"sound":"confirm"}]}]"#;
    let c = r#"{"id":"a","source":"s","do":[{"wait":2}],"expect":[{"sound":"confirm"}]}"#;
    assert_eq!(run(c, rules), Ok(()));
    let c = r#"{"id":"a","source":"s","do":[{"wait":2}],"expect":[{"sound":"pause"}]}"#;
    assert_eq!(run(c, rules), Err("the pause sound never played".into()));
}

#[test]
fn outcomes_and_their_words() {
    // The template's own Rust rule collects a coin before the data rules run,
    // so the win follows the score it adds.
    let rules = r#"[{"id":"a","when":{"counter":["score",">=",1]},"do":[{"end_run":"win"}]}]"#;
    let c = format!(
        r#"{{"id":"a","source":"s","given":{COIN_RIGHT},"do":[{{"until":{{"outcome":"win"}},"within":120,"hold":["right"]}}],"expect":[{{"outcome":"win"}}]}}"#
    );
    assert_eq!(run(&c, rules), Ok(()));
    let c = r#"{"id":"a","source":"s","do":[],"expect":[{"outcome":"win"}]}"#;
    assert_eq!(
        run(c, rules),
        Err("didn't win: the run is still going".into())
    );
    let c = format!(
        r#"{{"id":"a","source":"s","given":{COIN_RIGHT},"do":[{{"hold":["right"],"ticks":60}}],"expect":[{{"outcome":"playing"}}]}}"#
    );
    assert_eq!(run(&c, rules), Err("the run ended: it was won".into()));
}

#[test]
fn counter_change_and_count_say_what_they_saw() {
    let c = r#"{"id":"a","source":"s","do":[],"expect":[{"change":["lives","=",-1]}]}"#;
    assert_eq!(
        run(c, "[]"),
        Err("lives went from 3 to 3; expected a change of -1".into())
    );
    let c = r#"{"id":"a","source":"s","do":[],"expect":[{"counter":["lives",">=",5]}]}"#;
    assert_eq!(
        run(c, "[]"),
        Err("lives is 3; expected it to be at least 5".into())
    );
    let c = format!(
        r#"{{"id":"a","source":"s","given":{COIN_RIGHT},"do":[],"expect":[{{"count":["Coin","=",2]}}]}}"#
    );
    assert_eq!(
        run(&c, "[]"),
        Err("the Coin count is 1; expected it to be 2".into())
    );
}

#[test]
fn set_gives_starting_values_and_change_counts_from_them() {
    let c = r#"{"id":"a","source":"s","given":{"set":[["lives",1]]},"do":[{"wait":5}],"expect":[{"counter":["lives","=",1]},{"change":["lives","=",0]}]}"#;
    assert_eq!(run(c, "[]"), Ok(()));
}

#[test]
fn the_current_level_is_the_games_saved_level() {
    let mut cx = ctx("[]");
    cx.level = LevelFile {
        v: 1,
        things: vec![
            PlacedThing {
                id: "p".into(),
                kind: "Player".into(),
                at: [-3.0, 0.0],
                size: None,
            },
            PlacedThing {
                id: "c".into(),
                kind: "Coin".into(),
                at: [-1.0, 0.0],
                size: None,
            },
        ],
    };
    let current = check(
        r#"{"id":"a","source":"s","given":{"level":"current"},"do":[{"hold":["right"],"ticks":30}],"expect":[{"change":["score","=",1]}]}"#,
    );
    assert_eq!(run_check(&current, &cx), Ok(()));
    let empty = check(
        r#"{"id":"a","source":"s","do":[{"hold":["right"],"ticks":30}],"expect":[{"change":["score","=",1]}]}"#,
    );
    assert_eq!(
        run_check(&empty, &cx),
        Err("score went from 0 to 0; expected a change of 1".into())
    );
}

#[test]
fn positions_on_the_floor_rest_on_it() {
    let c = r#"{"id":"a","source":"s","given":{"player":{"at":[3,"floor"]},"place":[{"kind":"Coin","at":[5,"floor"]}]},"do":[{"hold":["right"],"ticks":30}],"expect":[{"change":["score","=",1]}]}"#;
    assert_eq!(run(c, "[]"), Ok(()));
}

#[test]
fn beside_needs_a_player() {
    let mut cx = ctx("[]");
    cx.level = LevelFile {
        v: 1,
        things: vec![PlacedThing {
            id: "c".into(),
            kind: "Coin".into(),
            at: [0.0, 0.0],
            size: None,
        }],
    };
    let c = check(
        r#"{"id":"a","source":"s","given":{"level":"current","place":[{"kind":"Coin","beside":"left"}]},"do":[],"expect":[{"outcome":"playing"}]}"#,
    );
    assert_eq!(run_check(&c, &cx), Err(NO_PLAYER.into()));
}

#[test]
fn the_seed_is_fixed() {
    let rules =
        r#"[{"id":"a","when":{"every":1},"if":[{"chance":50}],"do":[{"add":["score",1]}]}]"#;
    let cx = ctx(rules);
    let mut s = Scenario::new();
    s.seed(SEED).rules(cx.rules.clone());
    for (p, v) in &cx.tunes {
        s.tune(p, v.clone());
    }
    s.start().tick(120);
    let score = s.counter("score").unwrap();
    assert!(score > 0 && score < 121, "{score}");
    let c = check(&format!(
        r#"{{"id":"a","source":"s","do":[{{"wait":120}}],"expect":[{{"counter":["score","=",{score}]}}]}}"#
    ));
    assert_eq!(run_check(&c, &cx), Ok(()));
    assert_eq!(run_check(&c, &cx), Ok(()), "the same every time");
}
