//! The `time.*` host commands, and the `time` update every
//! [`UPDATE_EVERY`] ticks.

use bevy::prelude::*;
use gamebient_input::HostCommand;
use serde_json::Value as Json;

use super::seek::{self, Seek};
use super::{
    SPEEDS, STEP_MAX, TimeTravel, UPDATE_EVERY, answer, hold, in_playing, mark_edited,
    refresh_recording, unhold,
};
use crate::game::sim::SimTick;

/// `time.*` commands read this frame, applied by `apply_time_commands`.
#[derive(Resource, Default)]
pub struct TimeCommands(pub Vec<(String, Json)>);

pub fn read_time_commands(
    mut commands: MessageReader<HostCommand>,
    mut queue: ResMut<TimeCommands>,
) {
    for c in commands.read() {
        if let HostCommand::Custom { name, data } = c
            && name.starts_with("time.")
        {
            queue.0.push((
                name.clone(),
                serde_json::from_str(data).unwrap_or(Json::Null),
            ));
        }
    }
}

pub fn apply_time_commands(world: &mut World) {
    let queued = core::mem::take(&mut world.resource_mut::<TimeCommands>().0);
    for (name, data) in queued {
        apply(world, &name, &data);
    }
}

/// One command. Every known command is answered with `time`, taken or not;
/// a taken one marks the run edited.
pub fn apply(world: &mut World, name: &str, data: &Json) {
    let free = in_playing(world) && !world.contains_resource::<Seek>();
    match name {
        "time.pause" => {
            if free {
                pause(world);
            }
        }
        "time.resume" => {
            if free {
                unhold(world);
            }
        }
        "time.step" => {
            let n = data
                .get("n")
                .and_then(Json::as_u64)
                .map_or(1, |n| n.clamp(1, u64::from(STEP_MAX)) as u32);
            if free {
                pause(world);
                seek::run_ticks(world, n);
            }
        }
        "time.seek" => {
            let tick = data
                .get("tick")
                .and_then(Json::as_u64)
                .map_or(0, |t| u32::try_from(t).unwrap_or(u32::MAX));
            let before = data.get("before_branch").and_then(Json::as_bool) == Some(true);
            seek::start_seek(world, tick, before);
        }
        "time.speed" => {
            let x = data.get("x").and_then(Json::as_f64);
            if let Some(s) = SPEEDS.iter().copied().find(|s| Some(f64::from(*s)) == x) {
                set_speed(world, s);
            }
        }
        _ => return,
    }
    if world.resource::<TimeTravel>().start.is_some() {
        mark_edited(world);
    }
    answer(world);
}

/// The timeline's pause, with the recording brought up to date.
pub fn pause(world: &mut World) {
    refresh_recording(world);
    hold(world);
}

pub fn set_speed(world: &mut World, speed: f32) {
    world
        .resource_mut::<Time<Virtual>>()
        .set_relative_speed(speed);
    world.resource_mut::<TimeTravel>().speed = speed;
}

/// In `SimSet`, after `record_tick`: the `time` update every
/// [`UPDATE_EVERY`] ticks while the run plays (never during a seek).
pub fn report_every(world: &mut World) {
    if world.contains_resource::<Seek>()
        || !world.resource::<SimTick>().0.is_multiple_of(UPDATE_EVERY)
    {
        return;
    }
    answer(world);
}
