//! Composed music and sound effects for the house chip (`chip.rs`).
//!
//! A [`Song`] is four voice strings in a small tracker notation plus a table
//! of [`Instrument`]s; [`render_loop`] turns it into an in-memory WAV
//! `AudioSource` of exactly `bars` bars so it loops without a seam, and
//! [`render_sting`] adds a release tail for one-shots. [`Fx`] builds sound
//! effects frame by frame on the same chip, so a game's confirm blip, its
//! fail buzz and its menu loop share one timbre.
//!
//! # Notation (one string per voice, whitespace-separated, `|` is ignored)
//!
//! | token | meaning |
//! |---|---|
//! | `c4` `f#3` `bb2` | note on, one step |
//! | `c4:3` | note held three steps |
//! | `.` / `.:3` | rest (one / three steps) |
//! | `-` | extend the previous token by one step |
//! | `/c5` | glide from the previous pitch into this note over its length |
//! | `c4~` | vibrato on this note (the instrument's, or 25 cents at 6 Hz) |
//! | `@2` | following notes use instrument 2 |
//! | `v4` | following notes are 4 attenuation steps (8 dB) quieter |
//! | `x` / `x:2` | noise voice only: a hit |
//!
//! Envelopes and arpeggios run at [`FRAME_HZ`] like a classic tracker:
//! `env` is attenuation per frame since note-on (last value held), `arp`
//! cycles semitone offsets per frame, `release` is frames per attenuation
//! step after note-off.
//!
//! Games copy this file verbatim from the template; compose in `songs.rs`.
#![allow(dead_code)]

use bevy::audio::AudioSource;

use super::chip::{Chip, FLOOR_HZ, NoiseMode, NoiseRate, OFF, note_hz, semitones};
use super::synth::{MASTER_GAIN, SAMPLE_RATE, create_wav_audio_source};

/// Envelope / arpeggio clock.
pub const FRAME_HZ: f32 = 60.0;
/// Music loops and layers peak here (-12 dBFS) before `MASTER_GAIN`.
pub const MUSIC_PEAK: f32 = 0.251;
/// Stings and SFX peak here (-3 dBFS) before `MASTER_GAIN`.
pub const SFX_PEAK: f32 = 0.708;
/// `~` without an instrument vibrato: (cents, Hz).
pub const DEFAULT_VIBRATO: (f32, f32) = (25.0, 6.0);

const SAMPLES_PER_FRAME: f32 = SAMPLE_RATE as f32 / FRAME_HZ;
/// Voice parameters (pitch, envelope, vibrato) are recomputed every this
/// many samples (~1.5 ms, a 690 Hz control rate); the chip itself runs per
/// sample. Keeps a 30 s loop under the render budget on a Pi 4.
const CONTROL_SAMPLES: usize = 64;

/// How a voice plays a note. All `const fn` so songs are `const` data.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Instrument {
    /// Attenuation (0 loud .. 15 off) per frame from note-on; the last value holds.
    pub env: &'static [u8],
    /// Frames per attenuation step after note-off; 0 cuts the note.
    pub release: u8,
    /// Semitone offsets cycled per frame (a fast arpeggio reads as a chord).
    pub arp: &'static [i8],
    /// (depth in cents, rate in Hz) applied to every note of this instrument.
    pub vibrato: Option<(f32, f32)>,
    /// Set for noise-voice instruments.
    pub noise: Option<(NoiseMode, NoiseRate)>,
}

impl Instrument {
    pub const fn tone(env: &'static [u8]) -> Self {
        Self {
            env,
            release: 0,
            arp: &[],
            vibrato: None,
            noise: None,
        }
    }

    pub const fn noise(mode: NoiseMode, rate: NoiseRate, env: &'static [u8]) -> Self {
        Self {
            env,
            release: 0,
            arp: &[],
            vibrato: None,
            noise: Some((mode, rate)),
        }
    }

    pub const fn with_release(mut self, frames: u8) -> Self {
        self.release = frames;
        self
    }

    pub const fn with_arp(mut self, arp: &'static [i8]) -> Self {
        self.arp = arp;
        self
    }

