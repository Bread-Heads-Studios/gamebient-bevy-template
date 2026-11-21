//! Replacing the active look mid-run: the one step native hot reload
//! (`reload.rs`) and the web fetch of a large built-in pack share.
//!
//! Files in; optionally checked against the content hash the build
//! expected; validated against the game's contract; then `ActiveLook` is
//! replaced, `LookAssets` rebuilt and `LookChanged` sent, which every
//! applier (`apply.rs`, `shapes_apply.rs`, game code) already handles. Any
//! refusal leaves the current look, its assets and the message queue
//! untouched. Presentation only: the sim never reads any of this.
//!
//! Callers take `LookSwap` as an ordinary system parameter:
//!
//! ```ignore
//! fn arrive(mut swap: LookSwap, ...) {
//!     if let Err(report) = swap.apply(files, Selection::Fetch(id.clone()), Some(&id), Some(hash)) {
//!         log_report(&report);
//!     }
//! }
//! ```

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use super::apply::{LookAssets, LookChanged, build_assets};
use super::contract::Contract;
use super::runtime::ActiveLook;
use super::select::Selection;
use super::validate::{Files, Report, content_hash};

/// The resources a look swap writes. Image and audio stores are optional so
/// headless apps (tests, the replay verifier) can swap too.
#[derive(SystemParam)]
pub struct LookSwap<'w> {
    look: ResMut<'w, ActiveLook>,
    assets: ResMut<'w, LookAssets>,
    images: Option<ResMut<'w, Assets<Image>>>,
    audio: Option<ResMut<'w, Assets<AudioSource>>>,
    changed: MessageWriter<'w, LookChanged>,
}

impl LookSwap<'_> {
    /// Checks `files` (see `prepare`) and, if they pass, makes them the
    /// active look. On `Err` nothing changed; log the report.
    #[allow(clippy::result_large_err)] // Report is the full diagnosis
    pub fn apply(
        &mut self,
        files: Files,
        source: Selection,
        expected_id: Option<&str>,
        expected_hash: Option<&str>,
    ) -> Result<(), Report> {
        let next = prepare(
            self.look.contract,
            files,
            source,
            expected_id,
            expected_hash,
        )?;
        *self.look = next;
        *self.assets = build_assets(
            &self.look,
            self.images.as_deref_mut(),
            self.audio.as_deref_mut(),
        );
        self.changed.write(LookChanged);
        info!("look: now wearing {}", self.look.id());
        Ok(())
    }

    /// The look being worn: after a successful `apply`, the pack just
    /// validated, with its name and content hash.
    pub fn worn(&self) -> &ActiveLook {
        &self.look
    }

    /// Back to the game's own look (the editor's `look.clear`): no pack, no
    /// pack assets, and `LookChanged` so appliers put the defaults back.
    pub fn clear(&mut self) {
        let contract = self.look.contract;
        *self.look = ActiveLook::default_for(contract);
        *self.assets = LookAssets::default();
        self.changed.write(LookChanged);
        info!("look: now wearing default");
    }
}

/// The checks alone, touching nothing: the content hash first (when the
/// caller has one, so bytes the build did not ship are refused before any
/// parsing), then the validator, including `expected_id`.
#[allow(clippy::result_large_err)] // see LookSwap::apply
pub fn prepare(
    contract: &'static Contract,
    files: Files,
    source: Selection,
    expected_id: Option<&str>,
    expected_hash: Option<&str>,
) -> Result<ActiveLook, Report> {
    if let Some(expected) = expected_hash {
        let got = content_hash(&files);
        if got != expected {
            return Err(Report::read_error(format!(
                "content hash {got} does not match the build's {expected}"
            )));
        }
    }
    ActiveLook::from_files(contract, files, source, expected_id)
}
