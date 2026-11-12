//! Look Packs: a game's look as data. A pack is a flat folder holding
//! `look.json` (schema v1, `manifest.rs`) plus `.png` / `.ogg` files. It
//! restyles palette roles, strings, slot recipes, images and audio cues
//! inside the limits the game's contract (`ui::theme::LOOK`) allows.
//!
//! **A pack never touches the sim.** `LookPlugin` is added in `main.rs`,
//! never in `GamePlugin`; sim systems never read `ActiveLook`. Replays,
//! checksums and leaderboards are identical under every pack
//! (`tests/looks.rs` proves it).
//!
//! This folder is copied verbatim into games. A game's own look surface is
//! `src/ui/theme.rs` (`LOOK`). Authoring guide: docs/look-packs.md.

pub mod apply;
pub mod contract;
pub mod manifest;
pub mod pack;
#[cfg(not(target_arch = "wasm32"))]
pub mod reload;
pub mod runtime;
pub mod select;
pub mod sniff;
pub mod validate;

use bevy::prelude::*;

pub use apply::{LookAssets, LookChanged, LookColour, LookText};
pub use contract::{Contract, ParamKind, ParamSpec, RecipeSpec, SlotSpec, StringSpec};
pub use manifest::{EndCard, ImageSlot, LookKind, Manifest, Param, RecipeSlot, Slot};
pub use runtime::ActiveLook;
pub use select::Selection;
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
        app.insert_resource(selection)
            .insert_resource(look)
            .init_resource::<LookAssets>()
            .add_message::<LookChanged>()
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
                    apply::retint_material,
                    apply::retext,
                )
                    .in_set(LookSet),
            );
        #[cfg(not(target_arch = "wasm32"))]
        app.add_systems(Update, reload::hot_reload);
    }
}