    pub const fn with_vibrato(mut self, cents: f32, hz: f32) -> Self {
        self.vibrato = Some((cents, hz));
        self
    }
}

/// A composition: tempo, length and four voices.
#[derive(Clone, Copy, Debug)]
pub struct Song {
    pub bpm: f32,
    pub beats_per_bar: u8,
    pub steps_per_beat: u8,
    pub bars: u8,
    /// Tone voices 1-3. Voice 3 also clocks `NoiseRate::Tone3`.
    pub voices: [&'static str; 3],
    pub noise: &'static str,
    pub instruments: &'static [Instrument],
    pub noise_instruments: &'static [Instrument],
}

impl Song {
    pub const fn steps(&self) -> usize {
        self.bars as usize * self.beats_per_bar as usize * self.steps_per_beat as usize
    }

    pub fn step_secs(&self) -> f32 {
        60.0 / (self.bpm * self.steps_per_beat as f32)
    }

    pub fn loop_secs(&self) -> f32 {
        self.steps() as f32 * self.step_secs()
    }
}

#[derive(Debug, PartialEq)]
pub enum SongError {
    Parse {
        voice: &'static str,
        token: String,
    },
    Length {
        voice: &'static str,
        got: usize,
        want: usize,
    },
    Instrument {
        voice: &'static str,
        index: usize,
    },
    /// A note below the chip's divider floor (`chip::FLOOR_HZ`, just under
    /// A2): it would silently clamp to the same pitch as every other note
    /// down there. Write it an octave up, or use periodic noise.
    Pitch {
        voice: &'static str,
        token: String,
    },
}

const VOICE_NAMES: [&str; 4] = ["tone1", "tone2", "tone3", "noise"];

#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    Note(f32),
    Hit,
    Rest,
}

#[derive(Clone, Copy, Debug)]
struct Step {
    kind: Kind,
    len: usize,
    inst: usize,
    vol: u8,
    glide: bool,
    vibrato: bool,
}

fn split_len(token: &str) -> Option<(&str, usize)> {
    match token.split_once(':') {
        None => Some((token, 1)),
        Some((head, n)) => n
            .parse::<usize>()
            .ok()
            .filter(|n| *n > 0)
            .map(|n| (head, n)),
    }
}

fn parse_voice(voice: &'static str, text: &str, is_noise: bool) -> Result<Vec<Step>, SongError> {
    let mut steps: Vec<Step> = Vec::new();
    let mut inst = 0usize;
    let mut vol = 0u8;
    for raw in text.split_whitespace() {
        if raw == "|" {
            continue;
        }
        let err = || SongError::Parse {
            voice,
            token: raw.to_string(),
        };
        if raw == "-" {
            let last = steps.last_mut().ok_or_else(err)?;
            last.len += 1;
            continue;
        }
        if let Some(n) = raw.strip_prefix('@') {
            inst = n.parse().map_err(|_| err())?;
            continue;
        }
        if raw.len() > 1
            && let Some(n) = raw.strip_prefix('v')
            && let Ok(v) = n.parse::<u8>()
        {
            vol = v.min(OFF);
            continue;
        }
        let (body, len) = split_len(raw).ok_or_else(err)?;
        if body == "." {
            steps.push(Step {
                kind: Kind::Rest,
                len,
                inst,
                vol,
                glide: false,
                vibrato: false,
            });
            continue;
        }
        if is_noise {
            if body != "x" {
                return Err(err());
            }
            steps.push(Step {
                kind: Kind::Hit,
                len,
                inst,
                vol,
                glide: false,
                vibrato: false,
            });
            continue;
        }
        let (glide, body) = match body.strip_prefix('/') {
            Some(b) => (true, b),
            None => (false, body),
        };
        let (vibrato, body) = match body.strip_suffix('~') {
            Some(b) => (true, b),
            None => (false, body),
        };
        let hz = note_hz(body).ok_or_else(err)?;
        if hz < FLOOR_HZ {
            return Err(SongError::Pitch {
                voice,
                token: raw.to_string(),
            });
        }
        steps.push(Step {
            kind: Kind::Note(hz),
            len,
            inst,
            vol,
            glide,
            vibrato,
        });
    }
    Ok(steps)
}

