//! Tunables: gameplay numbers that live in `Reflect` resources, carry
//! labels and ranges as attributes, and can be changed by the editor at a
//! sim-tick boundary. See docs/superpowers/specs/2026-11-06-editor-design.md.
//!
//! - `attr.rs`: the `Tune` / `Group` attributes and the `Button` canon.
//! - `value.rs`: the closed `Value` set, bytes and JSON.
//! - `index.rs`: the ordered `TunableIndex`, reflective read and write.
//! - `apply.rs`: `PendingTunes` and the one system that drains it in
//!   `SimSet`; the `TuneLog` a replay carries.
//! - `hash.rs`: the content hash over the log.

pub mod apply;
pub mod attr;
pub mod hash;
pub mod index;
pub mod value;

use bevy::prelude::*;

pub use apply::{
    PendingTunes, TuneApplied, TuneChange, TuneLog, apply_pending_tunes, begin_run_tunables,
};
pub use attr::{Applies, Button, Group, GroupKind, Kind, Tune};
pub use index::{Entry, GroupEntry, TunableIndex, read, register, snapshot, write};
pub use value::Value;

use crate::game::states::GameState;

/// Registered by `GamePlugin` in both modes. Games register their tunable
/// resources *after* this plugin, in a fixed order, with [`register`].
pub struct TunablesPlugin;

impl Plugin for TunablesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TunableIndex>()
            .init_resource::<PendingTunes>()
            .init_resource::<TuneLog>()
            .add_message::<TuneApplied>()
            // Outside a run there is no tick to wait for: a change applies
            // at once and becomes a start value of the next run.
            .add_systems(
                Update,
                apply_pending_tunes.run_if(not(in_state(GameState::Playing))),
            );
    }
}
