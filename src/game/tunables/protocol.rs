//! The editor's host layer, on top of the `gamebient-input` host protocol
//! (feature `editor`). Names, in `gx:custom` / `custom` messages:
//!
//! | Direction   | Name             | Data                                              |
//! |-------------|------------------|---------------------------------------------------|
//! | host → game | `tunables.list`  | —                                                 |
//! | game → host | `tunables`       | the catalog: groups, tunables with values, checks, sounds, content, level, rules, looks |
//! | host → game | `tunables.set`   | `{ "<path>": value, … }`                          |
//! | game → host | `tunable`        | `{ path, value, applied: "tick"\|"now", tick }` or `{ path, error }` |
//! | host → game | `checks.run`     | `{ "names": [ … ] }`                              |
//! | game → host | `check`          | `{ name, ok, message }`, one per check            |
//! | host → game | `run.restart`    | —                                                 |
//! | host → game | `sound.preview`  | `{ name }`: an `SfxEvent` name or a cue/layer name |
//! | game → host | `sound`          | `{ name, ok: true }` or `{ name, ok: false, error }` |
//! | host → game | `content.set`    | `{ file, text }` (a `.json` file) or `{ file, png }` (base64, `sprites/<Kind>.png`) |
//! | game → host | `content`        | `{ file, ok, error?, hash }`; hash is FNV-1a 64 of the exact bytes |
//! | host → game | `level.edit`     | `{ on: bool }`: enter Editing, or leave it and start a run |
//! | host → game | `level.camera`   | `{ at: [x, y], zoom }`: the edit camera; ignored outside Editing |
//! | game → host | `level.view`     | `{ at: [x, y], zoom, canvas: [x, y, w, h] }`: zoom is CSS px per world unit, canvas in the frame's CSS px |
//! | game → host | `level.mode`     | `{ editing: bool }`, on every state change |
//! | host → game | `look.preview`   | `{ id, files: { "look.json": {text}, "<name>.png": {png}, "<name>.ogg": {ogg} } }`: validate and wear (src/look/editor.rs) |
//! | host → game | `look.preview.clear` | `{}`: back to the game's own look |
//! | game → host | `look`           | `{ id, ok, errors: ["<where>: <what>"], hash, warnings: ["<where>: <what>"] }`; hash is the pack's content hash when ok, else null |
//!
//! (`look.set` / `look.clear` / `look.applied` are the hosted looks'
//! commands, in every build: `src/look/hosted.rs`.)
//!
//! `content.set` files: `level.json` (applies at the next run start; send
//! `run.restart`), `tunables.json` (through `PendingTunes`, all or nothing),
//! `sprites.json`, `sprites/<Kind>.png`, `audio/songs.json`, `audio/sfx.json`,
//! `rules.json` (checked whole: the next run starts with it and a run in
//! progress takes it at the next tick).
//! While Editing, `level.json` respawns the shown level at once and no restart is needed.
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

/// The last `level.edit`: `Some(true)` enter Editing, `Some(false)` leave it
/// for a run. Applied by `apply_edit_request` once the fade is idle.
#[derive(Resource, Default)]
struct EditRequest(Option<bool>);

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
        .init_resource::<EditRequest>()
        // Around the menu-time drain (`apply_pending_tunes` in `Update`,
        // registered by `TunablesPlugin`): a set received this frame
        // applies this frame and is reported this frame. During a run
        // the drain is in `SimSet` and the report follows a tick later.
        .add_systems(
            Update,
            (
                handle_custom_commands,
                apply_content_sets,
                restart_run,
                apply_edit_request,
            )
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
                report_level_mode,
                report_level_view,
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
    mut edit: ResMut<EditRequest>,
    state: Res<State<GameState>>,
    mut view: ResMut<crate::game::content::edit::EditView>,
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
            "level.edit" => {
                if let Some(on) = data.get("on").and_then(Json::as_bool) {
                    edit.0 = Some(on);
                }
            }
            "level.camera" => {
                if *state.get() != GameState::Editing {
                    continue;
                }
                let at = data
                    .get("at")
                    .and_then(Json::as_array)
                    .filter(|a| a.len() == 2)
                    .and_then(|a| Some(Vec2::new(a[0].as_f64()? as f32, a[1].as_f64()? as f32)));
                let zoom = data.get("zoom").and_then(Json::as_f64).map(|z| z as f32);
                view.set_camera(at, zoom);
            }
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
                "cosmetic": e.tune.cosmetic,
                "value": value,
                "default": e.default.to_json(),
            });
            if !e.options.is_empty() {
                j["options"] = json!(e.options);
            }
            j
        })
        .collect();
    let mut j = json!({
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
    });
    if let Some(level) = level_catalog(world) {
        j["level"] = level;
    }
    j["rules"] = crate::game::rules::rules_catalog(world);
    if let Some(looks) = crate::look::editor::looks_catalog(world) {
        j["looks"] = looks;
    }
    j
}

