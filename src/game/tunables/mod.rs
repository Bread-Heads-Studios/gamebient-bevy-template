//! Tunables: gameplay numbers that live in `Reflect` resources, carry
//! labels and ranges as attributes, and can be changed by the editor at a
//! sim-tick boundary. See docs/superpowers/specs/2026-11-06-editor-design.md.

pub mod attr;
pub mod index;
pub mod value;

pub use attr::{Applies, Button, Group, GroupKind, Kind, Tune};
pub use index::{Entry, GroupEntry, TunableIndex, read, register, snapshot, write};
pub use value::Value;
