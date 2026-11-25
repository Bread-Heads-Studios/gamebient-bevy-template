//! What this game's rules may name, beyond the kinds (`thing_kind`) and the
//! sounds (`SfxEvent`): the counters rules read and write, the hooks for
//! actions that need the game's own physics, the screen `random` places
//! fall on, and the rules the selftest and the named checks run on.
//!
//! TEMPLATE NOTE: this file is the game's own (the rest of `rules/` is
//! copied verbatim). List every `GameData` field a rule may change in
//! `COUNTERS`, give `JUMP` a function when the game has bodies that can
//! jump (it must respect the game's own grounded check), and `STOP` one
//! that stops a thing's motion (`respawn` calls it). `test_rules()` is
//! built in Rust so a data save never changes the fixtures. List the
//! counters whose changes the editor's timeline marks in `MARKS`. Describe
//! the stage a data check's `"level": "empty"` builds in `STAGE`.

use bevy::prelude::*;

use super::rules::model::RulesFile;
use super::rules::{CounterDef, MarkDef, MarkWhen};
use super::scenario::StageDef;
use super::scoring::GameData;

fn set_score(d: &mut GameData, v: i64) {
    d.score = v as u32;
    d.high_score = d.high_score.max(d.score);
}

/// The counters rules may read and write, in the order the checksum folds
/// them.
pub const COUNTERS: &[CounterDef] = &[
    CounterDef {
        name: "score",
        label: "score",
        one: None,
        min: 0,
        max: 1_000_000,
        get: |d| i64::from(d.score),
        set: set_score,
    },
    CounterDef {
        name: "lives",
        label: "lives",
        one: Some("life"),
        min: 0,
        max: 99,
        get: |d| i64::from(d.lives),
        set: |d, v| d.lives = v as u32,
    },
];

/// The counter changes the editor's timeline marks.
pub const MARKS: &[MarkDef] = &[MarkDef {
    counter: "lives",
    when: MarkWhen::Falls,
    label: "Lost a life",
}];

/// The template's player has no body to jump with: no `jump` action.
pub const JUMP: Option<fn(&mut World, Entity)> = None;

/// The template's things have no motion to stop.
pub const STOP: Option<fn(&mut World, Entity)> = None;

/// Half the screen in world units (the camera shows about 22.1 x 16.6).
pub const HALF_SCREEN: Vec2 = Vec2::new(11.0, 8.3);

/// The stage a data check builds for `"level": "empty"`, in world units:
/// the player's kind and start, the floor (a Thing, or none), the line
/// Things rest on (`"floor"`), and how far `beside` places a Thing (centre
/// to centre). The template's player floats: no floor thing, and its feet
/// at y = -0.5.
pub const STAGE: StageDef = StageDef {
    player: "Player",
    floor: None,
    floor_top: -0.5,
    start: Vec2::new(0.0, 0.0),
    beside: 2.0,
};

/// The rules the replay selftest and the named checks run on
/// (`LevelSource::Test`). The template has none.
pub fn test_rules() -> RulesFile {
    RulesFile::default()
}
