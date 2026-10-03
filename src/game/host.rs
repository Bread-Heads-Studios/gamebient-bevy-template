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
//! | game → host | `highlight`            | a game wrote `frame::Highlight`        |
//! | game → host | run + replay           | leaving Playing (sealed GXR1, base64)  |
//! | host → game | pause / resume         | only while `Playing`; overlay follows  |
//! | host → game | mute / unmute          | `GlobalVolume` + live sinks            |
//!
//! A developer can also mute a local run: `GX_MUTE=1` natively, `?mute=1` on
//! the web page. That [`LocalMute`] silences sinks only, so the recorder still
//! captures full audio, and a host `unmute` can't undo it.
//!
//! Hosts treat everything here as untrusted (a score is not a leaderboard
//! entry); it exists for analytics, kiosk UX and the play-bonus timer. The
//! `run` event is the one a verifier actually consumes, replaying its ticks
//! and checking the result against the claimed score and checksum.

use bevy::audio::{AudioSinkPlayback, Volume};
use bevy::prelude::*;
use bevy::transform::TransformSystems;
use gamebient_input::{HostCommand, HostEvent};

use crate::frame::Highlight;
use crate::game::scoring::{GameData, LeaderboardScore};
use crate::game::sim;
use crate::game::states::{GameState, Paused};

/// Set by a host `mute`. New sounds honour it through `GlobalVolume`; sinks
/// already playing are muted directly, and sinks spawned while muted are
/// caught by [`mute_new_sinks`].
#[derive(Resource, Default)]
pub struct Muted(pub bool);

/// Set at startup by `GX_MUTE=1` (native) or `?mute=1` (web) so test runs
/// stay quiet. Mutes sinks only and leaves `GlobalVolume` alone, so the
/// recorder can still hear what a player would.
#[derive(Resource, Default)]
pub struct LocalMute(pub bool);

impl LocalMute {
    pub fn from_launch() -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        let on = env_mutes(std::env::var("GX_MUTE").ok().as_deref());
        #[cfg(target_arch = "wasm32")]
        let on = web_sys::window()
            .and_then(|w| w.location().search().ok())
            .is_some_and(|q| query_mutes(&q));
        Self(on)
    }
}

/// Whether the recorder should treat a sink as muted. A sink muted only by
/// [`LocalMute`] is recorded at full volume: the developer silenced their
/// speakers, not the footage.
pub fn recorded_mute(sink_muted: bool, host_muted: bool, local_muted: bool) -> bool {
    sink_muted && (host_muted || !local_muted)
}

/// `GX_MUTE` is on for any non-empty value but `0`.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
fn env_mutes(value: Option<&str>) -> bool {
    value.is_some_and(|v| !v.is_empty() && v != "0")
}

/// `?mute=1` (or bare `?mute`) in a page's query string.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
fn query_mutes(search: &str) -> bool {
    search
        .trim_start_matches('?')
        .split('&')
        .any(|pair| match pair.split_once('=') {
            Some((key, value)) => key == "mute" && env_mutes(Some(value)),
            None => pair == "mute",
        })
}

pub struct HostBridgePlugin;

impl Plugin for HostBridgePlugin {
    fn build(&self, app: &mut App) {
        let local = LocalMute::from_launch();
        if local.0 {
            info!("GX_MUTE: audio muted for this run");
        }
        app.init_resource::<Muted>()
            .insert_resource(local)
            // Registered here, in the half every build shares, so a sim
            // system can write a highlight under the headless verifier.
            .add_message::<Highlight>()
            .add_systems(
                Update,
                (
                    apply_host_commands,
                    mute_new_sinks.run_if(|m: Res<Muted>, l: Res<LocalMute>| m.0 || l.0),
                    report_score.run_if(resource_changed::<GameData>),
                    report_paused.run_if(resource_changed::<Paused>),
                    report_highlight,
                ),
            )
            // Before the audio set (which runs after transform propagation),
            // so a sound queued this frame starts muted instead of blipping
            // for a frame until `mute_new_sinks` catches its sink.
            .add_systems(
                PostUpdate,
                mute_new_players
                    .run_if(|m: Res<Muted>, l: Res<LocalMute>| m.0 || l.0)
                    .before(TransformSystems::Propagate),
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
    local: Res<LocalMute>,
    // `GlobalVolume` is only inserted by `AudioPlugin`, which the headless
    // build (no window, no audio) never adds — read as optional so a host
    // `mute` doesn't panic there.
    mut global_volume: Option<ResMut<GlobalVolume>>,
    mut sinks: Query<&mut AudioSink>,
    mut pending: ResMut<sim::PendingSeed>,
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
                if let Some(volume) = global_volume.as_deref_mut() {
                    volume.volume = Volume::Linear(if *mute { 0.0 } else { 1.0 });
                }
                for mut sink in &mut sinks {
                    if *mute || local.0 {
                        sink.mute();
                    } else {
                        sink.unmute();
                    }
                }
            }
            HostCommand::Hello { .. } => {}
            HostCommand::Seed(bytes) => pending.0 = Some(*bytes),
        }
    }
}

