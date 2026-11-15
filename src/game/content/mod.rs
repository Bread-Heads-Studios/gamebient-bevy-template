//! Editable game data: the files under `content/` that the editor can change.
//!
//! `files` embeds them at build time; `model` parses them with the same
//! limits the JSON schemas in `content/schema/` state. `kit` holds the kinds
//! of Thing a level places and `level` the level itself, which is run
//! content (it is in the replay content hash).

pub mod files;
pub mod kit;
pub mod level;
pub mod model;
pub mod saved;

use bevy::prelude::*;

use kit::Kinds;
use level::{Level, LevelHash, LevelSource};
use model::LevelFile;

/// Loads the embedded level. Added by `GamePlugin` in both modes, after the
/// game's `thing_kind` registrations (the level's kinds are checked against
/// them).
pub struct ContentPlugin;

impl Plugin for ContentPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Kinds>()
            .init_resource::<saved::ContentWarnings>()
            .insert_resource(LevelSource::Content)
            .insert_resource(Level(LevelFile {
                v: 1,
                things: Vec::new(),
            }))
            .insert_resource(LevelHash(0))
            .add_systems(Startup, kit::setup_thing_art);
        // Saved tunables are start values: applied here, at plugin build,
        // so they are in place before the first `Update` and before any
        // state transition can enter `Playing` (a `Startup` system is not
        // early enough: the first `StateTransition` runs before it). The
        // tunables are registered by now. Windowed and headless alike.
        saved::apply_embedded_saved(app.world_mut());
        if let Some(bytes) = files::embedded("level.json")
            && let Err(e) = level::set_level(app.world_mut(), bytes)
        {
            // A committed level that does not load is a build bug: the
            // level tests fail on it. At runtime the game starts empty.
            error!("content/level.json: {e}");
        }
    }
}
