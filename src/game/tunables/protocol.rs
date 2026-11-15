//! The editor's host layer, on top of the `gamebient-input` host protocol
//! (feature `editor`). Names, in `gx:custom` / `custom` messages:
//!
//! | Direction   | Name             | Data                                              |
//! |-------------|------------------|---------------------------------------------------|
//! | host → game | `tunables.list`  | —                                                 |
//! | game → host | `tunables`       | the catalog: groups, tunables with values, checks, sounds, content |
//! | host → game | `tunables.set`   | `{ "<path>": value, … }`                          |
//! | game → host | `tunable`        | `{ path, value, applied: "tick"\|"now", tick }` or `{ path, error }` |
//! | host → game | `checks.run`     | `{ "names": [ … ] }`                              |
//! | game → host | `check`          | `{ name, ok, message }`, one per check            |
//! | host → game | `run.restart`    | —                                                 |
//! | host → game | `sound.preview`  | `{ name }`: an `SfxEvent` name or a cue/layer name |
//! | game → host | `sound`          | `{ name, ok: true }` or `{ name, ok: false, error }` |
//! | host → game | `content.set`    | `{ file, text }` (a `.json` file) or `{ file, png }` (base64, `sprites/<Kind>.png`) |
//! | game → host | `content`        | `{ file, ok, error?, hash }`; hash is FNV-1a 64 of the exact bytes |
//!
//! `content.set` files: `level.json` (applies at the next run start; send
//! `run.restart`), `tunables.json` (through `PendingTunes`, all or nothing),
//! `sprites.json`, `sprites/<Kind>.png`, `audio/songs.json`, `audio/sfx.json`.
//! The catalog's `files` maps every content file the game runs to its hash.
//!
//! Everything here runs in `Update`; the only thing that touches run state
//! is a push onto `PendingTunes`, which `apply_pending_tunes` drains inside
//! `SimSet`. The catalog is also posted once on `OnEnter(Playing)`.

use bevy::prelude::*;
use gamebient_input::{HostCommand, HostEvent};
use serde_json::{Value as Json, json};

use crate::game::states::GameState;

use super::apply::{PendingTunes, TuneApplied, TuneLog, apply_pending_tunes};
use super::attr::Applies;
use super::checks::{CheckList, Tunes, run_all};
use super::hash;
use super::index::{TunableIndex, snapshot};
use super::value::Value;

/// What the game tells the editor beyond its tunables: the sound names a
/// sound picker may offer (and `sound.preview` takes). Starts as every
/// event and cue name (`audio::sound_names`).
#[derive(Resource, Default)]
pub struct EditorInfo {
    pub sounds: Vec<String>,
}

#[derive(Resource, Default)]
struct PreviewsRequested(Vec<String>);

#[derive(Resource, Default)]
struct ListRequested(bool);

#[derive(Resource, Default)]
struct ChecksRequested(Vec<String>);

/// `content.set` requests, applied by `apply_content_sets` (they need `&mut World`).
#[derive(Resource, Default)]
struct ContentSets(Vec<Json>);

/// Set by `run.restart`; cleared once the run has been re-entered.
#[derive(Resource, Default)]
pub struct RestartPending(pub bool);

pub struct EditorProtocolPlugin;

impl Plugin for EditorProtocolPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(EditorInfo {
            sounds: crate::game::audio::sound_names(),
        })
        .init_resource::<PreviewsRequested>()
        .init_resource::<ListRequested>()
        .init_resource::<ChecksRequested>()
        .init_resource::<ContentSets>()
        .init_resource::<RestartPending>()
        // Around the menu-time drain (`apply_pending_tunes` in `Update`,
        // registered by `TunablesPlugin`): a set received this frame
        // applies this frame and is reported this frame. During a run
        // the drain is in `SimSet` and the report follows a tick later.
        .add_systems(
            Update,
            (handle_custom_commands, apply_content_sets, restart_run)
                .chain()
                .before(apply_pending_tunes),
        )
        .add_systems(
            Update,
            (
                answer_tunables_list,
                run_requested_checks,
                report_tune_applied,
                run_sound_previews,
            )
                .chain()
                .after(apply_pending_tunes),
        )
        .add_systems(OnEnter(GameState::Playing), request_list)
        .add_systems(OnEnter(GameState::Menu), resume_restart);
    }
}

fn request_list(mut list: ResMut<ListRequested>) {
    list.0 = true;
}

