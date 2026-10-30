//! The template's own music: a reference pair every game replaces with its
//! composition (see `docs/audio-identity.md`). C major, friendly, plain, so
//! the kit is never silent and a port can hear what each cue slot is for.
//!
//! Compose here, not in `tracker.rs`. Keep a song's four voice strings
//! bar-aligned with `|` so a diff reads like a score.

use bevy::audio::AudioSource;

use super::chip::{NoiseMode, NoiseRate};
use super::tracker::{Instrument, Song, render_sting};

// --- Instruments --------------------------------------------------------------

/// Singing lead: slow decay, a touch of release.
pub const LEAD: Instrument = Instrument::tone(&[0, 0, 1, 1, 2, 2, 3, 3, 4]).with_release(2);
/// Major chord as a fast arpeggio, sitting under the lead.
pub const MAJ: Instrument = Instrument::tone(&[3, 3, 3, 3, 4])
    .with_arp(&[0, 4, 7])
    .with_release(1);
/// Minor chord, same voice.
pub const MIN: Instrument = Instrument::tone(&[3, 3, 3, 3, 4])
    .with_arp(&[0, 3, 7])
    .with_release(1);
/// Round bass.
pub const BASS: Instrument = Instrument::tone(&[0, 1, 2, 3, 4, 5]).with_release(1);
/// Short pluck for stings and counter-lines.
pub const PLUCK: Instrument = Instrument::tone(&[0, 3, 6, 9, 12]);

pub const KICK: Instrument =
    Instrument::noise(NoiseMode::Periodic, NoiseRate::Div2048, &[0, 2, 5, 9, 15]);
pub const HAT: Instrument = Instrument::noise(NoiseMode::White, NoiseRate::Div512, &[4, 8, 12, 15]);
pub const SNARE: Instrument =
    Instrument::noise(NoiseMode::White, NoiseRate::Div1024, &[1, 3, 6, 10, 15]);

pub const TONES: &[Instrument] = &[LEAD, MAJ, MIN, BASS, PLUCK];
pub const DRUMS: &[Instrument] = &[KICK, HAT, SNARE];

// --- Menu: "Title Card", 100 BPM, 8 bars -------------------------------------

pub const MENU: Song = Song {
    bpm: 100.0,
    beats_per_bar: 4,
    steps_per_beat: 4,
    bars: 8,
    voices: [
        concat!(
            "@0 e4:2 g4:2 a4:4 g4:2 e4:2 d4:4 |",
            " c4:4 .:2 d4:2 e4:4 .:4 |",
            " e4:2 g4:2 a4:4 b4:2 c5:2 a4:4 |",
            " g4:6 .:2 e4:4 .:4 |",
            " a4:2 g4:2 e4:4 d4:2 e4:2 g4:4 |",
            " a4:4 g4:4 e4:4 d4:4 |",
            " c4:2 d4:2 e4:2 g4:2 a4:4 g4:2 e4:2 |",
            " d4:8 c4:4 .:4",
        ),
        concat!(
            "v3 @1 c4:16 | @2 a3:16 | @1 f3:16 | @1 g3:16 |",
            " @1 c4:16 | @2 a3:16 | @1 f3:8 g3:8 | @1 c4:12 .:4",
        ),
        concat!(
            "@3 c3:4 g3:4 c3:4 g3:4 | a2:4 e3:4 a2:4 e3:4 |",
            " f3:4 c3:4 f3:4 c3:4 | g3:4 d3:4 g3:4 b2:4 |",
            " c3:4 g3:4 c3:4 g3:4 | a2:4 e3:4 a2:4 e3:4 |",
            " f3:4 c3:4 g3:4 d3:4 | c3:8 c3:4 .:4",
        ),
    ],
    noise: concat!(
        "v2 @0 x:2 v6 @1 x:2 x:2 x:2 v2 @0 x:2 v6 @1 x:2 x:2 x:2 |",
        " v2 @0 x:2 v6 @1 x:2 x:2 x:2 v2 @0 x:2 v6 @1 x:2 x:2 x:2 |",
        " v2 @0 x:2 v6 @1 x:2 x:2 x:2 v2 @0 x:2 v6 @1 x:2 x:2 x:2 |",
        " v2 @0 x:2 v6 @1 x:2 x:2 x:2 v2 @0 x:2 v6 @1 x:2 x:2 x:2 |",
        " v2 @0 x:2 v6 @1 x:2 x:2 x:2 v2 @0 x:2 v6 @1 x:2 x:2 x:2 |",
        " v2 @0 x:2 v6 @1 x:2 x:2 x:2 v2 @0 x:2 v6 @1 x:2 x:2 x:2 |",
        " v2 @0 x:2 v6 @1 x:2 x:2 x:2 v2 @0 x:2 v6 @1 x:2 x:2 x:2 |",
        " v2 @0 x:2 v6 @1 x:2 x:2 x:2 v2 @0 x:2 .:6",
    ),
    instruments: TONES,
    noise_instruments: DRUMS,
};

