# Audio Kit — Canonical Synthesis + Music Slots, Applied to the Silent Games

**Date:** 2026-07-20
**Status:** Approved
**Scope:** gamebient-bevy-template (canonical kit), pizza-pinball, BeerPong. voidrunner / Gravestone_Gauntlet / Hunted untouched (already fully set up: authored .ogg, authored .ogg, procedural respectively).

## Goal

Give the two silent games full arcade-synth sound effects and future-ready
music slots, via a canonical audio kit in the template that new games copy —
combining Hunted's procedural-synthesis approach (no audio assets, no
licensing) with voidrunner's state-music crossfade pattern.

## Reference implementations (lift, don't reinvent)

- **Synthesis**: Hunted `src/audio.rs` — `create_wav_audio_source(samples,
  sample_rate) -> AudioSource` (in-memory 16-bit WAV), `generate_samples`,
  pure `gen_*(t: f32) -> f32` sample functions, `SfxEvent` message +
  `play_sfx` dispatcher with despawn-on-finish playback.
- **Music**: voidrunner `src/game/audio.rs` — `MusicFadeIn` / `MusicFadeOut`
  components, `crossfade_to`, `update_music_fades`, per-state `play_*_music`
  systems loading tracks on demand.

## The kit (template: `src/game/audio.rs` + `src/game/audio/synth.rs`)

### Synth library (`synth.rs`)
Pure, unit-testable:
- `create_wav_audio_source` — verbatim from Hunted.
- `generate(duration_secs, sample_rate, f: impl Fn(f32) -> f32) -> AudioSource`.
- Arcade generator palette (each returns samples clamped to ±1.0):
  `blip(freq_hz)`, `ding(freq_hz, decay)`, `sweep(f_start, f_end)`,
  `thud()`, `noise_burst(decay)`, `arpeggio(&[freq_hz])`.
- `MASTER_GAIN` const applied inside `generate`.
- Unit tests: sample bounds within ±1.0, exact sample counts for requested
  durations, non-silent output (RMS above a floor) per generator.

### SFX bus (`audio.rs`)
- Per-game `SfxEvent` enum (message), `SfxAssets` resource with handles baked
  once at Startup, `play_sfx` dispatcher (`PlaybackSettings::DESPAWN`).
- SFX-emitting systems are state/pause-gated like all gameplay systems; the
  pause toggle's own blip is the one sound allowed at the pause boundary.

### Music slots (`audio.rs`)
- voidrunner's crossfade system lifted; driven by a per-game table:
  `const MUSIC: &[(GameState, Option<&str>)]` (paths under
  `assets/audio/music/`). `None` slots register no system work and load
  nothing — zero missing-asset log spam. Adding music later = drop in an
  `.ogg` + name it in the table.
- Template ships all slots `None`.

### Template demo wiring (working example, audible)
- Menu confirm → `blip(880)`; pause toggle → `blip(440)`; how-to-play entry →
  `sweep(300, 900)`. Registered through the same `SfxEvent` bus games use.
- `docs/conventions.md` gains an **Audio** section: how to add an SfxEvent,
  how to fill a music slot, the no-assets philosophy, and pointers to the
  authored-asset alternative (voidrunner/GG precedent).

## Per-game application

### pizza-pinball (`feat/audio-kit`)
SFX (all synthesized, arcade palette):
| Event | Sound | Hook site |
|---|---|---|
| Bumper hit | `ding` pitched per kind — pepperoni mid (100pt), olive low (50pt), pepper high (250pt) | bumper kick application (`src/game/bumpers.rs`) |
| Flipper actuate | short `thud`+`blip` thwack | flipper just-pressed edges (`flippers.rs`) |
| Launcher charge | `sweep` pitch rising with charge value | plunger held (`launcher.rs`) |
| Launch release | spring twang (`sweep` down + `blip`) | plunger release fire |
| Drain | low `thud`/womp | `DrainEvent` handler |
| Match end | descending sting (`arpeggio`) | game-over fade request site |
| Menu confirm / pause | kit blips | menu/pause systems |
Music slots: Menu / Playing / GameOver — all `None` initially.

### BeerPong (`feat/audio-kit`)
Cargo first: add `bevy_audio` + `vorbis` to the curated feature set (currently absent).
| Event | Sound | Hook site |
|---|---|---|
| Charge ramp | subtle rising `sweep` tick while held | charge start/tick (`aim.rs`) |
| Throw | `noise_burst` whoosh, gain scaled by release power | release site (`aim.rs`) |
| Table bounce | `thud` plop (slight pitch variance per bounce count) | bounce increment (`ball.rs:~76`) |
| Rim hit | high `blip` tick | ONLY if the collision code distinguishes rim contact from bounce (verified: it does not today — `ball.rs` has cup-capture and bounce only). Drop this sound rather than adding collision logic; revisit if rim physics ever land. |
| Cup sink | splash (`noise_burst`) + `ding` | `cup_hit` set (`ball.rs:~112`) |
| Miss | soft womp | outcome resolution (miss branch) |
| Match win | ascending `arpeggio` fanfare | win detection (`scoring.rs`) |
| Menu confirm / pause | kit blips | menu/pause systems |
Music slots: Menu / Playing / GameOver — all `None` initially.

## Invariants

- All generated samples clamped ±1.0; `MASTER_GAIN` per repo (start 0.5).
- No audio entities leak: all SFX use despawn-on-finish; music entities are
  managed solely by the crossfade system.
- Web autoplay: already handled by the boot-flow AudioContext unlock in every
  index.html — no changes needed.
- No behavior changes to gameplay systems beyond emitting events.

## Testing & verification

- Unit: synth generator tests (bounds/duration/energy) in all three repos;
  event-mapping tests where pure (e.g. bumper-kind → frequency).
- Headless: standard flow drive per repo — asserts no page errors with audio
  systems active (headless Chrome runs the audio graph silently).
- Audible confirmation is manual post-merge (headless can't hear); everything
  structural is asserted.

## Delivery

`feat/audio-kit` off `main` per repo; template first (canonical + verified),
then pizza-pinball and BeerPong in parallel; one PR each.
