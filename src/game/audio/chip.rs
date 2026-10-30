//! A model of the ColecoVision's sound chip, the Texas Instruments SN76489:
//! three square-wave tone voices, one noise voice and sixteen 2 dB volume
//! steps. Everything a chip game plays, music and SFX alike, is rendered
//! through one of these, so the fleet sounds like one console and a game's
//! composition is what makes it its own. See `docs/audio-identity.md`.
//!
//! The model is deliberately plain: no duty cycle, no triangle, no samples.
//! Chords are fast arpeggios, vibrato is a wobbling pitch, drums are shaped
//! noise. Pitches are quantized to the chip's 10-bit frequency divider, so a
//! high note lands a few cents off, as it did on the hardware.
//!
//! Games copy this file verbatim from the template (`tools/frame-accept.sh
//! --check-copies`); compose in `songs.rs` and `mod.rs`, not here.
#![allow(dead_code)]

use std::f32::consts::TAU;

use super::synth::SAMPLE_RATE;

/// NTSC master clock. Tone frequency is `CLOCK_HZ / (32 * divider)`.
pub const CLOCK_HZ: f32 = 3_579_545.0;

/// Linear gain per attenuation step: 0 is full, each step is -2 dB, 15 is off.
pub const ATTEN_GAIN: [f32; 16] = [
    1.0, 0.794_328, 0.630_957, 0.501_187, 0.398_107, 0.316_228, 0.251_189, 0.199_526, 0.158_489,
    0.125_893, 0.1, 0.079_433, 0.063_096, 0.050_119, 0.039_811, 0.0,
];

/// Attenuation index that silences a voice.
pub const OFF: u8 = 15;

/// Lowest pitch the chip can play (divider 1023): about 109.3 Hz, just under
/// A2. Write bass lines at A2 and above; lower thumps are periodic noise.
pub const FLOOR_HZ: f32 = CLOCK_HZ / (32.0 * 1023.0);

/// Low-pass corner that takes the aliasing edge off the naive squares.
const LOWPASS_HZ: f32 = 9_000.0;

/// The chip's 10-bit divider for a pitch, clamped to the register range.
pub fn divider_for_hz(hz: f32) -> u16 {
    let n = (CLOCK_HZ / (32.0 * hz.max(1.0))).round();
    n.clamp(1.0, 1023.0) as u16
}

/// The pitch a divider actually produces.
pub fn hz_for_divider(n: u16) -> f32 {
    CLOCK_HZ / (32.0 * n.max(1) as f32)
}

/// Snaps a pitch to what the chip can play.
pub fn quantize_hz(hz: f32) -> f32 {
    hz_for_divider(divider_for_hz(hz))
}

/// `hz` moved by `n` semitones (fractional allowed).
pub fn semitones(hz: f32, n: f32) -> f32 {
    hz * (n / 12.0).exp2()
}

/// Parses a note name such as `c4`, `f#3`, `bb2` (A4 = 440 Hz). Octaves 0-9.
pub fn note_hz(name: &str) -> Option<f32> {
    let mut chars = name.chars();
    let letter = chars.next()?.to_ascii_lowercase();
    let base: i32 = match letter {
        'c' => -9,
        'd' => -7,
        'e' => -5,
        'f' => -4,
        'g' => -2,
        'a' => 0,
        'b' => 2,
        _ => return None,
    };
    let rest: String = chars.collect();
    let (accidental, octave_str) = match rest.chars().next()? {
        '#' => (1, &rest[1..]),
        'b' => (-1, &rest[1..]),
        _ => (0, &rest[..]),
    };
    if octave_str.len() != 1 {
        return None;
    }
    let octave = octave_str.parse::<i32>().ok()?;
    let semis = base + accidental + (octave - 4) * 12;
    Some(440.0 * (semis as f32 / 12.0).exp2())
}

/// One square-wave voice.
#[derive(Clone, Copy, Debug)]
pub struct Tone {
    /// Requested pitch; quantized to the divider when it changes.
    pub hz: f32,
    pub atten: u8,
    phase: f32,
    last_hz: f32,
    step: f32,
}

impl Tone {
    const fn new() -> Self {
        Self {
            hz: 440.0,
            atten: OFF,
            phase: 0.0,
            last_hz: 0.0,
            step: 0.0,
        }
    }

