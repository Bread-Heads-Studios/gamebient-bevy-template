//! Hosted looks end to end, native-compiled: `look.set` / `look.clear`
//! arrive as host commands, a hand-filled slot stands in for the web
//! fetch, and every outcome is answered with `look.applied`.

use bevy::prelude::*;
use gamebient_game::look::fetch::{Arrival, PendingFetch};
use gamebient_game::look::validate::content_hash;
use gamebient_game::look::{self, ActiveLook, LookChanged, LookPlugin, Selection};
use gamebient_game::ui::theme;
use gamebient_input::{HostCommand, HostEvent};

#[derive(Resource, Default)]
struct Seen {
    applied: Vec<serde_json::Value>,
    changed: usize,
}

fn record(
    mut events: MessageReader<HostEvent>,
    mut changed: MessageReader<LookChanged>,
    mut seen: ResMut<Seen>,
) {
    for e in events.read() {
        if let HostEvent::Custom { name, data } = e
            && name == look::hosted::APPLIED
        {
            seen.applied.push(serde_json::from_str(data).unwrap());
        }
    }
    seen.changed += changed.read().count();
}

fn app(selection: Selection) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(selection)
        .add_plugins(LookPlugin {
            contract: &theme::LOOK,
        })
        .init_resource::<Seen>()
        .add_systems(Last, record);
    app
}

fn send(app: &mut App, name: &str, data: serde_json::Value) {
    app.world_mut().write_message(HostCommand::Custom {
        name: name.into(),
        data: data.to_string(),
    });
}

fn night_shift() -> (look::validate::Files, String) {
    let files = look::pack::embedded_files("night-shift").unwrap();
    let hash = content_hash(&files);
    (files, hash)
}

/// A hosted fetch of night-shift "in flight", as `start_hosted` leaves it.
fn in_flight(app: &mut App, hash: &str) -> Arrival {
    let (pending, arrival) = PendingFetch::new_hosted("night-shift", hash);
    app.insert_resource(pending);
    arrival
}

#[test]
fn a_hosted_pack_that_arrives_intact_is_worn_and_answered() {
    let mut app = app(Selection::Default);
    let (files, hash) = night_shift();
    let arrival = in_flight(&mut app, &hash);
    app.update();
    assert_eq!(app.world().resource::<ActiveLook>().id(), "default");

    *arrival.lock().unwrap() = Some(Ok(files));
    app.update();
    let look = app.world().resource::<ActiveLook>();
    assert_eq!(look.id(), "night-shift");
    assert_eq!(look.source, Selection::Hosted("night-shift".into()));
    let seen = app.world().resource::<Seen>();
    assert_eq!(
        seen.applied,
        vec![serde_json::json!({"id":"night-shift","ok":true})]
    );
    assert_eq!(seen.changed, 1);
}

#[test]
fn a_hash_mismatch_keeps_the_look_and_says_hash() {
    let mut app = app(Selection::Default);
    let (files, _) = night_shift();
    let arrival = in_flight(&mut app, &"0".repeat(64));
    *arrival.lock().unwrap() = Some(Ok(files));
    app.update();
    assert_eq!(app.world().resource::<ActiveLook>().id(), "default");
    assert_eq!(
        app.world().resource::<Seen>().applied,
        vec![serde_json::json!({"id":"night-shift","ok":false,"reason":"hash"})]
    );
}

#[test]
fn fetch_failures_say_fetch_or_caps() {
    for (err, reason) in [
        (
            "https://colecovisiongx.com/looks/x/look.json: HTTP 404",
            "fetch",
        ),
        ("caps: 17000000 bytes; at most 16777216", "caps"),
    ] {
        let mut app = app(Selection::Default);
        let arrival = in_flight(&mut app, &"0".repeat(64));
        *arrival.lock().unwrap() = Some(Err(err.into()));
        app.update();
        assert_eq!(
            app.world().resource::<Seen>().applied,
            vec![serde_json::json!({"id":"night-shift","ok":false,"reason":reason})]
        );
    }
}

#[test]
fn a_pack_the_validator_refuses_says_invalid() {
    let mut app = app(Selection::Default);
    let (mut files, _) = night_shift();
    files.insert("stray.png".into(), vec![0, 1, 2]); // not a PNG
    let arrival = in_flight(&mut app, &content_hash(&files));
    *arrival.lock().unwrap() = Some(Ok(files));
    app.update();
    let seen = app.world().resource::<Seen>();
    let reason = seen.applied[0]["reason"].as_str().unwrap();
    assert!(reason.starts_with("invalid: "), "{reason}");
    assert_eq!(app.world().resource::<ActiveLook>().id(), "default");
}

#[test]
fn clear_returns_to_the_default_and_drops_a_fetch_in_flight() {
    let mut app = app(Selection::Embedded("night-shift".into()));
    app.update();
    assert_eq!(app.world().resource::<ActiveLook>().id(), "night-shift");
    let (files, hash) = night_shift();
    let arrival = in_flight(&mut app, &hash);
    send(&mut app, look::hosted::CLEAR, serde_json::Value::Null);
    app.update();
    assert_eq!(app.world().resource::<ActiveLook>().id(), "default");
    assert!(!app.world().contains_resource::<PendingFetch>());
    // The stale fetch lands after the clear and is never read.
    *arrival.lock().unwrap() = Some(Ok(files));
    app.update();
    assert_eq!(app.world().resource::<ActiveLook>().id(), "default");
    let seen = app.world().resource::<Seen>();
    assert_eq!(
        seen.applied,
        vec![serde_json::json!({"id":"default","ok":true})]
    );
}

#[test]
fn refused_commands_are_answered_with_the_reason() {
    let mut app = app(Selection::Default);
    send(
        &mut app,
        look::hosted::SET,
        serde_json::json!({
            "id": "fizzix",
            "base": "https://evil.example/looks/",
            "hash": "0".repeat(64),
            "files": ["look.json"],
        }),
    );
    app.update();
    assert_eq!(
        app.world().resource::<Seen>().applied,
        vec![serde_json::json!({"id":"fizzix","ok":false,"reason":"origin"})]
    );
}

#[test]
fn native_builds_answer_a_good_set_unsupported() {
    let mut app = app(Selection::Default);
    let (_, hash) = night_shift();
    send(
        &mut app,
        look::hosted::SET,
        serde_json::json!({
            "id": "night-shift",
            "base": format!("{}template/{hash}/", look::hosted::HOSTED_ORIGIN),
            "hash": hash,
            "files": ["look.json"],
        }),
    );
    app.update();
    assert_eq!(
        app.world().resource::<Seen>().applied,
        vec![serde_json::json!({"id":"night-shift","ok":false,"reason":"unsupported"})]
    );
    assert!(!app.world().contains_resource::<PendingFetch>());
}

#[test]
fn other_custom_commands_are_left_alone() {
    let mut app = app(Selection::Default);
    send(
        &mut app,
        "tunables.set",
        serde_json::json!({"ammo.start": 8}),
    );
    app.update();
    assert!(app.world().resource::<Seen>().applied.is_empty());
}
