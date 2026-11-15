//! Editable game data: the files under `content/` that the editor can change.
//!
//! `files` embeds them at build time; `model` parses them with the same
//! limits the JSON schemas in `content/schema/` state. Not sim code: nothing
//! here is read during a run until a later task wires it in.

pub mod files;
pub mod model;
