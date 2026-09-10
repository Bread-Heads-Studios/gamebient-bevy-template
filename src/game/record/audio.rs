//! Audio capture for the recorder. Every frame (`Last`, after Bevy's audio
//! systems in `PostUpdate`) this logs each `AudioPlayer` playback: which
//! source, when it started and stopped on the sim-frame clock, and an
//! automation key whenever its per-ear gain, speed or pause state changes.
//! Volume is read from the live sink when there is one (it already includes
//! `GlobalVolume` and every later `set_volume`, i.e. music fades), else from
//! `PlaybackSettings × GlobalVolume`. Spatial playbacks reproduce rodio's
//! per-ear gains from the emitter and listener transforms. A voice is keyed
//! paused (silent, playhead held) until its source is in
//! `Assets<AudioSource>`, so a loading asset does not start early.
//!
//! Each source is decoded once, with Bevy's own decoder, the first frame it
//! is available; at `AppExit` the voices are mixed by `mixer::render` into
//! `audio.wav`. Sim frames convert to video frames by subtracting the sim
//! frame on which numbered frame 1 was captured, so sounds from the warm-up
//! are trimmed rather than shifted; voices that end before video time 0 are
//! dropped.
//!
//! Close rule. rodio plays on the wall clock, but the recorder's sim clock
//! is decoupled from it and often several times slower (every frame waits
//! on a PNG capture), so a one-shot's sink empties (and
//! `PlaybackMode::Despawn` / `Remove` strip it) after only a fraction of its
//! length in sim time. Closing it there would truncate it in the mix.
//! Instead each voice accumulates `played`, the clip seconds rodio has
//! actually played (`wall_dt × speed` while heard and not paused, `wall_dt`
//! from `std::time::Instant`, since `Time<Real>` is manual here too). When a
//! non-looping voice goes silent (sink empty, or entity / `AudioPlayer`
//! gone) having played its whole clip (`natural_end`), it gets no end and
//! the mixer stops it at the clip's end on the sim clock; if it went silent
//! early, the game stopped it (a despawn, a crossfade) and it ends that
//! frame. A handle change, a looping voice, a source that never loaded and
//! the close at exit always end that frame.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::time::Instant;

use bevy::app::AppExit;
use bevy::audio::{
    AudioPlayer, AudioSink, AudioSinkPlayback, AudioSource, Decodable, DefaultSpatialScale,
    GlobalVolume, PlaybackMode, PlaybackSettings, Source, SpatialAudioSink, SpatialListener,
};
use bevy::prelude::*;

use super::Recorder;
use super::mixer::{self, Clip, Key, Voice};

pub const OUT_RATE: u32 = 48_000;

/// (sim frame, gain_l, gain_r, speed, paused)
type RawKey = (u64, f32, f32, f32, bool);

struct Track {
    source: AssetId<AudioSource>,
    start: u64,
    /// `None` on a closed track: a one-shot the mixer runs to its clip's end.
    end: Option<u64>,
    looping: bool,
    downmix: bool,
    keys: Vec<RawKey>,
    /// A sink has been seen with sound queued; a later empty sink ends it.
    heard: bool,
    /// Clip seconds rodio has played, measured on the wall clock.
    played: f64,
    /// `end` is decided and never moves again.
    closed: bool,
}

impl Track {
    /// Credit the wall time since the last frame at the speed and pause
    /// state that held over it (the latest key).
    fn advance(&mut self, wall_dt: f64) {
        if self.closed || !self.heard {
            return;
        }
        if let Some(&(_, _, _, speed, paused)) = self.keys.last()
            && !paused
        {
            self.played += wall_dt * f64::from(speed);
        }
    }
}

/// Why a voice is closing.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Close {
    /// Its sink went empty, or its entity or `AudioPlayer` disappeared:
    /// either rodio finished it or the game stopped it.
    Silent,
    /// Its handle changed, or the recording is exiting.
    Cut,
}

/// True when a one-shot that just went silent had played its whole clip in
/// wall time (rodio finished it), false when the game stopped it early.
fn natural_end(played: f64, clip_secs: f64, wall_dt: f64) -> bool {
    played + (2.0 * wall_dt).max(0.1) >= clip_secs
}