/// A step placed on the sample timeline.
#[derive(Clone, Copy, Debug)]
struct Event {
    start: usize,
    end: usize,
    step: Step,
    /// Pitch a glide starts from (the previous note's), else the note's own.
    from_hz: f32,
}

fn lay_out(
    voice: &'static str,
    steps: &[Step],
    want_steps: usize,
    step_samples: f32,
    instruments: &[Instrument],
) -> Result<Vec<Event>, SongError> {
    let got: usize = steps.iter().map(|s| s.len).sum();
    if got != want_steps {
        return Err(SongError::Length {
            voice,
            got,
            want: want_steps,
        });
    }
    let mut events = Vec::with_capacity(steps.len());
    let mut at = 0usize;
    let mut last_hz = 0.0f32;
    for step in steps {
        if step.inst >= instruments.len() && step.kind != Kind::Rest {
            return Err(SongError::Instrument {
                voice,
                index: step.inst,
            });
        }
        let start = (at as f32 * step_samples).round() as usize;
        let end = ((at + step.len) as f32 * step_samples).round() as usize;
        let from_hz = match step.kind {
            Kind::Note(_) if step.glide && last_hz > 0.0 => last_hz,
            Kind::Note(hz) => hz,
            _ => last_hz,
        };
        if let Kind::Note(hz) = step.kind {
            last_hz = hz;
        }
        events.push(Event {
            start,
            end,
            step: *step,
            from_hz,
        });
        at += step.len;
    }
    Ok(events)
}

/// Plays one voice's events sample by sample.
struct VoiceCursor<'a> {
    events: &'a [Event],
    idx: usize,
    instruments: &'a [Instrument],
    /// The last sounding note: (its end sample, its instrument, its final
    /// attenuation) so the release can run through following rests.
    released: Option<(usize, usize, u8)>,
    /// A fresh hit restarts periodic noise.
    retrigger: bool,
}

struct VoiceOut {
    hz: f32,
    atten: u8,
    noise: Option<(NoiseMode, NoiseRate)>,
    retrigger: bool,
}

fn env_at(inst: &Instrument, frame: usize) -> u8 {
    if inst.env.is_empty() {
        return 0;
    }
    inst.env[frame.min(inst.env.len() - 1)]
}

impl<'a> VoiceCursor<'a> {
    fn new(events: &'a [Event], instruments: &'a [Instrument]) -> Self {
        Self {
            events,
            idx: 0,
            instruments,
            released: None,
            retrigger: false,
        }
    }

    fn at(&mut self, s: usize) -> VoiceOut {
        while self.idx < self.events.len() && self.events[self.idx].end <= s {
            let ev = &self.events[self.idx];
            if !matches!(ev.step.kind, Kind::Rest) {
                let inst = &self.instruments[ev.step.inst];
                let frames = ((ev.end - ev.start) as f32 / SAMPLES_PER_FRAME) as usize;
                let last = env_at(inst, frames.saturating_sub(1)).saturating_add(ev.step.vol);
                self.released = Some((ev.end, ev.step.inst, last.min(OFF)));
            }
            self.idx += 1;
            self.retrigger = true;
        }
        let Some(ev) = self.events.get(self.idx) else {
            return self.release_out(s);
        };
        if s < ev.start || matches!(ev.step.kind, Kind::Rest) {
            return self.release_out(s);
        }
        let inst = &self.instruments[ev.step.inst];
        let frame = ((s - ev.start) as f32 / SAMPLES_PER_FRAME) as usize;
        let t = (s - ev.start) as f32 / SAMPLE_RATE as f32;
        let atten = env_at(inst, frame).saturating_add(ev.step.vol).min(OFF);
        let retrigger = self.retrigger;
        self.retrigger = false;
        match ev.step.kind {
            Kind::Note(hz) => {
                let mut hz = if ev.step.glide && ev.from_hz > 0.0 {
                    let p = (s - ev.start) as f32 / (ev.end - ev.start).max(1) as f32;
                    ev.from_hz * (hz / ev.from_hz).powf(p)
                } else {
                    hz
                };
                if !inst.arp.is_empty() {
                    hz = semitones(hz, inst.arp[frame % inst.arp.len()] as f32);
                }
                let vib = inst.vibrato.or(if ev.step.vibrato {
                    Some(DEFAULT_VIBRATO)
                } else {
                    None
                });
                if let Some((cents, rate)) = vib {
                    hz = semitones(hz, cents / 100.0 * (std::f32::consts::TAU * rate * t).sin());
                }
                VoiceOut {
                    hz,
                    atten,
                    noise: inst.noise,
                    retrigger,
                }
            }
            Kind::Hit => VoiceOut {
                hz: 0.0,
                atten,
                noise: inst.noise,
                retrigger,
            },
            Kind::Rest => unreachable!(),
        }
    }

