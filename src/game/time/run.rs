//! The run's life as time travel sees it. A fresh run starts a new
//! timeline. Leaving `Playing` keeps what a later seek needs and tells the
//! editor (`over: true`). A seek asked for after the run ended enters
//! `Playing` again on the run's own start. Leaving the level editor that a
//! paused run went into comes back as a branch at the paused tick, on the
//! edited level.

use bevy::prelude::*;

use super::branch::branch_seek;
use super::seek::{Phase, PlaybackFeeder, Seek, put_starts};
use super::{
    Content, RunStart, TimeTravel, custom, hold, mark_edited, refresh_recording, time_json,
};
use crate::game::replay::recorder::RunEdited;
use crate::game::sim::{Checksum, PendingSeed, RunOver, RunSeed, SimTick};
use crate::game::states::{Paused, QuietPause};
use crate::game::tunables::{Value, snapshot};

/// Tunable values changed since the run left `Playing` (on the game-over
/// screen, in the level editor): they apply as changes once a re-entered
/// run reaches its tick.
pub fn value_edits(world: &World) -> Vec<(u16, Value)> {
    let tt = world.resource::<TimeTravel>();
    if tt.final_values.is_empty() {
        return Vec::new();
    }
    snapshot(world)
        .into_iter()
        .filter(|(id, v)| {
            tt.final_values
                .iter()
                .find(|(i, _)| i == id)
                .is_none_or(|(_, f)| f != v)
        })
        .collect()
}

/// `OnEnter(Playing)`, before the run-start chain. A return from the level
/// editor becomes a branch. Then a re-entering seek puts the run's own
/// start content, start values and seed in place, keeping the editor's
/// content for after.
pub fn prepare_reentry(world: &mut World) {
    if !world.contains_resource::<Seek>() {
        let back = world.resource_mut::<TimeTravel>().edit_return.take();
        if let Some((n, sum)) = back {
            let edits = value_edits(world);
            if let Some(seek) = branch_seek(world, n, Some(sum), Phase::Reenter, edits) {
                world.insert_resource(seek);
            }
        }
    }
    let reentering = world
        .get_resource::<Seek>()
        .is_some_and(|s| s.phase == Phase::Reenter);
    if !reentering {
        return;
    }
    let Some(start) = world.resource::<TimeTravel>().start.clone() else {
        return;
    };
    let live = Content::capture(world);
    start.content.put(world);
    put_starts(world, &start.starts);
    world.resource_mut::<PendingSeed>().0 = Some(start.seed.bytes);
    world.resource_mut::<Seek>().live = Some(live);
}

/// `OnEnter(Playing)`, after the run-start chain. A re-entering seek takes
/// over from here (`drive_seek`, still paused); the run is still edited,
/// though `BeginRun` just cleared `RunEdited`. A fresh run gets a new
/// timeline: its start captured, nothing edited, speed 1.
pub fn on_run_start(world: &mut World) {
    let reentering = world
        .get_resource::<Seek>()
        .is_some_and(|s| s.phase == Phase::Reenter);
    if reentering {
        let live = world.resource_mut::<Seek>().live.take();
        if let Some(live) = live {
            live.put(world);
        }
        let origin = world
            .resource::<TimeTravel>()
            .start
            .as_ref()
            .map(|s| s.seed.origin);
        if let Some(origin) = origin {
            world.resource_mut::<RunSeed>().origin = origin;
        }
        // A re-entry exists only because the timeline was used: the run
        // must never be sealed unedited and posted.
        mark_edited(world);
        world.resource_mut::<Seek>().phase = Phase::Feed;
        hold(world);
        return;
    }
    world.remove_resource::<PlaybackFeeder>();
    let start = RunStart::capture(world);
    let run = world.resource::<TimeTravel>().run + 1;
    *world.resource_mut::<TimeTravel>() = TimeTravel {
        run,
        start: Some(start),
        ..TimeTravel::default()
    };
    world.resource_mut::<RunEdited>().0 = false;
    world
        .resource_mut::<Time<Virtual>>()
        .set_relative_speed(1.0);
    super::answer(world);
}

/// `OnExit(Playing)`, before the run is sealed: the recording as it ended,
/// whether the run ended itself, whether the timeline held it, and every
/// tunable's value. A seek still in progress is dropped. The editor hears
/// `over: true`: the run is no longer playing (a seek still takes it back
/// in).
pub fn on_run_exit(world: &mut World) {
    world.remove_resource::<Seek>();
    refresh_recording(world);
    world.remove_resource::<PlaybackFeeder>();
    let held_at_exit = world.resource::<Paused>().0 && world.resource::<QuietPause>().0;
    let ended = world.resource::<RunOver>().0;
    let values = snapshot(world);
    let mut tt = world.resource_mut::<TimeTravel>();
    tt.held_at_exit = held_at_exit;
    tt.ended = ended;
    tt.final_values = values;
    let mut j = time_json(world);
    j["over"] = true.into();
    world.write_message(custom("time", j));
}

/// `OnEnter(Editing)`: a run the timeline held paused comes back as a
/// branch at this tick when the level editor hands back.
pub fn note_edit_return(world: &mut World) {
    let tick = world.resource::<SimTick>().0;
    let sum = world.resource::<Checksum>().0;
    let mut tt = world.resource_mut::<TimeTravel>();
    tt.edit_return = (tt.held_at_exit && tt.recording.is_some()).then_some((tick, sum));
}
