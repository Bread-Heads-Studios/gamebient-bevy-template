//! Rules as data: `content/rules.json`, run by one interpreter.
//!
//! - `model`: the file, its typed form and the checker every side runs
//!   (copied verbatim into the editor's server).
//! - `run`: `begin_run_rules` (`OnEnter(Playing)`) and `run_rules` (`SimSet`).
//!
//! The game declares what rules may name in `crate::game::rule_table`: its
//! counters (`COUNTERS`), the hooks for actions that need its physics
//! (`JUMP`, `STOP`), the screen (`HALF_SCREEN`) and the rules the selftest
//! and checks run (`test_rules`). Kinds come from the kit's `thing_kind`
//! registry and sounds from `SfxEvent`.
//!
//! Rules are run content: the run key folds their hash
//! (`tunables::hash::with_rules`). A file with no rules hashes as 0, "no
//! rules", which leaves every content hash as it was before rules existed.
//!
//! This folder is copied verbatim into every game (`tools/frame-accept.sh
//! --check-copies`); a game changes `rule_table.rs`, never these files.

pub mod model;
pub mod run;

pub use run::{begin_run_rules, run_rules};

use bevy::prelude::*;
use serde_json::{Value as Json, json};

use crate::game::audio::SfxEvent;
use crate::game::content::files::{embedded, fnv1a64};
use crate::game::content::kit::Kinds;
use crate::game::content::level::LevelSource;
use crate::game::replay::{MAX_RULES_SWAPS, RulesRecord};
use crate::game::rule_table;
use crate::game::scoring::GameData;
use crate::game::states::GameState;
use model::{RulesFile, Vocab};

/// One counter rules may read and write: a `GameData` field, by name.
pub struct CounterDef {
    /// The name a rules file uses (`"lives"`).
    pub name: &'static str,
    /// What a card calls it (`"lives"`).
    pub label: &'static str,
    /// One of it, for a countable counter (`"life"`); `None` for a score.
    pub one: Option<&'static str>,
    pub min: i64,
    pub max: i64,
    pub get: fn(&GameData) -> i64,
    pub set: fn(&mut GameData, i64),
}

/// The rules the next run starts with (`LevelSource::Content`).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct Rules(pub RulesFile);

/// FNV-1a 64 of the rules file's exact bytes; 0 for no rules.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RulesHash(pub u64);

/// A rules file a run takes mid-run: the checked file, its hash (0 for no
/// rules) and its exact bytes, which the replay records (`RULE`).
#[derive(Clone, Debug, PartialEq)]
pub struct RulesSwap {
    pub file: RulesFile,
    pub hash: u64,
    pub bytes: Vec<u8>,
}

/// A file `content.set` swapped in during a run (or a replay's recorded
/// swap): `run_rules` takes it at the start of the next tick.
#[derive(Resource, Debug, Default)]
pub struct PendingRules(pub Option<RulesSwap>);

/// The exact bytes of the file in [`Rules`] (empty before any file).
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct RulesText(pub Vec<u8>);

/// Every swap the run in progress took, on the tick it landed. Reset when
/// a run starts; sealed into the replay's `RULE` section.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct RulesLog(pub Vec<RulesRecord>);

/// What `set_rules` answers once a run has taken `MAX_RULES_SWAPS` swaps.
pub const TOO_MANY_SWAPS: &str =
    "This run has changed its rules 64 times. Restart the run to change them again.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Win,
    Lose,
}

/// How the run's rules ended it, if they did. Reset when a run starts.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RunEnd(pub Option<Outcome>);

/// Loads the embedded `content/rules.json`. Added by `GamePlugin` in both
/// modes, after `ContentPlugin` (the file names the kit's kinds).
pub struct RulesPlugin;

impl Plugin for RulesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Rules>()
            .init_resource::<RulesHash>()
            .init_resource::<PendingRules>()
            .init_resource::<RulesText>()
            .init_resource::<RulesLog>()
            .init_resource::<RunEnd>()
            .init_resource::<run::RunRules>();
        if let Some(bytes) = embedded("rules.json")
            && let Err(e) = set_rules(app.world_mut(), bytes)
        {
            // A committed file the game refuses is a build bug: the rules
            // tests fail on it. At runtime the game runs with no rules.
            error!("content/rules.json: {e}");
        }
    }
}

