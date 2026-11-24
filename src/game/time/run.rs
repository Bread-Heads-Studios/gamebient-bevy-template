//! The run's life as time travel sees it: a fresh run starts a new
//! timeline; leaving `Playing` keeps what a later seek needs.

use bevy::prelude::*;

use super::seek::{Phase, PlaybackFeeder, Seek};
use super::{RunStart, TimeTravel, answer, hold, mark_edited, refresh_recording};
use crate::game::sim::{RunOver, RunSeed};
use crate::game::states::{Paused, QuietPause};
use crate::game::tunables::snapshot;

/// `OnEnter(Playing)`, after the run-start chain. A seek or a branch that
/// re-enters `Playing` (`Phase::Reenter`) is the same run: its timeline
/// (the recording, the run before a branch, the held edits in the seek)
/// stays and is still edited (`BeginRun` just cleared `RunEdited`), and
/// the seek takes over from here, still paused. A fresh run
/// gets a new timeline (its start captured, speed 1). `BeginRun` has
/// already cleared `RunEdited` (`replay::recorder::begin_recording`).
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
    world
        .resource_mut::<Time<Virtual>>()
        .set_relative_speed(1.0);
    answer(world);
}

/// `OnExit(Playing)`, before the run is sealed: the recording as it ended,
/// whether the run ended itself, whether the timeline held it, and every
/// tunable's value.
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
}