    fn release_out(&self, s: usize) -> VoiceOut {
        let Some((end, inst, last)) = self.released else {
            return VoiceOut {
                hz: 0.0,
                atten: OFF,
                noise: None,
                retrigger: false,
            };
        };
        let inst = &self.instruments[inst];
        let atten = if inst.release == 0 {
            OFF
        } else {
            let frames = ((s.saturating_sub(end)) as f32 / SAMPLES_PER_FRAME) as usize;
            (last as usize + frames / inst.release as usize).min(OFF as usize) as u8
        };
        VoiceOut {
            hz: 0.0,
            atten,
            noise: inst.noise,
            retrigger: false,
        }
    }
}

/// Renders the song through one chip: the loop plus `tail_secs` of release.
fn render_raw(song: &Song, tail_secs: f32) -> Result<Vec<f32>, SongError> {
    let step_samples = song.step_secs() * SAMPLE_RATE as f32;
    let want = song.steps();
    let mut events: Vec<Vec<Event>> = Vec::with_capacity(4);
    for (i, text) in song.voices.iter().enumerate() {
        let steps = parse_voice(VOICE_NAMES[i], text, false)?;
        events.push(lay_out(
            VOICE_NAMES[i],
            &steps,
            want,
            step_samples,
            song.instruments,
        )?);
    }
    let noise_steps = parse_voice(VOICE_NAMES[3], song.noise, true)?;
    events.push(lay_out(
        VOICE_NAMES[3],
        &noise_steps,
        want,
        step_samples,
        song.noise_instruments,
    )?);

    let loop_samples = (want as f32 * step_samples).round() as usize;
    let total = loop_samples + (tail_secs * SAMPLE_RATE as f32).round() as usize;
    let mut cursors: Vec<VoiceCursor> = events
        .iter()
        .enumerate()
        .map(|(i, ev)| {
            VoiceCursor::new(
                ev,
                if i < 3 {
                    song.instruments
                } else {
                    song.noise_instruments
                },
            )
        })
        .collect();
    let mut chip = Chip::new();
    let mut out = Vec::with_capacity(total);
    for s in 0..total {
        if s % CONTROL_SAMPLES == 0 {
            let (tones, noise) = cursors.split_at_mut(3);
            for (cursor, tone) in tones.iter_mut().zip(chip.tones.iter_mut()) {
                let o = cursor.at(s);
                if o.hz > 0.0 {
                    tone.hz = o.hz;
                }
                tone.atten = o.atten;
            }
            let n = noise[0].at(s);
            if let Some((mode, rate)) = n.noise {
                chip.noise.mode = mode;
                chip.noise.rate = rate;
            }
            if n.retrigger {
                chip.noise.reset();
            }
            chip.noise.atten = n.atten;
        }
        out.push(chip.sample());
    }
    Ok(out)
}

/// Scales so the loudest sample sits at `peak` (silence stays silent).
pub fn normalize_peak(samples: &mut [f32], peak: f32) {
    let max = samples.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    if max > 1e-6 {
        let k = peak / max;
        for v in samples {
            *v *= k;
        }
    }
}

/// The loop, normalized to [`MUSIC_PEAK`] × `MASTER_GAIN`, as samples.
pub fn render_song_samples(song: &Song, tail_secs: f32) -> Result<Vec<f32>, SongError> {
    let mut s = render_raw(song, tail_secs)?;
    normalize_peak(&mut s, MUSIC_PEAK * MASTER_GAIN);
    Ok(s)
}