fn handle_custom_commands(
    mut commands: MessageReader<HostCommand>,
    idx: Res<TunableIndex>,
    mut pending: ResMut<PendingTunes>,
    mut list: ResMut<ListRequested>,
    mut checks: ResMut<ChecksRequested>,
    mut restart: ResMut<RestartPending>,
    mut previews: ResMut<PreviewsRequested>,
    mut sets: ResMut<ContentSets>,
    mut out: MessageWriter<HostEvent>,
) {
    for command in commands.read() {
        let HostCommand::Custom { name, data } = command else {
            continue;
        };
        let data: Json = serde_json::from_str(data).unwrap_or(Json::Null);
        match name.as_str() {
            "tunables.list" => list.0 = true,
            "tunables.set" => {
                let Some(obj) = data.as_object() else {
                    continue;
                };
                for (path, j) in obj {
                    let Some(entry) = idx.by_path(path) else {
                        out.write(custom(
                            "tunable",
                            json!({ "path": path, "error": "unknown tunable" }),
                        ));
                        continue;
                    };
                    match Value::from_json(entry.tune.kind, j) {
                        Some(v) => pending.0.push((entry.id, v)),
                        None => {
                            out.write(custom(
                                "tunable",
                                json!({ "path": path, "error": format!("not a {}", entry.tune.kind.name()) }),
                            ));
                        }
                    }
                }
            }
            "checks.run" => {
                if let Some(names) = data.get("names").and_then(Json::as_array) {
                    checks
                        .0
                        .extend(names.iter().filter_map(Json::as_str).map(str::to_string));
                }
            }
            "run.restart" => restart.0 = true,
            "content.set" => sets.0.push(data),
            "sound.preview" => previews.0.push(
                data.get("name")
                    .and_then(Json::as_str)
                    .unwrap_or_default()
                    .to_string(),
            ),
            _ => {}
        }
    }
}

fn custom(name: &str, data: Json) -> HostEvent {
    HostEvent::Custom {
        name: name.to_string(),
        data: data.to_string(),
    }
}

/// The catalog: everything the editor needs to build and validate cards.
pub fn catalog_json(world: &World) -> Json {
    let idx = world.resource::<TunableIndex>();
    let values = snapshot(world);
    let info = world.resource::<EditorInfo>();
    let checks = world.resource::<CheckList>().0;
    let name = world
        .get_resource::<gamebient_input::GxConfig>()
        .map(|c| c.name.clone())
        .unwrap_or_default();
    let tunables: Vec<Json> = idx
        .entries
        .iter()
        .map(|e| {
            let value = values
                .iter()
                .find(|(id, _)| *id == e.id)
                .map(|(_, v)| v.to_json())
                .unwrap_or(Json::Null);
            let mut j = json!({
                "path": e.path,
                "group": idx.groups[e.group].path,
                "label": e.tune.label,
                "type": e.tune.kind.name(),
                "min": e.tune.min,
                "max": e.tune.max,
                "step": e.tune.step,
                "unit": e.tune.unit,
                "applies": match e.tune.applies { Applies::Tick => "tick", Applies::NextRun => "next_run" },
                "value": value,
                "default": e.default.to_json(),
            });
            if !e.options.is_empty() {
                j["options"] = json!(e.options);
            }
            j
        })
        .collect();
    json!({
        "game": name,
        "build": env!("GX_BUILD_ID"),
        "state": format!("{:?}", world.resource::<State<GameState>>().get()),
        "tick": world.resource::<crate::game::sim::SimTick>().0,
        "pending": world.resource::<PendingTunes>().0.len(),
        "changes": world.resource::<TuneLog>().changes.len(),
        "content": hash::content(world.resource::<TuneLog>()).to_string(),
        "groups": idx.groups.iter().map(|g| json!({ "path": g.path, "label": g.label, "kind": g.kind.name() })).collect::<Vec<_>>(),
        "tunables": tunables,
        "checks": checks.iter().map(|c| json!({ "name": c.name, "title": c.title })).collect::<Vec<_>>(),
        "sounds": info.sounds,
        "files": world
            .get_resource::<crate::game::content::LiveFiles>()
            .map(|f| {
                f.0.iter()
                    .map(|(p, h)| (p.clone(), json!(crate::game::content::files::hex(*h))))
                    .collect::<serde_json::Map<String, Json>>()
            })
            .unwrap_or_default(),
        "content_warnings": world
            .get_resource::<crate::game::content::saved::ContentWarnings>()
            .map(|w| w.0.clone())
            .unwrap_or_default(),
    })
}

