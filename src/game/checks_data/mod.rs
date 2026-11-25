//! Plain-English checks as data: `content/checks.json`, run by one
//! interpreter.
//!
//! - `model`: the file, its typed form and the checker every side runs
//!   (copied verbatim into the editor's server).
//! - `run`: one check on a fresh `Scenario`, with a fixed seed, under the
//!   game's current rules, tunables and level.
//!
//! The vocabulary is the game's own: its counters (`rule_table::COUNTERS`),
//! its kinds (the kit's `thing_kind` registry; `place` takes the kinds the
//! level editor may place, never the player), its sounds (`SfxEvent`) and
//! its stage (`rule_table::STAGE`). Checks are not run content: no hash
//! folds them, so no replay or leaderboard changes when they do.
//!
//! The editor runs them by name (`data:<id>`, one per frame, through
//! `checks.run`); the publish gate runs every check that is on
//! ([`tests::data_checks_pass`]).
//!
//! This folder is copied verbatim into every game (`tools/frame-accept.sh
//! --check-copies`).

pub mod model;
pub mod run;

use bevy::prelude::*;
use serde_json::{Value as Json, json};

use crate::game::audio::SfxEvent;
use crate::game::content::files::embedded;
use crate::game::content::kit::Kinds;
use crate::game::rule_table::{COUNTERS, STAGE};
use model::{ChecksFile, CounterRange, Vocab};
use run::{CheckContext, context, run_check};

/// A data check's name in `checks.run` and the catalog: `data:<id>`.
pub const PREFIX: &str = "data:";
/// The one entry a checks file the game refuses gives.
pub const FILE_ENTRY: &str = "data:checks.json";
pub const FILE_TITLE: &str = "The checks file";
/// Starts each failing check's line in `data_checks_pass`'s message. The
/// editor's gate (`crates/editor-server/src/gate.rs`) reads these lines.
pub const FAILED_MARK: &str = "GX-CHECK-FAILED ";

/// The checks the game runs: the checked file, or why it was refused (a
/// refused embedded file; `set_checks` never stores a refusal).
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct DataChecks(pub Result<ChecksFile, String>);

impl Default for DataChecks {
    fn default() -> Self {
        Self(Ok(ChecksFile::default()))
    }
}

/// Loads the embedded `content/checks.json`. Added by `GamePlugin` in both
/// modes, after `RulesPlugin` (the file names the kit's kinds).
pub struct ChecksDataPlugin;

impl Plugin for ChecksDataPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DataChecks>();
        if let Some(bytes) = embedded("checks.json") {
            let file = model::check_file(bytes, &vocab(app.world()));
            if let Err(e) = &file {
                // A committed file the game refuses fails `data_checks_pass`;
                // at runtime it is one failing entry, `data:checks.json`.
                error!("content/checks.json: {e}");
            }
            app.insert_resource(DataChecks(file));
        }
    }
}

/// The names this game's checks may use.
pub fn vocab(world: &World) -> Vocab {
    let kinds = world.get_resource::<Kinds>();
    Vocab {
        counters: COUNTERS
            .iter()
            .map(|c| CounterRange {
                name: c.name.to_string(),
                min: c.min,
                max: c.max,
            })
            .collect(),
        placeable: kinds
            .map(|k| {
                k.0.iter()
                    .filter(|(n, p)| p.edit.placeable && n.as_str() != STAGE.player)
                    .map(|(n, _)| n.clone())
                    .collect()
            })
            .unwrap_or_default(),
        kinds: kinds
            .map(|k| k.0.iter().map(|(n, _)| n.clone()).collect())
            .unwrap_or_default(),
        sounds: SfxEvent::ALL.iter().map(|e| e.name()).collect(),
    }
}

/// Checks `bytes` whole and makes it the game's checks. On error nothing
/// changes.
pub fn set_checks(world: &mut World, bytes: &[u8]) -> Result<(), String> {
    let file = model::check_file(bytes, &vocab(world))?;
    world.insert_resource(DataChecks(Ok(file)));
    Ok(())
}

/// Whether `name` (from `checks.run`) is a data check's.
pub fn is_data(name: &str) -> bool {
    name.starts_with(PREFIX)
}

