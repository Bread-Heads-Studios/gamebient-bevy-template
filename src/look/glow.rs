//! [`LookGlow`]: a `StandardMaterial`'s emissive from a palette role, for a
//! glowing part whose emissive is **not** a multiple of its base colour (a
//! warm orange glow on a red bat, a cyan rim on a white cup).
//!
//! `LookColour::emissive(s)` writes `colour * s`, which can only reproduce
//! an emissive that is exactly a multiple of the base colour. `LookGlow`
//! complements it, it does not replace it: use `LookColour::new(role)`
//! (no `.emissive`) for the base colour and `LookGlow::new(role, emissive)`
//! for the glow, on the same entity or anchor. Never both `.emissive(s)`
//! and a `LookGlow` on one material: `retint_glow` runs after
//! `retint_material` and would win.
//!
//! - **Default look** (or a pack that leaves the role alone): the emissive
//!   is exactly the game's own `emissive`, bit for bit.
//! - **A pack's colour**: the pack colour (linear) times `k`, the game's
//!   own emissive-to-default brightness ratio (sum of linear RGB over sum of
//!   linear RGB), so the part keeps its glow strength and takes the pack's
//!   hue. Alpha stays the game's.
//! - Either way it is then dimmed by the pack's `glow` for the role
//!   (`ActiveLook::glow`, `0.0..=1.0`), never brightened.
//!
//! **Use the same role on both tags** (`LookColour::new("rail")` +
//! `LookGlow::new("rail", RAIL_EMISSIVE)`): one palette entry then recolours
//! body and glow together, and one `glow` entry dims it. List the role in
//! the contract's `glow`. A separate glow role is the exception, for a part
//! whose glow a pack should recolour on its own: the pack must then set and
//! dim that role, since setting the body's role leaves the glow in the
//! game's hue.
//!
//! The ratio is taken against the role's **contract default** (normally
//! the material's base colour). A role defaulting to black has no ratio;
//! the pack colour is then used at strength 1. The default is matched
//! exactly: a pack that writes the default as hex (`#e6331a` for
//! `srgb(0.9, 0.2, 0.1)`) is not bit-equal, so it takes the pack path
//! (the base's hue at the game's strength, not the authored glow hue).
//! Leave a role out of a pack to keep the game's own glow.
//!
//! Needs `bevy_pbr`: the whole module is compiled out under
//! `gx_look_no_pbr` (2D games have no `StandardMaterial`).

use bevy::prelude::*;

use super::apply::LookChanged;
use super::runtime::ActiveLook;

/// The emissive of the entity's `MeshMaterial3d<StandardMaterial>` follows
/// palette role `role`; `emissive` is the game's own (module docs).
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct LookGlow {
    pub role: &'static str,
    pub emissive: LinearRgba,
}

/// Below this, a default colour counts as black and gives no ratio.
const BLACK: f32 = 1e-4;

fn rgb_sum(c: LinearRgba) -> f32 {
    c.red + c.green + c.blue
}

impl LookGlow {
    pub const fn new(role: &'static str, emissive: LinearRgba) -> Self {
        Self { role, emissive }
    }

    /// The emissive under `look`.
    pub fn resolve(&self, look: &ActiveLook) -> LinearRgba {
        let colour = look.colour(self.role);
        let own = look.contract.default_colour(self.role);
        let e = match own {
            // The game's own colour (or a role the contract lacks, which
            // `colour` already warns about): the game's own emissive.
            Some(own) if own != colour => {
                let base = rgb_sum(LinearRgba::from(own));
                let k = if base < BLACK {
                    1.0
                } else {
                    rgb_sum(self.emissive) / base
                };
                let c = LinearRgba::from(colour);
                LinearRgba::new(c.red * k, c.green * k, c.blue * k, self.emissive.alpha)
            }
            _ => self.emissive,
        };
        let g = look.glow(self.role);
        if g >= 1.0 {
            e
        } else {
            LinearRgba::new(e.red * g, e.green * g, e.blue * g, e.alpha)
        }
    }
}