// --- Play: "Run Loop", 128 BPM, 16 bars (A A B A) ----------------------------

macro_rules! play_a_lead {
    () => {
        concat!(
            " @0 g4:2 g4:2 a4:2 g4:2 e4:4 .:4 |",
            " c5:2 c5:2 b4:2 a4:2 g4:4 .:4 |",
            " g4:2 g4:2 a4:2 g4:2 e4:4 d4:4 |",
            " e4:2 d4:2 c4:4 .:8 |",
        )
    };
}
macro_rules! play_b_lead {
    () => {
        concat!(
            " @0 a4:2 a4:2 c5:2 a4:2 f4:4 .:4 |",
            " d5:2 d5:2 c5:2 b4:2 g4:4 .:4 |",
            " a4:2 b4:2 c5:2 d5:2 e5:4 d5:4 |",
            " c5:2 b4:2 g4:4 .:8 |",
        )
    };
}
macro_rules! play_a_chord {
    () => {
        " v3 @1 c4:16 | @1 f3:16 | @1 c4:16 | @1 g3:16 |"
    };
}
macro_rules! play_b_chord {
    () => {
        " v3 @1 f3:16 | @1 g3:16 | @2 a3:16 | @1 g3:16 |"
    };
}
macro_rules! play_a_bass {
    () => {
        concat!(
            " @3 c3:2 c3:2 g3:2 c3:2 c3:2 c3:2 g3:2 c3:2 |",
            " f3:2 f3:2 c3:2 f3:2 f3:2 f3:2 c3:2 f3:2 |",
            " c3:2 c3:2 g3:2 c3:2 c3:2 c3:2 g3:2 c3:2 |",
            " g3:2 g3:2 d3:2 g3:2 g3:2 g3:2 b2:2 d3:2 |",
        )
    };
}
macro_rules! play_b_bass {
    () => {
        concat!(
            " @3 f3:2 f3:2 c3:2 f3:2 f3:2 f3:2 c3:2 f3:2 |",
            " g3:2 g3:2 d3:2 g3:2 g3:2 g3:2 d3:2 g3:2 |",
            " a2:2 a2:2 e3:2 a2:2 a2:2 a2:2 e3:2 a2:2 |",
            " g3:2 g3:2 d3:2 g3:2 g3:2 b2:2 d3:2 f3:2 |",
        )
    };
}
macro_rules! play_drums_bar {
    () => {
        " @0 x:2 v5 @1 x:2 v0 @2 x:2 v5 @1 x:2 v0 @0 x:2 v5 @1 x:2 v0 @2 x:2 v5 @1 x:2 |"
    };
}

pub const PLAY: Song = Song {
    bpm: 128.0,
    beats_per_bar: 4,
    steps_per_beat: 4,
    bars: 16,
    voices: [
        concat!(
            play_a_lead!(),
            play_a_lead!(),
            play_b_lead!(),
            play_a_lead!()
        ),
        concat!(
            play_a_chord!(),
            play_a_chord!(),
            play_b_chord!(),
            play_a_chord!()
        ),
        concat!(
            play_a_bass!(),
            play_a_bass!(),
            play_b_bass!(),
            play_a_bass!()
        ),
    ],
    noise: concat!(
        play_drums_bar!(),
        play_drums_bar!(),
        play_drums_bar!(),
        play_drums_bar!(),
        play_drums_bar!(),
        play_drums_bar!(),
        play_drums_bar!(),
        play_drums_bar!(),
        play_drums_bar!(),
        play_drums_bar!(),
        play_drums_bar!(),
        play_drums_bar!(),
        play_drums_bar!(),
        play_drums_bar!(),
        play_drums_bar!(),
        play_drums_bar!(),
    ),
    instruments: TONES,
    noise_instruments: DRUMS,
};