/// Exactly `bars` bars, ready for `PlaybackMode::Loop`.
pub fn render_loop(song: &Song) -> Result<AudioSource, SongError> {
    let s = render_song_samples(song, 0.0)?;
    Ok(create_wav_audio_source(&s, SAMPLE_RATE))
}

/// A one-shot: the song plus `tail_secs` for releases to fade, at SFX level.
pub fn render_sting(song: &Song, tail_secs: f32) -> Result<AudioSource, SongError> {
    let mut s = render_raw(song, tail_secs)?;
    normalize_peak(&mut s, SFX_PEAK * MASTER_GAIN);
    Ok(create_wav_audio_source(&s, SAMPLE_RATE))
}

/// Decodes one of our 16-bit mono WAVs back to samples.
pub fn wav_to_samples(source: &AudioSource) -> Vec<f32> {
    let bytes: &[u8] = &source.bytes;
    bytes
        .get(44..)
        .unwrap_or(&[])
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| i16::from_le_bytes(*c) as f32 / 32767.0)
        .collect()
}

/// Length in seconds of one of our WAVs (from the header).
pub fn wav_secs(source: &AudioSource) -> f32 {
    let b: &[u8] = &source.bytes;
    if b.len() < 44 {
        return 0.0;
    }
    let rate = u32::from_le_bytes([b[24], b[25], b[26], b[27]]) as f32;
    let channels = u16::from_le_bytes([b[22], b[23]]) as f32;
    let bits = u16::from_le_bytes([b[34], b[35]]) as f32;
    let data = u32::from_le_bytes([b[40], b[41], b[42], b[43]]) as f32;
    data / (rate * channels * (bits / 8.0))
}

// --- Fx: frame-programmed sound effects -------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
struct Seg {
    frames: u16,
    hz0: f32,
    hz1: f32,
    a0: u8,
    a1: u8,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct NoiseSeg {
    frames: u16,
    mode: NoiseMode,
    rate: NoiseRate,
    a0: u8,
    a1: u8,
    retrigger: bool,
}

/// A sound effect as per-voice segment lists, rendered through the chip and
/// normalized to [`SFX_PEAK`]. Build with the fluent methods, chain in time
/// with [`Fx::then`], stack simultaneously with [`Fx::with`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Fx {
    tones: [Vec<Seg>; 3],
    noise: Vec<NoiseSeg>,
}

fn seg_frames(segs: &[Seg]) -> u16 {
    segs.iter().map(|s| s.frames).sum()
}

fn noise_frames(segs: &[NoiseSeg]) -> u16 {
    segs.iter().map(|s| s.frames).sum()
}

impl Fx {
    pub fn new() -> Self {
        Self::default()
    }

    /// Total length in frames (the longest voice).
    pub fn frames(&self) -> u16 {
        self.tones
            .iter()
            .map(|v| seg_frames(v))
            .chain(std::iter::once(noise_frames(&self.noise)))
            .max()
            .unwrap_or(0)
    }

    pub fn secs(&self) -> f32 {
        self.frames() as f32 / FRAME_HZ
    }

    /// Tone `voice` (0-2) sweeps `hz0 -> hz1` and `a0 -> a1` over `frames`.
    pub fn tone(mut self, voice: usize, hz0: f32, hz1: f32, a0: u8, a1: u8, frames: u16) -> Self {
        self.tones[voice.min(2)].push(Seg {
            frames,
            hz0,
            hz1,
            a0,
            a1,
        });
        self
    }

    /// A plucked note: full volume decaying to silence over `frames`.
    pub fn hit(self, voice: usize, hz: f32, frames: u16) -> Self {
        self.tone(voice, hz, hz, 0, OFF, frames)
    }

    /// A pitch slide at steady volume, then cut.
    pub fn slide(self, voice: usize, hz0: f32, hz1: f32, frames: u16) -> Self {
        self.tone(voice, hz0, hz1, 0, 4, frames)
    }

