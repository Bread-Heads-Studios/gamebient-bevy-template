//! `look.json`, schema v1. Pure data: parsing never touches Bevy, and
//! unknown fields are rejected so a typo fails instead of being ignored.

use std::collections::BTreeMap;

use serde::Deserialize;

pub const SCHEMA_VERSION: u32 = 1;
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

/// What fills a slot: an image (sprite games, logos) or a recipe the game
/// draws procedurally (most of the fleet: meshes plus materials).
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
pub enum Slot {
    Image(ImageSlot),
    Recipe(RecipeSlot),
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

pub fn parse(bytes: &[u8]) -> Result<Manifest, String> {
    serde_json::from_slice(bytes).map_err(|e| e.to_string())
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
    fn unknown_kind_is_rejected() {
        assert!(parse(br#"{"v":1,"id":"x","name":"X","kind":"shader"}"#).is_err());
    }
}
