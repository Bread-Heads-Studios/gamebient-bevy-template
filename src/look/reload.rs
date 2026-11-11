//! Native hot reload: with `--look <dir>` / `GX_LOOK=<dir>`, the folder is
//! polled every half second; a change that validates replaces the look and
//! sends `LookChanged`, a change that does not is logged and ignored.

use std::path::PathBuf;

use bevy::prelude::*;

use super::apply::{LookAssets, LookChanged, build_assets};
use super::pack::dir_fingerprint;
use super::runtime::{ActiveLook, log_report};
use super::select::Selection;

pub fn hot_reload(
    time: Res<Time>,
    mut timer: Local<Option<Timer>>,
    mut last: Local<Option<u64>>,
    mut look: ResMut<ActiveLook>,
    mut assets: ResMut<LookAssets>,
    mut images: Option<ResMut<Assets<Image>>>,
    mut audio: Option<ResMut<Assets<AudioSource>>>,
    mut changed: MessageWriter<LookChanged>,
    selection: Res<Selection>,
) {
    let Selection::Dir(path) = selection.as_ref() else {
        return;
    };
    let timer = timer.get_or_insert_with(|| Timer::from_seconds(0.5, TimerMode::Repeating));
    if !timer.tick(time.delta()).just_finished() {
        return;
    }
    let path = PathBuf::from(path);
    let fp = dir_fingerprint(&path);
    let Some(prev) = *last else {
        *last = Some(fp);
        return;
    };
    if prev == fp {
        return;
    }
    *last = Some(fp);
    match ActiveLook::load_dir(look.contract, &path) {
        Ok(next) => {
            *look = next;
            *assets = build_assets(&look, images.as_deref_mut(), audio.as_deref_mut());
            changed.write(LookChanged);
            info!("look: reloaded {}", look.id());
        }
        Err(report) => {
            log_report(&report);
            warn!("look: kept the previous pack; fix the errors above");
        }
    }
}