    /// Noise voice segment; a new segment retriggers the register.
    pub fn noise(mut self, mode: NoiseMode, rate: NoiseRate, a0: u8, a1: u8, frames: u16) -> Self {
        self.noise.push(NoiseSeg {
            frames,
            mode,
            rate,
            a0,
            a1,
            retrigger: true,
        });
        self
    }

    /// Silence on every voice for `frames` after everything so far.
    pub fn rest(mut self, frames: u16) -> Self {
        self.pad_to(self.frames());
        for v in &mut self.tones {
            v.push(Seg {
                frames,
                hz0: 440.0,
                hz1: 440.0,
                a0: OFF,
                a1: OFF,
            });
        }
        self.noise.push(NoiseSeg {
            frames,
            mode: NoiseMode::White,
            rate: NoiseRate::Div1024,
            a0: OFF,
            a1: OFF,
            retrigger: false,
        });
        self
    }

    fn pad_to(&mut self, frames: u16) {
        for v in &mut self.tones {
            let have = seg_frames(v);
            if have < frames {
                v.push(Seg {
                    frames: frames - have,
                    hz0: 440.0,
                    hz1: 440.0,
                    a0: OFF,
                    a1: OFF,
                });
            }
        }
        let have = noise_frames(&self.noise);
        if have < frames {
            self.noise.push(NoiseSeg {
                frames: frames - have,
                mode: NoiseMode::White,
                rate: NoiseRate::Div1024,
                a0: OFF,
                a1: OFF,
                retrigger: false,
            });
        }
    }

    /// `other` starts when everything in `self` has finished.
    pub fn then(mut self, mut other: Fx) -> Self {
        self.pad_to(self.frames());
        other.pad_to(other.frames());
        for (mine, theirs) in self.tones.iter_mut().zip(other.tones) {
            mine.extend(theirs);
        }
        self.noise.extend(other.noise);
        self
    }

    /// `other` plays at the same time, on voices `self` leaves empty. A tone
    /// voice both use, or a second noise program, is dropped (debug-asserted).
    pub fn with(mut self, other: Fx) -> Self {
        for theirs in other.tones.into_iter().filter(|v| !v.is_empty()) {
            match self.tones.iter_mut().find(|v| v.is_empty()) {
                Some(slot) => *slot = theirs,
                None => debug_assert!(false, "Fx::with: no free tone voice"),
            }
        }
        if !other.noise.is_empty() {
            if self.noise.is_empty() {
                self.noise = other.noise;
            } else {
                debug_assert!(false, "Fx::with: both sides program the noise voice");
            }
        }
        self
    }

    /// Raw samples normalized to [`SFX_PEAK`] × `MASTER_GAIN`.
    pub fn samples(&self) -> Vec<f32> {
        let total = (self.frames() as f32 * SAMPLES_PER_FRAME).round() as usize;
        let mut chip = Chip::new();
        let mut out = Vec::with_capacity(total);
        let mut last_noise_seg: Option<usize> = None;
        for s in 0..total {
            if s % CONTROL_SAMPLES != 0 {
                out.push(chip.sample());
                continue;
            }
            let f = s as f32 / SAMPLES_PER_FRAME;
            for (v, segs) in self.tones.iter().enumerate() {
                match locate(segs.iter().map(|g| g.frames), f) {
                    Some((i, p)) => {
                        let g = &segs[i];
                        chip.tones[v].hz = g.hz0 * (g.hz1 / g.hz0).powf(p);
                        chip.tones[v].atten = lerp_atten(g.a0, g.a1, p);
                    }
                    None => chip.tones[v].atten = OFF,
                }
            }
            match locate(self.noise.iter().map(|g| g.frames), f) {
                Some((i, p)) => {
                    let g = &self.noise[i];
                    chip.noise.mode = g.mode;
                    chip.noise.rate = g.rate;
                    chip.noise.atten = lerp_atten(g.a0, g.a1, p);
                    if g.retrigger && last_noise_seg != Some(i) {
                        chip.noise.reset();
                    }
                    last_noise_seg = Some(i);
                }
                None => chip.noise.atten = OFF,
            }
            out.push(chip.sample());
        }
        normalize_peak(&mut out, SFX_PEAK * MASTER_GAIN);
        out
    }

