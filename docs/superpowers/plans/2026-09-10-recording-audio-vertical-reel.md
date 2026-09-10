# Recording v2 (audio, vertical, reel) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every recording gets a mixed audio track, 9:16 vertical versions of the tour and every beat clip, and the catalog gets a ~95 s sizzle reel in 16:9 and 9:16.

**Architecture:** `src/game/record.rs` becomes a folder module. `record/audio.rs` logs every `AudioPlayer` playback per frame (gain per ear, speed, paused) on a sim-frame clock and, at `AppExit`, decodes the sources with Bevy's own decoder and calls the pure `record/mixer.rs` to write `audio.wav`. `tools/record.sh` muxes it with loudnorm; `tools/cut_clips.py` keeps audio in clips and renders blurred-fill vertical versions with an rsvg banner; the skill (moved into the template) gains `tools/reel.py`, which assembles title cards and lead clips into two reels.

**Tech Stack:** Rust 2024, Bevy 0.18 (`bevy::audio`: `AudioPlayer`, `PlaybackSettings`, `AudioSinkPlayback`, `Decodable`, `Source`), Python 3 stdlib, ffmpeg 9 (no `drawtext`), rsvg-convert.

Spec: `docs/superpowers/specs/2026-09-10-recording-audio-vertical-reel-design.md`.

## Global Constraints

- No new crate dependencies; JSON and WAV are hand-rendered.
- Audio output: `audio.wav`, 48 000 Hz, 16-bit PCM, stereo. Limiter knee `0.9`, ceiling `0.99`.
- Loudness: `loudnorm=I=-16:TP=-1.5:LRA=11`, then `-ar 48000`, AAC `192k`.
- Vertical: 1080x1920; gameplay `scale=1080:-2` centered (y 656–1264); background blurred and darkened (`eq=brightness=-0.12`); banner PNG overlaid at 0,0.
- CTA text, verbatim: line 1 `Play the demo at`, line 2 `colecovisiongx.com`.
- Titles on banners and cards are the manifest `name`, upper-cased, XML-escaped; size `min(cap, int(width / (0.8 × len)))`.
- Fonts (rsvg-verified): titles `Arial Black, Helvetica Neue, Arial, sans-serif` weight 900; body `Helvetica Neue, Arial, sans-serif` bold.
- Reel: card `1.5` s, clip default `start 1.0` `length 4.5`, end card `3.0` s; segments 60 fps, h264 crf 20, yuv420p, AAC 48 kHz stereo; 0.15 s audio fades on clips; concat demuxer `-c copy`.
- Recordings run in the foreground, one at a time (`timeout 1500 tools/record.sh`, Bash timeout 1500000 ms).
- `build/` is never committed. Game repos commit on `main`; the template works on branch `feat/record-audio`. Nothing is pushed.
- Commits end with `Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>`.
- Template: `/Users/kelliott/Gamebient/colecovisiongx/libs/gamebient-bevy-template`. Workspace root: `/Users/kelliott/Gamebient/colecovisiongx` (not a git repo).

---

## File map

Template:

| File | Responsibility |
|---|---|
| `src/game/record/mod.rs` | existing recorder (moved from `record.rs`) + sim clock + manifest `audio` field |
| `src/game/record/mixer.rs` | pure mixer: `Clip`, `Key`, `Voice`, `render`, `limit`, `wav_bytes`, `spatial_gains` |
| `src/game/record/audio.rs` | `AudioCapture` resource, `capture_audio`, `finish_audio`, `decode` |
| `tools/record.sh` | mux `audio.wav` with loudnorm |
| `tools/cut_clips.py`, `tools/test_cut_clips.py` | audio in clips; vertical outputs |
| `tools/vertical-banner.svg` | 1080x1920 transparent banner template |
| `tools/rollout-record.sh` | copy the folder module + banner |
| `.claude/skills/recording-game-footage/**` | skill moved here; adds `tools/reel.py`, `tools/test_reel.py`, `tools/card-16x9.svg`, `tools/card-9x16.svg`, `tools/end-16x9.svg`, `tools/end-9x16.svg` |

---

### Task 1: Folder module, sim clock, manifest audio field

**Files:**
- Move: `src/game/record.rs` → `src/game/record/mod.rs`
- Modify: `src/game/record/mod.rs`

**Interfaces:**
- Produces: `Recorder.sim_frame: u64` (pub), `Recorder.first_video_sim_frame: Option<u64>` (private, visible to child modules), `Recorder.audio: Option<(usize, f32)>` (private), `Manifest.audio: Option<(usize, f32)>`; manifest JSON ends with `,"audio":null` or `,"audio":{"voices":N,"peak":P}` (peak three decimals).

- [ ] **Step 1: Move the file**

```bash
mkdir -p src/game/record && git mv src/game/record.rs src/game/record/mod.rs
cargo test --all-features record::tests
```
Expected: the existing tests still pass (module path unchanged).

- [ ] **Step 2: Update the manifest tests (failing first)**

In the test module, change `manifest_json_lists_beats_in_order` to build `Manifest { …, audio: None }` and expect the old string with `,"audio":null` inserted before the final `}`:

```rust
r#"{"name":"Gamebient Game","fps":60,"width":1920,"height":1080,"frames":3600,"duration_s":60.000,"beats":[{"name":"01-studio-logo","frame":72},{"name":"02-title","frame":150}],"audio":null}"#
```

Add:

```rust
    #[test]
    fn manifest_json_reports_audio_voices_and_peak() {
        let m = Manifest {
            name: "G".into(),
            fps: 60,
            width: 2,
            height: 2,
            frames: 60,
            beats: vec![],
            audio: Some((12, 0.8126)),
        };
        assert!(manifest_json(&m).ends_with(r#""beats":[],"audio":{"voices":12,"peak":0.813}}"#));
    }
```

Run: `cargo test --all-features record::tests` → compile error (`audio` field missing).

- [ ] **Step 3: Implement**

`Manifest` gains `pub audio: Option<(usize, f32)>`. In `manifest_json`, after the beats array:

```rust
    let audio = match m.audio {
        None => "null".to_string(),
        Some((voices, peak)) => json_obj(&[
            ("voices", voices.to_string()),
            ("peak", format!("{peak:.3}")),
        ]),
    };
```
and append `,"audio":{audio}` before the closing brace of the format string.

`Recorder` gains three fields (initialise to `0`, `None`, `None` in `open`):

```rust
    /// Every `capture_frame` call, warm-up included: the audio clock.
    pub sim_frame: u64,
    /// `sim_frame` on which numbered frame 1 was requested; converts audio
    /// times to video time (see `audio.rs`).
    first_video_sim_frame: Option<u64>,
    /// `(voices, peak)` from `audio::finish_audio`; `None` when nothing played.
    audio: Option<(usize, f32)>,
```

`capture_frame`: first line `rec.sim_frame += 1;`. After `rec.frame += 1;` add:

```rust
    if rec.frame == 1 {
        rec.first_video_sim_frame = Some(rec.sim_frame);
    }
```

`Recorder::finish` passes `audio: self.audio` into `Manifest`.

- [ ] **Step 4: Verify**

Run: `cargo test --all-features record::tests && cargo clippy --all-targets --all-features -- -D warnings && cargo fmt --all -- --check`
Expected: 10 record tests pass; clean.

- [ ] **Step 5: Commit**

```bash
git add -A src/game/record src/game/record.rs
git commit -m "refactor(record): folder module, sim clock, manifest audio field

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 2: Pure mixer

**Files:**
- Create: `src/game/record/mixer.rs`
- Modify: `src/game/record/mod.rs` (add `mod mixer;`)

**Interfaces:**
- Produces:
  - `pub struct Clip { pub channels: u16, pub rate: u32, pub samples: Vec<f32> }` (interleaved)
  - `pub struct Key { pub frame: f64, pub gain_l: f32, pub gain_r: f32, pub speed: f32, pub paused: bool }` (frame = video frame, may be negative)
  - `pub struct Voice { pub clip: usize, pub start: f64, pub end: Option<f64>, pub looping: bool, pub downmix: bool, pub keys: Vec<Key> }`
  - `pub fn render(voices: &[Voice], clips: &[Clip], fps: u32, out_rate: u32, total_frames: u64) -> Vec<f32>` (interleaved stereo, length `2 × round(total_frames / fps × out_rate)`)
  - `pub const KNEE: f32 = 0.9; pub const CEILING: f32 = 0.99; pub fn limit(buf: &mut [f32]) -> f32`
  - `pub fn wav_bytes(stereo: &[f32], rate: u32) -> Vec<u8>`
  - `pub fn spatial_gains(emitter: [f32; 3], left_ear: [f32; 3], right_ear: [f32; 3]) -> (f32, f32)`

- [ ] **Step 1: Write the failing tests**

`src/game/record/mixer.rs` starts with the module doc and the test module:

```rust
//! Pure offline mixer for the recorder. `audio.rs` turns captured playbacks
//! into `Voice`s and decoded `Clip`s; `render` mixes them to interleaved
//! stereo at `out_rate`, `limit` soft-limits, `wav_bytes` encodes 16-bit PCM.
//! Gains interpolate only between keys on consecutive frames (a fade); a key
//! after a gap is a step. Speed changes pitch and tempo together, as rodio
//! does. No Bevy types, so everything here is unit-tested.