/// The names this game's rules may use.
pub fn vocab(world: &World) -> Vocab {
    let own = |list: &[&str]| -> Vec<String> { list.iter().map(|s| (*s).to_string()).collect() };
    Vocab {
        triggers: own(model::TRIGGERS),
        conditions: own(model::CONDITIONS),
        actions: model::ACTIONS
            .iter()
            .filter(|a| rule_table::JUMP.is_some() || **a != "jump")
            .map(|a| (*a).to_string())
            .collect(),
        counters: rule_table::COUNTERS
            .iter()
            .map(|c| c.name.to_string())
            .collect(),
        kinds: world
            .get_resource::<Kinds>()
            .map(|k| k.0.iter().map(|(n, _)| n.clone()).collect())
            .unwrap_or_default(),
        sounds: SfxEvent::ALL.iter().map(|e| e.name()).collect(),
    }
}

/// A rules file's hash: its bytes', or 0 ("no rules") when it holds none.
pub fn rules_hash(file: &RulesFile, bytes: &[u8]) -> u64 {
    if file.rules.is_empty() {
        0
    } else {
        fnv1a64(bytes)
    }
}

/// The rules hash a `LevelSource::Test` run folds: 0 when the game's
/// `test_rules()` is empty, so the template's fixtures keep their hash.
pub fn test_rules_hash() -> u64 {
    if rule_table::test_rules().rules.is_empty() {
        0
    } else {
        fnv1a64(b"test-rules")
    }
}

/// Checks `bytes` whole and swaps it in: the next run starts with it, and
/// a run in progress on the content rules takes it at the next tick
/// (`PendingRules`), at most `MAX_RULES_SWAPS` times a run. On error
/// nothing changes.
pub fn set_rules(world: &mut World, bytes: &[u8]) -> Result<(), String> {
    let file = model::check_file(bytes, &vocab(world))?;
    let hash = rules_hash(&file, bytes);
    let playing = world
        .get_resource::<State<GameState>>()
        .is_some_and(|s| *s.get() == GameState::Playing);
    let test = world
        .get_resource::<LevelSource>()
        .is_some_and(|s| *s == LevelSource::Test);
    let live = playing && !test && world.contains_resource::<PendingRules>();
    let swaps = world.get_resource::<RulesLog>().map_or(0, |l| l.0.len());
    if live && swaps >= MAX_RULES_SWAPS {
        return Err(TOO_MANY_SWAPS.to_string());
    }
    if live {
        world.resource_mut::<PendingRules>().0 = Some(RulesSwap {
            file: file.clone(),
            hash,
            bytes: bytes.to_vec(),
        });
    }
    world.insert_resource(Rules(file));
    world.insert_resource(RulesHash(hash));
    world.insert_resource(RulesText(bytes.to_vec()));
    Ok(())
}

/// The catalog's `rules`: the vocabulary the editor builds its dropdowns
/// from, the counters with their words and ranges, and the limits.
pub fn rules_catalog(world: &World) -> Json {
    let v = vocab(world);
    json!({
        "triggers": v.triggers,
        "conditions": v.conditions,
        "actions": v.actions,
        "counters": rule_table::COUNTERS
            .iter()
            .map(|c| json!({ "name": c.name, "label": c.label, "one": c.one, "min": c.min, "max": c.max }))
            .collect::<Vec<_>>(),
        "kinds": v.kinds,
        "sounds": v.sounds,
        "limits": {
            "bytes": model::MAX_BYTES,
            "rules": model::MAX_RULES,
            "if": model::MAX_IF,
            "do": model::MAX_DO,
            "source": model::MAX_SOURCE,
            "ticks": 36000,
            "number": 1_000_000,
            "move": 1000,
            "place": 100_000,
            "timers": model::MAX_TIMERS,
            "timer_names": model::MAX_TIMER_NAMES,
        },
    })
}