    pub fn render(&self) -> AudioSource {
        create_wav_audio_source(&self.samples(), SAMPLE_RATE)
    }
}

/// Which segment frame `f` falls in and how far through it (0..1).
fn locate(frames: impl Iterator<Item = u16>, f: f32) -> Option<(usize, f32)> {
    let mut at = 0.0f32;
    for (i, n) in frames.enumerate() {
        let end = at + n as f32;
        if f < end {
            return Some((i, (f - at) / (n.max(1) as f32)));
        }
        at = end;
    }
    None
}

fn lerp_atten(a0: u8, a1: u8, p: f32) -> u8 {
    (a0 as f32 + (a1 as f32 - a0 as f32) * p)
        .round()
        .clamp(0.0, OFF as f32) as u8
}

#[cfg(test)]
mod tests {
    use super::super::synth;
    use super::*;

    const PLUCK: Instrument = Instrument::tone(&[0, 2, 4, 6, 9, 12]).with_release(2);
    const KICK: Instrument =
        Instrument::noise(NoiseMode::Periodic, NoiseRate::Div2048, &[0, 3, 8, 15]);

    fn song(v0: &'static str) -> Song {
        Song {
            bpm: 120.0,
            beats_per_bar: 4,
            steps_per_beat: 4,
            bars: 1,
            voices: [v0, ".:16", ".:16"],
            noise: "x . . . x . . . x . . . x . . .",
            instruments: &[PLUCK],
            noise_instruments: &[KICK],
        }
    }

    fn peak(s: &[f32]) -> f32 {
        s.iter().fold(0.0f32, |m, v| m.max(v.abs()))
    }

    #[test]
    fn loop_length_is_exact() {
        let s = render_song_samples(&song("c4 e4 g4 c5 . . . . c4:4 . . . ."), 0.0).unwrap();
        assert_eq!(s.len(), 88_200);
    }

    #[test]
    fn ties_lengths_and_bars_count_steps() {
        assert!(render_song_samples(&song("c4:4 - - - - | c4:8"), 0.0).is_ok());
        assert!(render_song_samples(&song("c4:4 - - - .:6 e4~ /g4:2"), 0.0).is_ok());
    }

    #[test]
    fn wrong_length_names_the_voice() {
        assert_eq!(
            render_song_samples(&song("c4 e4"), 0.0).unwrap_err(),
            SongError::Length {
                voice: "tone1",
                got: 2,
                want: 16
            }
        );
    }

    #[test]
    fn bad_tokens_name_themselves() {
        for bad in ["c4 zz:15", "x:16", "c4:0 .:15", "- c4:15"] {
            match render_song_samples(&song(bad), 0.0) {
                Err(SongError::Parse {
                    voice: "tone1",
                    token,
                }) => {
                    assert!(bad.contains(token.as_str()), "{bad} -> {token}");
                }
                other => panic!("{bad}: {other:?}"),
            }
        }
        let mut s = song("c4:16");
        s.noise = "c4:16";
        assert!(matches!(
            render_song_samples(&s, 0.0),
            Err(SongError::Parse { voice: "noise", .. })
        ));
    }

    #[test]
    fn notes_below_the_chip_floor_are_an_error() {
        assert_eq!(
            render_song_samples(&song("g2:16"), 0.0).unwrap_err(),
            SongError::Pitch {
                voice: "tone1",
                token: "g2:16".to_string()
            }
        );
        assert!(render_song_samples(&song("a2:16"), 0.0).is_ok());
    }

    #[test]
    fn missing_instrument_is_an_error() {
        assert_eq!(
            render_song_samples(&song("@3 c4:16"), 0.0).unwrap_err(),
            SongError::Instrument {
                voice: "tone1",
                index: 3
            }
        );
    }

    #[test]
    fn loop_is_normalized_to_music_peak() {
        let s = render_song_samples(&song("c4:16"), 0.0).unwrap();
        assert!(
            (peak(&s) - MUSIC_PEAK * MASTER_GAIN).abs() < 0.01,
            "{}",
            peak(&s)
        );
    }

