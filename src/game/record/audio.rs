//! Audio capture for the recorder. Every frame (`Last`, after Bevy's audio
//! systems in `PostUpdate`) this logs each `AudioPlayer` playback: which
//! source, when it started and stopped on the sim-frame clock, and an
//! automation key whenever its per-ear gain, speed or pause state changes.
//! Volume is read from the live sink when there is one (it already includes
//! `GlobalVolume` and every later `set_volume`, i.e. music fades), else from
//! `PlaybackSettings × GlobalVolume`. Spatial playbacks reproduce rodio's
//! per-ear gains from the emitter and listener transforms. At `AppExit` the
//! sources are decoded with Bevy's own decoder and mixed by `mixer::render`
//! into `audio.wav`. Sim frames convert to video frames by subtracting the
//! sim frame on which numbered frame 1 was captured, so sounds from the
//! warm-up are trimmed rather than shifted.

use std::collections::{HashMap, HashSet};
use std::fs;

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
    end: Option<u64>,
    looping: bool,
    downmix: bool,
    keys: Vec<RawKey>,
    /// A sink has been seen with sound queued; a later empty sink ends it.
    heard: bool,
}

#[derive(Resource, Default)]
pub struct AudioCapture {
    live: HashMap<Entity, Track>,
    done: Vec<Track>,
}

impl AudioCapture {
    fn close(&mut self, entity: Entity, now: u64) {
        if let Some(mut t) = self.live.remove(&entity) {
            t.end.get_or_insert(now);
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

pub fn capture_audio(
    mut cap: ResMut<AudioCapture>,
    rec: Res<Recorder>,
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
    let now = rec.sim_frame;
    let alive: HashSet<Entity> = players.iter().map(|p| p.0).collect();
    let gone: Vec<Entity> = cap
        .live
        .keys()
        .filter(|e| !alive.contains(e))
        .copied()
        .collect();
    for e in gone {
        cap.close(e, now);
    }
    let global = global.map_or(bevy::audio::Volume::Linear(1.0), |g| g.volume);
    let (left_ear, right_ear) = ears(&listeners);

    for (entity, player, settings, sink, spatial_sink, tf) in &players {
        let id = player.0.id();
        if cap.live.get(&entity).is_some_and(|t| t.source != id) {
            cap.close(entity, now);
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
        let track = cap.live.entry(entity).or_insert_with(|| Track {
            source: id,
            start: now,
            end: None,
            looping: matches!(settings.mode, PlaybackMode::Loop),
            downmix: settings.spatial,
            keys: Vec::new(),
            heard: false,
        });
        if track.end.is_some() {
            continue;
        }
        if has_sink && !empty {
            track.heard = true;
        }
        if track.heard && empty {
            track.end = Some(now);
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
/// record `(voices, peak)` on the `Recorder` for the manifest.
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
        cap.close(e, now);
    }
    let tracks = std::mem::take(&mut cap.done);
    let to_video = |f: u64| f as f64 - first as f64;
    let mut ids: Vec<AssetId<AudioSource>> = Vec::new();
    let mut clips: Vec<Clip> = Vec::new();
    let mut voices: Vec<Voice> = Vec::new();
    for t in &tracks {
        if t.keys.is_empty() {
            continue;
        }
        let clip = match ids.iter().position(|i| *i == t.source) {
            Some(i) => i,
            None => {
                let Some(source) = sources.get(t.source) else {
                    continue;
                };
                ids.push(t.source);
                clips.push(decode(source));
                clips.len() - 1
            }
        };
        voices.push(Voice {
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
        });
    }
    if voices.is_empty() {
        rec.audio = None;
        info!("record: no audio played");
        return;
    }
    let mut buf = mixer::render(&voices, &clips, rec.fps, OUT_RATE, rec.saved);
    let peak = mixer::limit(&mut buf);
    fs::write(rec.dir.join("audio.wav"), mixer::wav_bytes(&buf, OUT_RATE))
        .expect("record: write audio.wav");
    rec.audio = Some((voices.len(), peak));
    info!(
        "record: mixed {} voices from {} sources, peak {peak:.3}",
        voices.len(),
        clips.len()
    );
}
