//! Editable game data: the files under `content/` that the editor can change.
//!
//! `files` embeds them at build time; `model` parses them with the same
//! limits the JSON schemas in `content/schema/` state. `kit` holds the kinds
//! of Thing a level places and `level` the level itself, which is run
//! content (it is in the replay content hash). `sprites` swaps a kind's box
//! for a picture; it is presentation only. `audio` replaces songs and sound
//! effects by name; also presentation only. `edit` is the level editor's
//! Editing state: the level shown, frozen, under its own camera; presentation
//! only.

pub mod audio;
#[cfg(feature = "editor")]
pub mod base64;
pub mod edit;
pub mod files;
pub mod kit;
pub mod level;
pub mod model;
pub mod saved;
pub mod sprites;

use bevy::prelude::*;

use kit::Kinds;
use level::{Level, LevelHash, LevelSource};
use model::LevelFile;

/// Every content file the game is running: path under `content/` and the
/// FNV-1a 64 of its exact bytes. Seeded from the embedded files; the editor's
/// `content.set` updates it. Surfaced as the catalog's `files`.
#[derive(Resource, Default, Debug, Clone)]
pub struct LiveFiles(pub Vec<(String, u64)>);

impl LiveFiles {
    /// Records (or replaces) one file's hash, keeping the list sorted by path.
    pub fn set(&mut self, path: &str, hash: u64) {
        match self.0.binary_search_by(|(p, _)| p.as_str().cmp(path)) {
            Ok(i) => self.0[i].1 = hash,
            Err(i) => self.0.insert(i, (path.to_string(), hash)),
        }
    }
}

/// Loads the embedded level. Added by `GamePlugin` in both modes, after the
/// game's `thing_kind` registrations (the level's kinds are checked against
/// them).
pub struct ContentPlugin;

impl Plugin for ContentPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Kinds>()
            .init_resource::<kit::LevelGrid>()
            .insert_resource(LiveFiles(
                files::EMBEDDED
                    .iter()
                    .map(|(p, b)| ((*p).to_string(), files::fnv1a64(b)))
                    .collect(),
            ))
            .init_resource::<saved::ContentWarnings>()
            .insert_resource(LevelSource::Content)
            .insert_resource(Level(LevelFile {
                v: 1,
                things: Vec::new(),
            }))
            .insert_resource(LevelHash(0))
            .init_resource::<sprites::SpriteImages>()
            .init_resource::<sprites::SpriteMetaTable>()
            .add_systems(
                Startup,
                (kit::setup_thing_art, sprites::load_embedded_sprites).chain(),
            )
            // Songs and SFX files; a no-op without sound (headless).
            .add_systems(Startup, audio::load_embedded_audio)
            .add_systems(Update, sprites::animate_sprites);
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
