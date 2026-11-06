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
/// sound picker may offer. Filled by the game's plugin.
#[derive(Resource, Default)]
pub struct EditorInfo {
    pub sounds: Vec<&'static str>,
}

#[derive(Resource, Default)]
struct ListRequested(bool);

#[derive(Resource, Default)]
struct ChecksRequested(Vec<String>);

/// Set by `run.restart`; cleared once the run has been re-entered.
#[derive(Resource, Default)]
pub struct RestartPending(pub bool);

pub struct EditorProtocolPlugin;

impl Plugin for EditorProtocolPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EditorInfo>()
            .init_resource::<ListRequested>()
            .init_resource::<ChecksRequested>()
            .init_resource::<RestartPending>()
            // Around the menu-time drain (`apply_pending_tunes` in `Update`,
            // registered by `TunablesPlugin`): a set received this frame
            // applies this frame and is reported this frame. During a run
            // the drain is in `SimSet` and the report follows a tick later.
            .add_systems(
                Update,
                (handle_custom_commands, restart_run)
                    .chain()
                    .before(apply_pending_tunes),
            )
            .add_systems(
                Update,
                (
                    answer_tunables_list,
                    run_requested_checks,
                    report_tune_applied,
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
        "content": hash::content(world.resource::<TuneLog>()).to_string(),
        "groups": idx.groups.iter().map(|g| json!({ "path": g.path, "label": g.label, "kind": g.kind.name() })).collect::<Vec<_>>(),
        "tunables": tunables,
        "checks": checks.iter().map(|c| json!({ "name": c.name, "title": c.title })).collect::<Vec<_>>(),
        "sounds": info.sounds,
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

fn report_tune_applied(
    mut applied: MessageReader<TuneApplied>,
    idx: Res<TunableIndex>,
    mut out: MessageWriter<HostEvent>,
) {
    for a in applied.read() {
        let Some(entry) = idx.by_id(a.id) else {
            continue;
        };
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
fn restart_run(
    mut restart: ResMut<RestartPending>,
    state: Res<State<GameState>>,
    mut next: ResMut<NextState<GameState>>,
) {
    if !restart.0 {
        return;
    }
    match state.get() {
        GameState::Playing => next.set(GameState::Menu),
        GameState::Menu => {
            restart.0 = false;
            next.set(GameState::Playing);
        }
        _ => {
            restart.0 = false;
            next.set(GameState::Playing);
        }
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

    #[test]
    fn list_set_and_report_over_host_messages() {
        let mut app = build_headless_app();
        app.update();
        drain_custom(&mut app);
        send(&mut app, "tunables.list", "null");
        app.update();
        let events = drain_custom(&mut app);
        assert_eq!(events[0].0, "tunables");
        let j: Json = serde_json::from_str(&events[0].1).unwrap();
        assert_eq!(j["tunables"][0]["path"], "run.spare");
        assert_eq!(j["tunables"][0]["type"], "i32");
        assert_eq!(j["tunables"][0]["value"], 1);
        assert_eq!(j["tunables"][0]["applies"], "tick");
        assert_eq!(j["groups"][0]["kind"], "rule");
        assert!(j["content"].is_string());
        assert!(j["build"].as_str().unwrap().contains('+'));

        send(
            &mut app,
            "tunables.set",
            "{\"run.spare\":4,\"nope.x\":1,\"run.spare\":\"x\"}",
        );
        app.update();
        let events = drain_custom(&mut app);
        let applied: Vec<Json> = events
            .iter()
            .filter(|e| e.0 == "tunable")
            .map(|e| serde_json::from_str(&e.1).unwrap())
            .collect();
        // serde_json keeps the last duplicate key, so "x" wins and is
        // rejected; the unknown path is reported too (order is the map's).
        let spare = applied.iter().find(|j| j["path"] == "run.spare").unwrap();
        assert_eq!(spare["error"], "not a i32");
        let nope = applied.iter().find(|j| j["path"] == "nope.x").unwrap();
        assert_eq!(nope["error"], "unknown tunable");
        assert_eq!(applied.len(), 2);

        send(&mut app, "tunables.set", "{\"run.spare\":4}");
        app.update();
        let events = drain_custom(&mut app);
        let j: Json = serde_json::from_str(&events[0].1).unwrap();
        assert_eq!(events[0].0, "tunable");
        assert_eq!(j["path"], "run.spare");
        assert_eq!(j["value"], 4);
        assert_eq!(j["applied"], "now");
        assert_eq!(
            app.world()
                .resource::<crate::game::tuning::RunTuning>()
                .spare,
            4
        );
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
}