#[cfg(test)]
mod tests {
    use super::*;

    fn mono(samples: Vec<f32>) -> Clip {
        Clip { channels: 1, rate: 100, samples }
    }
    fn key(frame: f64, gain: f32) -> Key {
        Key { frame, gain_l: gain, gain_r: gain, speed: 1.0, paused: false }
    }
    fn voice(start: f64, keys: Vec<Key>) -> Voice {
        Voice { clip: 0, start, end: None, looping: false, downmix: false, keys }
    }
    fn ramp(n: usize) -> Vec<f32> {
        (0..n).map(|i| i as f32 / 1000.0).collect()
    }
    // fps 10, out_rate 100: 10 output samples per video frame.

    #[test]
    fn constant_gain_voice_plays_once_then_silence() {
        let out = render(&[voice(0.0, vec![key(0.0, 0.5)])], &[mono(vec![1.0; 100])], 10, 100, 20);
        assert_eq!(out.len(), 400);
        assert_eq!((out[0], out[1]), (0.5, 0.5));
        assert_eq!(out[99 * 2], 0.5);
        assert_eq!(out[100 * 2], 0.0);
    }

    #[test]
    fn speed_two_halves_the_rendered_length() {
        let mut k = key(0.0, 1.0);
        k.speed = 2.0;
        let out = render(&[voice(0.0, vec![k])], &[mono(vec![1.0; 100])], 10, 100, 20);
        assert_eq!(out[49 * 2], 1.0);
        assert_eq!(out[50 * 2], 0.0);
    }

    #[test]
    fn paused_span_holds_the_playhead_and_is_silent() {
        let mut p = key(2.0, 1.0);
        p.paused = true;
        let keys = vec![key(0.0, 1.0), p, key(4.0, 1.0)];
        let out = render(&[voice(0.0, keys)], &[mono(ramp(100))], 10, 100, 10);
        assert_eq!(out[19 * 2], ramp(100)[19]);
        assert_eq!(out[25 * 2], 0.0);
        assert_eq!(out[40 * 2], ramp(100)[20]);
    }

    #[test]
    fn looping_voice_wraps_until_its_end_frame() {
        let mut v = voice(0.0, vec![key(0.0, 1.0)]);
        v.looping = true;
        v.end = Some(3.0);
        let out = render(&[v], &[mono(ramp(10))], 10, 100, 10);
        assert_eq!(out[25 * 2], ramp(10)[5]);
        assert_eq!(out[30 * 2], 0.0);
    }

    #[test]
    fn negative_start_trims_instead_of_shifting() {
        let out = render(&[voice(-1.0, vec![key(-1.0, 1.0)])], &[mono(ramp(100))], 10, 100, 10);
        assert_eq!(out[0], ramp(100)[10]);
    }

    #[test]
    fn consecutive_keys_fade_but_gapped_keys_step() {
        let fade = render(&[voice(0.0, vec![key(0.0, 0.0), key(1.0, 1.0)])], &[mono(vec![1.0; 100])], 10, 100, 10);
        assert!((fade[5 * 2] - 0.5).abs() < 1e-6);
        let step = render(&[voice(0.0, vec![key(0.0, 0.0), key(5.0, 1.0)])], &[mono(vec![1.0; 100])], 10, 100, 10);
        assert_eq!(step[30 * 2], 0.0);
        assert_eq!(step[50 * 2], 1.0);
    }

    #[test]
    fn stereo_maps_left_right_and_downmix_averages() {
        let clip = Clip { channels: 2, rate: 100, samples: [1.0, 0.0].repeat(100) };
        let plain = render(&[voice(0.0, vec![key(0.0, 1.0)])], &[clip.clone()], 10, 100, 1);
        assert_eq!((plain[0], plain[1]), (1.0, 0.0));
        let mut v = voice(0.0, vec![key(0.0, 1.0)]);
        v.downmix = true;
        let mixed = render(&[v], &[clip], 10, 100, 1);
        assert_eq!((mixed[0], mixed[1]), (0.5, 0.5));
    }

