//! Look Packs: a game's look as data. A pack is a flat folder holding
//! `look.json` (schema v2, `manifest.rs`) plus `.png` / `.ogg` files. It
//! restyles palette roles, strings, slot recipes, shapes, images and audio cues
//! inside the limits the game's contract (`ui::theme::LOOK`) allows.
//!
//! **A pack never touches the sim.** `LookPlugin` is added in `main.rs`,
//! never in `GamePlugin`; sim systems never read `ActiveLook`. Replays,
//! checksums and leaderboards are identical under every pack
//! (`tests/looks.rs` proves it).
//!
//! This folder is copied verbatim into games. A game's own look surface is
//! `src/ui/theme.rs` (`LOOK`). Authoring guide: docs/look-packs.md.
//! `embed.rs` is the build-time generator for built-in packs: `build.rs`
//! includes it by path; it is not a module of the crate.

pub mod apply;
pub mod contract;
#[cfg(feature = "editor")]
pub mod editor;
pub mod end_card;
pub mod fetch;
// Needs bevy_pbr (StandardMaterial); see `apply.rs`.
#[cfg(not(gx_look_no_pbr))]
pub mod glow;
pub mod hash;
pub mod hosted;
pub mod manifest;
pub mod names;
pub mod node;
pub mod pack;
pub mod refit;
#[cfg(not(target_arch = "wasm32"))]
pub mod reload;
pub mod runtime;
pub mod scene;
pub mod select;
pub mod shape;
pub mod shapes_apply;
pub mod sniff;
pub mod split;
pub mod swap;
pub mod validate;

use bevy::prelude::*;

pub use apply::{LookAssets, LookChanged, LookColour, LookText};
pub use contract::{Contract, ParamKind, ParamSpec, RecipeSpec, ShapeSpec, SlotSpec, StringSpec};
pub use end_card::{EndCardPart, EndCardSlot, EndCardStyle, EndCardStyleMissing, Part};
#[cfg(not(gx_look_no_pbr))]
pub use glow::LookGlow;
pub use hosted::HostedLook;
pub use manifest::{
    EndCard, ImageSlot, LookKind, Manifest, Param, Prim, Primitive, RecipeSlot, ShapeSlot, Slot,
};
pub use node::{LookBorder, LookGradient, LookImage, LookShadow, LookTint};
pub use refit::LookFit;
pub use runtime::ActiveLook;
pub use scene::LookClearColour;
#[cfg(not(gx_look_no_pbr))]
pub use scene::{LookAmbient, LookLight};
pub use select::Selection;
pub use shape::{DEFAULT_DETAIL, DETAIL_RANGE, build_mesh, primitive_transform, vertex_count};
pub use shapes_apply::ShapeTargets;
pub use validate::{Files, Problem, Report, Severity};

/// Retint and retext, in `PostUpdate` so a tag spawned this frame is
/// coloured before the frame renders.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct LookSet;

/// Picks the pack (a `Selection` resource inserted before this plugin
/// wins, else the launch: `--look`, `GX_LOOK`, `?look=`), validates it
/// and keeps it as `ActiveLook`. Add in `main.rs` only, never in
/// `GamePlugin`: the sim must not see it.
pub struct LookPlugin {
    pub contract: &'static Contract,
}

impl Plugin for LookPlugin {
    fn build(&self, app: &mut App) {
        let selection = app
            .world()
            .get_resource::<Selection>()
            .cloned()
            .unwrap_or_else(select::from_launch);
        let look = ActiveLook::resolve(self.contract, selection.clone());
        info!("look: {}", look.id());
        app.insert_resource(selection.clone())
            .insert_resource(look)
            .init_resource::<LookAssets>()
            .init_resource::<ShapeTargets>()
            .add_message::<LookChanged>()
            // Host messages: registered by GxInputPlugin in a real game;
            // registering again is a no-op, and headless tests need them.
            .add_message::<gamebient_input::HostCommand>()
            .add_message::<gamebient_input::HostEvent>()
            .add_systems(
                Update,
                (
                    hosted::handle_commands,
                    fetch::arrive.run_if(resource_exists::<fetch::PendingFetch>),
                )
                    .chain(),
            )
            .add_systems(Startup, apply::load_assets)
            // Retexting must land before text layout (UI content measure and
            // Text2d layout, both in PostUpdate), or the frame a string
            // changes on still renders the game's own text.
            .configure_sets(
                PostUpdate,
                LookSet
                    .before(bevy::ui::UiSystems::Content)
                    .before(bevy::text::Text2dUpdateSystems),
            )
            .add_systems(
                PostUpdate,
                (
                    apply::retint_text,
                    apply::retint_background,
                    apply::retint_sprite,
                    apply::retint_color_material,
                    // Needs bevy_pbr; a 2D game opts out with the
                    // `gx_look_no_pbr` cfg (see `apply.rs`).
                    #[cfg(not(gx_look_no_pbr))]
                    apply::retint_material,
                    apply::retext,
                    node::retint_border,
                    node::retint_gradient,
                    node::retint_image,
                    node::retint_text_shadow,
                    node::retint_box_shadow,
                    refit::refit_text,
                )
                    .in_set(LookSet),
            )
            // Before asset events, so a reshaped mesh's `Aabb` is recomputed
            // on the frame it changes.
            .add_systems(
                PostUpdate,
                end_card::fill_slots
                    .before(LookSet)
                    .run_if(resource_exists::<end_card::EndCardStyle>),
            )
            .add_systems(
                PostUpdate,
                end_card::warn_missing_style.run_if(
                    not(resource_exists::<end_card::EndCardStyle>)
                        .and(not(resource_exists::<end_card::EndCardStyleMissing>)),
                ),
            )
            .add_systems(
                PostUpdate,
                shapes_apply::apply_shapes
                    .in_set(LookSet)
                    .before(bevy::asset::AssetEventSystems),
            );
        // The fit guarantee: a pack string's line, as laid out, fits.
        app.add_systems(
            PostUpdate,
            refit::clamp_fit.after(bevy::ui::UiSystems::PostLayout),
        );
        // ClearColor follows a role once the game inserts `LookClearColour`.
        app.add_systems(
            PostUpdate,
            scene::sync_clear_colour
                .in_set(LookSet)
                .run_if(resource_exists::<scene::LookClearColour>),
        );
        // Light colours (`LookLight`, `LookAmbient`); needs bevy_light.
        #[cfg(not(gx_look_no_pbr))]
        app.add_systems(
            PostUpdate,
            (
                scene::retint_lights,
                scene::sync_ambient.run_if(resource_exists::<scene::LookAmbient>),
            )
                .in_set(LookSet),
        );
        // After `retint_material`, so a `LookGlow` emissive wins (glow.rs).
        #[cfg(not(gx_look_no_pbr))]
        app.add_systems(
            PostUpdate,
            glow::retint_glow
                .in_set(LookSet)
                .after(apply::retint_material),
        );
        #[cfg(not(target_arch = "wasm32"))]
        app.add_systems(Update, reload::hot_reload);
        // The editor's look.preview / look.preview.clear (src/look/editor.rs).
        #[cfg(feature = "editor")]
        app.add_plugins(editor::LookEditorPlugin);
        // A large built-in pack (`Selection::Fetch`): boot on the default
        // look, fetch its files, swap them in when they arrive.
        #[cfg(target_arch = "wasm32")]
        if let Selection::Fetch(id) = &selection
            && let Some(pending) = fetch::start(id)
        {
            app.insert_resource(pending);
        }
    }
}