/// The catalog's `checks` entries for the data checks: `{name, title,
/// data: true}` plus `off: true` for a check that is off, or the one
/// `data:checks.json` entry for a refused file.
pub fn catalog_entries(world: &World) -> Vec<Json> {
    match world.get_resource::<DataChecks>().map(|d| &d.0) {
        Some(Ok(f)) => f
            .checks
            .iter()
            .map(|c| {
                let mut j =
                    json!({ "name": format!("{PREFIX}{}", c.id), "title": c.source, "data": true });
                if c.off {
                    j["off"] = json!(true);
                }
                j
            })
            .collect(),
        Some(Err(_)) => vec![json!({ "name": FILE_ENTRY, "title": FILE_TITLE, "data": true })],
        None => Vec::new(),
    }
}

/// The catalog's `checks_data`: the vocabulary the Checks tab builds its
/// pickers from, the stage, and the limits.
pub fn checks_catalog(world: &World) -> Json {
    let v = vocab(world);
    json!({
        "counters": COUNTERS
            .iter()
            .map(|c| json!({ "name": c.name, "label": c.label, "one": c.one, "min": c.min, "max": c.max }))
            .collect::<Vec<_>>(),
        "placeable": v.placeable,
        "kinds": v.kinds,
        "sounds": v.sounds,
        "player": STAGE.player,
        "buttons": model::BUTTONS,
        "stage": { "floor_top": STAGE.floor_top, "beside": STAGE.beside, "start": [STAGE.start.x, STAGE.start.y] },
        "limits": {
            "checks": model::MAX_CHECKS,
            "bytes": model::MAX_BYTES,
            "ticks": model::MAX_TICKS,
            "place": model::MAX_PLACE,
            "set": model::MAX_SET,
            "do": model::MAX_STEPS,
            "expect": model::MAX_EXPECT,
            "id": model::MAX_ID,
            "source": model::MAX_SOURCE,
        },
    })
}

/// Runs the data check `name` (`data:<id>`) under what `world` runs now.
/// A refused file answers its reason for `data:checks.json`; any name the
/// file lacks answers "no check named …", as a Rust check does.
pub fn run_named(world: &World, name: &str) -> Result<(), String> {
    let missing = || format!("no check named {name}");
    match world.get_resource::<DataChecks>().map(|d| &d.0) {
        Some(Err(e)) if name == FILE_ENTRY => Err(e.clone()),
        Some(Ok(f)) => {
            let id = name.strip_prefix(PREFIX).ok_or_else(missing)?;
            let c = f.checks.iter().find(|c| c.id == id).ok_or_else(missing)?;
            run_check(c, &context(world))
        }
        _ => Err(missing()),
    }
}

/// Every check that is on and fails under `ctx`: (id, sentence, reason).
pub fn failing(file: &ChecksFile, ctx: &CheckContext) -> Vec<(String, String, String)> {
    file.checks
        .iter()
        .filter(|c| !c.off)
        .filter_map(|c| {
            run_check(c, ctx)
                .err()
                .map(|r| (c.id.clone(), c.source.clone(), r))
        })
        .collect()
}

/// `data_checks_pass`'s message: one summary line that names the failing
/// checks by sentence ("A check fails: <sentence>: <reason>", or "2 checks
/// fail: <sentence>; <sentence>"), then one `FAILED_MARK` line of JSON
/// `{id, source, reason}` per check for the editor's gate.
pub fn failing_message(failed: &[(String, String, String)]) -> String {
    let one_line = |s: &str| s.replace('\n', " ");
    let head = match failed {
        [(_, s, r)] => format!("A check fails: {}: {}", one_line(s), one_line(r)),
        _ => format!(
            "{} checks fail: {}",
            failed.len(),
            failed
                .iter()
                .map(|(_, s, _)| one_line(s))
                .collect::<Vec<_>>()
                .join("; ")
        ),
    };
    let lines: Vec<String> = failed
        .iter()
        .map(|(id, s, r)| {
            format!(
                "{FAILED_MARK}{}",
                json!({ "id": id, "source": s, "reason": r })
            )
        })
        .collect();
    format!("{head}\n{}", lines.join("\n"))
}

