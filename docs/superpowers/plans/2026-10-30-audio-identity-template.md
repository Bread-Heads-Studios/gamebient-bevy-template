# Audio Identity: Template Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the template a composed-chip-music kit (SN76489 model, song tracker, SFX builder, house sting) and a music director with cues, layers, switches and ducking, so every game ported from it can own a complete, unique sound without shipping audio files.

**Architecture:** `chip.rs` is a pure sample-by-sample model of the ColecoVision's TI SN76489 (3 square tones + 1 noise, 16 attenuation steps). `tracker.rs` renders `Song` note data (a text DSL per voice) and `Fx` sound-effect programs through that chip into in-memory WAV `AudioSource`s, normalized to spec levels. `mod.rs` replaces the path-only `MUSIC` table with `Cue`s (baked or file, with layers and per-cue fades), a lazy `MusicCache`, gameplay-driven `CueEvent::Switch` and `LayerEvent`, and a sting duck. `house.rs` holds the shared studio sting, hash-pinned so games carry it verbatim.

**Tech Stack:** Rust 2024, Bevy 0.18 `bevy_audio` (`AudioSource`, `AudioSink`, `PlaybackSettings`), no new crates.

Spec: `/Users/kelliott/Gamebient/colecovisiongx/docs/plans/2026-10-30-audio-identity-pass.md`.

## Global Constraints

- Chip model: 3 square tones + 1 noise, 16 attenuation steps of 2 dB (index 15 = off), clock 3,579,545 Hz, tone Hz = clock / (32 · divider), divider 1..=1023. Noise: white (15-bit LFSR, taps 0 and 3) or periodic (bit 0), rates clock/512, clock/1024, clock/2048 or tone 3.
- Levels: music loops normalized to peak −12 dBFS (0.251), stings and SFX to −3 dBFS (0.708), then `synth::MASTER_GAIN` (0.5) like every other baked sound. Duck = −6 dB (×0.5) on music for the length of a sting.
- Fades: cue default 1.5 s out, 2.0 s in; layers 0.8 s both ways. All fades are linear in level per second.
- Envelope and arp resolution: 60 frames per second (`FRAME_HZ`).
- Loops render exactly `bars · beats_per_bar · steps_per_beat` steps; every voice pattern must tile to that length or rendering fails with a message naming the voice.
- Rendering all baked cues: ≤ 150 ms on a Pi 4 (asserted at ≤ 2.0 s in a debug test, timings printed).
- Presentation only: audio systems never read sim RNG or write sim resources; nothing here is added when `GamePlugin { headless: true }`.
- `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings` (stable and `+1.98.0`), `cargo test --all-targets --all-features`, `cargo check --target wasm32-unknown-unknown` all pass.
- Never commit `target/`, `dist/`, `build/` or exported WAVs.

## File Structure

- Create `src/game/audio/chip.rs` — SN76489 model (`Chip`, `Tone`, `Noise`, attenuation table, `divider_for_hz`, `note_hz`).
- Create `src/game/audio/tracker.rs` — `Instrument`, `Song`, DSL parser, `render_loop`, `render_sting`, `Fx`, `normalize_peak`, `wav_secs`.
- Create `src/game/audio/house.rs` — `house_sting()`, `HOUSE_STING_HASH`, `sample_hash`.
- Create `src/game/audio/songs.rs` — the template's reference cues: `MENU`, `PLAY`, `PLAY_LAYER_DANGER`, `game_over_sting()`, `demo_end_tag()`.
- Modify `src/game/audio/mod.rs` — `Source`, `Cue`, `MUSIC`, `MusicCache`, `CueEvent`, `LayerEvent`, `MusicVoice`, `Duck`, `bake()`, `SfxEvent::ALL`/`is_sting`, `export_inventory()`.
- Modify `src/game/mod.rs` — register the new resources, messages and systems in the `!headless` block.
- Create `tests/audio_export.rs` — `GX_AUDIO_EXPORT` WAV dump + render-time budget.
- Modify `tools/frame_accept.py` — `exact()` for `chip.rs`, `tracker.rs`, `house.rs`.
- Create `docs/audio-identity.md` — the template's own identity doc (the form every game fills).
- Modify `docs/conventions.md` (`## Audio`), `AGENTS.md` (how-to-add bullets).

