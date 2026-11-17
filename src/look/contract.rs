//! What a game lets a pack change. Each game declares one `Contract` in
//! `src/ui/theme.rs` (`LOOK`); the validator rejects anything outside it,
//! so a typo in a hand-written pack fails loudly instead of doing nothing.

use bevy::color::Color;

#[derive(Debug)]
pub struct Contract {
    /// Palette roles with the game's own colour for each.
    pub palette: &'static [(&'static str, Color)],
    pub strings: &'static [StringSpec],
    pub slots: &'static [SlotSpec],
    /// Audio cue names a pack may override (`sfx.<event>`, `music.<state>`).
    pub audio: &'static [&'static str],
    /// Names of fonts the game bundles; packs pick from these only.
    pub fonts: &'static [&'static str],
}

#[derive(Debug)]
pub struct StringSpec {
    pub key: &'static str,
    pub default: &'static str,
    /// Longest text that fits where the game draws it.
    pub max_chars: usize,
}

#[derive(Debug)]
pub struct SlotSpec {
    pub name: &'static str,
    /// Whether an image may fill the slot.
    pub image: bool,
    /// Recipes the game knows how to draw for this slot.
    pub recipes: &'static [RecipeSpec],
    /// Budget for a shape (schema v2); `None` = the slot takes no shape.
    pub shape: Option<ShapeSpec>,
}

/// Limits a shape in one slot must stay inside.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShapeSpec {
    pub max_prims: usize,
    pub max_vertices: usize,
    /// Per-axis half-size `[x, y, z]` of the box the shape lives in, centred
    /// on the slot origin. Each `at` component is bounded by its own axis, each
    /// `size` component by the largest axis (sizes are not axis-aligned before
    /// rotation), and the built mesh by its own axis per vertex coordinate.
    /// A wide thin wall might use `[7.5, 0.5, 0.25]`; square slots use
    /// [`ShapeSpec::cube`].
    pub extent: [f32; 3],
}

impl ShapeSpec {
    /// A spec whose box is a cube of half-width `e`.
    pub const fn cube(max_prims: usize, max_vertices: usize, e: f32) -> Self {
        Self {
            max_prims,
            max_vertices,
            extent: [e; 3],
        }
    }
}

#[derive(Debug)]
pub struct RecipeSpec {
    pub name: &'static str,
    pub params: &'static [ParamSpec],
}

#[derive(Debug)]
pub struct ParamSpec {
    pub name: &'static str,
    pub kind: ParamKind,
}

#[derive(Debug)]
pub enum ParamKind {
    Number {
        min: f64,
        max: f64,
    },
    Bool,
    Text {
        max_chars: usize,
    },
    /// `#rrggbb` / `#rrggbbaa`, or the name of a palette role.
    Colour,
}

impl Contract {
    pub fn default_colour(&self, role: &str) -> Option<Color> {
        self.palette
            .iter()
            .find(|(r, _)| *r == role)
            .map(|(_, c)| *c)
    }
    pub fn string_spec(&self, key: &str) -> Option<&StringSpec> {
        self.strings.iter().find(|s| s.key == key)
    }
    pub fn default_string(&self, key: &str) -> Option<&'static str> {
        self.string_spec(key).map(|s| s.default)
    }
    pub fn slot(&self, name: &str) -> Option<&SlotSpec> {
        self.slots.iter().find(|s| s.name == name)
    }
}

impl SlotSpec {
    pub fn recipe(&self, name: &str) -> Option<&RecipeSpec> {
        self.recipes.iter().find(|r| r.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const C: Contract = Contract {
        palette: &[("ink", Color::srgb(0.0, 0.0, 0.0))],
        strings: &[StringSpec {
            key: "pause",
            default: "TAKE FIVE",
            max_chars: 12,
        }],
        slots: &[SlotSpec {
            name: "player",
            image: false,
            recipes: &[RecipeSpec {
                name: "cuboid",
                params: &[],
            }],
            shape: None,
        }],
        audio: &["sfx.confirm"],
        fonts: &["Inter SemiBold"],
    };

    #[test]
    fn lookups_find_declared_names_only() {
        assert_eq!(C.default_colour("ink"), Some(Color::srgb(0.0, 0.0, 0.0)));
        assert_eq!(C.default_colour("nope"), None);
        assert_eq!(C.default_string("pause"), Some("TAKE FIVE"));
        assert!(C.slot("player").and_then(|s| s.recipe("cuboid")).is_some());
        assert!(C.slot("player").and_then(|s| s.recipe("sphere")).is_none());
    }
}