    fn sample(&mut self) -> f32 {
        let gain = ATTEN_GAIN[(self.atten.min(OFF)) as usize];
        if self.hz != self.last_hz {
            self.last_hz = self.hz;
            self.step = quantize_hz(self.hz) / SAMPLE_RATE as f32;
        }
        self.phase = (self.phase + self.step).fract();
        let level = if self.phase < 0.5 { 1.0 } else { -1.0 };
        level * gain
    }
}

/// White noise is the full 15-bit shift register; periodic noise is one
/// pulse every fifteen shifts, a buzzy pitched tone good for bass drums.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NoiseMode {
    White,
    Periodic,
}

/// How fast the shift register clocks. `Tone3` borrows tone voice 3's pitch
/// so the noise can be played as a tuned instrument.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NoiseRate {
    Div512,
    Div1024,
    Div2048,
    Tone3,
}

impl NoiseRate {
    fn hz(self, tone3_hz: f32) -> f32 {
        match self {
            NoiseRate::Div512 => CLOCK_HZ / 512.0,
            NoiseRate::Div1024 => CLOCK_HZ / 1024.0,
            NoiseRate::Div2048 => CLOCK_HZ / 2048.0,
            NoiseRate::Tone3 => quantize_hz(tone3_hz) * 2.0,
        }
    }
}

/// The noise voice.
#[derive(Clone, Copy, Debug)]
pub struct Noise {
    pub mode: NoiseMode,
    pub rate: NoiseRate,
    pub atten: u8,
    lfsr: u16,
    phase: f32,
    level: f32,
}

const LFSR_SEED: u16 = 0x4000;

impl Noise {
    const fn new() -> Self {
        Self {
            mode: NoiseMode::White,
            rate: NoiseRate::Div1024,
            atten: OFF,
            lfsr: LFSR_SEED,
            phase: 0.0,
            level: 1.0,
        }
    }

    /// Restarts the register so a periodic pulse train begins on a hit.
    pub fn reset(&mut self) {
        self.lfsr = LFSR_SEED;
        self.phase = 0.0;
    }

    fn shift(&mut self) {
        let bit0 = self.lfsr & 1;
        let feedback = match self.mode {
            NoiseMode::White => bit0 ^ ((self.lfsr >> 3) & 1),
            NoiseMode::Periodic => bit0,
        };
        self.lfsr = (self.lfsr >> 1) | (feedback << 14);
        if self.lfsr == 0 {
            self.lfsr = LFSR_SEED;
        }
        self.level = if bit0 == 1 { 1.0 } else { -1.0 };
    }

    fn sample(&mut self, tone3_hz: f32) -> f32 {
        let gain = ATTEN_GAIN[(self.atten.min(OFF)) as usize];
        self.phase += self.rate.hz(tone3_hz) / SAMPLE_RATE as f32;
        while self.phase >= 1.0 {
            self.phase -= 1.0;
            self.shift();
        }
        let level = match self.mode {
            NoiseMode::White => self.level,
            // One pulse in fifteen reads as a thin buzz; centre it so it
            // carries no DC.
            NoiseMode::Periodic => {
                if self.level > 0.0 {
                    1.0
                } else {
                    -1.0 / 14.0
                }
            }
        };
        level * gain
    }
}

/// The whole chip: set the voices, then pull samples.
#[derive(Clone, Debug)]
pub struct Chip {
    pub tones: [Tone; 3],
    pub noise: Noise,
    lowpass: f32,
    lowpass_a: f32,
}

impl Default for Chip {
    fn default() -> Self {
        Self::new()
    }
}

impl Chip {
    pub fn new() -> Self {
        Self {
            tones: [Tone::new(), Tone::new(), Tone::new()],
            noise: Noise::new(),
            lowpass: 0.0,
            lowpass_a: 1.0 - (-TAU * LOWPASS_HZ / SAMPLE_RATE as f32).exp(),
        }
    }

    /// Next output sample, the four voices summed at a quarter each so a
    /// full chord at full volume still fits in ±1.
    pub fn sample(&mut self) -> f32 {
        let tone3_hz = self.tones[2].hz;
        let mut mix = 0.0;
        for t in &mut self.tones {
            mix += t.sample();
        }
        mix += self.noise.sample(tone3_hz);
        let x = mix * 0.25;
        self.lowpass += self.lowpass_a * (x - self.lowpass);
        self.lowpass
    }

