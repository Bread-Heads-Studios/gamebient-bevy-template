// Palette functions are public API for games copying this template; not all are
// used by the template itself.
#![allow(dead_code)]

use std::f32::consts::TAU;
use std::sync::Arc;

use bevy::audio::AudioSource;

pub const SAMPLE_RATE: u32 = 44_100;
/// Applied to every generated sample; per-game loudness lever.
pub const MASTER_GAIN: f32 = 0.5;

/// Renders `f(t)` (t in seconds) into an in-memory WAV `AudioSource`,
/// clamped to ±1.0 after `MASTER_GAIN`.
pub fn generate(duration_secs: f32, f: impl Fn(f32) -> f32) -> AudioSource {
    let n = (duration_secs * SAMPLE_RATE as f32).round() as usize;
    let samples: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f32 / SAMPLE_RATE as f32;
            (f(t) * MASTER_GAIN).clamp(-1.0, 1.0)
        })
        .collect();
    create_wav_audio_source(&samples, SAMPLE_RATE)
}

/// Linear decay envelope from 1.0 at t=0 to 0.0 at t=dur.
fn env_decay(t: f32, dur: f32) -> f32 {
    (1.0 - t / dur).max(0.0)
}

/// Creates a minimal WAV file in memory from floating-point PCM samples and
/// wraps it in an `AudioSource`.
pub fn create_wav_audio_source(samples: &[f32], sample_rate: u32) -> AudioSource {
    let num_samples = samples.len();
    let bytes_per_sample = 2u16; // 16-bit
    let num_channels = 1u16;
    let byte_rate = sample_rate * num_channels as u32 * bytes_per_sample as u32;
    let block_align = num_channels * bytes_per_sample;
    let data_size = (num_samples * bytes_per_sample as usize) as u32;

    let mut wav: Vec<u8> = Vec::with_capacity(44 + data_size as usize);

    // RIFF header
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_size).to_le_bytes());
    wav.extend_from_slice(b"WAVE");

    // fmt chunk
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes()); // chunk size
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM format
    wav.extend_from_slice(&num_channels.to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&block_align.to_le_bytes());
    wav.extend_from_slice(&(bytes_per_sample * 8).to_le_bytes()); // bits per sample

    // data chunk
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_size.to_le_bytes());
    for &s in samples {
        let sample_i16 = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
        wav.extend_from_slice(&sample_i16.to_le_bytes());
    }

    let bytes: Arc<[u8]> = wav.into();
    AudioSource { bytes }
}

/// Short square-ish confirmation blip.
pub fn blip(freq: f32) -> AudioSource {
    let dur = 0.09;
    generate(dur, move |t| {
        (TAU * freq * t).sin().signum() * 0.6 * env_decay(t, dur)
    })
}

/// Bell-like ding with exponential decay; `decay` is total length in seconds.
pub fn ding(freq: f32, decay: f32) -> AudioSource {
    generate(decay, move |t| {
        let e = (-(6.0 * t / decay)).exp();
        ((TAU * freq * t).sin() + 0.5 * (TAU * freq * 2.0 * t).sin()) * 0.66 * e
    })
}

/// Linear-chirp sweep from `f0` to `f1` over `dur` seconds.
pub fn sweep(f0: f32, f1: f32, dur: f32) -> AudioSource {
    generate(dur, move |t| {
        let phase = TAU * (f0 * t + (f1 - f0) * t * t / (2.0 * dur));
        phase.sin() * env_decay(t, dur)
    })
}

/// Low pitch-dropping thump.
pub fn thud() -> AudioSource {
    let dur = 0.15;
    generate(dur, move |t| {
        let freq = 120.0 - 60.0 * (t / dur);
        (TAU * freq * t).sin() * env_decay(t, dur)
    })
}

/// Deterministic white-noise burst (index-hash noise; no rand dependency).
pub fn noise_burst(dur: f32) -> AudioSource {
    generate(dur, move |t| {
        let x = (t * SAMPLE_RATE as f32) as u32;
        let h = x.wrapping_mul(2_654_435_761).rotate_left(13) ^ x;
        ((h % 20_000) as f32 / 10_000.0 - 1.0) * env_decay(t, dur)
    })
}

/// Seconds per arpeggio note.
pub const NOTE_SECS: f32 = 0.09;

/// Plays each frequency for NOTE_SECS in order (fanfares, stings).
pub fn arpeggio(freqs: &[f32]) -> AudioSource {
    let freqs = freqs.to_vec();
    let dur = NOTE_SECS * freqs.len() as f32;
    generate(dur, move |t| {
        let idx = ((t / NOTE_SECS) as usize).min(freqs.len() - 1);
        let local = t - idx as f32 * NOTE_SECS;
        (TAU * freqs[idx] * t).sin() * env_decay(local, NOTE_SECS)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Decodes our own WAV back to f32 samples for assertions.
    fn wav_samples(source: &AudioSource) -> Vec<f32> {
        let bytes: &[u8] = &source.bytes;
        bytes[44..]
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32767.0)
            .collect()
    }

    fn assert_valid(source: &AudioSource, expected_secs: f32) {
        let s = wav_samples(source);
        let expected = (expected_secs * SAMPLE_RATE as f32).round() as usize;
        assert_eq!(s.len(), expected, "sample count");
        assert!(s.iter().all(|v| v.abs() <= 1.0), "bounds");
        let rms = (s.iter().map(|v| v * v).sum::<f32>() / s.len() as f32).sqrt();
        assert!(rms > 0.01, "non-silent (rms {rms})");
    }

    #[test]
    fn blip_ding_sweep_thud_noise_are_valid() {
        assert_valid(&blip(880.0), 0.09);
        assert_valid(&ding(1200.0, 0.4), 0.4);
        assert_valid(&sweep(300.0, 900.0, 0.3), 0.3);
        assert_valid(&thud(), 0.15);
        assert_valid(&noise_burst(0.2), 0.2);
    }

    #[test]
    fn arpeggio_length_scales_with_notes() {
        let a = arpeggio(&[440.0, 550.0, 660.0]);
        assert_valid(&a, 3.0 * NOTE_SECS);
    }

    #[test]
    fn generate_clamps_hot_signals() {
        let hot = generate(0.05, |_| 10.0);
        let s = wav_samples(&hot);
        assert!(s.iter().all(|v| v.abs() <= 1.0));
    }
}