fn answer_tunables_list(world: &mut World) {
    if !core::mem::take(&mut world.resource_mut::<ListRequested>().0) {
        return;
    }
    let catalog = catalog_json(world);
    world.write_message(custom("tunables", catalog));
}

fn run_requested_checks(world: &mut World) {
    let names: Vec<String> = core::mem::take(&mut world.resource_mut::<ChecksRequested>().0);
    if names.is_empty() {
        return;
    }
    let idx = world.resource::<TunableIndex>().clone();
    let tunes: Tunes = snapshot(world)
        .into_iter()
        .filter_map(|(id, v)| idx.by_id(id).map(|e| (e.path.clone(), v)))
        .collect();
    let list = world.resource::<CheckList>().0;
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let results = run_all(list, &names, &tunes);
    for (name, r) in results {
        let (ok, message) = match r {
            Ok(()) => (true, String::new()),
            Err(m) => (false, m),
        };
        world.write_message(custom(
            "check",
            json!({ "name": name, "ok": ok, "message": message }),
        ));
    }
}

/// The first sentence of trouble, or `None` when `file` was applied.
fn set_content(world: &mut World, file: &str, data: &Json) -> Result<u64, String> {
    use crate::game::content::{audio, base64, files, level, model::TunablesFile, sprites};
    let text = data.get("text").and_then(Json::as_str);
    let png = data.get("png").and_then(Json::as_str);
    let is_png = file.starts_with("sprites/") && file.ends_with(".png");
    let known = matches!(
        file,
        "level.json" | "tunables.json" | "sprites.json" | "audio/songs.json" | "audio/sfx.json"
    );
    if !known && !is_png {
        let shown: String = file.chars().take(40).collect();
        return Err(format!("There is no content file called \"{shown}\"."));
    }
    let bytes: Vec<u8> = if is_png {
        if text.is_some() && png.is_none() {
            return Err("A picture must be sent as \"png\", not \"text\".".into());
        }
        let Some(png) = png else {
            return Err("This picture has no data.".into());
        };
        base64::decode(png).ok_or("The picture data is not valid base64.")?
    } else {
        if png.is_some() && text.is_none() {
            return Err("A .json file must be sent as \"text\", not \"png\".".into());
        }
        let Some(text) = text else {
            return Err("This file has no text.".into());
        };
        text.as_bytes().to_vec()
    };
    let e = |r: Result<(), crate::game::content::model::ContentError>| r.map_err(|e| e.0);
    match file {
        "level.json" => e(level::set_level(world, &bytes))?,
        "sprites.json" => e(sprites::set_sprites_meta(world, &bytes))?,
        "audio/songs.json" => e(audio::set_songs(world, &bytes))?,
        "audio/sfx.json" => e(audio::set_sfx(world, &bytes))?,
        "tunables.json" => {
            let parsed = TunablesFile::parse(&bytes).map_err(|e| e.0)?;
            let idx = world.resource::<TunableIndex>().clone();
            let mut pushes = Vec::new();
            for (path, j) in &parsed.values {
                let Some(entry) = idx.by_path(path) else {
                    return Err(format!("There is no setting called \"{path}\"."));
                };
                let Some(v) = Value::from_json(entry.tune.kind, j) else {
                    return Err(format!(
                        "The setting \"{path}\" must be a {}.",
                        entry.tune.kind.name()
                    ));
                };
                pushes.push((entry.id, v));
            }
            world.resource_mut::<PendingTunes>().0.extend(pushes);
        }
        _ => {
            let kind = &file["sprites/".len()..file.len() - ".png".len()];
            e(sprites::set_sprite(world, kind, &bytes))?;
        }
    }
    Ok(files::fnv1a64(&bytes))
}