---

### Task 1: `chip.rs` — the SN76489 model

**Files:**
- Create: `src/game/audio/chip.rs`
- Modify: `src/game/audio/mod.rs` (add `pub mod chip;`)

**Interfaces:**
- Produces:
  - `pub const CLOCK_HZ: f32 = 3_579_545.0;`
  - `pub const ATTEN_GAIN: [f32; 16]` (index 0 = 1.0, 15 = 0.0, 2 dB per step)
  - `pub fn divider_for_hz(hz: f32) -> u16` (clamped 1..=1023), `pub fn hz_for_divider(n: u16) -> f32`
  - `pub fn note_hz(name: &str) -> Option<f32>` ("c4", "f#3", "bb2"; A4 = 440)
  - `pub fn semitones(hz: f32, n: f32) -> f32`
  - `#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub enum NoiseMode { White, Periodic }`
  - `#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub enum NoiseRate { Div512, Div1024, Div2048, Tone3 }`
  - `pub struct Tone { pub hz: f32, pub atten: u8, .. }`, `pub struct Noise { pub mode, pub rate, pub atten, .. }`
  - `pub struct Chip { pub tones: [Tone; 3], pub noise: Noise, .. }` with `Chip::new()`, `fn sample(&mut self) -> f32` (sum of four voices, each scaled ×0.25, through a one-pole 9 kHz low-pass), `fn render(&mut self, n: usize) -> Vec<f32>`.

- [ ] **Step 1: Write failing tests** (in `chip.rs` `mod tests`)

```rust
#[test] fn divider_round_trips_a440() {
    let n = divider_for_hz(440.0);
    assert_eq!(n, 254);
    assert!((hz_for_divider(n) - 440.3).abs() < 0.2);
}
#[test] fn divider_is_clamped() { assert_eq!(divider_for_hz(1.0), 1023); assert_eq!(divider_for_hz(1e9), 1); }
#[test] fn attenuation_table_is_two_db_steps() {
    assert_eq!(ATTEN_GAIN[0], 1.0); assert_eq!(ATTEN_GAIN[15], 0.0);
    for i in 1..15 { let db = 20.0 * (ATTEN_GAIN[i] / ATTEN_GAIN[i-1]).log10(); assert!((db + 2.0).abs() < 0.05, "step {i}: {db}"); }
}
#[test] fn note_names_parse() {
    assert!((note_hz("a4").unwrap() - 440.0).abs() < 1e-3);
    assert!((note_hz("c4").unwrap() - 261.63).abs() < 0.01);
    assert!((note_hz("f#3").unwrap() - 185.0).abs() < 0.01);
    assert!((note_hz("bb2").unwrap() - 116.54).abs() < 0.01);
    assert!(note_hz("h2").is_none()); assert!(note_hz("c").is_none());
}
#[test] fn tone_period_matches_sample_rate() {
    let mut c = Chip::new(); c.tones[0].hz = 441.0; c.tones[0].atten = 0;
    let s = c.render(44_100);
    let crossings = s.windows(2).filter(|w| w[0] <= 0.0 && w[1] > 0.0).count();
    assert!((crossings as i64 - 441).abs() <= 2, "{crossings}");
}
#[test] fn silent_chip_is_silent() { let mut c = Chip::new(); assert!(c.render(1000).iter().all(|v| v.abs() < 1e-6)); }
#[test] fn white_noise_has_both_signs_and_periodic_repeats() {
    let mut c = Chip::new(); c.noise.atten = 0; c.noise.rate = NoiseRate::Div512;
    let w = c.render(20_000); assert!(w.iter().any(|v| *v > 0.05) && w.iter().any(|v| *v < -0.05));
    let mut c = Chip::new(); c.noise.atten = 0; c.noise.mode = NoiseMode::Periodic;
    let p = c.render(20_000); let pos = p.iter().filter(|v| **v > 0.05).count() as f32 / p.len() as f32;
    assert!(pos < 0.2, "periodic noise is one pulse in 15: {pos}");
}
#[test] fn mix_is_bounded() { let mut c = Chip::new(); for t in &mut c.tones { t.hz = 300.0; t.atten = 0; } c.noise.atten = 0; assert!(c.render(5000).iter().all(|v| v.abs() <= 1.0)); }
```

