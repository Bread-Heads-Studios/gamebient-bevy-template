//! `look.json`, schema v2 (v1 packs still load). Pure data: parsing never touches Bevy, and
//! unknown fields are rejected so a typo fails instead of being ignored.

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, de::Error as _};

pub const SCHEMA_VERSION: u32 = 3;
pub const MANIFEST_FILE: &str = "look.json";

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// Schema version; this build reads [`SCHEMA_VERSION`].
    pub v: u32,
    /// `[a-z0-9-]{1,32}`; equals the pack's folder name.
    pub id: String,
    pub name: String,
    pub kind: LookKind,
    #[serde(default)]
    pub creator: Option<String>,
    /// Palette role -> `#rrggbb` or `#rrggbbaa`.
    #[serde(default)]
    pub palette: BTreeMap<String, String>,
    /// Palette role -> glow, 0.0 (dark) to 1.0 (the game's own). Schema
    /// v3; only roles the game lists in `Contract::glow`. Dims, never
    /// brightens.
    #[serde(default)]
    pub glow: BTreeMap<String, f32>,
    /// String key -> printable ASCII text.
    #[serde(default)]
    pub strings: BTreeMap<String, String>,
    #[serde(default)]
    pub slots: BTreeMap<String, Slot>,
    /// Audio cue (`sfx.<event>`, `music.<state>`) -> `.ogg` file.
    #[serde(default)]
    pub audio: BTreeMap<String, String>,
    /// `display` / `body` -> a face the game bundles. Consumed in a later step.
    #[serde(default)]
    pub fonts: BTreeMap<String, String>,
    /// `marquee` / `bezel` -> `.png`. Consumed in a later step.
    #[serde(default)]
    pub frame: BTreeMap<String, String>,
    /// Sponsor end card. Consumed in a later step.
    #[serde(default)]
    pub end_card: Option<EndCard>,
    /// Reserved for a bring-your-own protagonist (D4): accepted, ignored.
    #[serde(default)]
    pub protagonist: Option<serde_json::Value>,
}

#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LookKind {
    Palette,
    Skin,
    Sponsor,
    Genre,
}

/// What fills a slot: an image (sprite games, logos), a recipe the game
/// draws procedurally (most of the fleet: meshes plus materials), or, from
/// schema v2, a shape: a list of primitives merged into one mesh.
///
/// Deserialized by hand, not `untagged`: the slot object must carry exactly
/// one of `image`, `recipe` or `shape`, and that struct's own error (an
/// unknown field, a bad primitive and its index) is what the creator sees.
#[derive(Debug, Clone, PartialEq)]
pub enum Slot {
    Image(ImageSlot),
    Recipe(RecipeSlot),
    Shape(ShapeSlot),
}

const SLOT_KINDS: [&str; 3] = ["image", "recipe", "shape"];

impl<'de> Deserialize<'de> for Slot {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let map = serde_json::Map::<String, serde_json::Value>::deserialize(d)?;
        let kinds: Vec<&str> = SLOT_KINDS
            .into_iter()
            .filter(|k| map.contains_key(*k))
            .collect();
        let value = serde_json::Value::Object(map);
        let fail = |kind: &str, e: serde_json::Error| D::Error::custom(format!("{kind} slot: {e}"));
        match kinds.as_slice() {
            ["image"] => ImageSlot::deserialize(value)
                .map(Slot::Image)
                .map_err(|e| fail("image", e)),
            ["recipe"] => RecipeSlot::deserialize(value)
                .map(Slot::Recipe)
                .map_err(|e| fail("recipe", e)),
            ["shape"] => ShapeSlot::deserialize(value)
                .map(Slot::Shape)
                .map_err(|e| fail("shape", e)),
            [] => Err(D::Error::custom(
                "a slot needs one of `image`, `recipe` or `shape`",
            )),
            several => Err(D::Error::custom(format!(
                "a slot takes exactly one of `image`, `recipe` or `shape`, not {}",
                several
                    .iter()
                    .map(|k| format!("`{k}`"))
                    .collect::<Vec<_>>()
                    .join(" and ")
            ))),
        }
    }
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ImageSlot {
    pub image: String,
    /// Horizontal strip frame count; 1 = a still image.
    #[serde(default = "one")]
    pub frames: u32,
    #[serde(default)]
    pub fps: f32,
}