/// `content.set`: applies each requested file and answers `content`.
fn apply_content_sets(world: &mut World) {
    let sets: Vec<Json> = core::mem::take(&mut world.resource_mut::<ContentSets>().0);
    for data in sets {
        let file = data
            .get("file")
            .and_then(Json::as_str)
            .unwrap_or_default()
            .to_string();
        let reply = match set_content(world, &file, &data) {
            Ok(hash) => {
                if let Some(mut f) = world.get_resource_mut::<crate::game::content::LiveFiles>() {
                    f.set(&file, hash);
                }
                json!({ "file": file, "ok": true, "hash": crate::game::content::files::hex(hash) })
            }
            Err(error) => {
                // The hash of what is running, when the game has that file.
                let live = world
                    .get_resource::<crate::game::content::LiveFiles>()
                    .and_then(|f| f.0.iter().find(|(p, _)| *p == file).map(|(_, h)| *h));
                json!({
                    "file": file,
                    "ok": false,
                    "error": error,
                    "hash": live.map(crate::game::content::files::hex).unwrap_or_default(),
                })
            }
        };
        world.write_message(custom("content", reply));
    }
}

/// `sound.preview`: plays each requested sound and answers `sound`.
fn run_sound_previews(world: &mut World) {
    let names: Vec<String> = core::mem::take(&mut world.resource_mut::<PreviewsRequested>().0);
    for name in names {
        let reply = if crate::game::audio::preview(world, &name) {
            json!({ "name": name, "ok": true })
        } else {
            let known = crate::game::audio::sound_names().contains(&name);
            let error = if known {
                "This build plays no sound.".to_string()
            } else {
                format!("There is no sound called \"{name}\".")
            };
            json!({ "name": name, "ok": false, "error": error })
        };
        world.write_message(custom("sound", reply));
    }
}

fn report_tune_applied(
    mut applied: MessageReader<TuneApplied>,
    idx: Res<TunableIndex>,
    mut out: MessageWriter<HostEvent>,
) {
    for a in applied.read() {
        let Some(entry) = idx.by_id(a.id) else {
            continue;
        };
        if !a.ok {
            out.write(custom(
                "tunable",
                json!({ "path": entry.path, "value": a.value.to_json(), "error": "refused: wrong kind, out of range or unknown option" }),
            ));
            continue;
        }
        out.write(custom(
            "tunable",
            json!({
                "path": entry.path,
                "value": a.value.to_json(),
                "applied": if a.tick.is_some() { "tick" } else { "now" },
                "tick": a.tick,
            }),
        ));
    }
}

/// `run.restart`: leave a running game for the menu (its `OnExit` seals the
/// replay and cleans up), then re-enter `Playing` from `OnEnter(Menu)`.
/// From any other state, enter `Playing` directly.
///
/// Through the `ScreenFade` when the windowed build has one, and only once
/// it is idle: the studio logo, the menu and the game-over card all leave
/// through a fade that sets its own target state when it finishes, so a
/// bare `NextState` written during one is overruled half a second later
/// (a restart from the logo entered `Playing` and was yanked back to
/// `Menu` 31 ticks in). The headless build has no fade and switches at
/// once; `sim::end_run` takes the same `Option<ResMut<ScreenFade>>`.
fn restart_run(
    mut restart: ResMut<RestartPending>,
    state: Res<State<GameState>>,
    mut next: ResMut<NextState<GameState>>,
    fade: Option<ResMut<crate::ui::transition::ScreenFade>>,
) {
    if !restart.0 {
        return;
    }
    let target = if *state.get() == GameState::Playing {
        GameState::Menu
    } else {
        GameState::Playing
    };
    match fade {
        Some(mut fade) => {
            if !fade.is_idle() || !fade.request(target) {
                return; // try again next frame
            }
        }
        None => next.set(target),
    }
    if target == GameState::Playing {
        restart.0 = false;
    }
}

