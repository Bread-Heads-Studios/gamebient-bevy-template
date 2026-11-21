//! The editor's side of Look Packs (feature `editor`): the catalog's
//! `looks` key, and (`LookEditorPlugin`, below) the `look.set` /
//! `look.clear` host messages that dress the running game in a pack the
//! editor is writing, answered by `look`. Presentation only, like every
//! look: nothing here touches the sim. Guide: docs/look-packs.md ("The
//! editor").

use std::collections::BTreeMap;

use bevy::prelude::*;
use serde_json::{Value as Json, json};

use super::contract::Contract;
use super::end_card::ROLES;
use super::manifest;
use super::names::{MANIFEST_FILE, is_id};
use super::pack;
use super::runtime::ActiveLook;
use super::select::Selection;
use super::split::WEB_EMBED_MAX_BYTES;
use super::swap::LookSwap;
use super::validate::{Files, Report, limits};
use gamebient_input::{HostCommand, HostEvent};

/// Packs the editor wore with `look.set` this session: id to name. The
/// catalog lists them beside the built-in packs.
#[derive(Resource, Default, Debug, Clone, PartialEq)]
pub struct EditorLooks(pub BTreeMap<String, String>);

/// `#rrggbb` of a colour in sRGB, or `#rrggbbaa` when it is not opaque:
/// the forms `look.json` takes (`validate::parse_colour`).
pub fn hex(c: Color) -> String {
    let s = c.to_srgba();
    let ch = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    let rgb = format!("#{:02x}{:02x}{:02x}", ch(s.red), ch(s.green), ch(s.blue));
    match ch(s.alpha) {
        255 => rgb,
        a => format!("{rgb}{a:02x}"),
    }
}

/// The game's contract as data, every list in contract order: palette roles
/// with their default colours, string keys with defaults and the length the
/// validator allows, image slots with the validator's size cap, audio cues,
/// and whether the game shows end cards (it declares every end-card role).
pub fn contract_json(c: &Contract) -> Json {
    json!({
        "palette": c
            .palette
            .iter()
            .map(|(role, colour)| json!({ "role": role, "default": hex(*colour) }))
            .collect::<Vec<_>>(),
        "strings": c
            .strings
            .iter()
            .map(|s| json!({ "key": s.key, "default": s.default, "max": s.max_chars.min(limits::STRING_MAX) }))
            .collect::<Vec<_>>(),
        "images": c
            .slots
            .iter()
            .filter(|s| s.image)
            .map(|s| json!({ "slot": s.name, "max": [limits::PNG_DIM_MAX, limits::PNG_DIM_MAX] }))
            .collect::<Vec<_>>(),
        "audio": c.audio,
        "end_card": ROLES.iter().all(|r| c.default_colour(r).is_some()),
    })
}

/// The validator's caps (`validate::limits`) and the web-embed threshold.
pub fn limits_json() -> Json {
    json!({
        "file_bytes": {
            "look.json": limits::MANIFEST_MAX,
            "png": limits::PNG_MAX,
            "ogg": limits::OGG_MAX,
        },
        "files": limits::FILES_MAX,
        "pack_bytes": limits::PACK_MAX,
        "web_embed_bytes": WEB_EMBED_MAX_BYTES,
    })
}

/// The built-in packs (embedded, then on wasm the fetchable ones, named by
/// id since their files are not in the bundle) and the packs `look.set`
/// wore, as `[{id, name}]` sorted by id; a pack the editor wore wins.
pub fn packs_json(written: Option<&EditorLooks>) -> Json {
    let mut packs: BTreeMap<String, String> = BTreeMap::new();
    for id in pack::embedded_ids() {
        let name = pack::embedded_files(id)
            .and_then(|f| f.get(MANIFEST_FILE).and_then(|b| manifest::parse(b).ok()))
            .map_or_else(|| id.to_string(), |m| m.name);
        packs.insert(id.to_string(), name);
    }
    for id in pack::fetchable_ids() {
        packs
            .entry(id.to_string())
            .or_insert_with(|| id.to_string());
    }
    if let Some(w) = written {
        packs.extend(w.0.iter().map(|(k, v)| (k.clone(), v.clone())));
    }
    Json::Array(
        packs
            .into_iter()
            .map(|(id, name)| json!({ "id": id, "name": name }))
            .collect(),
    )
}

