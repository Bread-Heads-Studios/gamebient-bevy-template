//! Saved tunables: `content/tunables.json` overrides the Rust defaults.
//!
//! A saved value is a *start value*: it is written into the tunable's field
//! when `ContentPlugin` is built (not in a `Startup` system: the first state
//! transition runs before `Startup`) through [`tunables::write`], before any run, in both the
//! windowed and the headless app. It therefore lands in `TuneLog.starts` of
//! every run and in the release's default content hash, and a replay's own
//! staged starts (`build_verify_app`) are applied on top of it.

use bevy::prelude::*;

use super::files;
use super::model::TunablesFile;
use crate::game::tunables::{TunableIndex, Value, write};

/// Why saved values were skipped or the file was ignored; the editor shows
/// them in the catalog.
#[derive(Resource, Default, Debug, Clone, PartialEq)]
pub struct ContentWarnings(pub Vec<String>);

/// Writes every saved value into its tunable. Returns one warning per value
/// that was skipped (unknown path, wrong type, or refused by the tunable's
/// range or options); skipped values change nothing.
pub fn apply_saved(world: &mut World, file: &TunablesFile) -> Vec<String> {
    let idx = world.resource::<TunableIndex>().clone();
    let mut warnings = Vec::new();
    for (path, json) in &file.values {
        let Some(entry) = idx.by_path(path) else {
            warnings.push(format!(
                "Saved value for \"{path}\" was skipped: there is no such setting."
            ));
            continue;
        };
        let kind = entry.tune.kind;
        let Some(value) = Value::from_json(kind, json) else {
            warnings.push(format!(
                "Saved value for \"{path}\" was skipped: it is not a {}.",
                kind.name()
            ));
            continue;
        };
        if !write(world, entry, &value) {
            warnings.push(format!(
                "Saved value for \"{path}\" was skipped: it is out of range."
            ));
        }
    }
    warnings
}

/// Called by `ContentPlugin::build`: parses the embedded file and applies it.
pub fn apply_embedded_saved(world: &mut World) {
    apply_embedded_saved_from(world, files::embedded("tunables.json"));
}

/// The plugin's path with the file's bytes injectable (`None`: no file).
pub fn apply_embedded_saved_from(world: &mut World, bytes: Option<&[u8]>) {
    let Some(bytes) = bytes else {
        return;
    };
    let warnings = match TunablesFile::parse(bytes) {
        Ok(file) => apply_saved(world, &file),
        Err(e) => vec![format!("Saved settings were ignored: {e}")],
    };
    for w in &warnings {
        warn!("content/tunables.json: {w}");
    }
    world.resource_mut::<ContentWarnings>().0.extend(warnings);
}