    #[test]
    fn volume_offset_quietens() {
        let mut loud_song = song("c4:16");
        loud_song.noise = ".:16";
        let mut soft_song = song("v6 c4:16");
        soft_song.noise = ".:16";
        let loud = render_raw(&loud_song, 0.0).unwrap();
        let soft = render_raw(&soft_song, 0.0).unwrap();
        assert!(peak(&soft) < peak(&loud) * 0.6);
    }

    #[test]
    fn sting_adds_a_tail_at_sfx_level() {
        let src = render_sting(&song("c4:16"), 1.0).unwrap();
        assert!((wav_secs(&src) - 3.0).abs() < 0.01);
        let s = wav_to_samples(&src);
        assert!((peak(&s) - SFX_PEAK * MASTER_GAIN).abs() < 0.02);
    }

    #[test]
    fn release_runs_through_rests() {
        const CUT: [Instrument; 1] = [Instrument::tone(&[0])];
        const LONG: [Instrument; 1] = [Instrument::tone(&[0]).with_release(6)];
        let mut s = song("c4:4 .:12");
        s.instruments = &CUT;
        let a = render_raw(&s, 0.0).unwrap();
        s.instruments = &LONG;
        let b = render_raw(&s, 0.0).unwrap();
        let step = 44_100 / 8;
        let window = |v: &[f32]| peak(&v[step * 5..step * 6]);
        assert!(window(&a) < 0.01, "cut note is silent in step 6");
        assert!(window(&b) > 0.02, "released note still sounds in step 6");
    }

    #[test]
    fn glide_moves_pitch() {
        let crossings = |v: &[f32]| v.windows(2).filter(|w| w[0] <= 0.0 && w[1] > 0.0).count();
        let stepped = render_raw(&song("c4:8 c5:8"), 0.0).unwrap();
        let glided = render_raw(&song("c4:8 /c5:8"), 0.0).unwrap();
        let mid = 44_100 / 8 * 9..44_100 / 8 * 11;
        let a = crossings(&stepped[mid.clone()]);
        let b = crossings(&glided[mid]);
        assert!(b < a - 10, "stepped {a} glided {b}");
    }

    #[test]
    fn fx_then_and_with_lengths() {
        let a = Fx::new().tone(0, 440.0, 440.0, 0, 15, 6);
        let b = Fx::new().noise(NoiseMode::White, NoiseRate::Div512, 0, 15, 6);
        let per_frame = SAMPLE_RATE as f32 / FRAME_HZ;
        assert_eq!(
            a.clone().then(b.clone()).samples().len(),
            (12.0 * per_frame).round() as usize
        );
        assert_eq!(
            a.with(b).samples().len(),
            (6.0 * per_frame).round() as usize
        );
    }

    #[test]
    fn fx_with_takes_a_free_voice() {
        let a = Fx::new().hit(0, 440.0, 6);
        let b = Fx::new().hit(0, 660.0, 6);
        let both = a.with(b);
        assert_eq!(both.tones[0].len(), 1);
        assert_eq!(both.tones[1].len(), 1);
        assert!((both.tones[1][0].hz0 - 660.0).abs() < 1e-3);
    }

    #[test]
    fn fx_rest_pads_everything() {
        let f = Fx::new().hit(0, 440.0, 6).rest(6).hit(1, 220.0, 6);
        assert_eq!(f.frames(), 18);
        assert!((f.secs() - 0.3).abs() < 1e-6);
    }

    #[test]
    fn fx_is_normalized_to_sfx_peak_and_slides() {
        let s = Fx::new().tone(0, 440.0, 220.0, 0, 15, 12).samples();
        assert!((peak(&s) - SFX_PEAK * MASTER_GAIN).abs() < 0.01);
        let crossings = |v: &[f32]| v.windows(2).filter(|w| w[0] <= 0.0 && w[1] > 0.0).count();
        let per_frame = 735;
        assert!(crossings(&s[..per_frame * 3]) > crossings(&s[per_frame * 8..per_frame * 11]));
    }

    #[test]
    fn wav_helpers_read_our_header() {
        assert!((wav_secs(&synth::blip(880.0)) - 0.09).abs() < 1e-3);
        assert_eq!(wav_to_samples(&synth::blip(880.0)).len(), 3969);
    }
}
