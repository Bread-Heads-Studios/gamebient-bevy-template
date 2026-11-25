//! Plain-English checks as data: `content/checks.json`. See `model` for the
//! file and its checker, `run` for the interpreter.

pub mod model;
pub mod run;

use bevy::prelude::*;

use crate::game::audio::SfxEvent;
use crate::game::content::kit::Kinds;
use crate::game::rule_table::{COUNTERS, STAGE};
use model::{CounterRange, Vocab};

/// The names this game's checks may use.
pub fn vocab(world: &World) -> Vocab {
    let kinds = world.get_resource::<Kinds>();
    Vocab {
        counters: COUNTERS
            .iter()
            .map(|c| CounterRange {
                name: c.name.to_string(),
                min: c.min,
                max: c.max,
            })
            .collect(),
        placeable: kinds
            .map(|k| {
                k.0.iter()
                    .filter(|(n, p)| p.edit.placeable && n.as_str() != STAGE.player)
                    .map(|(n, _)| n.clone())
                    .collect()
            })
            .unwrap_or_default(),
        kinds: kinds
            .map(|k| k.0.iter().map(|(n, _)| n.clone()).collect())
            .unwrap_or_default(),
        sounds: SfxEvent::ALL.iter().map(|e| e.name()).collect(),
    }
}
