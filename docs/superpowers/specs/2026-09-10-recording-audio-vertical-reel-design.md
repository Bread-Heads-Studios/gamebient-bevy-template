# Recording harness v2: audio, vertical clips, sizzle reel — Design

**Date:** 2026-09-10 · **Status:** approved (design conversation with the owner)
**Builds on:** `2026-09-09-recording-harness-design.md` (the `record` feature,
`tools/record.sh`, `tools/cut_clips.py`, the `recording-game-footage` skill).

## Goal

Turn the silent 16:9 recordings into postable video: a real audio track in
every tour and clip, 9:16 vertical versions for Shorts/TikTok/Reels, and a
"16 games in ~90 seconds" sizzle reel in both aspect ratios.

## Decisions

| Question | Choice | Why |
|---|---|---|
| Audio source | Capture every `AudioPlayer` playback in-engine and mix offline at exit, inside the game process | Every game plays sound through Bevy's `AudioPlayer` + `PlaybackSettings`, so one generic capture covers SFX, music fades, pitched dings and Hunted's spatial audio with no per-game code and no new dependencies (`AudioSource::decoder()` is Bevy's own rodio decoder). OS loopback capture needs a driver and is out of sync by design (the sim clock is not real time). |
| Vertical framing | Blurred fill + banners | Full 16:9 frame scaled to 1080 wide, centered on a 1080x1920 canvas over a blurred, darkened, zoomed copy of itself; title banner on top, CTA at the bottom. Keeps every HUD and panel visible in every game. |
| CTA text | "Play the demo at colecovisiongx.com" | Matches what the site offers: a free browser demo page per game. |
| Text rendering | SVG → PNG with `rsvg-convert`, overlaid by ffmpeg | The installed ffmpeg has no `drawtext` (no freetype). rsvg-convert is already the cover pipeline's renderer; `designing-cartridge-covers/fonts.md` lists faces that render. |
| Loudness | EBU R128 −16 LUFS (`loudnorm=I=-16:TP=-1.5:LRA=11`) on tour, clips and reel | What social platforms target; makes games play at the same level back to back in the reel. |
| Skill home | Move `recording-game-footage` into the template's `.claude/skills/`, symlinked from the workspace root | Same arrangement as `designing-cartridge-covers` and `generating-cartridge-metadata`; makes it versioned. The template repo is public; the skill's references describe published, publicly playable games only. |

Out of scope: per-game showcase scenarios, music beds for the reel, burned-in
captions beyond the title/CTA banners, OS audio capture.

## 1. Audio capture and mix (template `src/game/record/`)

`src/game/record.rs` becomes a folder module; `pub mod record;` in
`GamePlugin` is unchanged, so no game wiring changes.

- `record/mod.rs` — the existing recorder (unchanged behaviour), plus a
  sim-frame clock: `Recorder.sim_frame` increments on every `capture_frame`
  call, including warm-up; `Recorder.first_video_sim_frame` is the sim frame
  on which numbered frame 1 was requested. Manifest gains
  `"audio":{"voices":N,"peak":P}` (or `"audio":null` when nothing played).
- `record/audio.rs` — capture. A `Last`-schedule system (not gated on
  warm-up) walks every entity with `AudioPlayer` + `PlaybackSettings`:
  - first sighting (or a changed handle on the same entity) opens a voice:
    source asset id, start sim frame, looping (`PlaybackMode::Loop`), spatial;
  - every frame it appends an automation key when anything changed:
    `gain_l`, `gain_r`, `speed`, `paused`. Volume comes from the entity's
    `AudioSink`/`SpatialAudioSink` when present (this includes global volume
    applied at play time and every later `set_volume`, i.e. music fades),
    else `settings.volume × GlobalVolume`. Muted ⇒ gain 0. For spatial voices
    the per-ear gains reproduce rodio 0.20's `Spatial::set_positions`
    (`diff_modifier × min(1, 1/dist²)` per ear) from the emitter's
    `GlobalTransform` and the listener's ears, both multiplied by
    `settings.spatial_scale.unwrap_or(DefaultSpatialScale)`; ears follow
    Bevy's `EarPositions` rule (one listener ⇒ its transformed ear offsets,
    otherwise the raw offsets);
  - until its source is in `Assets<AudioSource>` a voice is keyed paused
    (silent, playhead held), so an asset still loading does not start early;
  - a voice closes when its entity or `AudioPlayer` disappears, its handle
    changes, or its sink becomes empty after having played. rodio plays on
    the wall clock while the recorder's sim clock is decoupled from it and
    often several times slower (each frame waits on a PNG capture), so a
    one-shot's sink empties (and `PlaybackMode::Despawn`/`Remove` strip it)
    after only a fraction of its length in sim time; ending it there
    truncates it. So
    each voice accumulates `played`, the clip seconds rodio has played
    (`wall_dt × speed` while heard and not paused, `wall_dt` from
    `std::time::Instant`, since `Time<Real>` is manual under
    `ManualDuration` too). When a non-looping voice goes silent (sink empty,
    entity or `AudioPlayer` gone) with
    `played + max(2·wall_dt, 0.1 s) ≥ clip length`, rodio finished it: it
    gets no end and the mixer stops it at the clip's end on the sim clock.
    Otherwise the game stopped it early (a despawn, a crossfade) and it ends
    on that frame. A handle change, a looping voice, a source that never
    loaded and the close at exit always end on that frame. Known limit: a
    game that cuts a one-shot on the sim clock after rodio has already
    finished it on the wall clock gets the whole clip.
  - Each distinct source is decoded once, the first frame it is in
    `Assets<AudioSource>` (any still missing are decoded at exit), via
    `Decodable::decoder()` (`channels()`, `sample_rate()`, `i16` items →
    `f32`). At `AppExit` (before the manifest is written) it converts voice
    times to video time (`(sim_frame − first_video_sim_frame) / fps`), drops
    voices that stop before video time 0, calls the mixer, and writes
    `audio.wav` (48 kHz, 16-bit stereo) plus the manifest fields; with no
    saved frame or no remaining voice it writes no file and the manifest
    says `"audio":null`.