/// A rules file at the limits for `tools/test_extreme_content.sh`: 64
/// rules, each with 8 actions and a 200-character sentence, 16 timer
/// actions over 8 timer names, every action the game has, and as many
/// conditions as still fit in 32 KB. Every rule waits on a timer that no
/// rule can start, so the file proves a save at the limits never breaks
/// `cargo test` without making a test's run do anything.
pub fn extreme_rules(v: &Vocab) -> String {
    let c = v.counters.first().cloned().unwrap_or_default();
    let k = v.kinds.last().cloned().unwrap_or_default();
    let s = v.sounds.first().cloned().unwrap_or_default();
    let mut acts = vec![
        json!({ "add": [c, 1_000_000] }),
        json!({ "set": [c, -1_000_000] }),
        json!({ "remove": k }),
        json!({ "spawn": [k, "random"] }),
        json!({ "respawn": k }),
        json!({ "move": [k, 1000, -1000] }),
        json!({ "sound": s }),
        json!({ "end_run": "lose" }),
    ];
    if v.actions.iter().any(|a| a == "jump") {
        acts.push(json!({ "jump": k }));
    }
    for conds in (0..=model::MAX_IF).rev() {
        let rules: Vec<Json> = (0..model::MAX_RULES)
            .map(|i| {
                let name = format!("t{}", i % model::MAX_TIMER_NAMES);
                let mut r#do: Vec<Json> = (0..model::MAX_DO)
                    .map(|j| acts[(i + j) % acts.len()].clone())
                    .collect();
                if i < model::MAX_TIMERS {
                    r#do[0] = json!({ "timer": [name, 36000] });
                }
                let r#if: Vec<Json> = (0..conds)
                    .map(|j| {
                        if j % 2 == 0 {
                            json!({ "chance": 100 })
                        } else {
                            json!({ "compare": [c, ">=", -1_000_000] })
                        }
                    })
                    .collect();
                json!({
                    "id": format!("x{i}"),
                    "when": { "timer": name },
                    "if": r#if,
                    "do": r#do,
                    "source": "x".repeat(model::MAX_SOURCE),
                })
            })
            .collect();
        let text = model::canonical_text(&rules);
        if text.len() <= model::MAX_BYTES {
            return text;
        }
    }
    model::canonical_text(&[])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::content::files::{embedded, fnv1a64};
    use crate::game::replay::build_headless_app;

    /// A run in progress on the content source.
    fn playing() -> App {
        use crate::game::content::level::LevelSource;
        let mut app = build_headless_app();
        app.insert_resource(LevelSource::Content);
        app.update();
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.update();
        app
    }

    #[test]
    fn a_swap_during_a_run_carries_its_bytes() {
        let mut app = playing();
        let good =
            br#"{"v":1,"rules":[{"id":"a","when":{"start":true},"do":[{"add":["score",1]}]}]}"#;
        set_rules(app.world_mut(), good).unwrap();
        let swap = app
            .world()
            .resource::<PendingRules>()
            .0
            .clone()
            .expect("a run in progress takes it at the next tick");
        assert_eq!(swap.bytes, good.to_vec());
        assert_eq!(swap.hash, fnv1a64(good));
        assert_eq!(app.world().resource::<RulesText>().0, good.to_vec());
    }

    #[test]
    fn the_sixty_fifth_swap_of_a_run_is_refused() {
        use crate::game::replay::{MAX_RULES_SWAPS, RulesRecord};
        let mut app = playing();
        let before = app.world().resource::<Rules>().clone();
        app.world_mut().resource_mut::<RulesLog>().0 = (0..MAX_RULES_SWAPS as u32)
            .map(|tick| RulesRecord {
                tick,
                bytes: Vec::new(),
            })
            .collect();
        assert_eq!(
            set_rules(app.world_mut(), b"{\"v\":1,\"rules\":[]}"),
            Err(TOO_MANY_SWAPS.to_string())
        );
        assert_eq!(*app.world().resource::<Rules>(), before);
        assert!(app.world().resource::<PendingRules>().0.is_none());
    }

    /// The shipped file is valid, whatever it holds (never compared with
    /// what was shipped: the editor saves it).
    #[test]
    fn the_shipped_rules_file_is_valid() {
        let Some(bytes) = embedded("rules.json") else {
            return;
        };
        let mut app = build_headless_app();
        set_rules(app.world_mut(), bytes).unwrap();
        let file = model::check_file(bytes, &vocab(app.world())).unwrap();
        assert_eq!(app.world().resource::<Rules>().0, file);
    }

    #[test]
    fn names_the_game_lacks_are_refused_with_a_sentence() {
        let mut app = build_headless_app();
        let refuse =
            |app: &mut App, text: &str| set_rules(app.world_mut(), text.as_bytes()).unwrap_err();
        assert_eq!(
            refuse(
                &mut app,
                r#"{"v":1,"rules":[{"id":"a","when":{"start":true},"do":[{"add":["gems",1]}]}]}"#
            ),
            "Rule 1: This game has no counter called gems."
        );
        assert_eq!(
            refuse(
                &mut app,
                r#"{"v":1,"rules":[{"id":"a","when":{"touch":["Player","Ghost"]},"do":[{"add":["score",1]}]}]}"#
            ),
            "Rule 1: This game has no kind called Ghost."
        );
        assert_eq!(
            refuse(
                &mut app,
                r#"{"v":1,"rules":[{"id":"a","when":{"start":true},"do":[{"remove":"self"}]}]}"#
            ),
            "Rule 1: \"self\" and \"other\" work only in a rule that starts with a touch."
        );
    }

    #[test]
    fn a_refused_file_changes_nothing() {
        let mut app = build_headless_app();
        let good =
            br#"{"v":1,"rules":[{"id":"a","when":{"start":true},"do":[{"add":["score",1]}]}]}"#;
        set_rules(app.world_mut(), good).unwrap();
        let (rules, hash) = (
            app.world().resource::<Rules>().clone(),
            *app.world().resource::<RulesHash>(),
        );
        assert!(set_rules(app.world_mut(), b"{\"v\":2,\"rules\":[]}").is_err());
        assert_eq!(*app.world().resource::<Rules>(), rules);
        assert_eq!(*app.world().resource::<RulesHash>(), hash);
        assert_eq!(hash.0, fnv1a64(good));
    }

    #[test]
    fn a_file_with_no_rules_hashes_as_no_rules() {
        let mut app = build_headless_app();
        set_rules(app.world_mut(), b"{\n  \"v\": 1,\n  \"rules\": []\n}\n").unwrap();
        assert_eq!(app.world().resource::<RulesHash>().0, 0);
        assert!(app.world().resource::<Rules>().0.rules.is_empty());
    }

    #[test]
    fn the_catalog_lists_exactly_what_the_checker_takes() {
        let app = build_headless_app();
        let j = rules_catalog(app.world());
        assert_eq!(model::Vocab::from_json(&j), vocab(app.world()));
        let counters: Vec<&str> = crate::game::rule_table::COUNTERS
            .iter()
            .map(|c| c.name)
            .collect();
        let listed: Vec<&str> = j["counters"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["name"].as_str().unwrap())
            .collect();
        assert_eq!(listed, counters);
        assert_eq!(j["limits"]["rules"], 64);
        assert_eq!(j["limits"]["bytes"], 32768);
        let has_jump = j["actions"].as_array().unwrap().iter().any(|a| a == "jump");
        assert_eq!(has_jump, crate::game::rule_table::JUMP.is_some());
    }

    #[test]
    fn test_rules_use_only_the_games_names() {
        let app = build_headless_app();
        let v = vocab(app.world());
        for rule in crate::game::rule_table::test_rules().rules {
            if let model::Trigger::Touch(a, b) = &rule.when {
                assert!(v.kinds.contains(a) && v.kinds.contains(b), "{}", rule.id);
            }
            if let model::Trigger::Counter(c, ..) = &rule.when {
                assert!(v.counters.contains(c), "{}", rule.id);
            }
        }
    }

    #[test]
    fn extreme_rules_sit_at_every_limit_and_are_taken() {
        let app = build_headless_app();
        let v = vocab(app.world());
        let text = extreme_rules(&v);
        assert!(text.len() <= model::MAX_BYTES, "{} bytes", text.len());
        let file = model::check_file(text.as_bytes(), &v).unwrap();
        assert_eq!(file.rules.len(), model::MAX_RULES);
        assert!(file.rules.iter().all(|r| r.acts.len() == model::MAX_DO));
        assert!(
            file.rules
                .iter()
                .all(|r| r.source.as_deref().map(|s| s.chars().count()) == Some(model::MAX_SOURCE))
        );
        let timers = file
            .rules
            .iter()
            .flat_map(|r| &r.acts)
            .filter(|a| matches!(a, model::Act::Timer(..)))
            .count();
        assert_eq!(timers, model::MAX_TIMERS);
    }

    /// Writes the rules file `tools/test_extreme_content.sh` saves:
    /// `GX_EXTREME=max` the file at the limits, `min` an empty one.
    #[test]
    #[ignore = "run by tools/test_extreme_content.sh"]
    fn write_extreme_rules() {
        let out = std::env::var("GX_EXTREME_OUT").expect("GX_EXTREME_OUT names the file to write");
        let app = build_headless_app();
        let v = vocab(app.world());
        let text = match std::env::var("GX_EXTREME").as_deref() {
            Ok("max") => extreme_rules(&v),
            Ok("min") => model::canonical_text(&[]),
            other => panic!("GX_EXTREME must be max or min, not {other:?}"),
        };
        model::check_file(text.as_bytes(), &v).unwrap();
        std::fs::write(out, text).unwrap();
    }
}