/// Writes every tunable's Rust default back. Built-in test content (the
/// selftest, the checks harness) calls this so saved values never reach the
/// committed fixtures.
pub fn reset_to_defaults(world: &mut World) {
    let idx = world.resource::<TunableIndex>().clone();
    for e in &idx.entries {
        write(world, e, &e.default);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::replay::build_headless_app;
    use crate::game::sim::{PendingSeed, seed_bytes};
    use crate::game::states::GameState;
    use crate::game::tunables::{TuneLog, read};

    fn bumped(v: &Value) -> Option<Value> {
        Some(match v {
            Value::Float(f) => Value::Float(f + 0.5),
            Value::Int(i) => Value::Int(i + 1),
            Value::Bool(b) => Value::Bool(!b),
            _ => return None,
        })
    }

    /// First tunable we can change to an in-range different value.
    fn pick(app: &App) -> (crate::game::tunables::Entry, Value) {
        let idx = app.world().resource::<TunableIndex>();
        idx.entries
            .iter()
            .find_map(|e| {
                let v = bumped(&e.default)?;
                let ok = match &v {
                    Value::Float(f) => (e.tune.min..=e.tune.max).contains(&f64::from(*f)),
                    Value::Int(i) => (e.tune.min..=e.tune.max).contains(&(*i as f64)),
                    _ => true,
                };
                ok.then(|| (e.clone(), v))
            })
            .expect("a tunable that can be bumped")
    }

    fn file_with(path: &str, json: serde_json::Value) -> TunablesFile {
        let s = serde_json::json!({ "v": 1, "values": { path: json } }).to_string();
        TunablesFile::parse(s.as_bytes()).unwrap()
    }

    #[test]
    fn saved_values_override_defaults_and_become_starts() {
        let mut app = build_headless_app();
        let (entry, v) = pick(&app);
        let file = file_with(&entry.path, v.to_json());
        let warnings = apply_saved(app.world_mut(), &file);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(read(app.world(), &entry), Some(v.clone()));
        app.world_mut().resource_mut::<PendingSeed>().0 = Some(seed_bytes(1));
        app.update();
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.update();
        let log = app.world().resource::<TuneLog>();
        assert_eq!(log.starts[usize::from(entry.id)].1, v);
    }

    /// Even a run entered on the very first update starts with the saved
    /// value: the values are written before any update, as the plugin does.
    #[test]
    fn saved_values_land_before_a_run_entered_on_the_first_update() {
        let mut app = build_headless_app();
        let (entry, v) = pick(&app);
        let file = file_with(&entry.path, v.to_json());
        apply_saved(app.world_mut(), &file);
        app.world_mut().resource_mut::<PendingSeed>().0 = Some(seed_bytes(1));
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.update();
        app.update();
        let log = app.world().resource::<TuneLog>();
        assert_eq!(log.starts[usize::from(entry.id)].1, v);
    }

    /// The plugin's own path (bytes in, applied at build time), then a run
    /// entered on the very first update.
    #[test]
    fn plugin_path_lands_before_a_run_entered_on_the_first_update() {
        let mut app = build_headless_app();
        let (entry, v) = pick(&app);
        let text = serde_json::json!({ "v": 1, "values": { entry.path.clone(): v.to_json() } })
            .to_string();
        apply_embedded_saved_from(app.world_mut(), Some(text.as_bytes()));
        assert!(app.world().resource::<ContentWarnings>().0.is_empty());
        app.world_mut().resource_mut::<PendingSeed>().0 = Some(seed_bytes(1));
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.update();
        app.update();
        let log = app.world().resource::<TuneLog>();
        assert_eq!(log.starts[usize::from(entry.id)].1, v);
        // A broken file warns and changes nothing.
        let mut app = build_headless_app();
        apply_embedded_saved_from(app.world_mut(), Some(b"nope"));
        assert_eq!(app.world().resource::<ContentWarnings>().0.len(), 1);
    }

    #[test]
    fn reset_to_defaults_undoes_saved_values() {
        let mut app = build_headless_app();
        let (entry, v) = pick(&app);
        apply_saved(app.world_mut(), &file_with(&entry.path, v.to_json()));
        assert_eq!(read(app.world(), &entry), Some(v));
        reset_to_defaults(app.world_mut());
        assert_eq!(read(app.world(), &entry), Some(entry.default.clone()));
    }

    #[test]
    fn the_embedded_file_applies_cleanly() {
        let mut app = build_headless_app();
        app.update();
        assert!(app.world().resource::<ContentWarnings>().0.is_empty());
    }

    #[test]
    fn unknown_path_and_wrong_type_warn_and_change_nothing() {
        let mut app = build_headless_app();
        let (entry, _) = pick(&app);
        let before = read(app.world(), &entry);
        let mut file = file_with("no.such", serde_json::json!(1));
        let wrong = if matches!(entry.default, Value::Bool(_)) {
            serde_json::json!("text")
        } else {
            serde_json::json!(true)
        };
        file.values.insert(entry.path.clone(), wrong);
        let warnings = apply_saved(app.world_mut(), &file);
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        assert!(warnings.contains(
            &"Saved value for \"no.such\" was skipped: there is no such setting.".to_string()
        ));
        assert!(warnings.contains(&format!(
            "Saved value for \"{}\" was skipped: it is not a {}.",
            entry.path,
            entry.tune.kind.name()
        )));
        assert_eq!(read(app.world(), &entry), before);
    }
}
