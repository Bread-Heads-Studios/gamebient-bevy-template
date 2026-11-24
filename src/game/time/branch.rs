//! `time.branch` ("Try again from here"): the recording is cut at this
//! tick, the run is re-simulated up to it under the content as it now is
//! (a level edited while paused is the run's level from its first tick),
//! and play goes on live from there. The run before the branch is kept for
//! one undo.

use bevy::prelude::*;
use serde_json::json;

use super::seek::{HeldEdits, Phase, Seek, SeekKind};
use super::{Before, Content, Mark, TimeTravel, custom, push_mark, refresh_recording};
use crate::game::sim::{Checksum, SimTick};
use crate::game::tunables::Value;

/// The mark a branch onto an edited level leaves on its tick.
pub const LEVEL_EDITED: &str = "Edited the level";

/// Why a `time.branch` was not taken (the `time` answer's `refused`).
pub const NO_TIMELINE: &str = "There is nothing to try again from.";

/// `time.branch` while paused in `Playing`.
pub fn start_branch(world: &mut World) -> Result<(), &'static str> {
    refresh_recording(world);
    let n = world.resource::<SimTick>().0;
    let sum = world.resource::<Checksum>().0;
    let seek = branch_seek(world, n, Some(sum), Phase::Restart, Vec::new()).ok_or(NO_TIMELINE)?;
    world.insert_resource(seek);
    Ok(())
}

/// The seek a branch at `n` runs: the recording cut at `n`, the run's start
/// moved onto the level as it is now, the run before kept in `before`, and
/// the waiting edits (plus `edits`) held for the tick after `n`. `compare`
/// is the checksum at `n` before the edit.
///
/// The returned `Seek` owns the queued edits and `TimeTravel` is already on
/// the branch: insert it as the seek in progress, never drop it.
pub fn branch_seek(
    world: &mut World,
    n: u32,
    compare: Option<u64>,
    phase: Phase,
    edits: Vec<(u16, Value)>,
) -> Option<Seek> {
    let (start, recording) = {
        let tt = world.resource::<TimeTravel>();
        (tt.start.clone()?, tt.recording.clone()?)
    };
    let now = Content::capture(world);
    let held = HeldEdits::take(world, edits, true);
    let level_edited =
        now.level != start.content.level || now.level_hash != start.content.level_hash;
    let mut cut = recording.clone();
    cut.truncate(n);
    let mut next = start.clone();
    next.content.level = now.level;
    next.content.level_hash = now.level_hash;
    let mut tt = world.resource_mut::<TimeTravel>();
    let marks = core::mem::take(&mut tt.marks);
    let ended = tt.ended;
    tt.before = Some(Before {
        start,
        recording,
        marks,
        ended,
    });
    tt.start = Some(next);
    tt.recording = Some(cut);
    tt.ended = false;
    Some(Seek {
        target: n,
        kind: SeekKind::Branch,
        phase,
        held,
        compare,
        live: None,
        level_edited,
        send_marks: true,
        next: None,
    })
}

/// The branch reached its tick: say whether the earlier moves played out
/// differently (`time.diverged`), and mark a level edit on the tick.
pub fn settle(world: &mut World, compare: Option<u64>, level_edited: bool) {
    let tick = world.resource::<SimTick>().0;
    if let Some(sum) = compare
        && world.resource::<Checksum>().0 != sum
    {
        world.write_message(custom("time.diverged", json!({ "tick": tick })));
    }
    if level_edited {
        push_mark(world, Mark::new(tick, "edit", LEVEL_EDITED));
    }
}