/// The end a closing voice gets: `None` lets the mixer finish a one-shot's
/// clip on the sim clock, `Some(now)` cuts it on this frame. An uncached
/// clip (the asset never loaded) is cut.
fn close_end(
    reason: Close,
    looping: bool,
    played: f64,
    clip_secs: Option<f64>,
    wall_dt: f64,
    now: u64,
) -> Option<u64> {
    match (reason, looping, clip_secs) {
        (Close::Silent, false, Some(secs)) if natural_end(played, secs, wall_dt) => None,
        _ => Some(now),
    }
}

#[derive(Resource, Default)]
pub struct AudioCapture {
    live: HashMap<Entity, Track>,
    done: Vec<Track>,
    /// Every source seen, decoded once when it first appears in `Assets`.
    clips: HashMap<AssetId<AudioSource>, Clip>,
    last_tick: Option<Instant>,
}

impl AudioCapture {
    /// Decide `entity`'s end, once: a track already closed keeps its end.
    fn close(&mut self, entity: Entity, now: u64, reason: Close, wall_dt: f64) {
        let Some(t) = self.live.get_mut(&entity) else {
            return;
        };
        if !t.closed {
            t.closed = true;
            let secs = self.clips.get(&t.source).map(Clip::secs);
            t.end = close_end(reason, t.looping, t.played, secs, wall_dt, now);
        }
    }

    /// Close `entity`'s voice and stop tracking the entity.
    fn retire(&mut self, entity: Entity, now: u64, reason: Close, wall_dt: f64) {
        self.close(entity, now, reason, wall_dt);
        if let Some(t) = self.live.remove(&entity) {
            self.done.push(t);
        }
    }
}

/// Bevy's `EarPositions` rule: the first listener's ears in world space,
/// or the default offsets when there is no listener.
fn ears(listeners: &Query<(&GlobalTransform, &SpatialListener)>) -> (Vec3, Vec3) {
    listeners
        .iter()
        .next()
        .map(|(tf, l)| {
            (
                tf.transform_point(l.left_ear_offset),
                tf.transform_point(l.right_ear_offset),
            )
        })
        .unwrap_or_else(|| {
            let d = SpatialListener::default();
            (d.left_ear_offset, d.right_ear_offset)
        })
}