    #[test]
    fn spatial_gains_match_rodio_formula() {
        let (l, r) = spatial_gains([0.0, 0.0, 2.0], [-1.0, 0.0, 0.0], [1.0, 0.0, 0.0]);
        assert!((l - 0.15).abs() < 1e-6 && (r - 0.15).abs() < 1e-6);
        let (l, r) = spatial_gains([3.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [1.0, 0.0, 0.0]);
        assert!((l - 0.0625).abs() < 1e-6, "{l}");
        assert!((r - 0.125).abs() < 1e-6, "{r}");
    }

    #[test]
    fn limiter_caps_below_ceiling_and_reports_peak() {
        let mut buf = [0.5, 2.0, -3.0];
        let peak = limit(&mut buf);
        assert_eq!(peak, 3.0);
        assert_eq!(buf[0], 0.5);
        assert!(buf[1] > KNEE && buf[1] < CEILING);
        assert!(buf[2] < -KNEE && buf[2] > -CEILING);
    }

    #[test]
    fn wav_bytes_writes_a_pcm16_stereo_header() {
        let b = wav_bytes(&[0.5, -0.5, 0.0, 0.0], 48_000);
        assert_eq!(&b[0..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(b[4..8].try_into().unwrap()), 36 + 8);
        assert_eq!(&b[8..16], b"WAVEfmt ");
        assert_eq!(u16::from_le_bytes([b[22], b[23]]), 2);
        assert_eq!(u32::from_le_bytes(b[24..28].try_into().unwrap()), 48_000);
        assert_eq!(&b[36..40], b"data");
        assert_eq!(b.len(), 44 + 8);
        assert_eq!(i16::from_le_bytes([b[44], b[45]]), 16384);
    }
}
```

Add `mod mixer;` to `src/game/record/mod.rs` (next to the other declarations at the top, below the doc comment).

Run: `cargo test --all-features record::mixer` → compile errors (items not defined).

- [ ] **Step 2: Implement**

Above the test module:

```rust
/// A decoded sound: interleaved samples at `rate` Hz, `channels` wide.
#[derive(Clone, Debug, PartialEq)]
pub struct Clip {
    pub channels: u16,
    pub rate: u32,
    pub samples: Vec<f32>,
}

impl Clip {
    fn frames(&self) -> usize {
        if self.channels == 0 {
            0
        } else {
            self.samples.len() / usize::from(self.channels)
        }
    }

    /// (left, right) at fractional source frame `pos`, linearly interpolated.
    /// Mono feeds both ears; stereo maps L/R (extra channels ignored);
    /// `downmix` averages every channel into both.
    fn stereo_at(&self, pos: f64, downmix: bool) -> (f32, f32) {
        let n = self.frames();
        let ch = usize::from(self.channels);
        let i0 = pos.floor() as usize;
        if n == 0 || i0 >= n {
            return (0.0, 0.0);
        }
        let i1 = (i0 + 1).min(n - 1);
        let frac = (pos - i0 as f64) as f32;
        let at = |i: usize| -> (f32, f32) {
            let s = &self.samples[i * ch..i * ch + ch];
            if downmix || ch == 1 {
                let m = s.iter().sum::<f32>() / ch as f32;
                (m, m)
            } else {
                (s[0], s[1])
            }
        };
        let (l0, r0) = at(i0);
        let (l1, r1) = at(i1);
        (l0 + (l1 - l0) * frac, r0 + (r1 - r0) * frac)
    }
}

/// Automation at a video frame (may be negative: before numbered frame 1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Key {
    pub frame: f64,
    pub gain_l: f32,
    pub gain_r: f32,
    pub speed: f32,
    pub paused: bool,
}

/// One playback: which clip, when it began and stopped (video frames), and
/// its automation, sorted by frame with the first key at `start`.
#[derive(Clone, Debug, PartialEq)]
pub struct Voice {
    pub clip: usize,
    pub start: f64,
    pub end: Option<f64>,
    pub looping: bool,
    pub downmix: bool,
    pub keys: Vec<Key>,
}

pub fn render(voices: &[Voice], clips: &[Clip], fps: u32, out_rate: u32, total_frames: u64) -> Vec<f32> {
    let spf = f64::from(out_rate) / f64::from(fps);
    let total = (total_frames as f64 * spf).round() as i64;
    let mut out = vec![0.0f32; total.max(0) as usize * 2];
    for v in voices {
        let Some(clip) = clips.get(v.clip) else { continue };
        let n = clip.frames() as f64;
        if n == 0.0 || v.keys.is_empty() {
            continue;
        }
        let step = f64::from(clip.rate) / f64::from(out_rate);
        let end = v.end.map_or(total, |e| (e * spf).round() as i64).min(total);
        let mut s = (v.start * spf).round() as i64;
        let mut pos = 0.0f64;
        let mut k = 0usize;
        while s < end {
            let f = s as f64 / spf;
            while k + 1 < v.keys.len() && v.keys[k + 1].frame <= f {
                k += 1;
            }
            let key = v.keys[k];
            if !key.paused {
                if s >= 0 {
                    let (gl, gr) = match v.keys.get(k + 1) {
                        Some(next) if next.frame > key.frame && next.frame - key.frame <= 1.0 + 1e-9 => {
                            let a = ((f - key.frame) / (next.frame - key.frame)).clamp(0.0, 1.0) as f32;
                            (
                                key.gain_l + (next.gain_l - key.gain_l) * a,
                                key.gain_r + (next.gain_r - key.gain_r) * a,
                            )
                        }
                        _ => (key.gain_l, key.gain_r),
                    };
                    let (l, r) = clip.stereo_at(pos, v.downmix);
                    let i = s as usize * 2;
                    out[i] += l * gl;
                    out[i + 1] += r * gr;
                }
                pos += f64::from(key.speed) * step;
                if pos >= n {
                    if v.looping {
                        pos %= n;
                    } else {
                        break;
                    }
                }
            }
            s += 1;
        }
    }
    out
}

pub const KNEE: f32 = 0.9;
pub const CEILING: f32 = 0.99;

/// Soft-knee limiter above `KNEE`, asymptotic to `CEILING`. Returns the
/// pre-limit peak magnitude.
pub fn limit(buf: &mut [f32]) -> f32 {
    let range = CEILING - KNEE;
    let mut peak = 0.0f32;
    for x in buf.iter_mut() {
        let a = x.abs();
        peak = peak.max(a);
        if a > KNEE {
            let over = a - KNEE;
            *x = (KNEE + range * (over / (over + range))).copysign(*x);
        }
    }
    peak
}

/// 16-bit PCM stereo WAV.
pub fn wav_bytes(stereo: &[f32], rate: u32) -> Vec<u8> {
    let data_len = (stereo.len() * 2) as u32;
    let mut b = Vec::with_capacity(44 + data_len as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&rate.to_le_bytes());
    b.extend_from_slice(&(rate * 4).to_le_bytes());
    b.extend_from_slice(&4u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    for s in stereo {
        let v = (s.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16;
        b.extend_from_slice(&v.to_le_bytes());
    }
    b
}

/// Per-ear gains of a spatial emitter, exactly as rodio 0.20's
/// `Spatial::set_positions` computes them (what Bevy's `SpatialAudioSink`
/// plays): a left/right difference term times `min(1, 1/dist²)` per ear.
pub fn spatial_gains(emitter: [f32; 3], left_ear: [f32; 3], right_ear: [f32; 3]) -> (f32, f32) {
    let d2 = |a: [f32; 3], b: [f32; 3]| (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2);
    let (l2, r2) = (d2(left_ear, emitter), d2(right_ear, emitter));
    let max_diff = d2(left_ear, right_ear).sqrt();
    let (ldist, rdist) = ((1.0 / l2).min(1.0), (1.0 / r2).min(1.0));
    if max_diff == 0.0 {
        return (ldist, rdist);
    }
    let (l, r) = (l2.sqrt(), r2.sqrt());
    let ld = (((l - r) / max_diff + 1.0) / 4.0 + 0.5).min(1.0);
    let rd = (((r - l) / max_diff + 1.0) / 4.0 + 0.5).min(1.0);
    (ld * ldist, rd * rdist)
}
```

Until Task 3 uses them, add `#![allow(dead_code)]` at the top of `mixer.rs` with the comment `// Wired up by audio.rs (next task).` so clippy stays clean; Task 3 removes it.

- [ ] **Step 3: Verify**

Run: `cargo test --all-features record::mixer && cargo clippy --all-targets --all-features -- -D warnings && cargo fmt --all -- --check`
Expected: 10 mixer tests pass; clean.

- [ ] **Step 4: Commit**

```bash
git add src/game/record/mixer.rs src/game/record/mod.rs
git commit -m "feat(record): pure offline audio mixer

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 3: Audio capture, mix at exit, template smoke run

**Files:**
- Create: `src/game/record/audio.rs`
- Modify: `src/game/record/mod.rs` (module decl, plugin wiring, doc), `src/game/record/mixer.rs` (remove the temporary allow)

**Interfaces:**
- Consumes: Task 1 `Recorder.{sim_frame, first_video_sim_frame, audio, saved, fps, dir}`; Task 2 mixer API.
- Produces: `pub struct AudioCapture` (Resource, Default), `pub fn capture_audio(...)`, `pub fn finish_audio(...)`, `pub const OUT_RATE: u32 = 48_000`; `audio.wav` in `$RECORD_DIR`.

- [ ] **Step 1: Write `audio.rs`**

```rust
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
        .map(|(tf, l)| (tf.transform_point(l.left_ear_offset), tf.transform_point(l.right_ear_offset)))
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
    let gone: Vec<Entity> = cap.live.keys().filter(|e| !alive.contains(e)).copied().collect();
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
            Some((s.volume().to_linear(), s.speed(), s.is_paused(), s.is_muted(), s.empty()))
        } else {
            spatial_sink.map(|s| (s.volume().to_linear(), s.speed(), s.is_paused(), s.is_muted(), s.empty()))
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
    Clip { channels, rate, samples }
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
                let Some(source) = sources.get(t.source) else { continue };
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
                .map(|&(f, gain_l, gain_r, speed, paused)| Key { frame: to_video(f), gain_l, gain_r, speed, paused })
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
    fs::write(rec.dir.join("audio.wav"), mixer::wav_bytes(&buf, OUT_RATE)).expect("record: write audio.wav");
    rec.audio = Some((voices.len(), peak));
    info!("record: mixed {} voices from {} sources, peak {peak:.3}", voices.len(), clips.len());
}
```

If `decoder.map(|s| f32::from(s) …)` does not compile because the item type is not concretely `i16` to the compiler, use `bevy::audio::Sample` and `s.to_f32()` instead (rodio's `Sample` trait is re-exported as `bevy::audio::Sample`).

- [ ] **Step 2: Wire it**

In `record/mod.rs`: add `mod audio;` beside `mod mixer;`. In `RecordPlugin::build`, add `.init_resource::<audio::AudioCapture>()` and replace `.add_systems(Last, finish_on_exit)` with:

```rust
        .add_systems(
            Last,
            (audio::capture_audio, audio::finish_audio, finish_on_exit).chain(),
        )
```

Remove the temporary `#![allow(dead_code)]` from `mixer.rs`. Extend the module doc of `record/mod.rs` with one paragraph: "Audio: `audio.rs` captures every `AudioPlayer` playback and mixes it into `audio.wav` at exit (`mixer.rs`); the manifest's `audio` field reports voices and pre-limit peak, or `null` when nothing played."

- [ ] **Step 3: Verify build and tests**

Run: `cargo clippy --all-targets --all-features -- -D warnings && cargo clippy --all-targets --features autopilot -- -D warnings && cargo test --all-targets --all-features && cargo fmt --all -- --check`
Expected: clean; all tests pass (the 10 recorder, 10 mixer, plus existing).

- [ ] **Step 4: Smoke-run the template**

Run in the foreground: `rm -rf build/rec-smoke && RECORD_DIR=build/rec-smoke timeout 900 cargo run --release --features record` (Bash timeout 960000).
Then:
```bash
grep -o '"audio":[^}]*}' build/rec-smoke/manifest.json
ffprobe -v error -show_entries stream=codec_name,sample_rate,channels -show_entries format=duration -of compact build/rec-smoke/audio.wav
grep -c SfxEvent build/rec-smoke/events.jsonl
ffmpeg -y -loglevel error -i build/rec-smoke/audio.wav -filter_complex "showwavespic=s=1600x300:split_channels=1" -frames:v 1 build/rec-smoke/wave.png
```
Expected: `"audio":{"voices":N,"peak":P}` with N ≥ the SfxEvent count and P > 0; `pcm_s16le`, 48000, 2 channels, duration within 0.1 s of `frames/60`. Read `build/rec-smoke/wave.png` as an image: spikes exist and their count and spacing are consistent with the `SfxEvent` lines' `t` values. Then `rm -rf build/rec-smoke`.

- [ ] **Step 5: Commit**

```bash
git add src/game/record
git commit -m "feat(record): capture every playback and mix audio.wav at exit

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 4: Audio in tour and clips, vertical outputs

**Files:**
- Modify: `tools/record.sh`, `tools/cut_clips.py`, `tools/test_cut_clips.py`, `README.md` (Footage section), `AGENTS.md` (Footage bullet)
- Create: `tools/vertical-banner.svg`

**Interfaces:**
- Consumes: `audio.wav`, `manifest.json` (`name`, `beats`, `fps`, `duration_s`).
- Produces: `tour.mp4` with AAC; `clips/<beat>.mp4` with AAC when the tour has audio; `banner.png`, `clips/vertical/<beat>.mp4`, `tour-vertical.mp4`; Python helpers `title_size(title, width=900, cap=96) -> int`, `banner_svg(template: str, title: str, cta: tuple[str, str]) -> str`, `vertical_filter() -> str`, `vertical_args(src, banner, out) -> list[str]`, `clip_args(tour, start, length, out) -> list[str]`, constant `CTA = ("Play the demo at", "colecovisiongx.com")`.

- [ ] **Step 1: Failing Python tests**

Append to `tools/test_cut_clips.py` (and extend its import line with `CTA, banner_svg, clip_args, title_size, vertical_args, vertical_filter`):

```python
class VerticalTests(unittest.TestCase):
    def test_title_size_caps_short_titles_and_shrinks_long_ones(self):
        self.assertEqual(title_size("GULPER"), 96)
        self.assertEqual(title_size("GRAND THEFT AUTO-REPLY"), 51)

    def test_banner_svg_fills_and_escapes(self):
        tpl = "<t s='{{TITLE_SIZE}}'>{{TITLE}}</t><a>{{CTA_1}}</a><b>{{CTA_2}}</b>"
        out = banner_svg(tpl, "Dough & <Co>", CTA)
        self.assertIn("DOUGH &amp; &lt;CO&gt;", out)
        self.assertIn("s='96'", out)
        self.assertIn("<a>Play the demo at</a><b>colecovisiongx.com</b>", out)
        self.assertNotIn("{{", out)

    def test_vertical_filter_blurs_background_and_centers_gameplay(self):
        f = vertical_filter()
        self.assertIn("crop=1080:1920", f)
        self.assertIn("gblur", f)
        self.assertIn("eq=brightness=-0.12", f)
        self.assertIn("[fg]scale=1080:-2", f)
        self.assertIn("overlay=(W-w)/2:(H-h)/2", f)
        self.assertTrue(f.endswith("[base][1:v]overlay=0:0[v]"))

    def test_vertical_args_map_optional_audio(self):
        a = vertical_args("in.mp4", "banner.png", "out.mp4")
        self.assertEqual(a[:4], ["-i", "in.mp4", "-i", "banner.png"])
        self.assertIn("0:a?", a)
        self.assertEqual(a[-1], "out.mp4")

    def test_clip_args_keep_audio(self):
        a = clip_args("tour.mp4", 8.0, 6.0, "c.mp4")
        self.assertNotIn("-an", a)
        self.assertIn("0:a?", a)
        self.assertIn("aac", a)
        self.assertEqual(a[:6], ["-ss", "8.000", "-i", "tour.mp4", "-t", "6.000"])
```

Run: `cd tools && python3 -m unittest test_cut_clips` → ImportError.

- [ ] **Step 2: Implement the helpers in `cut_clips.py`**

Add `import xml.sax.saxutils` and below `still_time`:

```python
CTA = ("Play the demo at", "colecovisiongx.com")
BANNER_TEMPLATE = pathlib.Path(__file__).with_name("vertical-banner.svg")


def title_size(title, width=900, cap=96):
    """Font size that fits `title` (Arial Black, ~0.8 em per glyph) in `width` px."""
    return min(cap, int(width / (0.8 * max(1, len(title)))))


def banner_svg(template, title, cta):
    title = title.upper()
    esc = xml.sax.saxutils.escape
    return (template.replace("{{TITLE}}", esc(title))
            .replace("{{TITLE_SIZE}}", str(title_size(title)))
            .replace("{{CTA_1}}", esc(cta[0]))
            .replace("{{CTA_2}}", esc(cta[1])))


def vertical_filter():
    return ("[0:v]split=2[bg][fg];"
            "[bg]scale=1080:1920:force_original_aspect_ratio=increase,crop=1080:1920,"
            "scale=270:480,gblur=sigma=8,scale=1080:1920,eq=brightness=-0.12[bgb];"
            "[fg]scale=1080:-2[fgs];"
            "[bgb][fgs]overlay=(W-w)/2:(H-h)/2[base];"
            "[base][1:v]overlay=0:0[v]")


def vertical_args(src, banner, out):
    return ["-i", str(src), "-i", str(banner), "-filter_complex", vertical_filter(),
            "-map", "[v]", "-map", "0:a?", "-c:v", "libx264", "-crf", "20",
            "-pix_fmt", "yuv420p", "-c:a", "copy", str(out)]


def clip_args(tour, start, length, out):
    return ["-ss", f"{start:.3f}", "-i", str(tour), "-t", f"{length:.3f}",
            "-map", "0:v", "-map", "0:a?", "-c:v", "libx264", "-crf", "18",
            "-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "192k", str(out)]
```

In `main`, replace the clip `ffmpeg(...)` call with `ffmpeg(*clip_args(tour, start, length, out))`. After the beat loop:

```python
    if shutil.which("rsvg-convert") and BANNER_TEMPLATE.exists():
        svg = root / "banner.svg"
        svg.write_text(banner_svg(BANNER_TEMPLATE.read_text(encoding="utf-8"), manifest["name"], CTA),
                       encoding="utf-8")
        banner = root / "banner.png"
        subprocess.run(["rsvg-convert", str(svg), "-o", str(banner)], check=True)
        vdir = clips_dir / "vertical"
        vdir.mkdir(exist_ok=True)
        for beat in manifest["beats"]:
            name = beat["name"]
            ffmpeg(*vertical_args(clips_dir / f"{name}.mp4", banner, vdir / f"{name}.mp4"))
        ffmpeg(*vertical_args(tour, banner, root / "tour-vertical.mp4"))
        print(f"cut_clips: vertical 9:16 -> {vdir} and tour-vertical.mp4")
    else:
        print("cut_clips: rsvg-convert or vertical-banner.svg missing; skipping vertical outputs")
```

Also, before `chapters.md` is written, append to `written` one line per beat `clips/vertical/<beat>.mp4` and `tour-vertical.mp4` when `shutil.which("rsvg-convert") and BANNER_TEMPLATE.exists()`. Update the module docstring's output list with `banner.png`, `clips/vertical/<beat>.mp4`, `tour-vertical.mp4`, and note that clips carry the tour's audio.

- [ ] **Step 3: The banner template**

`tools/vertical-banner.svg`:

```svg
<svg xmlns="http://www.w3.org/2000/svg" width="1080" height="1920" viewBox="0 0 1080 1920">
  <!-- 9:16 clip banner, filled by tools/cut_clips.py ({{TITLE}}, {{TITLE_SIZE}},
       {{CTA_1}}, {{CTA_2}}) and rendered with rsvg-convert. Transparent except
       the two plates; the 16:9 gameplay is overlaid underneath at y 656-1264. -->
  <rect x="60" y="440" width="960" height="170" rx="28" fill="#000000" fill-opacity="0.55"/>
  <text x="540" y="555" text-anchor="middle" font-family="Arial Black, Helvetica Neue, Arial, sans-serif"
        font-weight="900" font-size="{{TITLE_SIZE}}" fill="#ffffff">{{TITLE}}</text>
  <rect x="120" y="1320" width="840" height="200" rx="28" fill="#000000" fill-opacity="0.55"/>
  <text x="540" y="1402" text-anchor="middle" font-family="Helvetica Neue, Arial, sans-serif"
        font-weight="bold" font-size="46" fill="#e8eef7">{{CTA_1}}</text>
  <text x="540" y="1478" text-anchor="middle" font-family="Helvetica Neue, Arial, sans-serif"
        font-weight="bold" font-size="58" fill="#ffd23f">{{CTA_2}}</text>
</svg>
```

- [ ] **Step 4: Mux audio in `record.sh`**

Replace the encode block with:

```bash
AUDIO="$RECORD_DIR/audio.wav"
if [ -f "$AUDIO" ]; then
  echo "record.sh: encoding $frames frames + audio"
  ffmpeg -y -loglevel error -framerate 60 -pattern_type glob -i "$RECORD_DIR/frames/*.png" -i "$AUDIO" \
    -c:v libx264 -crf 18 -pix_fmt yuv420p \
    -af loudnorm=I=-16:TP=-1.5:LRA=11 -ar 48000 -c:a aac -b:a 192k -shortest "$RECORD_DIR/tour.mp4"
else
  echo "record.sh: encoding $frames frames (no audio.wav: nothing played)"
  ffmpeg -y -loglevel error -framerate 60 -pattern_type glob -i "$RECORD_DIR/frames/*.png" \
    -c:v libx264 -crf 18 -pix_fmt yuv420p "$RECORD_DIR/tour.mp4"
fi
```
Update the header comment's output list (audio in the tour and clips; vertical outputs).

- [ ] **Step 5: Docs**

README "Footage (record)": say the tour and clips carry the game's mixed audio (normalized to −16 LUFS), and list `clips/vertical/<beat>.mp4` and `tour-vertical.mp4` (1080x1920, blurred fill, title and "Play the demo at colecovisiongx.com" banners; needs `rsvg-convert`). AGENTS.md Footage bullet: one sentence on `record/audio.rs` capturing every `AudioPlayer` and mixing `audio.wav` at exit, and one on the vertical outputs.

- [ ] **Step 6: Verify**

Run: `cd tools && python3 -m unittest test_cut_clips -v` (17 OK), `bash -n tools/record.sh`, then end to end in the foreground `timeout 1500 tools/record.sh` (Bash timeout 1500000). Check:

```bash
ffprobe -v error -show_entries stream=codec_type,codec_name,width,height,sample_rate -of compact build/record/tour.mp4
for f in build/record/clips/0*.mp4; do ffprobe -v error -select_streams a -show_entries stream=codec_name -of csv=p=0 "$f"; done | sort | uniq -c
ls build/record/clips/vertical | wc -l
ffprobe -v error -select_streams v -show_entries stream=width,height -of csv=p=0 build/record/tour-vertical.mp4
ffmpeg -y -loglevel error -ss 30 -i build/record/tour-vertical.mp4 -frames:v 1 build/record/vertical-30s.png
```
Expected: tour has an h264 1920x1080 stream and an aac 48000 stream; every clip reports `aac`; vertical count equals the beat count; `tour-vertical.mp4` is `1080,1920`. Read `build/record/vertical-30s.png`: blurred background, gameplay band centered, title plate above, CTA plate below, no text clipped.

- [ ] **Step 7: Commit**

```bash
git add tools/record.sh tools/cut_clips.py tools/test_cut_clips.py tools/vertical-banner.svg README.md AGENTS.md
git commit -m "feat(record): audio in tour and clips, 9:16 vertical outputs

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 5: Rollout script update and first game (Tire Stack)

**Files:**
- Modify: `tools/rollout-record.sh`
- In `games/tire-stack`: replace `src/game/record.rs` with `src/game/record/{mod,audio,mixer}.rs`; update `tools/{record.sh,cut_clips.py,test_cut_clips.py}`; add `tools/vertical-banner.svg`

**Interfaces:**
- Produces: `rollout-record.sh` copies the folder module and banner; idempotent; still prints `HAND EDIT:` lines for irregular layouts.

- [ ] **Step 1: Update the copy block**

Replace the two lines that copy `record.rs` and the tools with:

```bash
mkdir -p "$GAME/src/game/record"
rm -f "$GAME/src/game/record.rs"
cp "$TEMPLATE"/src/game/record/*.rs "$GAME/src/game/record/"
mkdir -p "$GAME/tools"
cp "$TEMPLATE/tools/record.sh" "$TEMPLATE/tools/cut_clips.py" "$TEMPLATE/tools/test_cut_clips.py" \
   "$TEMPLATE/tools/vertical-banner.svg" "$GAME/tools/"
chmod +x "$GAME/tools/record.sh"
```

If `$GAME/src/record.rs` exists (flat layout, e.g. Hunted), print:
`HAND EDIT: flat layout — move src/game/record/ to src/record/, delete src/record.rs and the now-empty src/game/`.
Update the Cargo feature comment the script inserts to say `src/game/record/`. Update the header comment.

- [ ] **Step 2: Verify the script on a scratch copy**

```bash
bash -n tools/rollout-record.sh
S=$(mktemp -d) && git -C ../../games/tire-stack archive HEAD | tar -x -C "$S"
tools/rollout-record.sh "$S" && ls "$S/src/game/record" && test ! -e "$S/src/game/record.rs"
tools/rollout-record.sh "$S" && echo idempotent
rm -rf "$S"
```
Expected: `audio.rs mixer.rs mod.rs`, no `record.rs`, second run changes nothing and prints no `HAND EDIT`.

- [ ] **Step 3: Roll out to Tire Stack and record**

```bash
tools/rollout-record.sh ../../games/tire-stack
cd ../../games/tire-stack
git status --short
cargo clippy --all-targets --all-features -- -D warnings && cargo test --all-features record:: && (cd tools && python3 -m unittest test_cut_clips)
timeout 1500 tools/record.sh
```
Run the Task 4 Step 6 checks in this game, plus `grep -o '"audio":[^}]*}' build/record/manifest.json` (peak > 0). Read `build/record/vertical-30s.png` (extract it as in Task 4) — the title reads "TIRE STACK".

- [ ] **Step 4: Commit both repos**

```bash
cd libs/gamebient-bevy-template && git add tools/rollout-record.sh && git commit -m "feat(record): rollout copies the record folder module and banner

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
cd ../../games/tire-stack && git add -A src/game tools && git commit -m "feat(record): audio capture and vertical clips from the template

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 6: Skill into the template, reel tool

**Files:**
- Move: workspace `.claude/skills/recording-game-footage/` → template `.claude/skills/recording-game-footage/`; workspace path becomes a symlink `../../libs/gamebient-bevy-template/.claude/skills/recording-game-footage`
- Create in the skill: `tools/reel.py`, `tools/test_reel.py`, `tools/card-16x9.svg`, `tools/card-9x16.svg`, `tools/end-16x9.svg`, `tools/end-9x16.svg`
- Modify in the skill: `SKILL.md`, `references/irregular-games.md`

**Interfaces:**
- Produces: `python3 .claude/skills/recording-game-footage/tools/reel.py [--root DIR] [--init] [--aspect 16x9|9x16|both]`; config `docs/reel.json`; outputs `build/reel/reel-16x9.mp4`, `build/reel/reel-9x16.mp4` under `--root` (default cwd). Helpers: `fit_size(text, width, cap) -> int`, `parse_index(markdown) -> list[dict]`, `init_config(markdown) -> dict`, `load_config(text) -> dict`, `resolve_clip(clip, beat_names) -> str`, `fill(template, **values) -> str`, `card_args(png, secs, size, out) -> list`, `clip_args(src, start, length, size, has_audio, out) -> list`, `concat_list(paths) -> str`.

- [ ] **Step 1: Move the skill**

```bash
W=/Users/kelliott/Gamebient/colecovisiongx
T=$W/libs/gamebient-bevy-template
mv "$W/.claude/skills/recording-game-footage" "$T/.claude/skills/recording-game-footage"
ln -s ../../libs/gamebient-bevy-template/.claude/skills/recording-game-footage "$W/.claude/skills/recording-game-footage"
ls -la "$W/.claude/skills/" && test -f "$W/.claude/skills/recording-game-footage/SKILL.md"
```

- [ ] **Step 2: Failing tests** — `tools/test_reel.py`:

```python
import unittest

from reel import (DEFAULTS, card_args, clip_args, concat_list, fill, fit_size,
                  init_config, load_config, parse_index, resolve_clip)

INDEX = """# Video notes index

| Game | Folder | Tour length | Lead clip | Hook (one sentence) | Top payoff not reached by the tour |
|---|---|---|---|---|---|
| Tire Stack | tire-stack | 01:02.48 | `clips/06-delivery-popup.mp4` | Tires drop. | A topple. |
| Pack The Ripper | pack-the-ripper | 01:02.02 | `clips/signature.mp4` (= `06-pack-burst.mp4`) | Rip. | Chase card. |
"""


class IndexTests(unittest.TestCase):
    def test_parse_index_reads_folder_and_first_clip(self):
        self.assertEqual(parse_index(INDEX), [
            {"folder": "tire-stack", "clip": "06-delivery-popup"},
            {"folder": "pack-the-ripper", "clip": "signature"},
        ])

    def test_init_config_applies_defaults(self):
        cfg = init_config(INDEX)
        self.assertEqual(cfg["cta"], ["Play the demo at", "colecovisiongx.com"])
        self.assertEqual(cfg["card_secs"], 1.5)
        self.assertEqual(cfg["end_secs"], 3.0)
        self.assertEqual(cfg["games"][0], {"folder": "tire-stack", "clip": "06-delivery-popup",
                                           "start": 1.0, "length": 4.5})


class ConfigTests(unittest.TestCase):
    def test_load_config_fills_missing_fields(self):
        cfg = load_config('{"games": [{"folder": "gulper", "clip": "06-digest", "length": 3.0}]}')
        self.assertEqual(cfg["card_secs"], DEFAULTS["card_secs"])
        self.assertEqual(cfg["games"][0]["start"], 1.0)
        self.assertEqual(cfg["games"][0]["length"], 3.0)

    def test_resolve_clip_maps_aliases(self):
        beats = ["01-studio-logo", "06-pack-burst", "09-game-over"]
        self.assertEqual(resolve_clip("signature", beats), "06-pack-burst")
        self.assertEqual(resolve_clip("game-over", beats), "09-game-over")
        self.assertEqual(resolve_clip("01-studio-logo", beats), "01-studio-logo")
        with self.assertRaises(ValueError):
            resolve_clip("07-late-play", beats)


class RenderingTests(unittest.TestCase):
    def test_fit_size(self):
        self.assertEqual(fit_size("GULPER", 820, 110), 110)
        self.assertEqual(fit_size("GRAND THEFT AUTO-REPLY", 820, 110), 46)

    def test_fill_escapes_and_replaces_every_slot(self):
        out = fill("<t>{{TITLE}}</t><s>{{TITLE_SIZE}}</s>", TITLE="A & B", TITLE_SIZE=40)
        self.assertEqual(out, "<t>A &amp; B</t><s>40</s>")

    def test_card_args_add_silent_audio_and_fixed_format(self):
        a = card_args("card.png", 1.5, (1920, 1080), "seg.mp4")
        self.assertEqual(a[:6], ["-loop", "1", "-t", "1.500", "-i", "card.png"])
        self.assertIn("anullsrc=r=48000:cl=stereo", a)
        self.assertIn("scale=1920:1080,fps=60,format=yuv420p", a)
        self.assertEqual(a[-1], "seg.mp4")

    def test_clip_args_trim_fade_and_normalize(self):
        a = clip_args("c.mp4", 1.0, 4.5, (1080, 1920), True, "seg.mp4")
        self.assertEqual(a[:6], ["-ss", "1.000", "-t", "4.500", "-i", "c.mp4"])
        af = a[a.index("-af") + 1]
        self.assertIn("afade=t=in:d=0.15", af)
        self.assertIn("afade=t=out:st=4.350:d=0.15", af)
        self.assertIn("loudnorm=I=-16:TP=-1.5:LRA=11", af)
        self.assertNotIn("anullsrc=r=48000:cl=stereo", a)

    def test_clip_args_without_audio_use_silence(self):
        a = clip_args("c.mp4", 1.0, 4.5, (1920, 1080), False, "seg.mp4")
        self.assertIn("anullsrc=r=48000:cl=stereo", a)
        self.assertIn("1:a", a)

    def test_concat_list_quotes_paths(self):
        self.assertEqual(concat_list(["/a/b.mp4", "/it's.mp4"]),
                         "file '/a/b.mp4'\nfile '/it'\\''s.mp4'\n")


if __name__ == "__main__":
    unittest.main()
```

Run: `cd <skill>/tools && python3 -m unittest test_reel` → ImportError.

- [ ] **Step 3: Implement `tools/reel.py`**

```python
#!/usr/bin/env python3
"""Build the catalog sizzle reel from each game's recording.

Run from the workspace root (or pass --root):
  reel.py --init        write docs/reel.json from docs/video-notes-index.md
  reel.py               build build/reel/reel-16x9.mp4 and reel-9x16.mp4
  reel.py --aspect 9x16 build one aspect only

docs/reel.json: {"cta": [line1, line2], "card_secs": 1.5, "end_secs": 3.0,
"games": [{"folder", "clip", "start", "length"}]}. `clip` is a beat name
(`06-delivery-popup`) or an alias (`signature`, `game-over`). Each game gets
a title card (its assets/cartridge.png + title), then its clip trimmed to
start/length (16:9 from build/record/clips/, 9:16 from clips/vertical/);
the reel ends on a CTA card. Needs ffmpeg, ffprobe, rsvg-convert. Stdlib only.
"""

import argparse
import json
import pathlib
import re
import shutil
import subprocess
import sys
import xml.sax.saxutils

HERE = pathlib.Path(__file__).resolve().parent
DEFAULTS = {"cta": ["Play the demo at", "colecovisiongx.com"], "card_secs": 1.5, "end_secs": 3.0}
GAME_DEFAULTS = {"start": 1.0, "length": 4.5}
ASPECTS = {"16x9": ((1920, 1080), "clips"), "9x16": ((1080, 1920), "clips/vertical")}
FADE = 0.15
LOUDNORM = "loudnorm=I=-16:TP=-1.5:LRA=11"
SILENCE = "anullsrc=r=48000:cl=stereo"
ENCODE = ["-c:v", "libx264", "-crf", "20", "-pix_fmt", "yuv420p",
          "-c:a", "aac", "-b:a", "192k", "-ar", "48000", "-ac", "2"]


def fit_size(text, width, cap):
    return min(cap, int(width / (0.8 * max(1, len(text)))))


def parse_index(markdown):
    rows, header = [], None
    for line in markdown.splitlines():
        if not line.startswith("|"):
            continue
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if header is None:
            header = cells
            continue
        if set("".join(cells)) <= set("-: "):
            continue
        folder = cells[header.index("Folder")]
        m = re.search(r"clips/([A-Za-z0-9_-]+)\.mp4", cells[header.index("Lead clip")])
        if folder and m:
            rows.append({"folder": folder, "clip": m.group(1)})
    return rows


def init_config(markdown):
    return {**DEFAULTS, "games": [{**g, **GAME_DEFAULTS} for g in parse_index(markdown)]}


def load_config(text):
    cfg = {**DEFAULTS, **json.loads(text)}
    cfg["games"] = [{**GAME_DEFAULTS, **g} for g in cfg.get("games", [])]
    return cfg


def resolve_clip(clip, beat_names):
    if clip == "signature":
        found = [b for b in beat_names if b.startswith("06-")]
    elif clip == "game-over":
        found = [b for b in beat_names if b == "09-game-over"]
    else:
        found = [b for b in beat_names if b == clip]
    if not found:
        raise ValueError(f"clip {clip!r} not among beats {beat_names}")
    return found[0]


def fill(template, **values):
    out = template
    for key, value in values.items():
        out = out.replace("{{" + key + "}}", xml.sax.saxutils.escape(str(value)))
    return out


def card_args(png, secs, size, out):
    w, h = size
    return ["-loop", "1", "-t", f"{secs:.3f}", "-i", str(png),
            "-f", "lavfi", "-t", f"{secs:.3f}", "-i", SILENCE,
            "-vf", f"scale={w}:{h},fps=60,format=yuv420p",
            "-map", "0:v", "-map", "1:a", *ENCODE, "-shortest", str(out)]


def clip_args(src, start, length, size, has_audio, out):
    w, h = size
    args = ["-ss", f"{start:.3f}", "-t", f"{length:.3f}", "-i", str(src)]
    if not has_audio:
        args += ["-f", "lavfi", "-t", f"{length:.3f}", "-i", SILENCE]
    af = f"afade=t=in:d={FADE},afade=t=out:st={length - FADE:.3f}:d={FADE},{LOUDNORM}"
    return args + ["-vf", f"scale={w}:{h},fps=60,format=yuv420p", "-af", af,
                   "-map", "0:v", "-map", "0:a" if has_audio else "1:a", *ENCODE, str(out)]


def concat_list(paths):
    return "".join("file '" + str(p).replace("'", "'\\''") + "'\n" for p in paths)


def run(*args):
    subprocess.run(["ffmpeg", "-y", "-loglevel", "error", *args], check=True)


def has_audio(path):
    out = subprocess.run(["ffprobe", "-v", "error", "-select_streams", "a",
                          "-show_entries", "stream=index", "-of", "csv=p=0", str(path)],
                         capture_output=True, text=True, check=True).stdout
    return bool(out.strip())


def render_svg(template_name, work, png_name, **values):
    svg = work / (png_name[:-4] + ".svg")
    svg.write_text(fill((HERE / template_name).read_text(encoding="utf-8"), **values), encoding="utf-8")
    png = work / png_name
    subprocess.run(["rsvg-convert", str(svg), "-o", str(png)], check=True)
    return png


def build(root, cfg, aspect):
    size, clip_dir = ASPECTS[aspect]
    out_dir = root / "build" / "reel"
    work = out_dir / "work" / aspect
    work.mkdir(parents=True, exist_ok=True)
    segments = []
    for i, g in enumerate(cfg["games"]):
        rec = root / "games" / g["folder"] / "build" / "record"
        manifest_path = rec / "manifest.json"
        if not manifest_path.exists():
            sys.exit(f"reel: no recording for {g['folder']}; run tools/record.sh there")
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        beat = resolve_clip(g["clip"], [b["name"] for b in manifest["beats"]])
        title = manifest["name"].upper()
        gwork = work / f"{i:02d}-{g['folder']}"
        gwork.mkdir(exist_ok=True)
        shutil.copyfile(root / "games" / g["folder"] / "assets" / "cartridge.png", gwork / "cover.png")
        card = render_svg(f"card-{aspect}.svg", gwork, "card.png", TITLE=title,
                          TITLE_SIZE=fit_size(title, 820 if aspect == "16x9" else 960, 110))
        seg = gwork / "card.mp4"
        run(*card_args(card, cfg["card_secs"], size, seg))
        segments.append(seg)
        src = rec / clip_dir / f"{beat}.mp4"
        if not src.exists():
            sys.exit(f"reel: {src} missing; re-run tools/record.sh in {g['folder']}")
        seg = gwork / "clip.mp4"
        run(*clip_args(src, g["start"], g["length"], size, has_audio(src), seg))
        segments.append(seg)
    end = render_svg(f"end-{aspect}.svg", work, "end.png", COUNT=f"{len(cfg['games'])} GAMES",
                     CTA_1=cfg["cta"][0], CTA_2=cfg["cta"][1])
    seg = work / "end.mp4"
    run(*card_args(end, cfg["end_secs"], size, seg))
    segments.append(seg)
    listing = work / "segments.txt"
    listing.write_text(concat_list(segments), encoding="utf-8")
    out = out_dir / f"reel-{aspect}.mp4"
    run("-f", "concat", "-safe", "0", "-i", str(listing), "-c", "copy", str(out))
    print(f"reel: {out}")
    return out


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--root", default=".")
    p.add_argument("--init", action="store_true")
    p.add_argument("--aspect", choices=["16x9", "9x16", "both"], default="both")
    args = p.parse_args(argv)
    root = pathlib.Path(args.root).resolve()
    config_path = root / "docs" / "reel.json"
    if args.init:
        index = root / "docs" / "video-notes-index.md"
        if not index.exists():
            sys.exit(f"reel: {index} not found")
        config_path.write_text(json.dumps(init_config(index.read_text(encoding="utf-8")), indent=2) + "\n",
                               encoding="utf-8")
        print(f"reel: wrote {config_path}; reorder or trim it, then run reel.py")
        return
    if not config_path.exists():
        sys.exit(f"reel: {config_path} not found; run reel.py --init first")
    for tool in ("ffmpeg", "ffprobe", "rsvg-convert"):
        if shutil.which(tool) is None:
            sys.exit(f"reel: {tool} not found on PATH")
    cfg = load_config(config_path.read_text(encoding="utf-8"))
    for aspect in (["16x9", "9x16"] if args.aspect == "both" else [args.aspect]):
        build(root, cfg, aspect)


if __name__ == "__main__":
    main()
```

- [ ] **Step 4: Card and end-card templates**

`tools/card-16x9.svg`:

```svg
<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="1920" height="1080">
  <!-- Reel title card (16:9). reel.py fills {{TITLE}} / {{TITLE_SIZE}}; cover.png is copied beside this file. -->
  <rect width="1920" height="1080" fill="#0b0f1a"/>
  <image x="180" y="110" width="645" height="860" xlink:href="cover.png" href="cover.png"/>
  <text x="960" y="520" font-family="Arial Black, Helvetica Neue, Arial, sans-serif" font-weight="900"
        font-size="{{TITLE_SIZE}}" fill="#ffffff">{{TITLE}}</text>
  <text x="962" y="600" font-family="Helvetica Neue, Arial, sans-serif" font-weight="bold" font-size="40"
        fill="#ffd23f">ColecoVision GX</text>
</svg>
```

`tools/card-9x16.svg`:

```svg
<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="1080" height="1920">
  <!-- Reel title card (9:16). reel.py fills {{TITLE}} / {{TITLE_SIZE}}; cover.png is copied beside this file. -->
  <rect width="1080" height="1920" fill="#0b0f1a"/>
  <image x="225" y="260" width="630" height="840" xlink:href="cover.png" href="cover.png"/>
  <text x="540" y="1330" text-anchor="middle" font-family="Arial Black, Helvetica Neue, Arial, sans-serif"
        font-weight="900" font-size="{{TITLE_SIZE}}" fill="#ffffff">{{TITLE}}</text>
  <text x="540" y="1420" text-anchor="middle" font-family="Helvetica Neue, Arial, sans-serif" font-weight="bold"
        font-size="44" fill="#ffd23f">ColecoVision GX</text>
</svg>
```

`tools/end-16x9.svg`:

```svg
<svg xmlns="http://www.w3.org/2000/svg" width="1920" height="1080">
  <!-- Reel end card (16:9). reel.py fills {{COUNT}}, {{CTA_1}}, {{CTA_2}}. -->
  <rect width="1920" height="1080" fill="#0b0f1a"/>
  <text x="960" y="400" text-anchor="middle" font-family="Arial Black, Helvetica Neue, Arial, sans-serif"
        font-weight="900" font-size="120" fill="#ffffff">{{COUNT}}</text>
  <text x="960" y="560" text-anchor="middle" font-family="Helvetica Neue, Arial, sans-serif" font-weight="bold"
        font-size="64" fill="#e8eef7">{{CTA_1}}</text>
  <text x="960" y="680" text-anchor="middle" font-family="Helvetica Neue, Arial, sans-serif" font-weight="bold"
        font-size="96" fill="#ffd23f">{{CTA_2}}</text>
  <text x="960" y="960" text-anchor="middle" font-family="Helvetica Neue, Arial, sans-serif" font-size="36"
        fill="#8a94a8">Bread Heads Studios · ColecoVision GX</text>
</svg>
```

`tools/end-9x16.svg`:

```svg
<svg xmlns="http://www.w3.org/2000/svg" width="1080" height="1920">
  <!-- Reel end card (9:16). reel.py fills {{COUNT}}, {{CTA_1}}, {{CTA_2}}. -->
  <rect width="1080" height="1920" fill="#0b0f1a"/>
  <text x="540" y="760" text-anchor="middle" font-family="Arial Black, Helvetica Neue, Arial, sans-serif"
        font-weight="900" font-size="110" fill="#ffffff">{{COUNT}}</text>
  <text x="540" y="920" text-anchor="middle" font-family="Helvetica Neue, Arial, sans-serif" font-weight="bold"
        font-size="58" fill="#e8eef7">{{CTA_1}}</text>
  <text x="540" y="1030" text-anchor="middle" font-family="Helvetica Neue, Arial, sans-serif" font-weight="bold"
        font-size="76" fill="#ffd23f">{{CTA_2}}</text>
  <text x="540" y="1700" text-anchor="middle" font-family="Helvetica Neue, Arial, sans-serif" font-size="34"
        fill="#8a94a8">Bread Heads Studios · ColecoVision GX</text>
</svg>
```

- [ ] **Step 5: Probe-render the templates**

In a scratch dir, fill each template with `TITLE="GRAND THEFT AUTO-REPLY"`, the matching `TITLE_SIZE` from `fit_size`, `COUNT="16 GAMES"`, the CTA lines, copy any game's `assets/cartridge.png` beside it as `cover.png`, `rsvg-convert` each, and Read the four PNGs: cover visible on the title cards, no text clipped or falling back to a serif. Delete the scratch dir.

- [ ] **Step 6: Update SKILL.md and irregular-games.md**

SKILL.md:
- Overview: recordings now include a mixed audio track (`audio.wav`, muxed into `tour.mp4` at −16 LUFS) and 9:16 versions (`clips/vertical/<beat>.mp4`, `tour-vertical.mp4`).
- Verify step: add `ffprobe` shows an `aac` stream on `tour.mp4` and every clip; manifest `audio.peak > 0` (a `null` audio field means no `AudioPlayer` played — check the game isn't muting itself under the harness); `ls build/record/clips/vertical | wc -l` equals the beat count; extract and Read one vertical frame.
- New section "Sizzle reel": from the workspace root, `python3 .claude/skills/recording-game-footage/tools/reel.py --init` once (writes `docs/reel.json` from `docs/video-notes-index.md`; reorder, drop, or retime entries there), then `python3 .claude/skills/recording-game-footage/tools/reel.py`; outputs `build/reel/reel-16x9.mp4` and `reel-9x16.mp4`; verify with `ffprobe` durations (≈ 16 × (1.5 + 4.5) + 3 s) and by reading one frame from a card, a clip, and the end card.
- Common mistakes: add "Rolling out over an old `record.rs`" → the script deletes it; "Hunted's flat layout" → `src/record/` (see irregular-games.md).
- Note the skill's canonical home is the template's `.claude/skills/`, symlinked from the workspace root.

irregular-games.md: Hunted's row says the recorder is the folder module `src/record/` (mod.rs, audio.rs, mixer.rs); after `rollout-record.sh`, move `src/game/record/` to `src/record/`, delete `src/record.rs` and the empty `src/game/`.

- [ ] **Step 7: Verify and commit**

Run: `cd .claude/skills/recording-game-footage/tools && python3 -m unittest test_reel -v` (10 OK) and `python3 reel.py --help`.

```bash
cd /Users/kelliott/Gamebient/colecovisiongx/libs/gamebient-bevy-template
git add .claude/skills/recording-game-footage
git commit -m "feat(skills): recording-game-footage skill with the sizzle reel tool

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 7: Roll out to the other 15 games

**Files:** in each of `games/{pizza-pinball, sundae-shooter, pack-the-ripper, ladder-legend, attic-excavator, cannonball-putt, dough-io, gulper, grand-theft-auto-reply, grand-theft-otto, dive-rise, voidrunner, Gravestone_Gauntlet, BeerPong, Hunted}`: `src/game/record/` (or `src/record/` for Hunted) replacing `record.rs`; `tools/*` updated; `tools/vertical-banner.svg` added.

**Interfaces:** Consumes Task 5's script.

- [ ] **Step 1: Run the script on each game**

```bash
cd /Users/kelliott/Gamebient/colecovisiongx
for g in pizza-pinball sundae-shooter pack-the-ripper ladder-legend attic-excavator cannonball-putt dough-io gulper grand-theft-auto-reply grand-theft-otto dive-rise voidrunner Gravestone_Gauntlet BeerPong Hunted; do
  echo "=== $g"; libs/gamebient-bevy-template/tools/rollout-record.sh games/$g
done
```

- [ ] **Step 2: Hunted's flat layout**

```bash
cd games/Hunted
rm -f src/record.rs && rm -rf src/record && mv src/game/record src/record && rmdir src/game
git status --short
```
Expected status: `src/record.rs` deleted; `src/record/{mod,audio,mixer}.rs` added; tools changed.

- [ ] **Step 3: Check each game**

For every game, sequentially: `git diff --stat` touches only the record module, `tools/`, and possibly `Cargo.toml`'s feature comment; then `cargo clippy --all-targets --all-features -- -D warnings && cargo test --all-features record:: && (cd tools && python3 -m unittest test_cut_clips)`. A game with anything else in its diff is reported, not committed.

- [ ] **Step 4: Commit each passing game**

`git add -A src tools Cargo.toml && git commit -m "feat(record): audio capture and vertical clips from the template" -m "Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"`

---

### Task 8: Re-record the catalog

**Files:** each game's `build/record/` (not committed); `docs/video-notes.md` only when its chapters table or lead clips change.

- [ ] **Step 1: One game at a time**

For each of the 15 games other than Tire Stack (recorded in Task 5), in the foreground: `timeout 1500 tools/record.sh`. If the tour ends with fewer than 2000 frames and "No windows are open, exiting", re-run once.

- [ ] **Step 2: Verify per game**

```bash
grep -o '"audio":[^}]*}' build/record/manifest.json
ffprobe -v error -show_entries stream=codec_type,codec_name -of csv=p=0 build/record/tour.mp4
ls build/record/clips/vertical | wc -l
ffmpeg -y -loglevel error -ss 30 -i build/record/tour-vertical.mp4 -frames:v 1 build/record/vertical-30s.png
ffmpeg -y -loglevel error -i build/record/clips/05-mid-play.mp4 -filter_complex "showwavespic=s=1200x240" -frames:v 1 build/record/wave-05.png
```
Expected: `audio` non-null with peak > 0; video + aac streams; vertical count = beats; Read both PNGs (banner correct, waveform not flat). Voidrunner's music and Hunted's spatial creature sounds should be audible in the waveform as sustained energy.

- [ ] **Step 3: Refresh notes only if needed**

Compare the new `build/record/chapters.md` against the chapters table in `docs/video-notes.md`. If beats or times moved, replace that table and fix any lead-clip timestamp that no longer matches; add one line under "Lead clips": "Vertical versions: `clips/vertical/<beat>.mp4`; full tour: `tour-vertical.mp4`." Commit `docs: refresh video notes after audio re-record` only when the file changed.

---

### Task 9: Build the reels

- [ ] **Step 1: Generate and review the config**

```bash
cd /Users/kelliott/Gamebient/colecovisiongx
python3 .claude/skills/recording-game-footage/tools/reel.py --init
cat docs/reel.json
```
Expected: 16 games in index order with each game's lead clip.

- [ ] **Step 2: Build**

`python3 .claude/skills/recording-game-footage/tools/reel.py` (Bash timeout 1800000).

- [ ] **Step 3: Verify**

```bash
for a in 16x9 9x16; do ffprobe -v error -show_entries stream=codec_type,width,height -show_entries format=duration -of compact build/reel/reel-$a.mp4; done
ffmpeg -y -loglevel error -ss 0.7 -i build/reel/reel-16x9.mp4 -frames:v 1 build/reel/check-card.png
ffmpeg -y -loglevel error -ss 3.5 -i build/reel/reel-9x16.mp4 -frames:v 1 build/reel/check-clip.png
ffmpeg -y -loglevel error -sseof -1.5 -i build/reel/reel-16x9.mp4 -frames:v 1 build/reel/check-end.png
```
Expected: both reels ≈ 16 × 6.0 + 3.0 = 99 s (± 1 s), video + audio streams, 1920x1080 and 1080x1920. Read the three PNGs: the first game's title card with cover art, a vertical clip with its banner, the CTA end card.

---

## Self-review

- **Spec coverage:** §1 audio → T1 (clock, manifest), T2 (mixer), T3 (capture/finish); §2 encode/clips/vertical → T4; §3 reel → T6 (tool) + T9 (build); §4 skill move, rollout, re-record → T5, T6, T7, T8; testing section → T2/T4/T6 unit tests, T3/T4/T5/T8/T9 end-to-end checks.
- **Placeholders:** none; the one conditional (decoder item conversion) names the exact alternative.
- **Type consistency:** `Recorder.{sim_frame, first_video_sim_frame, audio}` defined T1, used T3; `Voice/Key/Clip/render/limit/wav_bytes/spatial_gains` defined T2, used T3; `CTA`, `vertical_args`, `clip_args` (cut_clips) defined T4; reel's own `clip_args` is a separate function in a separate file (T6) with a different signature, named in its interfaces block.
