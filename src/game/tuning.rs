//! The game's tunables: the numbers a card may change, as `Reflect`
//! resources with `Tune` attributes (see `tunables/attr.rs`). Register each
//! one in `GamePlugin` with `tunables::register::<T>(app)` in a fixed order;
//! that order is the index the replay and the checksum use, so never
//! reorder registrations between releases.
//!
//! TEMPLATE NOTE: replace `RunTuning` with the game's own resources (a
//! `PlayerTuning` with its speed, a `LivesTuning` with the starting count,
//! …), and make the systems that used to read constants read these.

use bevy::prelude::*;

use super::tunables::{Group, Tune};

/// One placeholder tunable so the template's own tests exercise the
/// pipeline. Nothing in the template sim reads it.
#[derive(Resource, Reflect, Clone, Debug, PartialEq)]
#[reflect(Resource, @Group::rule("run", "Run"))]
pub struct RunTuning {
    #[reflect(@Tune::int("Spare", 0, 10))]
    pub spare: u32,
}

impl Default for RunTuning {
    fn default() -> Self {
        Self { spare: 1 }
    }
}