#[allow(clippy::type_complexity)]
pub fn capture_audio(
    mut cap: ResMut<AudioCapture>,
    rec: Res<Recorder>,
    sources: Res<Assets<AudioSource>>,
    global: Option<Res<GlobalVolume>>,
    default_scale: Option<Res<DefaultSpatialScale>>,
    players: Query<(
        Entity,
        &AudioPlayer,
        &PlaybackSettings,
        Option<&AudioSink>,
        Option<&SpatialAudioSink>,
        Option<&GlobalTransform>,
    )>,
    listeners: Query<(&GlobalTransform, &SpatialListener)>,
) {
    let cap = &mut *cap;
    let now = rec.sim_frame;
    let tick = Instant::now();
    let wall_dt = cap
        .last_tick
        .map_or(0.0, |t| tick.duration_since(t).as_secs_f64());
    cap.last_tick = Some(tick);
    for t in cap.live.values_mut() {
        t.advance(wall_dt);
    }
    for (_, player, ..) in &players {
        let id = player.0.id();
        if !cap.clips.contains_key(&id)
            && let Some(source) = sources.get(id)
        {
            cap.clips.insert(id, decode(source));
        }
    }

    let alive: HashSet<Entity> = players.iter().map(|p| p.0).collect();
    let gone: Vec<Entity> = cap
        .live
        .keys()
        .filter(|e| !alive.contains(e))
        .copied()
        .collect();
    for e in gone {
        cap.retire(e, now, Close::Silent, wall_dt);
    }
    let global = global.map_or(bevy::audio::Volume::Linear(1.0), |g| g.volume);
    let (left_ear, right_ear) = ears(&listeners);

    for (entity, player, settings, sink, spatial_sink, tf) in &players {
        let id = player.0.id();
        if cap.live.get(&entity).is_some_and(|t| t.source != id) {
            cap.retire(entity, now, Close::Cut, wall_dt);
        }
        let state = if let Some(s) = sink {
            Some((
                s.volume().to_linear(),
                s.speed(),
                s.is_paused(),
                s.is_muted(),
                s.empty(),
            ))
        } else {
            spatial_sink.map(|s| {
                (
                    s.volume().to_linear(),
                    s.speed(),
                    s.is_paused(),
                    s.is_muted(),
                    s.empty(),
                )
            })
        };
        let has_sink = state.is_some();
        let (volume, speed, paused, muted, empty) = state.unwrap_or((
            (settings.volume * global).to_linear(),
            settings.speed,
            settings.paused,
            settings.muted,
            false,
        ));
        // Not loaded yet: silent, playhead held, until the source exists.
        let paused = paused || !cap.clips.contains_key(&id);
        let track = cap.live.entry(entity).or_insert_with(|| Track {
            source: id,
            start: now,
            end: None,
            looping: matches!(settings.mode, PlaybackMode::Loop),
            downmix: settings.spatial,
            keys: Vec::new(),
            heard: false,
            played: 0.0,
            closed: false,
        });
        if track.closed {
            continue;
        }
        if has_sink && !empty {
            track.heard = true;
        }
        if track.heard && empty {
            // Stays tracked: a `PlaybackMode::Once` entity keeps its empty
            // sink and must not reopen as a new voice.
            cap.close(entity, now, Close::Silent, wall_dt);
            continue;
        }
        let gain = if muted { 0.0 } else { volume };
        let (gl, gr) = if settings.spatial {
            let scale = settings
                .spatial_scale
                .or_else(|| default_scale.as_ref().map(|d| d.0))
                .map_or(Vec3::ONE, |s| s.0);
            let pos = tf.map_or(Vec3::ZERO, |t| t.translation()) * scale;
            let (a, b) = mixer::spatial_gains(
                pos.to_array(),
                (left_ear * scale).to_array(),
                (right_ear * scale).to_array(),
            );
            (gain * a, gain * b)
        } else {
            (gain, gain)
        };
        let changed = track
            .keys
            .last()
            .is_none_or(|k| (k.1, k.2, k.3, k.4) != (gl, gr, speed, paused));
        if changed {
            track.keys.push((now, gl, gr, speed, paused));
        }
    }
}

fn decode(source: &AudioSource) -> Clip {
    let decoder = source.decoder();
    let channels = decoder.channels();
    let rate = decoder.sample_rate();
    let samples = decoder.map(|s| f32::from(s) / 32768.0).collect();
    Clip {
        channels,
        rate,
        samples,
    }
}

