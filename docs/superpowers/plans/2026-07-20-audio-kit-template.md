# Audio Kit — Template Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Canonical audio kit in the template: pure-synthesis SFX (no assets), an event-driven SFX bus, table-driven music slots with crossfades, audibly wired to the template's own screens.

**Architecture:** `src/game/audio/synth.rs` (pure generators → in-memory WAV `AudioSource`, unit-tested) + `src/game/audio/mod.rs` (SfxEvent bus, SfxAssets baked at Startup, music director driven by a `const MUSIC` table with voidrunner's crossfade components). Kit demo: menu confirm/pause/how-to-play sounds.

**Tech Stack:** Rust 2024 / Bevy 0.18 (`bevy_audio` + `vorbis` already in features).

**Spec:** `docs/superpowers/specs/2026-07-20-audio-kit-design.md`.
**Lift sources:** `git -C ../Hunted show main:src/audio.rs` (`create_wav_audio_source`, lines ~101-140 — copy verbatim) and `git -C ../voidrunner show main:src/game/audio.rs` (`MusicFadeIn`, `MusicFadeOut`, `crossfade_to`, `update_music_fades` — copy verbatim, adjust only import paths).
**Branch:** existing `feat/audio-kit` (spec commit on it).

---

### Task 1 (TDD): Synth library

**Files:** Create `src/game/audio/synth.rs`, `src/game/audio/mod.rs` (module decl only for now); Modify `src/game/mod.rs` (add `pub mod audio;`).

- [ ] **Step 1:** Create `src/game/audio/mod.rs` containing only `pub mod synth;` and `src/game/audio/synth.rs` with the constants, `create_wav_audio_source` (verbatim from Hunted), `generate`, and the failing tests below. Then the palette in Step 3.

Constants + generate:

```rust
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
```

- [ ] **Step 2 (failing tests):** append the test module:

```rust
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
```

Run `cargo test synth` — expected: FAIL (palette functions missing).

- [ ] **Step 3:** implement the palette:

```rust
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
```

Note: `create_wav_audio_source` from Hunted returns `AudioSource { bytes: Arc<[u8]> }`-shaped data — read the lifted code and match its exact construction; the `Arc` import above is for it.

- [ ] **Step 4:** `cargo test synth` — 3 tests pass. `cargo test` all green.
- [ ] **Step 5:** Commit: `feat: synth library — pure generators to in-memory WAV`

### Task 2: SFX bus + music director

**Files:** Modify `src/game/audio/mod.rs`; Modify `src/game/mod.rs` (plugin wiring).

- [ ] **Step 1:** `src/game/audio/mod.rs` becomes:

```rust
pub mod synth;

use bevy::prelude::*;

use crate::game::states::GameState;

/// Game-specific sound effects. Emit via `MessageWriter<SfxEvent>`; the
/// dispatcher spawns a despawn-on-finish player. Extend per game.
#[derive(Message, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum SfxEvent {
    Confirm,
    Pause,
    ScreenSweep,
}

/// Handles baked once at startup — synthesis happens exactly once.
#[derive(Resource)]
pub struct SfxAssets {
    confirm: Handle<AudioSource>,
    pause: Handle<AudioSource>,
    screen_sweep: Handle<AudioSource>,
}

impl SfxAssets {
    fn get(&self, event: SfxEvent) -> Handle<AudioSource> {
        match event {
            SfxEvent::Confirm => self.confirm.clone(),
            SfxEvent::Pause => self.pause.clone(),
            SfxEvent::ScreenSweep => self.screen_sweep.clone(),
        }
    }
}

fn setup_sfx(mut commands: Commands, mut sources: ResMut<Assets<AudioSource>>) {
    commands.insert_resource(SfxAssets {
        confirm: sources.add(synth::blip(880.0)),
        pause: sources.add(synth::blip(440.0)),
        screen_sweep: sources.add(synth::sweep(300.0, 900.0, 0.3)),
    });
}

/// Spawns a one-shot player per received event.
fn play_sfx(
    mut commands: Commands,
    mut events: MessageReader<SfxEvent>,
    sfx: Res<SfxAssets>,
) {
    for event in events.read() {
        commands.spawn((AudioPlayer(sfx.get(*event)), PlaybackSettings::DESPAWN));
    }
}

// --- Music slots -----------------------------------------------------------

/// Per-state music table. `None` = silence in that state (nothing is loaded,
/// no missing-asset errors). To add music: drop an .ogg under
/// assets/audio/music/ and name it here.
const MUSIC: &[(GameState, Option<&str>)] = &[
    (GameState::StudioLogo, None),
    (GameState::Menu, None),
    (GameState::HowToPlay, None),
    (GameState::Playing, None),
    (GameState::GameOver, None),
];

/// Currently-playing track path, if any.
#[derive(Resource, Default)]
struct CurrentTrack(Option<&'static str>);

/// Crossfades to the entering state's slot when it differs from the current
/// track. Runs on every state change; no-ops while the slot matches.
fn music_director(
    mut commands: Commands,
    state: Res<State<GameState>>,
    mut current: ResMut<CurrentTrack>,
    asset_server: Res<AssetServer>,
    playing: Query<Entity, With<MusicTrack>>,
) {
    if !state.is_changed() {
        return;
    }
    let want = MUSIC
        .iter()
        .find(|(s, _)| s == state.get())
        .and_then(|(_, path)| *path);
    if want == current.0 {
        return;
    }
    current.0 = want;
    crossfade_to(&mut commands, &playing, want.map(|p| asset_server.load(p)));
}
```

then append the voidrunner lift: `MusicTrack` marker (voidrunner calls it this or similar — read the lifted file; keep its name consistently), `MusicFadeIn`, `MusicFadeOut`, `update_music_fades` verbatim, and adapt `crossfade_to` to accept `Option<Handle<AudioSource>>` (None = fade out everything, start nothing). Read voidrunner's version first; keep its fade timings and component fields; the only signature change is the `Option`.

- [ ] **Step 2:** Wire in `src/game/mod.rs` inside `GamePlugin::build`:

```rust
            .add_message::<audio::SfxEvent>()
            .init_resource::<audio::CurrentTrack>()
            .add_systems(Startup, audio::setup_sfx)
            .add_systems(Update, (audio::play_sfx, audio::music_director, audio::update_music_fades))
```

(make `CurrentTrack`, `setup_sfx`, `play_sfx`, `music_director`, `update_music_fades` `pub(crate)` or `pub` as needed for this wiring.)

- [ ] **Step 3:** Demo wiring:
  - `src/ui/menu.rs` `menu_input`: on successful `fade.request(...)`, `sfx.write(SfxEvent::Confirm)` (add `mut sfx: MessageWriter<audio::SfxEvent>` param; only when request returned true).
  - `src/ui/how_to_play.rs`: same Confirm on launch; and register a small system or extend spawn to fire `SfxEvent::ScreenSweep` on `OnEnter(GameState::HowToPlay)` (one-line system `fn sweep_on_enter(mut sfx: MessageWriter<SfxEvent>) { sfx.write(SfxEvent::ScreenSweep); }`).
  - `src/game/mod.rs` `toggle_pause`: fire `SfxEvent::Pause` on every successful toggle (both directions).
- [ ] **Step 4:** `cargo test` (all green), `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check`, `cargo build`.
- [ ] **Step 5:** Commit: `feat: SFX bus, music director with table-driven slots, demo wiring`

### Task 3: Conventions doc

- [ ] Append to `docs/conventions.md`:

```markdown
## Audio

The kit is asset-free by default: SFX are synthesized at startup
(`src/game/audio/synth.rs` — pure `f(t)` generators rendered to in-memory
WAV). To add a sound: add an `SfxEvent` variant, bake its source in
`setup_sfx`, emit the event from gameplay. Never play audio directly from
gameplay systems — always go through the bus (keeps mixing/despawn policy in
one place).

Music is table-driven: fill a slot in `MUSIC` (src/game/audio/mod.rs) with a
path under `assets/audio/music/` and the crossfade director handles the rest.
`None` slots load nothing. Authored-asset precedent: voidrunner /
Gravestone_Gauntlet; procedural precedent: Hunted.

Web autoplay is already handled by the boot flow's AudioContext unlock.
```

- [ ] Commit: `docs: audio kit conventions`

### Task 4: Gates + handoff
- [ ] Full gates; do NOT push/PR/build_web (controller verifies headlessly, then ships).