/// The catalog's `looks`: `None` when the game has no `LookPlugin` (no
/// `ActiveLook`), so the editor hides the Looks tab.
pub fn looks_catalog(world: &World) -> Option<Json> {
    let look = world.get_resource::<ActiveLook>()?;
    Some(json!({
        "contract": contract_json(look.contract),
        "packs": packs_json(world.get_resource::<EditorLooks>()),
        "limits": limits_json(),
    }))
}

/// `look.set` / `look.clear` from the editor, answered with `look`. Added by
/// `LookPlugin` in editor builds only.
pub struct LookEditorPlugin;

impl Plugin for LookEditorPlugin {
    fn build(&self, app: &mut App) {
        // The game's host layer registers both too; `add_message` is a no-op
        // the second time, so a look-only app (the tests) speaks the protocol.
        app.add_message::<HostCommand>()
            .add_message::<HostEvent>()
            .init_resource::<EditorLooks>()
            .add_systems(Update, handle_look_messages);
    }
}

/// A pack's files from `look.set`'s `files`: `look.json` as `{text}`, a
/// `.png` as `{png}` and an `.ogg` as `{ogg}` (standard base64). `Err` is
/// one `<name>: <why>` line per file that could not be read. The names are
/// the validator's to judge; only the extension picks the field.
pub fn decode_files(files: &Json) -> Result<Files, Vec<String>> {
    let Some(map) = files.as_object() else {
        return Err(vec!["pack: files must be an object of file names".into()]);
    };
    let mut out = Files::new();
    let mut errors = Vec::new();
    for (name, f) in map {
        let field = if name == MANIFEST_FILE {
            "text"
        } else if name.ends_with(".png") {
            "png"
        } else if name.ends_with(".ogg") {
            "ogg"
        } else {
            errors.push(format!(
                "{name}: a pack holds look.json, .png and .ogg files only"
            ));
            continue;
        };
        let Some(s) = f.get(field).and_then(Json::as_str) else {
            errors.push(format!("{name}: send it as \"{field}\""));
            continue;
        };
        let bytes = if field == "text" {
            Some(s.as_bytes().to_vec())
        } else {
            crate::game::content::base64::decode(s)
        };
        match bytes {
            Some(b) => {
                out.insert(name.clone(), b);
            }
            None => errors.push(format!("{name}: not valid base64")),
        }
    }
    if errors.is_empty() {
        Ok(out)
    } else {
        Err(errors)
    }
}

/// The validator's errors as `<where>: <what>` lines, in its order.
pub fn error_lines(report: &Report) -> Vec<String> {
    report
        .errors()
        .map(|p| format!("{}: {}", p.at, p.message))
        .collect()
}

/// A worn pack: its content hash and the validator's warnings, as
/// `<where>: <what>` lines.
pub struct Worn {
    pub hash: String,
    pub warnings: Vec<String>,
}

/// `look`'s data: the content hash and any warnings when the pack is worn,
/// else the errors. `warnings` is beside the spec's fields: a pack with
/// warnings is worn but is not one to build in (built-in packs ship clean),
/// so the editor saves it as a draft.
pub fn verdict(id: &str, result: &Result<Worn, Vec<String>>) -> Json {
    match result {
        Ok(w) => {
            json!({ "id": id, "ok": true, "errors": [], "hash": w.hash, "warnings": w.warnings })
        }
        Err(errors) => {
            json!({ "id": id, "ok": false, "errors": errors, "hash": null, "warnings": [] })
        }
    }
}