/// Sounds queued while muted are created with a muted sink.
fn mute_new_players(mut players: Query<&mut PlaybackSettings, Added<PlaybackSettings>>) {
    for mut settings in &mut players {
        settings.muted = true;
    }
}

/// Sinks that start while muted (host or local; music crossfades, SFX) are
/// muted too.
fn mute_new_sinks(mut sinks: Query<&mut AudioSink, Added<AudioSink>>) {
    for mut sink in &mut sinks {
        sink.mute();
    }
}

fn report_started(mut out: MessageWriter<HostEvent>) {
    out.write(HostEvent::Started);
}

fn report_game_over(data: Res<GameData>, mut out: MessageWriter<HostEvent>) {
    out.write(HostEvent::Score(u64::from(data.leaderboard_score())));
    out.write(HostEvent::GameOver);
}

fn report_score(data: Res<GameData>, mut out: MessageWriter<HostEvent>) {
    out.write(HostEvent::Score(u64::from(data.leaderboard_score())));
}

fn report_paused(paused: Res<Paused>, mut out: MessageWriter<HostEvent>) {
    // The resource's insertion also counts as a change; that's not a pause.
    if paused.is_added() {
        return;
    }
    out.write(HostEvent::Paused(paused.0));
}

/// Relays the game's highlight beats to web hosts. The host applies its own
/// rate limit; the native frame reads `Highlight` directly.
fn report_highlight(mut highlights: MessageReader<Highlight>, mut out: MessageWriter<HostEvent>) {
    for highlight in highlights.read() {
        out.write(HostEvent::Highlight {
            color: highlight.hex(),
        });
    }
}

#[cfg(test)]
mod tests {
    use bevy::input::InputPlugin;
    use bevy::state::app::StatesPlugin;

    use super::*;
    use crate::frame::Highlight;

    fn read_host_events(app: &App) -> Vec<HostEvent> {
        let events = app.world().resource::<Messages<HostEvent>>();
        let mut cursor = events.get_cursor();
        cursor.read(events).cloned().collect()
    }

    #[test]
    fn gx_mute_is_on_for_any_value_but_zero() {
        assert!(env_mutes(Some("1")));
        assert!(env_mutes(Some("true")));
        assert!(!env_mutes(Some("0")));
        assert!(!env_mutes(Some("")));
        assert!(!env_mutes(None));
    }

    #[test]
    fn mute_query_parameter_is_found_among_others() {
        assert!(query_mutes("?mute=1"));
        assert!(query_mutes("?debug=1&mute"));
        assert!(!query_mutes("?mute=0"));
        assert!(!query_mutes("?unmute=1"));
        assert!(!query_mutes(""));
    }

    #[test]
    fn recorder_hears_through_a_local_mute_only() {
        assert!(!recorded_mute(true, false, true), "local mute alone");
        assert!(recorded_mute(true, true, true), "host mute still silences");
        assert!(recorded_mute(true, true, false));
        assert!(!recorded_mute(false, false, false));
    }

    #[test]
    fn highlight_is_relayed_to_the_host_with_its_colour() {
        let mut app = App::new();
        app.add_message::<Highlight>()
            .add_message::<HostEvent>()
            .add_systems(Update, report_highlight);
        app.world_mut()
            .write_message(Highlight::rgb(0xff, 0xcc, 0x00));
        app.world_mut().write_message(Highlight::plain());
        app.update();
        assert_eq!(
            read_host_events(&app),
            vec![
                HostEvent::Highlight {
                    color: Some("#ffcc00".into())
                },
                HostEvent::Highlight { color: None },
            ]
        );
    }

    /// A sim system may write `Highlight`, so the message must exist in the
    /// headless build too or that system would panic in the verifier.
    #[test]
    fn headless_build_registers_the_highlight_message() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(StatesPlugin)
            .add_plugins(InputPlugin)
            .add_plugins(crate::game::GamePlugin { headless: true });
        app.update();
        assert!(app.world().contains_resource::<Messages<Highlight>>());
    }
}