- [ ] **Step 2: Run** `cargo test chip::` → fails to compile (module missing).
- [ ] **Step 3: Implement** `chip.rs`: phase-accumulator squares (`phase += hz / SAMPLE_RATE`, output `if phase.fract() < 0.5 { 1 } else { -1 }`), LFSR noise shifted on its own accumulator (`White`: new bit = bit0 ^ bit3; `Periodic`: bit0; output = bit0 ? 1 : −1, seed 0x4000), `NoiseRate::Tone3` uses `tones[2].hz`, one-pole low-pass `y += a·(x − y)` with `a = 1 − exp(−2π·9000/SR)`. `note_hz` parses letter, optional `#`/`b`, octave digit.
- [ ] **Step 4: Run** `cargo test chip::` → all pass.
- [ ] **Step 5: Commit** `feat(audio): SN76489 chip model`.

### Task 2: `tracker.rs` — songs, DSL, Fx

**Files:**
- Create: `src/game/audio/tracker.rs`
- Modify: `src/game/audio/mod.rs` (add `pub mod tracker;`)

**Interfaces:**
- Consumes: `chip::{Chip, NoiseMode, NoiseRate, note_hz, semitones, ATTEN_GAIN}`, `synth::{SAMPLE_RATE, MASTER_GAIN, create_wav_audio_source}`.
- Produces:
  - `pub const FRAME_HZ: f32 = 60.0;` `pub const MUSIC_PEAK: f32 = 0.251;` `pub const SFX_PEAK: f32 = 0.708;`
  - `pub struct Instrument { pub env: &'static [u8], pub release: u8, pub arp: &'static [i8], pub vibrato: Option<(f32, f32)>, pub noise: Option<(NoiseMode, NoiseRate)> }` with `pub const fn tone(env) -> Self`, `pub const fn noise(mode, rate, env) -> Self`, builder `const fn with_arp`, `with_vibrato`, `with_release`.
  - `pub struct Song { pub bpm: f32, pub beats_per_bar: u8, pub steps_per_beat: u8, pub bars: u8, pub voices: [&'static str; 3], pub noise: &'static str, pub instruments: &'static [Instrument], pub noise_instruments: &'static [Instrument] }`
  - `pub fn render_loop(song: &Song) -> Result<AudioSource, SongError>`; `pub fn render_sting(song: &Song, tail_secs: f32) -> Result<AudioSource, SongError>`; `pub fn render_song_samples(song, tail) -> Result<Vec<f32>, SongError>`
  - `#[derive(Debug, PartialEq)] pub enum SongError { Parse { voice: &'static str, token: String }, Length { voice: &'static str, got: usize, want: usize }, Instrument { voice: &'static str, index: usize } }`
  - `pub struct Fx { .. }` with `Fx::new()`, `.tone(voice: usize, hz0: f32, hz1: f32, atten0: u8, atten1: u8, frames: u8)`, `.noise(mode, rate, atten0, atten1, frames)`, `.rest(frames)`, `.then(Fx)`, `.with(Fx)` (parallel), `.render() -> AudioSource` (peak → `SFX_PEAK`), `.samples() -> Vec<f32>`.
  - `pub fn normalize_peak(s: &mut [f32], peak: f32)`, `pub fn wav_secs(source: &AudioSource) -> f32`.

DSL (per voice string, whitespace-separated, `|` ignored): `c4` note (1 step), `c4:3` three steps, `.` rest, `.:n`, `-` extend previous note by one step, `/c4` glide from the previous pitch over this step, `c4~` vibrato (25 cents, 6 Hz unless the instrument sets one), `@n` instrument index for following notes, `vN` attenuation offset 0–15 for following notes. Noise voice: `x` hit, `x:n`, `.`, `-`, `@n`, `vN`.

- [ ] **Step 1: Write failing tests**

