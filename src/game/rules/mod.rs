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

use bevy::prelude::*;
use serde_json::{Value as Json, json};

use crate::game::audio::SfxEvent;
use crate::game::content::files::{embedded, fnv1a64};
use crate::game::content::kit::Kinds;
use crate::game::content::level::LevelSource;
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

/// A file `content.set` swapped in during a run, and its hash: `run_rules`
/// takes it at the start of the next tick.
#[derive(Resource, Debug, Default)]
pub struct PendingRules(pub Option<(RulesFile, u64)>);

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
            .init_resource::<RunEnd>();
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
/// (`PendingRules`). On error nothing changes.
pub fn set_rules(world: &mut World, bytes: &[u8]) -> Result<(), String> {
    let file = model::check_file(bytes, &vocab(world))?;
    let hash = rules_hash(&file, bytes);
    let playing = world
        .get_resource::<State<GameState>>()
        .is_some_and(|s| *s.get() == GameState::Playing);
    let test = world
        .get_resource::<LevelSource>()
        .is_some_and(|s| *s == LevelSource::Test);
    if playing
        && !test
        && let Some(mut pending) = world.get_resource_mut::<PendingRules>()
    {
        pending.0 = Some((file.clone(), hash));
    }
    world.insert_resource(Rules(file));
    world.insert_resource(RulesHash(hash));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::content::files::{embedded, fnv1a64};
    use crate::game::replay::build_headless_app;

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
}