/// Danger layer for `PLAY`: a plucked counter-line and driving hats, same
/// length and tempo, faded in over the loop by `LayerEvent::Start(0)`.
macro_rules! layer_line {
    () => {
        concat!(
            " @4 e5:2 .:2 e5:2 .:2 d5:2 .:2 c5:2 .:2 |",
            " a4:2 .:2 a4:2 .:2 c5:2 .:2 a4:2 .:2 |",
            " e5:2 .:2 e5:2 .:2 d5:2 .:2 c5:2 .:2 |",
            " b4:2 .:2 d5:2 .:2 g4:2 .:6 |",
        )
    };
}
macro_rules! layer_hats_bar {
    () => {
        " v2 @1 x x x x x x x x x x x x x x x x |"
    };
}

pub const PLAY_LAYER: Song = Song {
    bpm: 128.0,
    beats_per_bar: 4,
    steps_per_beat: 4,
    bars: 16,
    voices: [
        concat!(layer_line!(), layer_line!(), layer_line!(), layer_line!()),
        ".:256",
        ".:256",
    ],
    noise: concat!(
        layer_hats_bar!(),
        layer_hats_bar!(),
        layer_hats_bar!(),
        layer_hats_bar!(),
        layer_hats_bar!(),
        layer_hats_bar!(),
        layer_hats_bar!(),
        layer_hats_bar!(),
        layer_hats_bar!(),
        layer_hats_bar!(),
        layer_hats_bar!(),
        layer_hats_bar!(),
        layer_hats_bar!(),
        layer_hats_bar!(),
        layer_hats_bar!(),
        layer_hats_bar!(),
    ),
    instruments: TONES,
    noise_instruments: DRUMS,
};

// --- Stings -------------------------------------------------------------------

const GAME_OVER: Song = Song {
    bpm: 100.0,
    beats_per_bar: 4,
    steps_per_beat: 4,
    bars: 1,
    voices: [
        "@4 g4:4 e4:4 d4:4 @0 c4:4",
        "v4 @1 c4:8 @2 a3:4 @1 c4:4",
        "@3 c3:4 .:4 g3:4 c3:4",
    ],
    noise: "@0 x:4 .:4 .:4 v4 @2 x:4",
    instruments: TONES,
    noise_instruments: DRUMS,
};

/// Non-looping; the results screen is silent after it.
pub fn game_over_sting() -> AudioSource {
    render_sting(&GAME_OVER, 1.5).expect("GAME_OVER sting is well-formed")
}

const DEMO_END: Song = Song {
    bpm: 100.0,
    beats_per_bar: 4,
    steps_per_beat: 4,
    bars: 2,
    voices: [
        "@0 e4:2 g4:2 a4:4 g4:2 e4:2 d4:4 | c4:4 .:2 d4:2 e4:4 .:4",
        "v3 @1 c4:16 | @2 a3:16",
        "@3 c3:4 g3:4 c3:4 g3:4 | a2:4 e3:4 a2:8",
    ],
    noise: "v2 @0 x:4 .:4 x:4 .:4 | x:4 .:4 x:8",
    instruments: TONES,
    noise_instruments: DRUMS,
};

/// "To be continued": the menu theme's first phrase, then silence.
pub fn demo_end_tag() -> AudioSource {
    render_sting(&DEMO_END, 1.0).expect("DEMO_END tag is well-formed")
}

#[cfg(test)]
mod tests {
    use super::super::tracker::{render_loop, wav_secs, wav_to_samples};
    use super::*;

    #[test]
    fn reference_songs_render() {
        render_loop(&MENU).unwrap();
        render_loop(&PLAY).unwrap();
        render_loop(&PLAY_LAYER).unwrap();
    }

    #[test]
    fn layer_matches_play_length() {
        let a = wav_secs(&render_loop(&PLAY).unwrap());
        let b = wav_secs(&render_loop(&PLAY_LAYER).unwrap());
        assert!((a - b).abs() < 1e-4, "{a} vs {b}");
        assert!(
            (a - 30.0).abs() < 0.01,
            "16 bars at 128 BPM is 30 s, got {a}"
        );
    }

    #[test]
    fn stings_end_in_silence_and_stay_short() {
        for (name, s) in [
            ("game over", game_over_sting()),
            ("demo end", demo_end_tag()),
        ] {
            let secs = wav_secs(&s);
            assert!(secs <= 6.0, "{name}: {secs}s");
            let v = wav_to_samples(&s);
            let tail = &v[v.len() - 200..];
            assert!(tail.iter().all(|x| x.abs() < 0.02), "{name} tail");
        }
    }
}