/// Writes every tagged material's emissive on the frame the tag spawns and
/// on every `LookChanged`. It mutates the material asset, so every entity
/// drawing that handle glows with it (the anchor pattern relies on this).
pub fn retint_glow(
    look: Res<ActiveLook>,
    mut changed: MessageReader<LookChanged>,
    materials: Option<ResMut<Assets<StandardMaterial>>>,
    q: Query<(Ref<LookGlow>, &MeshMaterial3d<StandardMaterial>)>,
) {
    let all = changed.read().count() > 0;
    let Some(mut materials) = materials else {
        return;
    };
    for (tag, handle) in &q {
        if !(all || tag.is_added()) {
            continue;
        }
        let want = tag.resolve(&look);
        // `get_mut` marks the asset changed (a GPU re-upload): look first.
        if materials.get(&handle.0).is_some_and(|m| m.emissive != want)
            && let Some(m) = materials.get_mut(&handle.0)
        {
            m.emissive = want;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::look::Contract;
    use crate::look::manifest::MANIFEST_FILE;
    use crate::look::select::Selection;
    use std::collections::BTreeMap;

    const BAT: Color = Color::srgb(0.9, 0.2, 0.1);
    /// Not a multiple of BAT: a warm glow with almost no blue.
    const BAT_GLOW: LinearRgba = LinearRgba::new(1.4, 0.6, 0.02, 1.0);

    static C: Contract = Contract {
        palette: &[("bat", BAT), ("ink", Color::BLACK)],
        strings: &[],
        slots: &[],
        audio: &[],
        fonts: &[],
        glow: &["bat", "ink"],
    };

    fn look(json: &str) -> ActiveLook {
        let files = BTreeMap::from([(MANIFEST_FILE.to_string(), json.as_bytes().to_vec())]);
        ActiveLook::from_files(&C, files, Selection::Default, None).expect("valid pack")
    }

    #[test]
    fn the_default_look_writes_the_games_own_emissive_exactly() {
        let g = LookGlow::new("bat", BAT_GLOW);
        assert_eq!(g.resolve(&ActiveLook::default_for(&C)), BAT_GLOW);
        // A pack that leaves the role alone, too.
        let other =
            look(r##"{"v":1,"id":"p","name":"P","kind":"palette","palette":{"ink":"#102030"}}"##);
        assert_eq!(g.resolve(&other), BAT_GLOW);
    }

    #[test]
    fn a_pack_colour_keeps_the_games_glow_strength() {
        let g = LookGlow::new("bat", BAT_GLOW);
        let teal =
            look(r##"{"v":1,"id":"p","name":"P","kind":"palette","palette":{"bat":"#00ffcc"}}"##);
        let got = g.resolve(&teal);
        let k = rgb_sum(BAT_GLOW) / rgb_sum(LinearRgba::from(BAT));
        let c = LinearRgba::from(teal.colour("bat"));
        assert!((got.red - c.red * k).abs() < 1e-6);
        assert!((got.green - c.green * k).abs() < 1e-6);
        assert!((got.blue - c.blue * k).abs() < 1e-6);
        assert_eq!(got.alpha, BAT_GLOW.alpha);
        // The pack's hue, not the game's: no red in a teal glow.
        assert_eq!(got.red, 0.0);
    }

    #[test]
    fn a_packs_glow_only_dims() {
        let g = LookGlow::new("bat", BAT_GLOW);
        let dim = look(r#"{"v":3,"id":"p","name":"P","kind":"palette","glow":{"bat":0.5}}"#);
        let got = g.resolve(&dim);
        assert_eq!(got.red, BAT_GLOW.red * 0.5);
        assert_eq!(got.green, BAT_GLOW.green * 0.5);
        assert_eq!(got.alpha, BAT_GLOW.alpha);
    }

    #[test]
    fn a_black_default_uses_the_pack_colour_at_strength_one() {
        let g = LookGlow::new("ink", LinearRgba::new(0.3, 0.1, 0.0, 1.0));
        let p =
            look(r##"{"v":1,"id":"p","name":"P","kind":"palette","palette":{"ink":"#ffffff"}}"##);
        let got = g.resolve(&p);
        assert!(got.red.is_finite() && (got.red - 1.0).abs() < 1e-6);
    }

    #[test]
    fn the_system_writes_on_spawn_and_restores_on_clear() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<LookChanged>()
            .init_resource::<Assets<StandardMaterial>>()
            .insert_resource(look(
                r##"{"v":1,"id":"p","name":"P","kind":"palette","palette":{"bat":"#00ffcc"}}"##,
            ))
            .add_systems(Update, retint_glow);
        let handle = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial {
                base_color: BAT,
                emissive: BAT_GLOW,
                ..default()
            });
        let tag = LookGlow::new("bat", BAT_GLOW);
        app.world_mut().spawn((MeshMaterial3d(handle.clone()), tag));
        app.update();
        let emissive = |app: &App| {
            app.world()
                .resource::<Assets<StandardMaterial>>()
                .get(&handle)
                .unwrap()
                .emissive
        };
        let want = tag.resolve(app.world().resource::<ActiveLook>());
        assert_eq!(emissive(&app), want);
        assert_ne!(want, BAT_GLOW);

        app.insert_resource(ActiveLook::default_for(&C));
        app.world_mut().write_message(LookChanged);
        app.update();
        assert_eq!(emissive(&app), BAT_GLOW);
    }
}