/// A checks file at the limits for `tools/test_extreme_content.sh`: 64
/// checks with 48-character ids and 200-character sentences; the first at
/// every per-check limit (16 placed things, 16 starting values, 16 steps
/// of 1800 ticks in all, 8 expectations), the second on the saved level.
/// Every expectation always holds (a counter inside its own range, a count
/// of at least 0), so the file proves a save at the limits never breaks
/// `cargo test` whatever the other content holds.
pub fn extreme_checks(v: &Vocab) -> String {
    let (c, min, max) = v
        .counters
        .first()
        .map_or((String::new(), 0, 0), |c| (c.name.clone(), c.min, c.max));
    let kind = v.kinds.last().cloned().unwrap_or_default();
    let always = json!({ "counter": [c, ">=", min] });
    let mut first = json!({
        "id": format!("a{}", "x".repeat(model::MAX_ID - 1)),
        "source": "x".repeat(model::MAX_SOURCE),
        "given": { "set": vec![json!([c, max]); model::MAX_SET] },
        "do": (0..13).map(|_| json!({ "wait": 100 }))
            .chain([
                json!({ "hold": model::BUTTONS, "ticks": 299 }),
                json!({ "press": "A" }),
                json!({ "until": always, "within": 200 }),
            ])
            .collect::<Vec<_>>(),
        "expect": [
            always,
            { "counter": [c, ">=", min], "ever": true },
            { "counter": [c, "<=", max] },
            { "change": [c, ">=", min - max] },
            { "change": [c, ">=", min - max], "ever": true },
            { "count": [kind, ">=", 0] },
            { "count": [kind, ">=", 0], "ever": true },
            { "counter": [c, "<=", max], "ever": true },
        ],
    });
    if let Some(p) = v.placeable.last() {
        let sides = ["left", "right", "above", "below"];
        first["given"]["place"] = (0..model::MAX_PLACE)
            .map(|i| {
                if i % 2 == 0 {
                    json!({ "kind": p, "beside": sides[i / 2 % 4] })
                } else {
                    json!({ "kind": p, "at": [i as i64 * 10, "floor"] })
                }
            })
            .collect();
    }
    let mut checks = vec![first];
    for i in 1..model::MAX_CHECKS {
        let mut c = json!({
            "id": format!("c{i:0>47}"),
            "source": "x".repeat(model::MAX_SOURCE),
            "do": [],
            "expect": [always],
        });
        if i == 1 {
            c["given"] = json!({ "level": "current" });
        }
        checks.push(c);
    }
    model::canonical_text(&checks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::replay::build_headless_app;

    /// The publish gate: every check that is on in the committed
    /// `content/checks.json` passes under the committed rules, level and
    /// tunables. Inherited by every game. It reads the content on purpose:
    /// a failing check is a game the creator said must not ship.
    #[test]
    fn data_checks_pass() {
        let app = build_headless_app();
        let file = match &app.world().resource::<DataChecks>().0 {
            Ok(f) => f.clone(),
            Err(e) => panic!(
                "{}",
                failing_message(&[("checks.json".into(), FILE_TITLE.into(), e.clone())])
            ),
        };
        let failed = failing(&file, &context(app.world()));
        assert!(failed.is_empty(), "{}", failing_message(&failed));
    }

    #[test]
    fn a_refused_file_is_one_failing_entry() {
        let mut app = build_headless_app();
        app.insert_resource(DataChecks(Err("Check 1: x".into())));
        assert_eq!(
            catalog_entries(app.world()),
            vec![json!({ "name": FILE_ENTRY, "title": FILE_TITLE, "data": true })]
        );
        assert_eq!(run_named(app.world(), FILE_ENTRY), Err("Check 1: x".into()));
        assert_eq!(
            run_named(app.world(), "data:a"),
            Err("no check named data:a".into())
        );
    }

    #[test]
    fn set_checks_refuses_a_bad_file_and_keeps_the_old_one() {
        let mut app = build_headless_app();
        let good = br#"{"checks":[{"id":"a","source":"The run keeps going","do":[],"expect":[{"outcome":"playing"}]}]}"#;
        set_checks(app.world_mut(), good).unwrap();
        let before = app.world().resource::<DataChecks>().clone();
        let bad =
            br#"{"checks":[{"id":"b","source":"s","do":[],"expect":[{"count":["Ghost","=",0]}]}]}"#;
        assert_eq!(
            set_checks(app.world_mut(), bad),
            Err("Check 1: This game has no kind called Ghost.".into())
        );
        assert_eq!(*app.world().resource::<DataChecks>(), before);
        assert_eq!(
            catalog_entries(app.world()),
            vec![json!({ "name": "data:a", "title": "The run keeps going", "data": true })]
        );
        assert_eq!(run_named(app.world(), "data:a"), Ok(()));
        assert!(is_data("data:a") && !is_data("coin_is_collected"));
    }

    #[test]
    fn a_check_that_is_off_is_listed_off_and_never_fails_the_gate() {
        let mut app = build_headless_app();
        let text = br#"{"checks":[{"id":"a","source":"It is won","off":true,"do":[],"expect":[{"outcome":"win"}]}]}"#;
        set_checks(app.world_mut(), text).unwrap();
        assert_eq!(catalog_entries(app.world())[0]["off"], true);
        let Ok(file) = app.world().resource::<DataChecks>().0.clone() else {
            panic!("taken");
        };
        assert!(failing(&file, &context(app.world())).is_empty());
        assert_eq!(
            run_named(app.world(), "data:a"),
            Err("didn't win: the run is still going".into()),
            "Run still runs a check that is off"
        );
    }

    #[test]
    fn the_vocabulary_places_neither_the_player_nor_runtime_only_kinds() {
        let app = build_headless_app();
        let v = vocab(app.world());
        let kinds = app.world().resource::<Kinds>();
        assert!(!v.placeable.iter().any(|k| k == STAGE.player));
        for k in &v.placeable {
            let props = kinds.props(kinds.index(k).unwrap()).unwrap();
            assert!(props.edit.placeable, "{k}");
            assert!(v.kinds.contains(k));
        }
        assert_eq!(v.kinds.len(), kinds.0.len());
        assert_eq!(Vocab::from_json(&checks_catalog(app.world())), v);
    }

    #[test]
    fn the_failing_message_names_every_check_by_sentence() {
        let one = failing_message(&[("a".into(), "S".into(), "R".into())]);
        assert_eq!(
            one,
            "A check fails: S: R\nGX-CHECK-FAILED {\"id\":\"a\",\"reason\":\"R\",\"source\":\"S\"}"
        );
        let two = failing_message(&[
            ("a".into(), "S".into(), "R".into()),
            ("b".into(), "T".into(), "Q".into()),
        ]);
        assert!(
            two.starts_with("2 checks fail: S; T\nGX-CHECK-FAILED "),
            "{two}"
        );
        assert_eq!(two.lines().count(), 3);
    }

    #[test]
    fn extreme_checks_sit_at_every_limit_and_are_taken() {
        let app = build_headless_app();
        let v = vocab(app.world());
        let text = extreme_checks(&v);
        assert!(text.len() <= model::MAX_BYTES, "{} bytes", text.len());
        let file = model::check_file(text.as_bytes(), &v).unwrap();
        assert_eq!(file.checks.len(), model::MAX_CHECKS);
        let first = &file.checks[0];
        assert_eq!(first.id.len(), model::MAX_ID);
        assert_eq!(first.source.chars().count(), model::MAX_SOURCE);
        assert_eq!(first.given.set.len(), model::MAX_SET);
        assert_eq!(first.steps.len(), model::MAX_STEPS);
        assert_eq!(
            first.steps.iter().map(model::Step::ticks).sum::<u32>(),
            model::MAX_TICKS
        );
        assert_eq!(first.expect.len(), model::MAX_EXPECT);
        if !v.placeable.is_empty() {
            assert_eq!(first.given.place.len(), model::MAX_PLACE);
        }
        assert!(failing(&file, &context(app.world())).is_empty());
    }

    /// Writes the checks file `tools/test_extreme_content.sh` saves:
    /// `GX_EXTREME=max` the file at the limits, `min` an empty one.
    #[test]
    #[ignore = "run by tools/test_extreme_content.sh"]
    fn write_extreme_checks() {
        let out = std::env::var("GX_EXTREME_OUT").expect("GX_EXTREME_OUT names the file to write");
        let app = build_headless_app();
        let v = vocab(app.world());
        let text = match std::env::var("GX_EXTREME").as_deref() {
            Ok("max") => extreme_checks(&v),
            Ok("min") => model::canonical_text(&[]),
            other => panic!("GX_EXTREME must be max or min, not {other:?}"),
        };
        model::check_file(text.as_bytes(), &v).unwrap();
        std::fs::write(out, text).unwrap();
    }
}