fn one() -> u32 {
    1
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RecipeSlot {
    /// A recipe name the game's contract lists for this slot.
    pub recipe: String,
    #[serde(default)]
    pub params: BTreeMap<String, Param>,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ShapeSlot {
    #[serde(deserialize_with = "primitives")]
    pub shape: Vec<Primitive>,
}

/// Each primitive on its own, so an error names its index: `shape[2]: ...`.
fn primitives<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Primitive>, D::Error> {
    Vec::<serde_json::Value>::deserialize(d)?
        .into_iter()
        .enumerate()
        .map(|(i, v)| {
            Primitive::deserialize(v).map_err(|e| D::Error::custom(format!("shape[{i}]: {e}")))
        })
        .collect()
}

/// One primitive of a shape. `size` arity depends on `prim`; the validator
/// checks it. `rot` is Euler degrees, XYZ.
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Primitive {
    pub prim: Prim,
    pub size: Vec<f32>,
    #[serde(default)]
    pub at: Option<[f32; 3]>,
    #[serde(default)]
    pub rot: Option<[f32; 3]>,
    #[serde(default)]
    pub scale: Option<[f32; 3]>,
    #[serde(default)]
    pub detail: Option<u32>,
}

#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Prim {
    Sphere,
    Cuboid,
    Cylinder,
    Cone,
    Torus,
    Capsule,
    Quad,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
pub enum Param {
    Bool(bool),
    Number(f64),
    Text(String),
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EndCard {
    pub headline: String,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub image: Option<String>,
}

/// Strict parse: unknown fields anywhere are an error. Gate on
/// [`version`] first, or a newer pack fails here with a serde error
/// instead of saying it is newer.
pub fn parse(bytes: &[u8]) -> Result<Manifest, String> {
    serde_json::from_slice(bytes).map_err(|e| e.to_string())
}

/// The bare `v`, read leniently (every other field ignored), so a pack from
/// a newer schema, full of fields and primitives this build has never
/// heard of, can still be told apart. `None` when `look.json` is not JSON
/// or has no integer `v`; the strict parse reports that.
pub fn version(bytes: &[u8]) -> Option<u32> {
    #[derive(Deserialize)]
    struct Head {
        v: u32,
    }
    serde_json::from_slice::<Head>(bytes).ok().map(|h| h.v)
}

/// The schema gate: `Err` for a version this build cannot read.
pub fn check_version(v: u32) -> Result<(), String> {
    if v == 0 {
        Err("unknown schema version (0)".into())
    } else if v > SCHEMA_VERSION {
        Err(format!("made for a newer version of this game (v{v})"))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_minimal_manifest_parses() {
        let m = parse(br#"{"v":1,"id":"x","name":"X","kind":"palette"}"#).unwrap();
        assert_eq!(m.v, 1);
        assert_eq!(m.kind, LookKind::Palette);
        assert!(m.palette.is_empty() && m.slots.is_empty() && m.protagonist.is_none());
    }

    #[test]
    fn slots_are_images_or_recipes() {
        let m = parse(
            br##"{"v":1,"id":"x","name":"X","kind":"skin","slots":{
                "a":{"image":"a.png","frames":4,"fps":8},
                "b":{"recipe":"cuboid","params":{"size":1.5,"glow":true,"tint":"#ff0000"}}
            }}"##,
        )
        .unwrap();
        assert_eq!(
            m.slots["a"],
            Slot::Image(ImageSlot {
                image: "a.png".into(),
                frames: 4,
                fps: 8.0
            })
        );
        let Slot::Recipe(r) = &m.slots["b"] else {
            panic!("b is a recipe")
        };
        assert_eq!(r.recipe, "cuboid");
        assert_eq!(r.params["size"], Param::Number(1.5));
        assert_eq!(r.params["glow"], Param::Bool(true));
        assert_eq!(r.params["tint"], Param::Text("#ff0000".into()));
    }

    #[test]
    fn image_slot_defaults_to_one_still_frame() {
        let m =
            parse(br#"{"v":1,"id":"x","name":"X","kind":"skin","slots":{"a":{"image":"a.png"}}}"#)
                .unwrap();
        assert_eq!(
            m.slots["a"],
            Slot::Image(ImageSlot {
                image: "a.png".into(),
                frames: 1,
                fps: 0.0
            })
        );
    }

    #[test]
    fn unknown_fields_are_rejected() {
        assert!(parse(br#"{"v":1,"id":"x","name":"X","kind":"palette","colours":{}}"#).is_err());
        assert!(parse(br#"{"v":1,"id":"x","name":"X","kind":"palette","slots":{"a":{"image":"a.png","zoom":2}}}"#).is_err());
    }

    #[test]
    fn protagonist_is_accepted_as_opaque_json() {
        let m =
            parse(br#"{"v":1,"id":"x","name":"X","kind":"skin","protagonist":{"avatar":"later"}}"#)
                .unwrap();
        assert!(m.protagonist.is_some());
    }

    #[test]
    fn slots_resolve_unambiguously_to_image_recipe_or_shape() {
        let m = parse(
            br#"{"v":2,"id":"x","name":"X","kind":"skin","slots":{
                "a":{"image":"a.png"},
                "b":{"recipe":"cuboid"},
                "c":{"shape":[{"prim":"capsule","size":[0.1,0.4],"at":[0,1,0],"rot":[0,90,0],"scale":[1,2,1],"detail":6},{"prim":"quad","size":[1,1]}]},
                "d":{"shape":[]}
            }}"#,
        )
        .unwrap();
        assert!(matches!(m.slots["a"], Slot::Image(_)));
        assert!(matches!(m.slots["b"], Slot::Recipe(_)));
        let Slot::Shape(c) = &m.slots["c"] else {
            panic!("c is a shape")
        };
        assert_eq!(c.shape[0].prim, Prim::Capsule);
        assert_eq!(c.shape[0].size, vec![0.1, 0.4]);
        assert_eq!(c.shape[0].at, Some([0.0, 1.0, 0.0]));
        assert_eq!(c.shape[0].detail, Some(6));
        assert_eq!(c.shape[1].prim, Prim::Quad);
        assert_eq!(c.shape[1].at, None);
        assert!(matches!(&m.slots["d"], Slot::Shape(s) if s.shape.is_empty()));
    }

    #[test]
    fn shape_structs_reject_unknown_fields_and_kinds() {
        let slot = |body: &str| {
            format!(r#"{{"v":2,"id":"x","name":"X","kind":"skin","slots":{{"a":{body}}}}}"#)
        };
        let err = |body: &str| parse(slot(body).as_bytes()).unwrap_err();
        let e =
            err(r#"{"shape":[{"prim":"sphere","size":[1]},{"prim":"sphere","size":[1],"hue":1}]}"#);
        assert!(
            e.contains("shape[1]") && e.contains("unknown field `hue`"),
            "{e}"
        );
        let e = err(r#"{"shape":[{"prim":"cube","size":[1]}]}"#);
        assert!(
            e.contains("shape[0]") && e.contains("unknown variant `cube`"),
            "{e}"
        );
        let e = err(r#"{"shape":[],"image":"a.png"}"#);
        assert!(
            e.contains("exactly one of") && e.contains("`image` and `shape`"),
            "{e}"
        );
        let e = err(r#"{"shape":[],"zoom":2}"#);
        assert!(
            e.contains("shape slot") && e.contains("unknown field `zoom`"),
            "{e}"
        );
        let e = err(r#"{"frames":2}"#);
        assert!(
            e.contains("needs one of `image`, `recipe` or `shape`"),
            "{e}"
        );
    }

    #[test]
    fn image_and_recipe_errors_name_the_field() {
        let e = parse(br#"{"v":1,"id":"x","name":"X","kind":"skin","slots":{"a":{"image":"a.png","zoom":2}}}"#).unwrap_err();
        assert!(
            e.contains("image slot") && e.contains("unknown field `zoom`"),
            "{e}"
        );
        let e = parse(
            br#"{"v":1,"id":"x","name":"X","kind":"skin","slots":{"a":{"recipe":"c","parms":{}}}}"#,
        )
        .unwrap_err();
        assert!(
            e.contains("recipe slot") && e.contains("unknown field `parms`"),
            "{e}"
        );
    }

    #[test]
    fn version_reads_v_leniently() {
        assert_eq!(
            version(br#"{"v":4,"glitter":true,"slots":{"a":{"shape":[{"prim":"blob"}]}}}"#),
            Some(4)
        );
        assert_eq!(version(br#"{"id":"x"}"#), None);
        assert_eq!(version(b"not json"), None);
        assert!(check_version(1).is_ok() && check_version(SCHEMA_VERSION).is_ok());
        assert_eq!(check_version(0).unwrap_err(), "unknown schema version (0)");
        assert_eq!(
            check_version(SCHEMA_VERSION + 1).unwrap_err(),
            format!(
                "made for a newer version of this game (v{})",
                SCHEMA_VERSION + 1
            )
        );
    }

    #[test]
    fn unknown_kind_is_rejected() {
        assert!(parse(br#"{"v":1,"id":"x","name":"X","kind":"shader"}"#).is_err());
    }
}
