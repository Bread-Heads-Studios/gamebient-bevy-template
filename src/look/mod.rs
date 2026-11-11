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

pub mod contract;
pub mod manifest;
pub mod pack;
pub mod select;
pub mod sniff;
pub mod validate;

pub use contract::{Contract, ParamKind, ParamSpec, RecipeSpec, SlotSpec, StringSpec};
pub use manifest::{EndCard, ImageSlot, LookKind, Manifest, Param, RecipeSlot, Slot};
pub use select::Selection;
pub use validate::{Files, Problem, Report, Severity};
