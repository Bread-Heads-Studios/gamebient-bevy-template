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
use super::names::MANIFEST_FILE;
use super::pack;
use super::runtime::ActiveLook;
use super::split::WEB_EMBED_MAX_BYTES;
use super::validate::limits;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::look::validate::parse_colour;
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
}
