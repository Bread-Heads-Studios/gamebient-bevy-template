//! Pure offline mixer for the recorder. `audio.rs` turns captured playbacks
//! into `Voice`s and decoded `Clip`s; `render` mixes them to interleaved
//! stereo at `out_rate`, `limit` soft-limits, `wav_bytes` encodes 16-bit PCM.
//! Gains interpolate only between keys on consecutive frames (a fade); a key
//! after a gap is a step. Speed changes pitch and tempo together, as rodio
//! does. No Bevy types, so everything here is unit-tested.

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

    /// Length in seconds at speed 1.
    pub fn secs(&self) -> f64 {
        if self.rate == 0 {
            0.0
        } else {
            self.frames() as f64 / f64::from(self.rate)
        }
    }

    /// (left, right) at fractional source frame `pos`, linearly interpolated.
    /// Mono feeds both ears; stereo maps L/R (extra channels ignored);
    /// `downmix` sums every channel into both, clamped to ±1, as rodio 0.20's
    /// `ChannelVolume` (under `Spatial`) does with `saturating_add` on the
    /// decoder's i16 samples.
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
            if ch == 1 {
                (s[0], s[0])
            } else if downmix {
                let m = s.iter().sum::<f32>().clamp(-1.0, 1.0);
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

pub fn render(
    voices: &[Voice],
    clips: &[Clip],
    fps: u32,
    out_rate: u32,
    total_frames: u64,
) -> Vec<f32> {
    let spf = f64::from(out_rate) / f64::from(fps);
    let total = (total_frames as f64 * spf).round() as i64;
    let mut out = vec![0.0f32; total.max(0) as usize * 2];
    for v in voices {
        let Some(clip) = clips.get(v.clip) else {
            continue;
        };
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
                        Some(next)
                            if next.frame > key.frame && next.frame - key.frame <= 1.0 + 1e-9 =>
                        {
                            let a =
                                ((f - key.frame) / (next.frame - key.frame)).clamp(0.0, 1.0) as f32;
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

/// Video frame at which a voice with no `end` runs off the end of its clip,
/// following its speed and pause keys as `render` does; infinite when it
/// never does (looping, or held paused or at speed 0 for good).
pub fn natural_stop(v: &Voice, clip: &Clip, fps: u32) -> f64 {
    if v.looping || fps == 0 {
        return f64::INFINITY;
    }
    let mut left = clip.secs();
    for (i, k) in v.keys.iter().enumerate() {
        let next = v.keys.get(i + 1).map_or(f64::INFINITY, |n| n.frame);
        if k.paused || k.speed <= 0.0 {
            continue;
        }
        // Clip seconds consumed per video frame.
        let rate = f64::from(k.speed) / f64::from(fps);
        let stop = k.frame + left / rate;
        if stop <= next {
            return stop;
        }
        left -= (next - k.frame) * rate;
    }
    f64::INFINITY
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
    let d2 = |a: [f32; 3], b: [f32; 3]| {
        (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)
    };
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

#[cfg(test)]
mod tests {
    use super::*;

    fn mono(samples: Vec<f32>) -> Clip {
        Clip {
            channels: 1,
            rate: 100,
            samples,
        }
    }
    fn key(frame: f64, gain: f32) -> Key {
        Key {
            frame,
            gain_l: gain,
            gain_r: gain,
            speed: 1.0,
            paused: false,
        }
    }
    fn voice(start: f64, keys: Vec<Key>) -> Voice {
        Voice {
            clip: 0,
            start,
            end: None,
            looping: false,
            downmix: false,
            keys,
        }
    }
    fn ramp(n: usize) -> Vec<f32> {
        (0..n).map(|i| i as f32 / 1000.0).collect()
    }
    // fps 10, out_rate 100: 10 output samples per video frame.

    #[test]
    fn natural_stop_follows_speed_and_pauses() {
        // 100 frames at 100 Hz = 1 s = 10 video frames at fps 10.
        let clip = mono(vec![1.0; 100]);
        assert_eq!(
            natural_stop(&voice(-3.0, vec![key(-3.0, 1.0)]), &clip, 10),
            7.0
        );
        let fast = Key {
            speed: 2.0,
            ..key(0.0, 1.0)
        };
        assert_eq!(natural_stop(&voice(0.0, vec![fast]), &clip, 10), 5.0);
        let held = Key {
            paused: true,
            ..key(4.0, 1.0)
        };
        let v = voice(0.0, vec![key(0.0, 1.0), held, key(9.0, 1.0)]);
        assert_eq!(natural_stop(&v, &clip, 10), 15.0);
        let looping = Voice {
            looping: true,
            ..voice(0.0, vec![key(0.0, 1.0)])
        };
        assert_eq!(natural_stop(&looping, &clip, 10), f64::INFINITY);
        let frozen = Key {
            paused: true,
            ..key(0.0, 1.0)
        };
        assert_eq!(
            natural_stop(&voice(0.0, vec![frozen]), &clip, 10),
            f64::INFINITY
        );
    }

    #[test]
    fn constant_gain_voice_plays_once_then_silence() {
        let out = render(
            &[voice(0.0, vec![key(0.0, 0.5)])],
            &[mono(vec![1.0; 100])],
            10,
            100,
            20,
        );
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
        let out = render(
            &[voice(-1.0, vec![key(-1.0, 1.0)])],
            &[mono(ramp(100))],
            10,
            100,
            10,
        );
        assert_eq!(out[0], ramp(100)[10]);
    }

    #[test]
    fn consecutive_keys_fade_but_gapped_keys_step() {
        let fade = render(
            &[voice(0.0, vec![key(0.0, 0.0), key(1.0, 1.0)])],
            &[mono(vec![1.0; 100])],
            10,
            100,
            10,
        );
        assert!((fade[5 * 2] - 0.5).abs() < 1e-6);
        let step = render(
            &[voice(0.0, vec![key(0.0, 0.0), key(5.0, 1.0)])],
            &[mono(vec![1.0; 100])],
            10,
            100,
            10,
        );
        assert_eq!(step[30 * 2], 0.0);
        assert_eq!(step[50 * 2], 1.0);
    }

    #[test]
    fn stereo_maps_left_right_and_downmix_sums_like_rodio() {
        let clip = Clip {
            channels: 2,
            rate: 100,
            samples: [1.0, 0.0].repeat(100),
        };
        let plain = render(
            &[voice(0.0, vec![key(0.0, 1.0)])],
            std::slice::from_ref(&clip),
            10,
            100,
            1,
        );
        assert_eq!((plain[0], plain[1]), (1.0, 0.0));
        let mut v = voice(0.0, vec![key(0.0, 1.0)]);
        v.downmix = true;
        let mixed = render(&[v.clone()], std::slice::from_ref(&clip), 10, 100, 1);
        assert_eq!((mixed[0], mixed[1]), (1.0, 1.0));
        // Both channels loud: the sum saturates at 1, like rodio's i16 add.
        let loud = Clip {
            samples: [0.8, 0.8].repeat(100),
            ..clip
        };
        let clamped = render(&[v], &[loud], 10, 100, 1);
        assert_eq!((clamped[0], clamped[1]), (1.0, 1.0));
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