/// Validates the pack with the game's validator and, when it passes, wears
/// it (`LookSwap::apply`); its content hash and warnings on success. A
/// refused pack changes nothing.
fn try_set(
    swap: &mut LookSwap<'_>,
    written: &mut EditorLooks,
    id: &str,
    data: &Json,
) -> Result<Worn, Vec<String>> {
    if !is_id(id) {
        return Err(vec![format!(
            "id: lowercase letters, digits and -, at most {}",
            limits::ID_MAX
        )]);
    }
    let files = decode_files(data.get("files").unwrap_or(&Json::Null))?;
    // The validator sees the bytes first; the name and hash come from the
    // look it accepted and now wears.
    swap.apply(files, Selection::Editor(id.to_string()), Some(id), None)
        .map_err(|r| error_lines(&r))?;
    let worn = swap.worn();
    let hash = worn.content_hash.clone().unwrap_or_default();
    let warnings = worn
        .warnings
        .iter()
        .map(|p| format!("{}: {}", p.at, p.message))
        .collect();
    let name = worn.manifest.as_ref().map(|m| m.name.clone());
    written
        .0
        .insert(id.to_string(), name.unwrap_or_else(|| id.to_string()));
    Ok(Worn { hash, warnings })
}

fn handle_look_messages(
    mut commands: MessageReader<HostCommand>,
    mut swap: LookSwap,
    mut written: ResMut<EditorLooks>,
    mut out: MessageWriter<HostEvent>,
) {
    for command in commands.read() {
        let HostCommand::Custom { name, data } = command else {
            continue;
        };
        let reply = match name.as_str() {
            "look.set" => match serde_json::from_str::<Json>(data) {
                Ok(data) => {
                    let id = data
                        .get("id")
                        .and_then(Json::as_str)
                        .unwrap_or_default()
                        .to_string();
                    verdict(&id, &try_set(&mut swap, &mut written, &id, &data))
                }
                Err(_) => verdict("", &Err(vec!["pack: not JSON".into()])),
            },
            "look.clear" => {
                swap.reset();
                json!({ "id": "default", "ok": true, "errors": [], "hash": null, "warnings": [] })
            }
            _ => continue,
        };
        out.write(HostEvent::Custom {
            name: "look".into(),
            data: reply.to_string(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::look::validate::{content_hash, parse_colour};
    use crate::ui::theme::LOOK;

    #[test]
    fn hex_is_rgb_when_opaque_and_rgba_otherwise() {
        assert_eq!(hex(Color::srgb(1.0, 0.85, 0.2)), "#ffd933");
        assert_eq!(hex(Color::srgba(0.0, 0.0, 0.0, 0.0)), "#00000000");
        assert_eq!(hex(Color::srgba(1.0, 1.0, 1.0, 0.5)), "#ffffff80");
    }

    #[test]
    fn every_default_reads_back_as_a_pack_colour() {
        for (role, c) in LOOK.palette {
            let back = parse_colour(&hex(*c)).unwrap_or_else(|| panic!("{role}"));
            let (a, b) = (back.to_srgba(), c.to_srgba());
            for (x, y) in [
                (a.red, b.red),
                (a.green, b.green),
                (a.blue, b.blue),
                (a.alpha, b.alpha),
            ] {
                assert!((x - y).abs() <= 0.5 / 255.0 + 1e-6, "{role}: {x} vs {y}");
            }
        }
    }

    #[test]
    fn the_contract_reads_out_in_contract_order() {
        let j = contract_json(&LOOK);
        let palette = j["palette"].as_array().unwrap();
        assert_eq!(palette.len(), LOOK.palette.len());
        for (row, (role, c)) in palette.iter().zip(LOOK.palette) {
            assert_eq!(row, &json!({ "role": role, "default": hex(*c) }));
        }
        let strings = j["strings"].as_array().unwrap();
        assert_eq!(strings.len(), LOOK.strings.len());
        for (row, s) in strings.iter().zip(LOOK.strings) {
            assert_eq!(
                row,
                &json!({ "key": s.key, "default": s.default, "max": s.max_chars.min(limits::STRING_MAX) })
            );
        }
        let images: Vec<&str> = j["images"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["slot"].as_str().unwrap())
            .collect();
        let want: Vec<&str> = LOOK
            .slots
            .iter()
            .filter(|s| s.image)
            .map(|s| s.name)
            .collect();
        assert_eq!(images, want);
        for row in j["images"].as_array().unwrap() {
            assert_eq!(
                row["max"],
                json!([limits::PNG_DIM_MAX, limits::PNG_DIM_MAX])
            );
        }
        assert_eq!(j["audio"], json!(LOOK.audio));
        let end_card = ROLES.iter().all(|r| LOOK.default_colour(r).is_some());
        assert_eq!(j["end_card"], json!(end_card));
    }

    #[test]
    fn a_contract_without_the_end_card_roles_has_no_end_card() {
        static BARE: Contract = Contract {
            palette: &[("ink", Color::BLACK)],
            strings: &[],
            slots: &[],
            audio: &[],
            fonts: &[],
            glow: &[],
        };
        let j = contract_json(&BARE);
        assert_eq!(j["end_card"], json!(false));
        assert_eq!(j["images"], json!([]));
        assert_eq!(
            j["palette"],
            json!([{ "role": "ink", "default": "#000000" }])
        );
    }

    #[test]
    fn limits_are_the_validators_own() {
        assert_eq!(
            limits_json(),
            json!({
                "file_bytes": {
                    "look.json": limits::MANIFEST_MAX,
                    "png": limits::PNG_MAX,
                    "ogg": limits::OGG_MAX,
                },
                "files": limits::FILES_MAX,
                "pack_bytes": limits::PACK_MAX,
                "web_embed_bytes": WEB_EMBED_MAX_BYTES,
            })
        );
    }

    #[test]
    fn packs_list_every_built_in_pack_by_name_then_the_editors() {
        let j = packs_json(None);
        let ids: Vec<&str> = j
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["id"].as_str().unwrap())
            .collect();
        let mut want: Vec<&str> = pack::embedded_ids().collect();
        want.sort();
        assert_eq!(ids, want);
        for p in j.as_array().unwrap() {
            let files = pack::embedded_files(p["id"].as_str().unwrap()).unwrap();
            let m = manifest::parse(&files[MANIFEST_FILE]).unwrap();
            assert_eq!(p["name"], json!(m.name));
        }
        let mut written = EditorLooks::default();
        written.0.insert("zz-new".into(), "Brand New".into());
        let j = packs_json(Some(&written));
        assert_eq!(
            j.as_array().unwrap().last().unwrap(),
            &json!({ "id": "zz-new", "name": "Brand New" })
        );
    }

    #[test]
    fn looks_catalog_needs_a_look_plugin() {
        let mut world = World::new();
        assert!(looks_catalog(&world).is_none());
        world.insert_resource(ActiveLook::default_for(&LOOK));
        let j = looks_catalog(&world).unwrap();
        assert_eq!(j["contract"], contract_json(&LOOK));
        assert_eq!(j["limits"], limits_json());
        assert_eq!(j["packs"], packs_json(None));
    }

    use crate::look::{LookChanged, LookPlugin};
    use gamebient_input::{HostCommand, HostEvent};

    #[derive(Resource, Default)]
    struct Announced(usize);

    fn count(mut seen: MessageReader<LookChanged>, mut n: ResMut<Announced>) {
        n.0 += seen.read().count();
    }

    /// A look-only app: the plugin brings the host messages itself.
    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(Selection::Default);
        app.add_plugins(LookPlugin { contract: &LOOK });
        app.init_resource::<Announced>()
            .add_systems(PostUpdate, count);
        app.update();
        app
    }

    /// Sends one host message, runs a frame, returns the `look` answers.
    fn send(app: &mut App, name: &str, data: Json) -> Vec<Json> {
        app.world_mut().write_message(HostCommand::Custom {
            name: name.into(),
            data: data.to_string(),
        });
        app.update();
        app.world_mut()
            .resource_mut::<Messages<HostEvent>>()
            .drain()
            .filter_map(|e| match e {
                HostEvent::Custom { name, data } if name == "look" => {
                    Some(serde_json::from_str(&data).unwrap())
                }
                _ => None,
            })
            .collect()
    }

    /// A valid pack under any contract: its first palette role recoloured.
    fn pack(id: &str) -> Json {
        let mut palette = serde_json::Map::new();
        palette.insert(LOOK.palette[0].0.into(), json!("#123456"));
        let manifest = json!({ "v": 1, "id": id, "name": "Editor Test", "kind": "palette", "palette": palette });
        json!({ "id": id, "files": { "look.json": { "text": manifest.to_string() } } })
    }

    /// Standard base64, for sending built-in packs back.
    fn encode(bytes: &[u8]) -> String {
        const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut s = String::new();
        for c in bytes.chunks(3) {
            let n = (u32::from(c[0]) << 16)
                | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
                | u32::from(*c.get(2).unwrap_or(&0));
            for i in 0..4 {
                if i <= c.len() {
                    s.push(A[((n >> (18 - 6 * i)) & 63) as usize] as char);
                } else {
                    s.push('=');
                }
            }
        }
        s
    }

    #[test]
    fn files_decode_by_extension_and_name_what_failed() {
        let ok = decode_files(&json!({
            "look.json": { "text": "{}" },
            "a.png": { "png": "iVBORw==" },
            "b.ogg": { "ogg": "T2dnUw==" },
        }))
        .unwrap();
        assert_eq!(ok["look.json"], b"{}".to_vec());
        assert_eq!(ok["a.png"], vec![0x89, b'P', b'N', b'G']);
        assert_eq!(ok["b.ogg"], b"OggS".to_vec());
        let err = decode_files(&json!({
            "a.png": { "text": "x" },
            "c.ttf": { "text": "x" },
            "d.ogg": { "ogg": "%%%" },
        }))
        .unwrap_err();
        assert_eq!(
            err,
            vec![
                "a.png: send it as \"png\"".to_string(),
                "c.ttf: a pack holds look.json, .png and .ogg files only".to_string(),
                "d.ogg: not valid base64".to_string(),
            ]
        );
        assert!(decode_files(&Json::Null).is_err());
    }

    #[test]
    fn a_valid_pack_is_worn_and_answers_its_hash() {
        let mut app = app();
        let data = pack("editor-test");
        let files = decode_files(&data["files"]).unwrap();
        let before = app.world().resource::<Announced>().0;
        let r = send(&mut app, "look.set", data);
        assert_eq!(
            r,
            vec![
                json!({ "id": "editor-test", "ok": true, "errors": [], "hash": content_hash(&files), "warnings": [] })
            ]
        );
        let look = app.world().resource::<ActiveLook>();
        assert_eq!(look.id(), "editor-test");
        assert_eq!(look.source, Selection::Editor("editor-test".into()));
        assert_eq!(hex(look.colour(LOOK.palette[0].0)), "#123456");
        assert_eq!(
            app.world().resource::<Announced>().0,
            before + 1,
            "LookChanged fires so appliers restyle"
        );
        assert_eq!(
            app.world()
                .resource::<EditorLooks>()
                .0
                .get("editor-test")
                .map(String::as_str),
            Some("Editor Test")
        );
    }

    #[test]
    fn a_payload_that_is_not_json_is_refused_as_such() {
        let mut app = app();
        send(&mut app, "look.set", pack("good"));
        app.world_mut().write_message(HostCommand::Custom {
            name: "look.set".into(),
            data: "{not json".into(),
        });
        app.update();
        let r: Vec<Json> = app
            .world_mut()
            .resource_mut::<Messages<HostEvent>>()
            .drain()
            .filter_map(|e| match e {
                HostEvent::Custom { name, data } if name == "look" => {
                    Some(serde_json::from_str(&data).unwrap())
                }
                _ => None,
            })
            .collect();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0]["ok"], json!(false));
        assert_eq!(r[0]["errors"], json!(["pack: not JSON"]));
        assert_eq!(app.world().resource::<ActiveLook>().id(), "good");
    }

    #[test]
    fn a_pack_with_warnings_is_worn_and_names_them() {
        let mut app = app();
        let mut p = pack("spare");
        // A file look.json never names: a warning, not an error.
        p["files"]["spare.png"] =
            json!({ "png": encode(include_bytes!("../../tests/fixtures/sprite-a.png")) });
        let r = send(&mut app, "look.set", p);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0]["ok"], json!(true), "{r:?}");
        assert!(r[0]["hash"].is_string());
        assert_eq!(
            r[0]["warnings"],
            json!(["spare.png: not referenced by look.json"])
        );
        assert_eq!(app.world().resource::<ActiveLook>().id(), "spare");
        let clean = send(&mut app, "look.set", pack("clean"));
        assert_eq!(clean[0]["warnings"], json!([]));
    }

    #[test]
    fn an_invalid_pack_is_refused_and_the_last_good_look_stays() {
        let mut app = app();
        send(&mut app, "look.set", pack("good"));
        let mut bad = pack("bad");
        bad["files"]["look.json"]["text"] = json!(
            r##"{"v":1,"id":"bad","name":"Bad","kind":"palette","palette":{"no-such-role":"#000000"}}"##
        );
        let before = app.world().resource::<Announced>().0;
        let r = send(&mut app, "look.set", bad);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0]["ok"], json!(false));
        assert_eq!(r[0]["hash"], Json::Null);
        let errors = r[0]["errors"].as_array().unwrap();
        assert!(
            errors.iter().any(|e| e
                .as_str()
                .unwrap()
                .starts_with("palette.no-such-role: unknown role")),
            "{errors:?}"
        );
        assert_eq!(app.world().resource::<ActiveLook>().id(), "good");
        assert_eq!(
            app.world().resource::<Announced>().0,
            before,
            "nothing changed"
        );
        assert!(!app.world().resource::<EditorLooks>().0.contains_key("bad"));
    }

    #[test]
    fn the_id_must_be_a_folder_name_and_match_look_json() {
        let mut app = app();
        let r = send(&mut app, "look.set", json!({ "id": "../x", "files": {} }));
        assert_eq!(
            r[0]["errors"],
            json!([format!(
                "id: lowercase letters, digits and -, at most {}",
                limits::ID_MAX
            )])
        );
        let mut other = pack("one");
        other["id"] = json!("two");
        let r = send(&mut app, "look.set", other);
        assert!(
            r[0]["errors"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e == r#"id: "one" must match the folder name "two""#),
            "{r:?}"
        );
        assert_eq!(app.world().resource::<ActiveLook>().id(), "default");
    }

    #[test]
    fn look_clear_goes_back_to_the_games_own_look() {
        let mut app = app();
        send(&mut app, "look.set", pack("worn"));
        let before = app.world().resource::<Announced>().0;
        let r = send(&mut app, "look.clear", json!({}));
        assert_eq!(
            r,
            vec![
                json!({ "id": "default", "ok": true, "errors": [], "hash": null, "warnings": [] })
            ]
        );
        let look = app.world().resource::<ActiveLook>();
        assert_eq!(look.id(), "default");
        assert_eq!(look.colour(LOOK.palette[0].0), LOOK.palette[0].1);
        assert_eq!(app.world().resource::<Announced>().0, before + 1);
        let listed = packs_json(Some(app.world().resource::<EditorLooks>()));
        assert!(listed.as_array().unwrap().iter().any(|p| p["id"] == "worn"));
    }

    #[test]
    fn a_built_in_pack_sent_back_answers_its_build_time_hash() {
        let mut app = app();
        for id in pack::embedded_ids() {
            let files = pack::embedded_files(id).unwrap();
            let built = ActiveLook::resolve(&LOOK, Selection::Embedded(id.into()))
                .content_hash
                .unwrap();
            let wire: serde_json::Map<String, Json> = files
                .iter()
                .map(|(name, bytes)| {
                    let f = if name == MANIFEST_FILE {
                        json!({ "text": String::from_utf8(bytes.clone()).unwrap() })
                    } else if name.ends_with(".png") {
                        json!({ "png": encode(bytes) })
                    } else {
                        json!({ "ogg": encode(bytes) })
                    };
                    (name.clone(), f)
                })
                .collect();
            let r = send(&mut app, "look.set", json!({ "id": id, "files": wire }));
            assert_eq!(r[0]["hash"], json!(built), "{id}: {r:?}");
        }
    }
}
