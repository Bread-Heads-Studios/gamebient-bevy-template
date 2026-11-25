//! The headless harness (`game::scenario`) on the template's kinds (Player,
//! Coin) and counters (score, lives). Template only: a game's own harness
//! tests live beside its own helpers.

use bevy::prelude::*;
use gamebient_game::game::content::model::{LevelFile, PlacedThing};
use gamebient_game::game::rule_table::{COUNTERS, STAGE};
use gamebient_game::game::scenario::{Buttons, FLOOR_TOP, START, Scenario};

#[test]
fn the_stage_is_the_player_at_its_start_and_no_floor() {
    let mut s = Scenario::new();
    s.start();
    assert_eq!(s.count("Player"), 1);
    assert_eq!(s.count("Coin"), 0);
    let p = s.pos("Player").unwrap();
    assert!((p - START).length() < 0.01, "{p}");
    assert_eq!(STAGE.player, "Player");
    assert!(STAGE.floor.is_none());
    assert_eq!(FLOOR_TOP, -0.5);
}

#[test]
fn the_player_walks_and_collects_a_coin_for_a_point() {
    let mut s = Scenario::new();
    s.place("Coin", START.x + STAGE.beside, START.y).start();
    let score = s.counter("score").unwrap();
    s.hold(Buttons::RIGHT, 30);
    assert_eq!(s.count("Coin"), 0, "walking right picks the coin up");
    assert_eq!(s.counter("score"), Some(score + 1));
    let p = s.pos("Player").unwrap();
    assert!(p.x > START.x + 3.0, "{p}");
}

#[test]
fn counters_are_read_and_set_by_name_and_clamp() {
    let mut s = Scenario::new();
    s.start();
    for c in COUNTERS {
        s.set_counter(c.name, c.max + 5);
        assert_eq!(s.counter(c.name), Some(c.max), "{}", c.name);
        s.set_counter(c.name, c.min - 5);
        assert_eq!(s.counter(c.name), Some(c.min), "{}", c.name);
    }
    assert_eq!(s.counter("gems"), None);
    s.set_counter("gems", 3);
}

#[test]
fn put_moves_the_player_before_the_start_and_teleports_after() {
    let mut s = Scenario::new();
    assert!(s.put("Player", 4.0, 1.0));
    assert_eq!(s.placed_pos("Player"), Some(Vec2::new(4.0, 1.0)));
    assert!(!s.put("Coin", 0.0, 0.0), "no coin to move");
    s.start();
    assert!((s.pos("Player").unwrap() - Vec2::new(4.0, 1.0)).length() < 0.01);
    assert!(s.put("Player", -2.0, 0.0));
    assert!((s.pos("Player").unwrap() - Vec2::new(-2.0, 0.0)).length() < 0.01);
}

#[test]
fn a_level_it_is_handed_is_placed() {
    let level = LevelFile {
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
                at: [3.0, 0.0],
                size: None,
            },
            PlacedThing {
                id: "d".into(),
                kind: "Coin".into(),
                at: [5.0, 0.0],
                size: None,
            },
        ],
    };
    let mut s = Scenario::with_level(&level);
    assert_eq!(s.placed_pos("Player"), Some(Vec2::new(-3.0, 0.0)));
    s.start();
    assert_eq!(s.count("Coin"), 2);
    assert_eq!(s.size_of("Coin"), Some(Vec2::new(0.6, 0.6)));
    assert_eq!(s.size_of("Ghost"), None);
}

#[test]
fn step_holds_for_one_tick_and_press_taps() {
    let mut s = Scenario::new();
    s.start();
    let t = s.now();
    let x = s.pos("Player").unwrap().x;
    s.step(Buttons::RIGHT).step(Buttons::RIGHT);
    assert_eq!(s.now(), t + 2);
    assert!(s.pos("Player").unwrap().x > x);
    s.press(Buttons::A);
    assert_eq!(s.now(), t + 3);
}

#[test]
fn sounds_are_logged_and_heard_by_name() {
    use gamebient_game::game::rules::model::check_file;
    use gamebient_game::game::rules::vocab;
    let mut s = Scenario::new();
    let v = vocab(s.app.world());
    let rules = check_file(
        br#"{"v":1,"rules":[{"id":"a","when":{"start":true},"do":[{"sound":"confirm"}]}]}"#,
        &v,
    )
    .unwrap();
    s.rules(rules).start().tick(2);
    assert!(s.heard("confirm"));
    assert!(!s.heard("pause"));
    assert!(!s.sounds().is_empty());
}