/// On the `AppExit` frame: mix everything captured into `audio.wav` and
/// record `(voices, peak)` on the `Recorder` for the manifest. Writes no
/// file (and reports `None`) when no frame was saved or no voice is audible
/// in video time.
pub fn finish_audio(
    mut exits: MessageReader<AppExit>,
    mut cap: ResMut<AudioCapture>,
    mut rec: ResMut<Recorder>,
    sources: Res<Assets<AudioSource>>,
) {
    if exits.read().next().is_none() {
        return;
    }
    let now = rec.sim_frame;
    let first = rec.first_video_sim_frame.unwrap_or(now);
    let open: Vec<Entity> = cap.live.keys().copied().collect();
    for e in open {
        cap.retire(e, now, Close::Cut, 0.0);
    }
    let tracks = std::mem::take(&mut cap.done);
    let mut cache = std::mem::take(&mut cap.clips);
    let to_video = |f: u64| f as f64 - first as f64;
    let mut index: HashMap<AssetId<AudioSource>, usize> = HashMap::new();
    let mut clips: Vec<Clip> = Vec::new();
    let mut voices: Vec<Voice> = Vec::new();
    for t in &tracks {
        if t.keys.is_empty() {
            continue;
        }
        let clip = match index.get(&t.source) {
            Some(&i) => i,
            None => {
                let Some(decoded) = cache
                    .remove(&t.source)
                    .or_else(|| sources.get(t.source).map(decode))
                else {
                    continue;
                };
                clips.push(decoded);
                index.insert(t.source, clips.len() - 1);
                clips.len() - 1
            }
        };
        let voice = Voice {
            clip,
            start: to_video(t.start),
            end: t.end.map(to_video),
            looping: t.looping,
            downmix: t.downmix,
            keys: t
                .keys
                .iter()
                .map(|&(f, gain_l, gain_r, speed, paused)| Key {
                    frame: to_video(f),
                    gain_l,
                    gain_r,
                    speed,
                    paused,
                })
                .collect(),
        };
        let stop = voice
            .end
            .unwrap_or_else(|| mixer::natural_stop(&voice, &clips[clip], rec.fps));
        if stop <= 0.0 {
            continue;
        }
        voices.push(voice);
    }
    if rec.saved == 0 || voices.is_empty() {
        rec.audio = None;
        info!("record: no audio in the video, audio.wav not written");
        return;
    }
    let used: HashSet<usize> = voices.iter().map(|v| v.clip).collect();
    let mut buf = mixer::render(&voices, &clips, rec.fps, OUT_RATE, rec.saved);
    let peak = mixer::limit(&mut buf);
    fs::write(rec.dir.join("audio.wav"), mixer::wav_bytes(&buf, OUT_RATE))
        .expect("record: write audio.wav");
    rec.audio = Some((voices.len(), peak));
    info!(
        "record: mixed {} voices from {} sources, peak {peak:.3}",
        voices.len(),
        used.len()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn natural_end_full_play_is_natural() {
        assert!(natural_end(0.3, 0.3, 1.0 / 60.0));
        assert!(natural_end(0.45, 0.3, 1.0 / 60.0));
    }

    #[test]
    fn natural_end_half_played_is_early() {
        assert!(!natural_end(1.5, 3.0, 1.0 / 60.0));
        assert!(!natural_end(0.0, 0.3, 0.05));
    }

    #[test]
    fn natural_end_tolerates_a_frame_or_a_tenth_of_a_second() {
        // Floor of 0.1 s.
        assert!(natural_end(0.21, 0.3, 0.01));
        assert!(!natural_end(0.19, 0.3, 0.01));
        // Two wall frames when frames are slow.
        assert!(natural_end(0.5, 0.8, 0.2));
        assert!(!natural_end(0.3, 0.8, 0.2));
    }

    #[test]
    fn close_end_lets_only_finished_one_shots_run() {
        let dt = 1.0 / 60.0;
        assert_eq!(close_end(Close::Silent, false, 0.3, Some(0.3), dt, 9), None);
        assert_eq!(
            close_end(Close::Silent, false, 0.1, Some(3.0), dt, 9),
            Some(9)
        );
        assert_eq!(
            close_end(Close::Silent, true, 5.0, Some(3.0), dt, 9),
            Some(9)
        );
        assert_eq!(close_end(Close::Cut, false, 0.3, Some(0.3), dt, 9), Some(9));
        assert_eq!(close_end(Close::Silent, false, 0.3, None, dt, 9), Some(9));
    }

    fn track(heard: bool, keys: Vec<RawKey>) -> Track {
        Track {
            source: AssetId::default(),
            start: 0,
            end: None,
            looping: false,
            downmix: false,
            keys,
            heard,
            played: 0.0,
            closed: false,
        }
    }

    #[test]
    fn advance_counts_heard_unpaused_wall_time_at_speed() {
        let mut t = track(true, vec![(0, 1.0, 1.0, 2.0, false)]);
        t.advance(0.25);
        assert_eq!(t.played, 0.5);
        let mut unheard = track(false, vec![(0, 1.0, 1.0, 1.0, false)]);
        unheard.advance(0.25);
        assert_eq!(unheard.played, 0.0);
        let mut paused = track(true, vec![(0, 1.0, 1.0, 1.0, true)]);
        paused.advance(0.25);
        assert_eq!(paused.played, 0.0);
    }

    #[test]
    fn closing_twice_keeps_the_first_end() {
        let mut cap = AudioCapture::default();
        let e = Entity::from_raw_u32(1).unwrap();
        let mut t = track(true, vec![(0, 1.0, 1.0, 1.0, false)]);
        t.played = 0.3;
        cap.clips.insert(
            t.source,
            Clip {
                channels: 1,
                rate: 10,
                samples: vec![0.0; 3],
            },
        );
        cap.live.insert(e, t);
        cap.close(e, 5, Close::Silent, 0.0);
        cap.retire(e, 20, Close::Cut, 0.0);
        assert!(cap.live.is_empty());
        assert_eq!(cap.done[0].end, None);
    }
}
