//! Host protocol glue: what this game reports to an embedding host (the
//! ColecoVision GX site, a cabinet, a dock) and how it honours the host's
//! commands. Protocol: gamebient-input/docs/host-protocol.md.
//!
//! The crate already posts `ready` and every `GameState` transition
//! (`StateEvents`). This module adds the events a host acts on and the
//! commands a host may send:
//!
//! | Direction   | Message                | When                                   |
//! |-------------|------------------------|----------------------------------------|
//! | game → host | `started`              | entering `Playing`                     |
//! | game → host | `gameover` + `score`   | entering `GameOver`                    |
//! | game → host | `score`                | `GameData.score` changes               |
//! | game → host | `paused`               | `Paused` changes (player or host)      |
//! | host → game | pause / resume         | only while `Playing`; overlay follows  |
//! | host → game | mute / unmute          | `GlobalVolume` + live sinks            |
//!
//! Hosts treat everything here as untrusted (a score is not a leaderboard
//! entry); it exists for analytics, kiosk UX and the play-bonus timer.

use bevy::audio::{AudioSinkPlayback, Volume};
use bevy::prelude::*;
use gamebient_input::{HostCommand, HostEvent};

use crate::game::scoring::GameData;
use crate::game::states::{GameState, Paused};

/// Set by a host `mute`. New sounds honour it through `GlobalVolume`; sinks
/// already playing are muted directly, and sinks spawned while muted are
/// caught by [`mute_new_sinks`].
#[derive(Resource, Default)]
pub struct Muted(pub bool);

pub struct HostBridgePlugin;

impl Plugin for HostBridgePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Muted>()
            .add_systems(
                Update,
                (
                    apply_host_commands,
                    mute_new_sinks.run_if(|m: Res<Muted>| m.0),
                    report_score.run_if(resource_changed::<GameData>),
                    report_paused.run_if(resource_changed::<Paused>),
                ),
            )
            .add_systems(OnEnter(GameState::Playing), report_started)
            .add_systems(OnEnter(GameState::GameOver), report_game_over);
    }
}

/// Applies host commands. Pause/resume only mean something during a run; the
/// pause overlay follows the `Paused` resource, so a host pause looks exactly
/// like a player pause.
fn apply_host_commands(
    mut commands: MessageReader<HostCommand>,
    state: Res<State<GameState>>,
    mut paused: ResMut<Paused>,
    mut muted: ResMut<Muted>,
    mut global_volume: ResMut<GlobalVolume>,
    mut sinks: Query<&mut AudioSink>,
) {
    for command in commands.read() {
        match command {
            HostCommand::Pause | HostCommand::Resume => {
                if *state.get() != GameState::Playing {
                    continue;
                }
                let want = matches!(command, HostCommand::Pause);
                if paused.0 != want {
                    paused.0 = want;
                }
            }
            HostCommand::Mute(mute) => {
                muted.0 = *mute;
                global_volume.volume = Volume::Linear(if *mute { 0.0 } else { 1.0 });
                for mut sink in &mut sinks {
                    if *mute {
                        sink.mute();
                    } else {
                        sink.unmute();
                    }
                }
            }
            HostCommand::Hello { .. } => {}
        }
    }
}

/// Sinks that start while muted (music crossfades, SFX) are muted too.
fn mute_new_sinks(mut sinks: Query<&mut AudioSink, Added<AudioSink>>) {
    for mut sink in &mut sinks {
        sink.mute();
    }
}

fn report_started(mut out: MessageWriter<HostEvent>) {
    out.write(HostEvent::Started);
}

fn report_game_over(data: Res<GameData>, mut out: MessageWriter<HostEvent>) {
    out.write(HostEvent::Score(u64::from(data.score)));
    out.write(HostEvent::GameOver);
}

fn report_score(data: Res<GameData>, mut out: MessageWriter<HostEvent>) {
    out.write(HostEvent::Score(u64::from(data.score)));
}

fn report_paused(paused: Res<Paused>, mut out: MessageWriter<HostEvent>) {
    // The resource's insertion also counts as a change; that's not a pause.
    if paused.is_added() {
        return;
    }
    out.write(HostEvent::Paused(paused.0));
}