- `record/mixer.rs` — pure, unit-tested:
  `render(voices, clips, fps, out_rate, total_frames) -> Vec<f32>` (interleaved
  stereo), `limit(&mut [f32]) -> f32` (returns pre-limit peak; soft-knee
  above 0.9, hard ceiling 0.99), `wav_bytes(&[f32], rate) -> Vec<u8>`.
  Per voice: linear-interpolated source playhead advancing
  `speed × src_rate / out_rate` per output sample (pitch and speed change
  together, as in rodio); gains linearly interpolated between keys; paused
  holds the playhead and outputs silence; non-looping voices stop at the end
  of the source; looping voices wrap until their end frame; spatial
  (`downmix`) voices sum channels to mono, clamped to ±1, before the
  per-ear gains (rodio 0.20's `ChannelVolume` sums with `saturating_add`
  on i16 samples, so a stereo source is not halved);
  mono sources feed both ears; stereo sources map L/R. Voices that start
  before video time 0 are trimmed, not shifted.
- Known limit: a screenshot dropped mid-run closes up in the video while
  audio stays on the true clock (drift of one frame per drop); the manifest's
  requested-vs-saved counts expose it.

## 2. Encode, clips, vertical (template `tools/`)

- `record.sh`: when `audio.wav` exists, encode it into `tour.mp4`
  (`-c:a aac -b:a 192k -af loudnorm=I=-16:TP=-1.5:LRA=11 -shortest`);
  otherwise video-only as today.