/// `"#rrggbb"` of a colour in sRGB, for the editor's palette swatches.
pub fn colour_hex(c: Color) -> String {
    let s = c.to_srgba();
    let ch = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", ch(s.red), ch(s.green), ch(s.blue))
}

/// The catalog's `level`: the grid and every placeable kind, in
/// registration order (runtime-only kinds are left out, so the palette
/// never offers them). `None` when the game registers no kinds (the editor
/// then hides Edit).
pub fn level_catalog(world: &World) -> Option<Json> {
    use crate::game::content::kit::{Kinds, LevelGrid};
    let kinds = world.get_resource::<Kinds>()?;
    if kinds.0.is_empty() {
        return None;
    }
    let grid = world
        .get_resource::<LevelGrid>()
        .copied()
        .unwrap_or_default()
        .0;
    let files = world.get_resource::<crate::game::content::LiveFiles>();
    let rows: Vec<Json> = kinds
        .0
        .iter()
        .filter(|(_, p)| p.edit.placeable)
        .map(|(name, p)| {
            let path = format!("sprites/{name}.png");
            let sprite = files
                .is_some_and(|f| f.0.iter().any(|(q, _)| *q == path))
                .then_some(path);
            json!({
                "kind": name,
                "size": [p.size.x, p.size.y],
                "colour": colour_hex(p.colour),
                "sprite": sprite,
                "resizable": p.edit.resizable,
                "max": p.edit.max,
            })
        })
        .collect();
    Some(json!({ "grid": grid, "kinds": rows }))
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
        "level.json"
            | "tunables.json"
            | "sprites.json"
            | "audio/songs.json"
            | "audio/sfx.json"
            | "rules.json"
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
        "level.json" => {
            e(level::set_level(world, &bytes))?;
            // While Editing the shown level follows at once; otherwise the
            // new level waits for the next run start.
            if world
                .get_resource::<State<GameState>>()
                .is_some_and(|s| *s.get() == GameState::Editing)
            {
                crate::game::content::edit::respawn_edit_level(world);
            }
        }
        "sprites.json" => e(sprites::set_sprites_meta(world, &bytes))?,
        "audio/songs.json" => e(audio::set_songs(world, &bytes))?,
        "audio/sfx.json" => e(audio::set_sfx(world, &bytes))?,
        // Checked whole; the next run starts with it, and a run in progress
        // takes it at the next tick (`rules::PendingRules`).
        "rules.json" => crate::game::rules::set_rules(world, &bytes)?,
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

/// `level.edit`: into Editing, or out of it into a normal run start. Through
/// the `ScreenFade` when there is one, exactly like `restart_run`. A request
/// for the state the game is already in is dropped.
fn apply_edit_request(
    mut edit: ResMut<EditRequest>,
    state: Res<State<GameState>>,
    mut next: ResMut<NextState<GameState>>,
    fade: Option<ResMut<crate::ui::transition::ScreenFade>>,
) {
    let Some(on) = edit.0 else {
        return;
    };
    if on == (*state.get() == GameState::Editing) {
        edit.0 = None;
        return;
    }
    let target = if on {
        GameState::Editing
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
    edit.0 = None;
}

/// `level.view`'s data.
pub fn view_json(v: &crate::game::content::edit::EditView) -> Json {
    json!({ "at": [v.at.x, v.at.y], "zoom": v.zoom, "canvas": v.canvas })
}

/// `level.mode` on every state change (a run ending in GameOver included).
fn report_level_mode(state: Res<State<GameState>>, mut out: MessageWriter<HostEvent>) {
    if state.is_changed() {
        out.write(custom(
            "level.mode",
            json!({ "editing": *state.get() == GameState::Editing }),
        ));
    }
}

/// `level.view` whenever the edit camera or the canvas changes while Editing.
fn report_level_view(
    state: Res<State<GameState>>,
    view: Res<crate::game::content::edit::EditView>,
    mut out: MessageWriter<HostEvent>,
) {
    if *state.get() == GameState::Editing && view.is_changed() {
        out.write(custom("level.view", view_json(&view)));
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
    use crate::game::replay::{build_builtin_app, build_headless_app};
    use crate::game::sim::SimTick;
    use crate::game::tunables::index::bumped_in_range;

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

    /// Generic over the game's first tunable, so this test survives a game
    /// replacing `tuning.rs` (this file is copied into every game).
    #[test]
    fn list_set_and_report_over_host_messages() {
        use crate::game::tunables::read;
        let mut app = build_builtin_app();
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
        assert_eq!(j["tunables"][0]["cosmetic"], entry.tune.cosmetic);
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

        let next = bumped_in_range(&entry, &entry.default);
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
        let mut app = build_builtin_app();
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
        let mut app = build_builtin_app();
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
        let mut app = build_builtin_app();
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
        let mut app = build_builtin_app();
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
        let mut app = build_builtin_app();
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
        let mut app = build_builtin_app();
        app.update();
        drain_custom(&mut app);
        let entry = app.world().resource::<TunableIndex>().entries[0].clone();
        let next = bumped_in_range(&entry, &entry.default);
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
        let mut app = build_builtin_app();
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
        let mut app = build_builtin_app();
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
        let mut app = build_builtin_app();
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
        let mut app = build_builtin_app();
        app.update();
        let j = catalog_json(app.world());
        for (p, b) in crate::game::content::files::EMBEDDED {
            assert_eq!(
                j["files"][*p],
                crate::game::content::files::hex(crate::game::content::files::fnv1a64(b))
            );
        }
    }

    #[test]
    fn the_catalog_level_lists_every_placeable_kind_with_its_rules() {
        use crate::game::content::kit::{Kinds, LevelGrid};
        let mut app = build_builtin_app();
        app.update();
        let j = catalog_json(app.world());
        let level = &j["level"];
        assert_eq!(level["grid"], json!(app.world().resource::<LevelGrid>().0));
        let kinds = app.world().resource::<Kinds>();
        let placeable: Vec<_> = kinds.0.iter().filter(|(_, p)| p.edit.placeable).collect();
        let rows = level["kinds"].as_array().unwrap();
        assert_eq!(rows.len(), placeable.len());
        for (row, (name, p)) in rows.iter().zip(placeable) {
            assert_eq!(row["kind"], json!(name));
            assert_eq!(row["size"], json!([p.size.x, p.size.y]));
            assert_eq!(row["colour"], json!(colour_hex(p.colour)));
            assert_eq!(row["resizable"], json!(p.edit.resizable));
            assert_eq!(row["max"], json!(p.edit.max));
            let sprite = format!("sprites/{name}.png");
            let has = crate::game::content::files::embedded(&sprite).is_some();
            assert_eq!(row["sprite"], if has { json!(sprite) } else { Json::Null });
        }
    }

    #[test]
    fn a_live_sprite_shows_in_the_catalog_level() {
        let mut app = build_builtin_app();
        app.update();
        let first = app.world().resource::<crate::game::content::kit::Kinds>().0[0]
            .0
            .clone();
        send(
            &mut app,
            "content.set",
            &json!({ "file": format!("sprites/{first}.png"), "png": PIXEL_PNG }).to_string(),
        );
        app.update();
        let j = catalog_json(app.world());
        assert_eq!(
            j["level"]["kinds"][0]["sprite"],
            json!(format!("sprites/{first}.png"))
        );
    }

    #[test]
    fn a_kind_that_is_not_placeable_is_left_out() {
        use crate::game::content::kit::Kinds;
        let mut app = build_builtin_app();
        app.update();
        let first = {
            let mut kinds = app.world_mut().resource_mut::<Kinds>();
            kinds.0[0].1.edit.placeable = false;
            kinds.0[0].0.clone()
        };
        let j = catalog_json(app.world());
        let rows = j["level"]["kinds"].as_array().unwrap();
        assert!(rows.iter().all(|r| r["kind"] != json!(first)), "{rows:?}");
        let n = app.world().resource::<Kinds>().0.len();
        assert_eq!(rows.len(), n - 1);
    }

    #[test]
    fn a_game_without_kinds_sends_no_level() {
        let mut app = build_builtin_app();
        app.update();
        app.world_mut()
            .resource_mut::<crate::game::content::kit::Kinds>()
            .0
            .clear();
        assert!(catalog_json(app.world()).get("level").is_none());
    }

    #[test]
    fn colours_are_lowercase_srgb_hex() {
        assert_eq!(colour_hex(Color::srgb(1.0, 0.85, 0.2)), "#ffd933");
        assert_eq!(colour_hex(Color::BLACK), "#000000");
    }

    fn named(app: &mut App, name: &str) -> Vec<Json> {
        drain_custom(app)
            .iter()
            .filter(|e| e.0 == name)
            .map(|e| serde_json::from_str(&e.1).unwrap())
            .collect()
    }

    /// `level.edit {on:true}` from wherever the game is, two frames: the
    /// request, then the transition and the reports.
    fn enter_editing(app: &mut App) -> Vec<(String, String)> {
        send(app, "level.edit", r#"{"on":true}"#);
        app.update();
        app.update();
        assert_eq!(
            *app.world().resource::<State<GameState>>().get(),
            GameState::Editing
        );
        drain_custom(app)
    }

    #[test]
    fn level_edit_enters_editing_reports_mode_and_view_then_plays() {
        let mut app = build_headless_app();
        app.update();
        drain_custom(&mut app);
        let events = enter_editing(&mut app);
        let mode: Vec<Json> = events
            .iter()
            .filter(|e| e.0 == "level.mode")
            .map(|e| serde_json::from_str(&e.1).unwrap())
            .collect();
        assert_eq!(mode, vec![json!({ "editing": true })]);
        let view: Json = serde_json::from_str(
            &events
                .iter()
                .find(|e| e.0 == "level.view")
                .expect("a view")
                .1,
        )
        .unwrap();
        let v = *app
            .world()
            .resource::<crate::game::content::edit::EditView>();
        assert_eq!(view, view_json(&v));
        assert_eq!(view["canvas"].as_array().unwrap().len(), 4);

        send(&mut app, "level.edit", r#"{"on":false}"#);
        app.update();
        app.update();
        assert_eq!(
            *app.world().resource::<State<GameState>>().get(),
            GameState::Playing
        );
        assert_eq!(
            named(&mut app, "level.mode"),
            vec![json!({ "editing": false })]
        );
    }

    #[test]
    fn level_edit_off_outside_editing_changes_nothing() {
        let mut app = build_headless_app();
        app.update();
        let before = *app.world().resource::<State<GameState>>().get();
        send(&mut app, "level.edit", r#"{"on":false}"#);
        app.update();
        app.update();
        assert_eq!(*app.world().resource::<State<GameState>>().get(), before);
    }

    #[test]
    fn level_camera_moves_and_clamps_only_while_editing() {
        use crate::game::content::edit::{EditView, ZOOM_MAX, ZOOM_MIN};
        let mut app = build_headless_app();
        app.update();
        // Outside Editing: ignored.
        let before = *app.world().resource::<EditView>();
        send(&mut app, "level.camera", r#"{"at":[5,5],"zoom":2}"#);
        app.update();
        assert_eq!(*app.world().resource::<EditView>(), before);

        enter_editing(&mut app);
        send(&mut app, "level.camera", r#"{"at":[10,20],"zoom":3}"#);
        app.update();
        let v = *app.world().resource::<EditView>();
        assert_eq!((v.at, v.zoom), (Vec2::new(10.0, 20.0), 3.0));
        assert_eq!(named(&mut app, "level.view"), vec![view_json(&v)]);

        send(&mut app, "level.camera", r#"{"zoom":0}"#);
        app.update();
        assert_eq!(app.world().resource::<EditView>().zoom, ZOOM_MIN);
        // 1e300 is finite JSON but not a finite f32: the zoom stays.
        send(&mut app, "level.camera", r#"{"zoom":1e300,"at":[1e300,0]}"#);
        app.update();
        let v = *app.world().resource::<EditView>();
        assert_eq!((v.at, v.zoom), (Vec2::new(10.0, 20.0), ZOOM_MIN));
        send(&mut app, "level.camera", r#"{"zoom":500}"#);
        app.update();
        assert_eq!(app.world().resource::<EditView>().zoom, ZOOM_MAX);
    }

    /// The first `level.view` carries the zoom that fits the level in the
    /// canvas the page reports.
    #[test]
    fn the_first_level_view_fits_the_reported_canvas() {
        use crate::game::content::edit::{CanvasProbe, EditView, fit_view, level_bounds};
        use crate::game::content::kit::{Kinds, LevelGrid};
        use crate::game::content::level::Level;
        fn small() -> Option<[f32; 4]> {
            Some([0.0, 40.0, 497.0, 372.75])
        }
        let mut app = build_headless_app();
        app.insert_resource(CanvasProbe(small));
        app.update();
        drain_custom(&mut app);
        let events = enter_editing(&mut app);
        let view: Json = serde_json::from_str(
            &events
                .iter()
                .find(|e| e.0 == "level.view")
                .expect("a view")
                .1,
        )
        .unwrap();
        assert_eq!(view["canvas"], json!([0.0, 40.0, 497.0, 372.75]));
        let w = app.world();
        let (_, zoom) = fit_view(
            level_bounds(&w.resource::<Level>().0, w.resource::<Kinds>()),
            w.resource::<LevelGrid>().0,
            Vec2::new(497.0, 372.75),
        );
        assert_eq!(view["zoom"].as_f64().unwrap() as f32, zoom);
        assert_eq!(w.resource::<EditView>().zoom, zoom);
    }

    #[test]
    fn level_view_follows_a_letterbox_change() {
        use crate::game::content::edit::EditView;
        let mut app = build_headless_app();
        app.update();
        enter_editing(&mut app);
        app.world_mut().resource_mut::<EditView>().canvas = [10.0, 0.0, 480.0, 360.0];
        app.update();
        let views = named(&mut app, "level.view");
        assert_eq!(views.len(), 1);
        assert_eq!(views[0]["canvas"], json!([10.0, 0.0, 480.0, 360.0]));
        app.update();
        assert!(named(&mut app, "level.view").is_empty(), "only on a change");
    }

    #[test]
    fn content_set_level_while_editing_respawns_at_once() {
        use crate::game::content::edit::EditThing;
        let mut app = build_headless_app();
        app.update();
        enter_editing(&mut app);
        let text = r#"{"v":1,"things":[{"id":"coin-9","kind":"Coin","at":[1,1]}]}"#;
        send(
            &mut app,
            "content.set",
            &json!({ "file": "level.json", "text": text }).to_string(),
        );
        app.update();
        let shown: Vec<String> = app
            .world_mut()
            .query_filtered::<&crate::game::content::kit::ThingId, With<EditThing>>()
            .iter(app.world())
            .map(|id| id.0.clone())
            .collect();
        assert_eq!(shown, vec!["coin-9".to_string()]);
        assert_eq!(
            *app.world().resource::<State<GameState>>().get(),
            GameState::Editing,
            "no restart"
        );
    }

    #[test]
    fn a_run_ending_in_game_over_reports_the_mode() {
        let mut app = build_headless_app();
        app.update();
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.update();
        drain_custom(&mut app);
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::GameOver);
        app.update();
        assert_eq!(
            named(&mut app, "level.mode"),
            vec![json!({ "editing": false })]
        );
    }

    #[test]
    fn the_catalog_carries_looks_only_with_a_look_plugin() {
        let mut app = build_builtin_app();
        app.update();
        assert!(catalog_json(app.world()).get("looks").is_none());
        app.insert_resource(crate::look::ActiveLook::default_for(
            &crate::ui::theme::LOOK,
        ));
        let j = catalog_json(app.world());
        assert_eq!(
            j["looks"],
            crate::look::editor::looks_catalog(app.world()).unwrap()
        );
    }

    #[test]
    fn content_set_rules_applies_to_the_run_in_progress() {
        use crate::game::content::files;
        use crate::game::rules::Rules;
        use crate::game::rules::run::RunRules;
        use crate::game::scoring::GameData;
        let mut app = build_headless_app();
        app.update();
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.update();
        drain_custom(&mut app);
        let text = r#"{"v":1,"rules":[{"id":"a","when":{"every":1},"do":[{"add":["score",1]}]}]}"#;
        send(
            &mut app,
            "content.set",
            &json!({ "file": "rules.json", "text": text }).to_string(),
        );
        app.update();
        let r = content_replies(&mut app);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0]["ok"], true);
        assert_eq!(r[0]["file"], "rules.json");
        assert_eq!(r[0]["hash"], files::hex(files::fnv1a64(text.as_bytes())));
        let score = app.world().resource::<GameData>().score;
        app.update();
        app.update();
        assert!(
            app.world().resource::<GameData>().score >= score + 2,
            "the run in progress follows the new rules"
        );
        assert_eq!(
            *app.world().resource::<State<GameState>>().get(),
            GameState::Playing
        );
        assert_eq!(
            app.world().resource::<RunRules>().file,
            app.world().resource::<Rules>().0
        );
        assert_eq!(
            catalog_json(app.world())["files"]["rules.json"],
            files::hex(files::fnv1a64(text.as_bytes()))
        );
    }

    #[test]
    fn content_set_refuses_a_rules_file_with_its_sentence() {
        let mut app = build_headless_app();
        app.update();
        drain_custom(&mut app);
        let before = catalog_json(app.world())["files"]["rules.json"].clone();
        send(
            &mut app,
            "content.set",
            &json!({ "file": "rules.json", "text": r#"{"v":1,"rules":[{"id":"a","when":{"start":true},"do":[{"add":["gems",1]}]}]}"# }).to_string(),
        );
        app.update();
        let r = content_replies(&mut app);
        assert_eq!(r[0]["ok"], false);
        assert_eq!(
            r[0]["error"],
            "Rule 1: This game has no counter called gems."
        );
        assert_eq!(r[0]["hash"], before);
        assert_eq!(catalog_json(app.world())["files"]["rules.json"], before);
    }

    #[test]
    fn the_catalog_carries_the_rules_vocabulary() {
        let app = build_headless_app();
        let j = catalog_json(app.world());
        assert_eq!(j["rules"], crate::game::rules::rules_catalog(app.world()));
        assert!(
            j["rules"]["kinds"]
                .as_array()
                .unwrap()
                .iter()
                .any(|k| k == "Player")
        );
        assert_eq!(j["rules"]["triggers"].as_array().unwrap().len(), 6);
    }
}