fn resume_restart(mut restart: ResMut<RestartPending>, mut next: ResMut<NextState<GameState>>) {
    if restart.0 {
        restart.0 = false;
        next.set(GameState::Playing);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::replay::build_headless_app;
    use crate::game::sim::SimTick;

    fn drain_custom(app: &mut App) -> Vec<(String, String)> {
        app.world_mut()
            .resource_mut::<Messages<HostEvent>>()
            .drain()
            .filter_map(|e| match e {
                HostEvent::Custom { name, data } => Some((name, data)),
                _ => None,
            })
            .collect()
    }

    fn send(app: &mut App, name: &str, data: &str) {
        app.world_mut().write_message(HostCommand::Custom {
            name: name.into(),
            data: data.into(),
        });
    }

    /// Changes a number or a bool; leaves other kinds alone.
    fn bump(v: &Value) -> Value {
        match v {
            Value::Float(f) => Value::Float(f + 1.0),
            Value::Int(i) => Value::Int(i + 1),
            Value::Bool(b) => Value::Bool(!b),
            other => other.clone(),
        }
    }

    /// Generic over the game's first tunable, so this test survives a game
    /// replacing `tuning.rs` (this file is copied into every game).
    #[test]
    fn list_set_and_report_over_host_messages() {
        use crate::game::tunables::read;
        let mut app = build_headless_app();
        app.update();
        drain_custom(&mut app);
        let entry = app.world().resource::<TunableIndex>().entries[0].clone();
        let group = app.world().resource::<TunableIndex>().groups[entry.group].clone();
        send(&mut app, "tunables.list", "null");
        app.update();
        let events = drain_custom(&mut app);
        assert_eq!(events[0].0, "tunables");
        let j: Json = serde_json::from_str(&events[0].1).unwrap();
        assert_eq!(j["tunables"][0]["path"], entry.path);
        assert_eq!(j["tunables"][0]["type"], entry.tune.kind.name());
        assert_eq!(j["tunables"][0]["value"], entry.default.to_json());
        assert_eq!(j["tunables"][0]["default"], entry.default.to_json());
        assert_eq!(j["groups"][entry.group]["kind"], group.kind.name());
        assert_eq!(j["groups"][entry.group]["label"], group.label);
        assert!(j["content"].is_string());
        assert!(j["build"].as_str().unwrap().contains('+'));
        assert_eq!(j["tick"], 0);
        assert!(j["state"].is_string());
        assert!(j["checks"].is_array() && j["sounds"].is_array());
        assert_eq!(j["content_warnings"], serde_json::json!([]));

        let path = entry.path.clone();
        send(
            &mut app,
            "tunables.set",
            &format!("{{\"nope.x\":1,\"{path}\":[\"wrong\",\"shape\",1,2,3]}}"),
        );
        app.update();
        let events = drain_custom(&mut app);
        let applied: Vec<Json> = events
            .iter()
            .filter(|e| e.0 == "tunable")
            .map(|e| serde_json::from_str(&e.1).unwrap())
            .collect();
        let wrong = applied.iter().find(|j| j["path"] == path).unwrap();
        assert_eq!(wrong["error"], format!("not a {}", entry.tune.kind.name()));
        let nope = applied.iter().find(|j| j["path"] == "nope.x").unwrap();
        assert_eq!(nope["error"], "unknown tunable");
        assert_eq!(applied.len(), 2);
        assert_eq!(
            read(app.world(), &entry),
            Some(entry.default.clone()),
            "untouched"
        );

        let next = bump(&entry.default);
        send(
            &mut app,
            "tunables.set",
            &serde_json::json!({ path.clone(): next.to_json() }).to_string(),
        );
        app.update();
        let events = drain_custom(&mut app);
        assert_eq!(events[0].0, "tunable");
        let j: Json = serde_json::from_str(&events[0].1).unwrap();
        assert_eq!(j["path"], path);
        assert_eq!(j["value"], next.to_json());
        assert_eq!(j["applied"], "now");
        assert_eq!(read(app.world(), &entry), Some(next));
    }

    #[test]
    fn sound_preview_answers_and_the_catalog_lists_sounds() {
        let mut app = build_headless_app();
        app.update();
        drain_custom(&mut app);
        let j = catalog_json(app.world());
        let sounds: Vec<&str> = j["sounds"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Json::as_str)
            .collect();
        for want in [
            "confirm",
            "pause",
            "screensweep",
            "house",
            "menu",
            "play_layer",
        ] {
            assert!(sounds.contains(&want), "{want}: {sounds:?}");
        }
        send(&mut app, "sound.preview", "{\"name\":\"nope\"}");
        send(&mut app, "sound.preview", "{\"name\":\"menu\"}");
        app.update();
        let events: Vec<Json> = drain_custom(&mut app)
            .iter()
            .filter(|e| e.0 == "sound")
            .map(|e| serde_json::from_str(&e.1).unwrap())
            .collect();
        assert_eq!(
            events[0],
            json!({ "name": "nope", "ok": false, "error": "There is no sound called \"nope\"." })
        );
        // The headless sim has no sound; the windowed path is tested in
        // `content::audio`.
        assert_eq!(
            events[1],
            json!({ "name": "menu", "ok": false, "error": "This build plays no sound." })
        );
    }

    #[test]
    fn catalog_lists_content_warnings() {
        let mut app = build_headless_app();
        app.update();
        app.world_mut()
            .resource_mut::<crate::game::content::saved::ContentWarnings>()
            .0
            .push("careful".to_string());
        let j = catalog_json(app.world());
        assert_eq!(j["content_warnings"], serde_json::json!(["careful"]));
    }

    #[test]
    fn checks_run_against_the_current_values_and_unknown_names_answer() {
        let mut app = build_headless_app();
        app.update();
        drain_custom(&mut app);
        send(&mut app, "checks.run", "{\"names\":[\"no_such_check\"]}");
        app.update();
        let events = drain_custom(&mut app);
        assert_eq!(events[0].0, "check");
        let j: Json = serde_json::from_str(&events[0].1).unwrap();
        assert_eq!(j["ok"], false);
        assert_eq!(j["message"], "no check named no_such_check");
    }

    #[test]
    fn restart_enters_playing_and_posts_the_catalog() {
        let mut app = build_headless_app();
        app.update();
        drain_custom(&mut app);
        send(&mut app, "run.restart", "null");
        app.update(); // restart_run: Menu? no — StudioLogo → Playing directly
        app.update(); // transition applied, OnEnter(Playing) requests the list
        app.update(); // list answered
        assert_eq!(
            *app.world().resource::<State<GameState>>().get(),
            GameState::Playing
        );
        let events = drain_custom(&mut app);
        assert!(events.iter().any(|(n, _)| n == "tunables"), "{events:?}");
        for _ in 0..5 {
            app.update();
        }
        let before = app.world().resource::<SimTick>().0;
        assert!(before > 0);
        send(&mut app, "run.restart", "null");
        for _ in 0..6 {
            app.update();
        }
        assert_eq!(
            *app.world().resource::<State<GameState>>().get(),
            GameState::Playing
        );
        assert!(app.world().resource::<SimTick>().0 < before, "a fresh run");
    }

    const PIXEL_PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==";

    fn content_replies(app: &mut App) -> Vec<Json> {
        drain_custom(app)
            .iter()
            .filter(|e| e.0 == "content")
            .map(|e| serde_json::from_str(&e.1).unwrap())
            .collect()
    }

    #[test]
    fn content_set_level_answers_hash_and_applies_on_restart() {
        use crate::game::content::{files, level::Level};
        let mut app = build_headless_app();
        app.update();
        drain_custom(&mut app);
        let text = r#"{ "v":1,"things":[{"id":"p","kind":"Player","at":[1,2]}] }"#;
        send(
            &mut app,
            "content.set",
            &json!({ "file": "level.json", "text": text }).to_string(),
        );
        app.update();
        let r = content_replies(&mut app);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0]["ok"], true);
        assert_eq!(r[0]["file"], "level.json");
        assert_eq!(r[0]["hash"], files::hex(files::fnv1a64(text.as_bytes())));
        assert_eq!(app.world().resource::<Level>().0.things.len(), 1);
        send(&mut app, "run.restart", "null");
        for _ in 0..4 {
            app.update();
        }
        let n = app
            .world_mut()
            .query::<&crate::game::content::kit::ThingId>()
            .iter(app.world())
            .count();
        assert_eq!(n, 1, "the new level spawned");
        // The catalog reflects the hash of the exact text.
        let j = catalog_json(app.world());
        assert_eq!(
            j["files"]["level.json"],
            files::hex(files::fnv1a64(text.as_bytes()))
        );
        assert!(j["files"]["tunables.json"].is_string());
    }

    #[test]
    fn content_set_invalid_level_changes_nothing() {
        use crate::game::content::level::LevelHash;
        let mut app = build_headless_app();
        app.update();
        drain_custom(&mut app);
        let before = app.world().resource::<LevelHash>().0;
        let files_before = catalog_json(app.world())["files"].clone();
        send(
            &mut app,
            "content.set",
            &json!({ "file": "level.json", "text": "{\"v\":1,\"things\":[{\"id\":\"g\",\"kind\":\"Ghost\",\"at\":[0,0]}]}" }).to_string(),
        );
        app.update();
        let r = content_replies(&mut app);
        assert_eq!(r[0]["ok"], false);
        assert_eq!(r[0]["error"], "There is no kind called \"Ghost\".");
        assert_eq!(app.world().resource::<LevelHash>().0, before);
        assert_eq!(catalog_json(app.world())["files"], files_before);
    }

    #[test]
    fn content_set_tunables_goes_through_pending_and_is_atomic() {
        use crate::game::tunables::read;
        let mut app = build_headless_app();
        app.update();
        drain_custom(&mut app);
        let entry = app.world().resource::<TunableIndex>().entries[0].clone();
        let next = bump(&entry.default);
        let good = json!({ "v": 1, "values": { entry.path.clone(): next.to_json() } }).to_string();
        send(
            &mut app,
            "content.set",
            &json!({ "file": "tunables.json", "text": good }).to_string(),
        );
        app.update();
        let events = drain_custom(&mut app);
        let r: Json =
            serde_json::from_str(&events.iter().find(|e| e.0 == "content").unwrap().1).unwrap();
        assert_eq!(r["ok"], true);
        assert!(
            events.iter().any(|e| e.0 == "tunable"),
            "logged like tunables.set"
        );
        assert_eq!(read(app.world(), &entry), Some(next.clone()));

        // One bad path: nothing is applied, even the good value.
        let bad = json!({ "v": 1, "values": { entry.path.clone(): entry.default.to_json(), "nope.x": 1 } }).to_string();
        send(
            &mut app,
            "content.set",
            &json!({ "file": "tunables.json", "text": bad }).to_string(),
        );
        app.update();
        let r = content_replies(&mut app);
        assert_eq!(r[0]["ok"], false);
        assert_eq!(r[0]["error"], "There is no setting called \"nope.x\".");
        assert_eq!(read(app.world(), &entry), Some(next));
    }

    #[test]
    fn content_set_sprite_decodes_base64() {
        use crate::game::content::{base64, files};
        let mut app = build_headless_app();
        app.update();
        drain_custom(&mut app);
        send(
            &mut app,
            "content.set",
            &json!({ "file": "sprites/Coin.png", "png": PIXEL_PNG }).to_string(),
        );
        send(
            &mut app,
            "content.set",
            &json!({ "file": "sprites/Ghost.png", "png": PIXEL_PNG }).to_string(),
        );
        send(
            &mut app,
            "content.set",
            &json!({ "file": "sprites/Coin.png", "text": "x" }).to_string(),
        );
        send(
            &mut app,
            "content.set",
            &json!({ "file": "level.json", "png": PIXEL_PNG }).to_string(),
        );
        app.update();
        let r = content_replies(&mut app);
        assert_eq!(r[0]["ok"], true, "{r:?}");
        let bytes = base64::decode(PIXEL_PNG).unwrap();
        assert_eq!(r[0]["hash"], files::hex(files::fnv1a64(&bytes)));
        assert_eq!(r[1]["ok"], false);
        assert_eq!(r[1]["error"], "There is no kind called \"Ghost\".");
        assert_eq!(r[2]["ok"], false);
        assert_eq!(r[3]["ok"], false);
        let j = catalog_json(app.world());
        assert_eq!(j["files"]["sprites/Coin.png"], r[0]["hash"]);
    }

    #[test]
    fn content_set_unknown_file_is_refused_and_name_is_capped() {
        let mut app = build_headless_app();
        app.update();
        drain_custom(&mut app);
        send(
            &mut app,
            "content.set",
            r#"{"file":"nope.json","text":"{}"}"#,
        );
        let long = "x".repeat(100);
        send(
            &mut app,
            "content.set",
            &json!({ "file": long, "text": "{}" }).to_string(),
        );
        app.update();
        let r = content_replies(&mut app);
        assert_eq!(r[0]["ok"], false);
        assert_eq!(
            r[0]["error"],
            "There is no content file called \"nope.json\"."
        );
        assert_eq!(
            r[1]["error"],
            format!("There is no content file called \"{}\".", "x".repeat(40))
        );
    }

    #[test]
    fn content_set_audio_answers_for_a_headless_build() {
        let mut app = build_headless_app();
        app.update();
        drain_custom(&mut app);
        send(
            &mut app,
            "content.set",
            r#"{"file":"audio/sfx.json","text":"{\"v\":1,\"sfx\":{}}"}"#,
        );
        app.update();
        let r = content_replies(&mut app);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0]["file"], "audio/sfx.json");
    }

    #[test]
    fn catalog_lists_every_embedded_file() {
        let mut app = build_headless_app();
        app.update();
        let j = catalog_json(app.world());
        for (p, b) in crate::game::content::files::EMBEDDED {
            assert_eq!(
                j["files"][*p],
                crate::game::content::files::hex(crate::game::content::files::fnv1a64(b))
            );
        }
    }
}
