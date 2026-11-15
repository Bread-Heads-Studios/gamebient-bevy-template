//! Native hot reload: with `--look <dir>` / `GX_LOOK=<dir>`, the folder is
//! polled every half second; a change that validates replaces the look and
//! sends `LookChanged` (through `swap::LookSwap`, the step the web fetch
//! shares), a change that does not is logged and ignored.

use std::path::PathBuf;

use bevy::prelude::*;

use super::pack::{dir_fingerprint, dir_id, read_dir};
use super::runtime::log_report;
use super::select::Selection;
use super::swap::LookSwap;
use super::validate::Report;

#[allow(clippy::result_large_err)] // Report is the full diagnosis
pub fn hot_reload(
    time: Res<Time>,
    mut timer: Local<Option<Timer>>,
    mut last: Local<Option<u64>>,
    selection: Res<Selection>,
    mut swap: LookSwap,
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
    let applied = read_dir(&path)
        .map_err(Report::read_error)
        .and_then(|files| {
            swap.apply(
                files,
                Selection::Dir(path.display().to_string()),
                dir_id(&path).as_deref(),
                None,
            )
        });
    if let Err(report) = applied {
        log_report(&report);
        warn!("look: kept the previous pack; fix the errors above");
    }
}