    /// Renders `n` samples with the current voice settings.
    pub fn render(&mut self, n: usize) -> Vec<f32> {
        (0..n).map(|_| self.sample()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn divider_round_trips_a440() {
        let n = divider_for_hz(440.0);
        assert_eq!(n, 254);
        assert!((hz_for_divider(n) - 440.3).abs() < 0.2);
    }

    #[test]
    fn floor_is_just_under_a2() {
        let floor = hz_for_divider(1023);
        assert!((floor - FLOOR_HZ).abs() < 1e-3);
        assert!(floor < note_hz("a2").unwrap() && floor > 109.0, "{floor}");
        assert_eq!(divider_for_hz(floor), 1023);
    }

    #[test]
    fn divider_is_clamped() {
        assert_eq!(divider_for_hz(1.0), 1023);
        assert_eq!(divider_for_hz(1e9), 1);
    }

    #[test]
    fn attenuation_table_is_two_db_steps() {
        assert_eq!(ATTEN_GAIN[0], 1.0);
        assert_eq!(ATTEN_GAIN[15], 0.0);
        for i in 1..15 {
            let db = 20.0 * (ATTEN_GAIN[i] / ATTEN_GAIN[i - 1]).log10();
            assert!((db + 2.0).abs() < 0.05, "step {i}: {db}");
        }
    }

    #[test]
    fn note_names_parse() {
        assert!((note_hz("a4").unwrap() - 440.0).abs() < 1e-3);
        assert!((note_hz("c4").unwrap() - 261.63).abs() < 0.01);
        assert!((note_hz("f#3").unwrap() - 185.0).abs() < 0.01);
        assert!((note_hz("bb2").unwrap() - 116.54).abs() < 0.01);
        assert!((note_hz("C5").unwrap() - 523.25).abs() < 0.01);
        assert!(note_hz("h2").is_none());
        assert!(note_hz("c").is_none());
        assert!(note_hz("c#").is_none());
        assert!(note_hz("c44").is_none());
    }

    #[test]
    fn semitones_moves_by_octaves() {
        assert!((semitones(220.0, 12.0) - 440.0).abs() < 1e-3);
        assert!((semitones(440.0, -12.0) - 220.0).abs() < 1e-3);
    }

    #[test]
    fn tone_period_matches_sample_rate() {
        let mut c = Chip::new();
        c.tones[0].hz = 441.0;
        c.tones[0].atten = 0;
        let s = c.render(44_100);
        let crossings = s.windows(2).filter(|w| w[0] <= 0.0 && w[1] > 0.0).count();
        assert!((crossings as i64 - 441).abs() <= 2, "{crossings}");
    }

    #[test]
    fn silent_chip_is_silent() {
        let mut c = Chip::new();
        assert!(c.render(1000).iter().all(|v| v.abs() < 1e-6));
    }

    #[test]
    fn white_noise_has_both_signs_and_periodic_is_sparse() {
        let mut c = Chip::new();
        c.noise.atten = 0;
        c.noise.rate = NoiseRate::Div512;
        let w = c.render(20_000);
        assert!(w.iter().any(|v| *v > 0.05) && w.iter().any(|v| *v < -0.05));

        let mut c = Chip::new();
        c.noise.atten = 0;
        c.noise.mode = NoiseMode::Periodic;
        let p = c.render(20_000);
        let pos = p.iter().filter(|v| **v > 0.05).count() as f32 / p.len() as f32;
        assert!(pos < 0.2, "periodic noise is one pulse in 15: {pos}");
    }

    #[test]
    fn tone3_rate_follows_the_third_voice() {
        let mut slow = Chip::new();
        slow.noise.atten = 0;
        slow.noise.rate = NoiseRate::Tone3;
        slow.noise.mode = NoiseMode::Periodic;
        slow.tones[2].hz = 110.0;
        let mut fast = slow.clone();
        fast.tones[2].hz = 880.0;
        let pulses = |s: &[f32]| s.windows(2).filter(|w| w[0] <= 0.0 && w[1] > 0.05).count();
        let a = pulses(&slow.render(44_100));
        let b = pulses(&fast.render(44_100));
        assert!(b > a * 4, "slow {a} fast {b}");
    }

    #[test]
    fn mix_is_bounded() {
        let mut c = Chip::new();
        for t in &mut c.tones {
            t.hz = 300.0;
            t.atten = 0;
        }
        c.noise.atten = 0;
        assert!(c.render(5000).iter().all(|v| v.abs() <= 1.0));
    }
}
