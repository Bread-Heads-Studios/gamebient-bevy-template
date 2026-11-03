//! The house sting: the one sound every Gamebient game shares, played under
//! the "BREAD HEADS STUDIOS PRESENTS" logo. Like the logo it is the label,
//! so it is the same bytes in every game: `HOUSE_STING_HASH` pins it and
//! `tools/frame-accept.sh --check-copies` proves a game's copy is verbatim.
//! Only the template changes it.
//!
//! D major, 1.4 s: a loaf rising. Tone 1 climbs D4-F#4-A4-D5 in eighths,
//! tone 2 holds D3 under it, tone 3 answers A4-D5 an eighth late, and one
//! periodic-noise oven tick opens it. It is silent before the logo fades.

use bevy::audio::AudioSource;

use super::chip::{NoiseMode, NoiseRate};
use super::tracker::Fx;

/// Length of the sting in seconds (84 frames).
pub const HOUSE_STING_SECS: f32 = 1.4;

/// FNV-1a of the sting's program (`format!("{:?}", house_fx())`), which is
/// identical on every platform where the rendered samples might differ in
/// the last bit. Update only in the template, on purpose.
pub const HOUSE_STING_HASH: u64 = 0x147c80a4c535563e;

const D3: f32 = 146.83;
const D4: f32 = 293.66;
const FS4: f32 = 369.99;
const A4: f32 = 440.0;
const D5: f32 = 587.33;

/// The sting as a chip program.
pub fn house_fx() -> Fx {
    let climb = Fx::new()
        .tone(0, D4, D4, 0, 3, 11)
        .tone(0, FS4, FS4, 0, 3, 11)
        .tone(0, A4, A4, 0, 3, 11)
        .tone(0, D5, D5, 0, 15, 51);
    let bed = Fx::new().tone(1, D3, D3, 2, 15, 84);
    let answer = Fx::new()
        .tone(2, A4, A4, 15, 15, 17)
        .tone(2, A4, A4, 2, 5, 11)
        .tone(2, D5, D5, 3, 15, 56);
    let tick = Fx::new().noise(NoiseMode::Periodic, NoiseRate::Div2048, 0, 15, 4);
    climb.with(bed).with(answer).with(tick)
}

pub fn house_sting() -> AudioSource {
    house_fx().render()
}

/// FNV-1a over a chip program's debug form.
pub fn program_hash(fx: &Fx) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in format!("{fx:?}").bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::super::tracker::{wav_secs, wav_to_samples};
    use super::*;

    #[test]
    fn house_sting_is_byte_pinned() {
        let h = program_hash(&house_fx());
        assert_eq!(
            h, HOUSE_STING_HASH,
            "the house sting changed: new hash {h:#018x}. Only the template may change it; games copy house.rs verbatim."
        );
    }

    #[test]
    fn house_sting_is_1_4_seconds_and_ends_quiet() {
        let s = house_sting();
        assert!((wav_secs(&s) - HOUSE_STING_SECS).abs() < 0.02);
        let v = wav_to_samples(&s);
        let tail = &v[v.len() - 300..];
        assert!(
            tail.iter().all(|x| x.abs() < 0.05),
            "tail peak {}",
            tail.iter().fold(0.0f32, |m, x| m.max(x.abs()))
        );
    }
}