```rust
const PLUCK: Instrument = Instrument::tone(&[0, 2, 4, 6, 9, 12]).with_release(2);
const KICK: Instrument = Instrument::noise(NoiseMode::Periodic, NoiseRate::Div2048, &[0, 3, 8, 15]);
fn song(v0: &'static str) -> Song { Song { bpm: 120.0, beats_per_bar: 4, steps_per_beat: 4, bars: 1, voices: [v0, ".:16", ".:16"], noise: "x . . . x . . . x . . . x . . .", instruments: &[PLUCK], noise_instruments: &[KICK] } }
#[test] fn loop_length_is_exact() { let s = render_song_samples(&song("c4 e4 g4 c5 . . . . c4:4 . . . ."), 0.0).unwrap(); assert_eq!(s.len(), 88_200); }
#[test] fn ties_and_lengths_count_steps() { assert!(render_song_samples(&song("c4:4 - - - c4:8"), 0.0).is_ok()); }
#[test] fn wrong_length_names_the_voice() { assert!(matches!(render_song_samples(&song("c4 e4"), 0.0), Err(SongError::Length { voice: "tone1", got: 2, want: 16 }))); }
#[test] fn bad_token_names_itself() { assert!(matches!(render_song_samples(&song("c4 zz:15"), 0.0), Err(SongError::Parse { token, .. }) if token == "zz:15")); }
#[test] fn missing_instrument_is_an_error() { assert!(matches!(render_song_samples(&song("@3 c4:16"), 0.0), Err(SongError::Instrument { index: 3, .. }))); }
#[test] fn loop_is_normalized_to_music_peak() { let s = render_song_samples(&song("c4:16"), 0.0).unwrap(); let peak = s.iter().fold(0.0f32, |m, v| m.max(v.abs())); assert!((peak - MUSIC_PEAK * MASTER_GAIN).abs() < 0.01, "{peak}"); }
#[test] fn sting_adds_a_tail() { let src = render_sting(&song("c4:16"), 1.0).unwrap(); assert!((wav_secs(&src) - 3.0).abs() < 0.01); }
#[test] fn glide_moves_pitch() { /* render "c4:8 /c5:8" vs "c4:8 c5:8"; the glided version has fewer zero crossings in steps 9-12 than the stepped one */ }
#[test] fn fx_then_and_with_lengths() {
    let a = Fx::new().tone(0, 440.0, 440.0, 0, 15, 6); let b = Fx::new().noise(NoiseMode::White, NoiseRate::Div512, 0, 15, 6);
    assert_eq!(a.clone().then(b.clone()).samples().len(), (12.0 / FRAME_HZ * SAMPLE_RATE as f32).round() as usize);
    assert_eq!(a.with(b).samples().len(), (6.0 / FRAME_HZ * SAMPLE_RATE as f32).round() as usize);
}
#[test] fn fx_is_normalized_to_sfx_peak() { let s = Fx::new().tone(0, 440.0, 220.0, 0, 15, 12).samples(); let peak = s.iter().fold(0.0f32, |m, v| m.max(v.abs())); assert!((peak - SFX_PEAK * MASTER_GAIN).abs() < 0.01); }
#[test] fn wav_secs_reads_our_header() { assert!((wav_secs(&synth::blip(880.0)) - 0.09).abs() < 1e-3); }
```