- `cut_clips.py`: clips keep audio (`-c:a aac -b:a 192k`, `-an` removed, audio
  mapped optionally so silent tours still cut). New outputs:
  `clips/vertical/<beat>.mp4` for every beat clip and `tour-vertical.mp4`.
  Vertical filter graph: `split` → background
  `scale=1080:1920:force_original_aspect_ratio=increase,crop=1080:1920,gblur=sigma=30,eq=brightness=-0.12`
  → foreground `scale=1080:-2` overlaid centered → banner PNG overlaid at 0,0.
  The banner is rendered once per run from `tools/vertical-banner.svg`
  (1080x1920, transparent; placeholders `{{TITLE}}`, `{{TITLE_SIZE}}`,
  `{{CTA_1}}`, `{{CTA_2}}`) with `rsvg-convert`; title = manifest name,
  upper-cased, XML-escaped, size `min(96, 960 / (0.72 × len))`; fonts from the
  verified list (Arial Black title, Helvetica Neue bold CTA, each with a
  fallback stack). Missing `rsvg-convert` ⇒ vertical outputs are skipped with
  a message; 16:9 outputs are unaffected.

## 3. Sizzle reel (skill `tools/reel.py`)

- Config `docs/reel.json` at the workspace root:
  `{"cta": ["Play the demo at", "colecovisiongx.com"], "card_secs": 1.5,
  "end_secs": 3.0, "games": [{"folder", "clip", "start", "length"}]}`.
  `reel.py --init` generates it from `docs/video-notes-index.md` (first
  `clips/<name>.mp4` in the lead-clip column; `signature` resolves to the
  game's `06-*` beat via its manifest; defaults `start` 1.0, `length` 4.5).
- `reel.py` builds `build/reel/reel-16x9.mp4` and `build/reel/reel-9x16.mp4`
  at the workspace root. Per game: a title card (the game's
  `assets/cartridge.png` + title, from `card-16x9.svg` / `card-9x16.svg`;
  the cover is copied beside the SVG because rsvg only loads images next to
  it), then the lead clip trimmed to `start`/`length` (16:9 from `clips/`,
  9:16 from `clips/vertical/`). Ends on a CTA card. Every segment is encoded
  to identical parameters (60 fps, h264 crf 20, yuv420p, AAC 48 kHz stereo;
  cards get silent audio from `anullsrc`; clips get 0.15 s audio fades and
  loudnorm), then joined with the concat demuxer (`-c copy`).
- Pure helpers (config load/defaults, init parsing, segment plan, ffmpeg arg
  builders) are unit-tested; `python3 -m unittest` from the skill's `tools/`.

## 4. Skill, rollout, re-record

- Move `.claude/skills/recording-game-footage/` into the template's
  `.claude/skills/`, replace the workspace directory with a symlink; add
  `tools/reel.py`, `tools/test_reel.py`, `tools/card-16x9.svg`,
  `tools/card-9x16.svg` to the skill; SKILL.md gains the audio check, the
  vertical outputs and a reel section.
- `rollout-record.sh`: copies `src/game/record/` (all three files), deletes a
  stale `src/game/record.rs`, copies `tools/vertical-banner.svg`.
  `irregular-games.md`: Hunted's recorder becomes `src/record/`.
- Re-record all 16 games one at a time (foreground, never two at once),
  refresh each `docs/video-notes.md` chapters table and lead clips only if
  they changed, then `reel.py --init` and `reel.py`.

## Testing and verification

- Rust (mixer): gain interpolation between keys; speed 2.0 halves the rendered
  length of a non-looping voice; paused span holds the playhead; looping wraps
  until end frame; negative start trims; downmix + per-ear gains; spatial gain
  function matches rodio's formula on fixed positions; limiter output ≤ 0.99
  and returns the pre-limit peak; `wav_bytes` header fields.
- Python: banner templating (escaping, size rule), vertical filter string,
  reel config defaults, `--init` parsing, segment plan, arg builders.
- End to end (template and each game): `ffprobe` shows an AAC stream on
  `tour.mp4`, every clip and every vertical clip at 1080x1920; manifest
  `audio.peak > 0` for games with sound; one clip's waveform PNG
  (`ffmpeg -filter_complex showwavespic`) inspected against the `SfxEvent`
  frames in `events.jsonl`; reel durations ≈ 16 × (card + clip) + end card.