- [ ] **Step 2: Run** `cargo test tracker::` → compile failure.
- [ ] **Step 3: Implement.** Parse each voice into `Vec<Step>` (`Note { hz, steps, inst, vol, glide, vibrato } | Rest`); render step by step: per sample compute frame index since note-on, look up `env[min(frame, last)] + vol` (clamp 15), add arp offset `arp[frame % len]` semitones, vibrato `depth · sin(2π·rate·t)` cents, glide lerp from previous hz over the step; on note end, hold the final attenuation and step +1 per `release` frames until 15. Noise voice sets `chip.noise` the same way. Render through one `Chip`, then `normalize_peak(MUSIC_PEAK)` and `× MASTER_GAIN`. `Fx` keeps per-voice `Vec<Segment { frames, hz0, hz1, a0, a1 }>` for 3 tones and noise; `.then` appends to the longest voice and pads others with rests; `.with` merges voice lists (second Fx's voices go to the first free tone voice); `.render` normalizes to `SFX_PEAK`.
- [ ] **Step 4: Run** `cargo test tracker::` → pass.
- [ ] **Step 5: Commit** `feat(audio): song tracker and Fx builder on the chip`.

### Task 3: `house.rs` and `songs.rs` — the shared sting and the reference pair

**Files:**
- Create: `src/game/audio/house.rs`, `src/game/audio/songs.rs`
- Modify: `src/game/audio/mod.rs` (`pub mod house; pub mod songs;`)

**Interfaces:**
- Produces: `house::house_sting() -> AudioSource` (D major loaf-rising figure, 1.4 s), `house::HOUSE_STING_HASH: u64`, `house::sample_hash(&AudioSource) -> u64` (FNV-1a over the WAV bytes); `songs::MENU: Song`, `songs::PLAY: Song`, `songs::PLAY_LAYER: Song` (same bars/bpm as `PLAY`), `songs::game_over_sting() -> AudioSource`, `songs::demo_end_tag() -> AudioSource`.

- [ ] **Step 1: Tests**

```rust
// house.rs
#[test] fn house_sting_is_byte_pinned() { let h = sample_hash(&house_sting()); assert_eq!(h, HOUSE_STING_HASH, "house sting changed: new hash {h:#018x}. Only the template may change it; games copy house.rs verbatim."); }
#[test] fn house_sting_is_1_4_seconds() { assert!((wav_secs(&house_sting()) - 1.4).abs() < 0.02); }
// songs.rs
#[test] fn reference_songs_render() { render_loop(&MENU).unwrap(); render_loop(&PLAY).unwrap(); render_loop(&PLAY_LAYER).unwrap(); }
#[test] fn layer_matches_play_length() { assert_eq!(wav_secs(&render_loop(&PLAY).unwrap()), wav_secs(&render_loop(&PLAY_LAYER).unwrap())); }
#[test] fn stings_end_in_silence() { for s in [game_over_sting(), demo_end_tag()] { let v = tracker::wav_to_samples(&s); assert!(v[v.len()-200..].iter().all(|x| x.abs() < 0.02)); } }
```

- [ ] **Step 2: Run → fail. Step 3: Compose.** House sting as an `Fx`: tone 0 D4→F#4→A4→D5 eighths at 0.18 s each, tone 1 D3 held, tone 2 A4-D5 an eighth late, one periodic-noise tick at frame 0. First run with `HOUSE_STING_HASH = 0` to read the real hash from the failure, then pin it. Reference songs: `MENU` C major 100 BPM 8 bars (tone 1 melody, tone 2 fifths, tone 3 bass, noise brush), `PLAY` 128 BPM 16 bars, `PLAY_LAYER` 16 bars of hats + counter-line only (silence on the other voices). `game_over_sting()` = `render_sting` of a 1-bar descending cadence with a 1.5 s tail; `demo_end_tag()` = the first 2 bars of `MENU`'s melody as a sting.
- [ ] **Step 4: Run → pass. Step 5: Commit** `feat(audio): house sting and reference songs`.

### Task 4: the director — cues, cache, layers, switches, duck

**Files:**
- Modify: `src/game/audio/mod.rs`, `src/game/mod.rs`

**Interfaces:**
- Produces:
  - `pub enum Source { Baked(fn() -> AudioSource), File(&'static str) }`
  - `pub struct Cue { pub source: Source, pub looping: bool, pub fade_in: f32, pub fade_out: f32, pub volume: f32, pub layers: &'static [Source] }` with `pub const fn loop_baked(f) -> Cue`, `const fn sting_baked(f)`, `const fn loop_file(path)`, `const fn with_layers(self, &'static [Source])`, `const fn at(self, volume)`.
  - `pub static MENU_CUE / PLAY_CUE / GAME_OVER_CUE / DEMO_END_CUE: Cue`; `pub const MUSIC: &[(GameState, Option<&'static Cue>)]` with `StudioLogo → Some(&HOUSE_CUE)`.
  - `#[derive(Resource, Default)] pub struct MusicCache(HashMap<usize, Handle<AudioSource>>)` with `fn handle(&mut self, sources: &mut Assets<AudioSource>, asset_server: &AssetServer, source: &Source) -> Handle<AudioSource>` (Baked keyed by `f as usize`), `pub fn prewarm_baked(...)` (renders every Baked source in `MUSIC` and layers).
  - `#[derive(Message)] pub enum CueEvent { Switch(&'static Cue), Resume }`; `#[derive(Message)] pub enum LayerEvent { Start(usize), Stop(usize) }`.
  - `#[derive(Component)] pub struct MusicVoice { pub level: f32, pub target: f32, pub rate: f32, pub base: f32, pub despawn_when_silent: bool, pub layer: Option<usize> }`; `#[derive(Component)] pub struct MusicTrack;` kept as the marker of the active cue's entities.
  - `#[derive(Resource, Default)] pub struct Duck { pub remaining: f32 }`.
  - `#[derive(Resource)] pub struct CurrentCue { pub cue: Option<&'static Cue>, pub override_: Option<&'static Cue> }`.
  - `pub fn bake(event: SfxEvent) -> AudioSource`; `impl SfxEvent { pub const ALL: &[SfxEvent]; pub fn is_sting(self) -> bool; pub fn name(self) -> &'static str }`; `SfxAssets` stores `(Handle, secs)` per event.
  - `pub fn export_inventory() -> Vec<(String, AudioSource)>` ("music/menu", "music/play", "music/play-layer-0", "music/house-sting", "music/game-over", "music/demo-end", "sfx/<name>").
  - Systems: `setup_sfx`, `play_sfx` (sets `Duck.remaining = secs` on stings), `music_director` (state change or `CueEvent` → crossfade to the wanted cue, spawning its layers at level 0 alongside), `layer_control`, `update_music_voices` (moves `level` toward `target` at `rate`/s, sets sink volume = `level · base · duck`, despawns silent voices flagged for it), `prewarm_music` (OnEnter(Menu), once).

- [ ] **Step 1: Tests** (pure parts, in `mod tests`)

```rust
#[test] fn music_table_covers_every_state() { for s in [StudioLogo, Menu, HowToPlay, Playing, GameOver, DemoEnd] { assert!(MUSIC.iter().any(|(st, _)| *st == s), "{s:?} has no row"); } }
#[test] fn every_sfx_bakes_and_is_short() { for e in SfxEvent::ALL { let s = bake(*e); assert!(wav_secs(&s) <= 6.0, "{e:?}"); } }
#[test] fn sting_events_duck() { assert!(SfxEvent::Confirm.is_sting() == false); }
#[test] fn export_inventory_names_are_unique_and_slashed() { let inv = export_inventory(); let mut names: Vec<_> = inv.iter().map(|(n, _)| n.clone()).collect(); names.sort(); names.dedup(); assert_eq!(names.len(), inv.len()); assert!(names.iter().all(|n| n.contains('/'))); }
#[test] fn voice_level_moves_toward_target_and_despawns() { let mut v = MusicVoice { level: 1.0, target: 0.0, rate: 2.0, base: 1.0, despawn_when_silent: true, layer: None }; assert!(!v.step(0.25)); assert!((v.level - 0.5).abs() < 1e-6); assert!(v.step(0.3)); }
```

Plus an app-level test: build `MinimalPlugins + StatesPlugin + AssetPlugin + AudioPlugin`-free harness is not possible for sinks; instead test `MusicCache::handle` caches by pointer: two calls with the same `Source::Baked(f)` return equal handles and `Assets<AudioSource>` has one entry.

- [ ] **Step 2: Run → fail. Step 3: Implement.** Keep `crossfade_to` semantics (fade out everything marked `MusicTrack`, spawn new at level 0 with `target = 1`), but through `MusicVoice`. `music_director` reads `State<GameState>` changes and `CueEvent`; `override_` wins while set and is cleared on state change. `Duck.remaining -= dt`, multiplier `0.5` while `> 0`. Startup: render the house sting into the cache so the first frame has it. Remove `CurrentTrack`.
- [ ] **Step 4: Wire** in `src/game/mod.rs` `!headless` block: `init_resource::<MusicCache>`, `CurrentCue`, `Duck`; `add_message::<CueEvent>`, `add_message::<LayerEvent>` (these two in the common half so sim code can emit them headless too); systems `audio::play_sfx, audio::music_director, audio::layer_control, audio::update_music_voices` in `Update`; `audio::prewarm_music` on `OnEnter(GameState::Menu)`.
- [ ] **Step 5: Run** `cargo test` and `cargo run` (GX_MUTE is set; use the record/autopilot harness later) → pass. **Step 6: Commit** `feat(audio): cue director with layers, switches and ducking`.

### Task 5: export test, copy check, docs

**Files:**
- Create: `tests/audio_export.rs`, `docs/audio-identity.md`
- Modify: `tools/frame_accept.py` (after `exact("src/ui/fit.rs", ...)`: `exact("src/game/audio/chip.rs", ...)`, `tracker.rs`, `house.rs`, flat variants `src/audio/...`), `docs/conventions.md` `## Audio`, `AGENTS.md` how-to-add bullets.

- [ ] **Step 1: `tests/audio_export.rs`**

```rust
use gamebient_game::game::audio::{export_inventory, tracker::wav_secs};
#[test] fn baked_audio_renders_within_budget_and_exports() {
    let t0 = std::time::Instant::now();
    let inv = export_inventory();
    let secs = t0.elapsed().as_secs_f32();
    for (name, src) in &inv { println!("{name}: {:.2}s", wav_secs(src)); }
    println!("rendered {} cues in {secs:.3}s", inv.len());
    assert!(secs < 2.0, "baked audio took {secs:.2}s to render (debug build; release target is 150 ms on a Pi 4)");
    if let Ok(dir) = std::env::var("GX_AUDIO_EXPORT") {
        for (name, src) in &inv { let path = std::path::Path::new(&dir).join(format!("{}.wav", name.replace('/', "-"))); std::fs::create_dir_all(&dir).unwrap(); std::fs::write(&path, &src.bytes[..]).unwrap(); println!("wrote {}", path.display()); }
    }
}
```

(The crate name comes from `Cargo.toml` `[lib] name`; use what `tests/display_shape.rs` uses.)

- [ ] **Step 2: Run** `GX_AUDIO_EXPORT=/tmp/gx-audio cargo test --test audio_export -- --nocapture` → WAVs written; listen-check by the owner.
- [ ] **Step 3: Docs.** `docs/audio-identity.md` with the template's values in the spec's form (Key and tempo / Instruments / Cues per state / SFX voice / Mix / Silence policy / Sources and licences). Rewrite `## Audio` in `docs/conventions.md`: chip, tracker DSL cheat-sheet, cues/layers/switches, duck, export, "no stock sounds", the house sting rule. AGENTS.md: a "**A sound:**" and "**Music:**" bullet in How to add things, and the export command in the visual-check bullet's neighbourhood.
- [ ] **Step 4: Checks.** `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo +1.98.0 clippy --all-targets --all-features -- -D warnings`, `cargo test --all-targets --all-features`, `cargo check --target wasm32-unknown-unknown`, `python3 tools/frame_accept.py --check-copies .` (self-check should pass trivially).
- [ ] **Step 5: Commit** `docs(audio): identity doc, conventions, export test`; push; open PR "Audio identity kit: chip tracker, house sting, cue director".

### Task 6: port brief

**Files:**
- Create (workspace): `/Users/kelliott/Gamebient/colecovisiongx/docs/plans/2026-10-30-audio-identity-port-brief.md`

- [ ] Write the brief on the model of `2026-10-02-title-typography-port-brief.md`: branch rules, copy `chip.rs`/`tracker.rs`/`house.rs` verbatim, replace `MUSIC`/`SfxEvent` per the spec's direction, identity doc, checks, `GX_AUDIO_EXPORT` dump to `<scratchpad>/audio/<game>/`, `record` capture, PR not merged, final report format.

## Self-review

- Spec coverage: chip (T1), tracker + Fx + DSL (T2), house sting + hash (T3), reference pair (T3), Cue/Source/layers/switch/duck/lazy cache/prewarm (T4), export + budget (T5), copy check (T5), docs (T5), brief (T6). Per-game work is the fleet phase, not this plan.
- Names used across tasks: `render_loop`, `render_sting`, `render_song_samples`, `wav_secs`, `Fx::{tone,noise,rest,then,with,render,samples}`, `MusicVoice::step`, `MusicCache::handle`, `export_inventory`, `bake`, `SfxEvent::{ALL,is_sting,name}` — consistent.
- `tracker::wav_to_samples` is referenced in T3's test: add it to T2's exports (`pub fn wav_to_samples(&AudioSource) -> Vec<f32>`).
