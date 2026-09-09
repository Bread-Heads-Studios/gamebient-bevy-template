# Recording harness — Design

**Date:** 2026-09-09 · **Status:** approved (design conversation with the owner)

## Goal

Generate play-video footage and editorial video notes for every published
Gamebient game without a human at the keyboard. Each game grows a `record`
cargo feature that renders the existing autopilot tour offline into a 60 fps
MP4, auto-cut clips around logged beats, an event log, and a chapters table.
A workspace-level Claude skill rolls the feature out, runs it, verifies the
output, and writes the per-game `docs/video-notes.md`.

## Decisions

| Question | Choice | Why |
|---|---|---|
| Capture | In-engine offline render: fixed 1/60 s manual clock, `Screenshot` every frame, ffmpeg assembles | Deterministic, no macOS screen-recording permission, works on any machine and in CI, any resolution. GPU readback cost is irrelevant because sim time is decoupled from wall time. |
| Output per game | Full tour MP4 + auto-cut clips + `events.jsonl` + `chapters.md` | Zero per-game bot work for v1; games that already have a real signature beat get a real clip for free. |
| Audio | None in v1 | Every game's SFX are baked WAVs fired by an `SfxEvent` message. Logging those events with frame timestamps is the seed for an offline mix later; nothing in v1 blocks it. |
| Skill location | `.claude/skills/recording-game-footage/` at the workspace root | Sits beside `publishing-games`, `designing-cartridge-covers`, `generating-cartridge-metadata`, which are the same shape (per-game procedure driven from the workspace). |

Out of scope for v1: audio mix, custom per-game showcase scripts, web/Playwright
capture, OS screen capture.

## Capture: `record` feature (`src/game/record.rs`)

- `Cargo.toml`: `record = ["autopilot"]`. The autopilot script is untouched;
  `record` layers on top of it. Never compiled into shipping builds.
- `RecordPlugin`, registered next to `AutopilotPlugin` under
  `#[cfg(feature = "record")]`:
  - Startup: insert `TimeUpdateStrategy::ManualDuration(1/60 s)` so every
    rendered frame advances the sim by exactly one frame. Create
    `$RECORD_DIR/frames/` (default `build/record`). Window scale comes from
    `AUTOPILOT_SCALE` as today; `tools/record.sh` defaults it to `1.5`
    (1920x1080).
  - Every frame (`Last` schedule): bump `Recorder.frame`, spawn
    `Screenshot::primary_window()` observed by `save_to_disk` into
    `frames/{frame:06}.png`.
  - `Recorder` resource: `frame: u64`, `fps: u32`, the open `events.jsonl`
    writer. `Recorder::log(kind, data)` writes one line
    `{"frame":N,"t":N/fps,"kind":"...","data":{...}}`.
- Shared event sources, all in the template:
  - `state`: `GameState` enter transitions (`OnEnter` for each variant).
  - `beat`: the autopilot's `shot()` fires a `RecordBeat(name)` message on
    the frame it screenshots; the recorder logs it. Beat names stay the
    01..09 contract.
  - `score`: every `ScoreEvent` (points, running total).
  - `pause`: `Paused` enter/exit.
- Per-game hook: `record::log_messages::<T>(app)` for any `T: Message + Debug`
  logs each message as `kind = type name`, `data = {"debug": "..."}`. Each
  game adds one line for its `SfxEvent` and may add others (combo, wave,
  mission). The template does not need to know the types.
- Exit: the autopilot already sends `AppExit` after `09-game-over`. On exit
  the recorder flushes the log and writes `manifest.json`:
  `{name, fps, width, height, frames, duration_s, beats: [{name, frame}]}`.
  `name` comes from `assets/info.json`.

## Assembly: `tools/record.sh` and `tools/cut_clips.py`

`tools/record.sh [--keep-frames]`:

1. Fails fast if `ffmpeg` is not on `PATH`.
2. `rm -rf build/record && cargo run --release --features record`
   (env: `RECORD_DIR=build/record`, `AUTOPILOT_DIR=build/record/shots`,
   `AUTOPILOT_SCALE=${AUTOPILOT_SCALE:-1.5}`).
3. `ffmpeg -framerate 60 -i build/record/frames/%06d.png -c:v libx264 -pix_fmt yuv420p -crf 18 build/record/tour.mp4`.
4. `python3 tools/cut_clips.py build/record`.
5. Deletes `frames/` unless `--keep-frames`.

`tools/cut_clips.py <record_dir>` (stdlib only):

- Reads `manifest.json` and `events.jsonl`.
- For every `beat` line, cuts `clips/<beat>.mp4` from `tour.mp4`: 2 s before
  to 4 s after the beat frame, clamped to the tour bounds (ffmpeg
  `-ss/-t -c copy`; falls back to re-encode if the copy fails on a keyframe
  boundary). `06-*` is additionally copied to `clips/signature.mp4`,
  `09-game-over` to `clips/game-over.mp4`.
- Writes `chapters.md`: a table of `mm:ss.f | kind | detail` for state
  changes, beats, pause, and score milestones (first score, then every
  time the running total crosses a power-of-ten boundary), followed by the
  clip list.
- Pure helpers (`clip_window`, `milestones`, `fmt_time`, `render_chapters`)
  are unit-tested with `python3 -m unittest tools/test_cut_clips.py`.

`build/` is already gitignored in the template and every game.

## Rollout

Template first:

- `src/game/record.rs`, the feature line, the `RecordPlugin` registration,
  the `RecordBeat` message in `autopilot.rs`, `tools/record.sh`,
  `tools/cut_clips.py`, `tools/test_cut_clips.py`, a README section, and a
  CI step `cargo check --features record` so the feature cannot rot.
- `AGENTS.md` gains the recording contract next to the autopilot contract.

Each game (16 published, then the rest): copy the three `tools/` files and
`record.rs`, add the feature line, register the plugin, add
`log_messages::<SfxEvent>`, add the `RecordBeat` write to its `shot()`. About
ten lines of diff, no bot changes. The skill performs this.

## Skill: `.claude/skills/recording-game-footage/`

- `SKILL.md` triggers: record footage / play video / video notes / roll out
  the recorder, or a game whose `docs/video-notes.md` is missing or stale.
- Procedure:
  1. If the game lacks the `record` feature, roll it out from the template
     (file copies plus the wiring edits above), run `cargo check --features
     record`, and commit.
  2. Run `tools/record.sh`. Read `manifest.json`; verify `frames` matches
     the tour length within tolerance, `tour.mp4` exists, and every beat in
     the manifest has a clip.
  3. Write `docs/video-notes.md` from `references/notes-template.md`:
     the auto `chapters.md` table, then editorial sections produced by
     reading the game's README, design specs, and gameplay source: hook line,
     controls, escalation with real numbers, named content, spectacle
     moments with when they happen, flavor text to caption, hidden mechanics,
     rough edges to keep off camera, and which clips to lead with. Every
     claim cites a file.
  4. Report clip paths and anything the autopilot never reached (a boss,
     FEVER, a chase card) as candidates for a later custom scenario.
- `references/notes-template.md` is the section skeleton;
  `references/catalog-2026-09-09.md` seeds it with the sixteen per-game beat
  lists already written for the published catalog.

## Testing and failure modes

- Rust unit tests in `record.rs`: frame filename padding, jsonl line shape,
  manifest serialization from a fixed `Recorder`.
- Python unit tests for the clip cutter helpers.
- CI: existing fmt / clippy / test plus `cargo check --features record`.
- `record.sh` exits non-zero with a clear message if ffmpeg is missing, the
  frames dir is not writable, or the run produced zero frames. A run that
  dies mid-tour still leaves `events.jsonl` and the frames captured so far;
  `cut_clips.py` works on a partial log, and the skill can write notes
  without video.

## Success criteria

1. `./tools/record.sh` in the template yields `tour.mp4` at 1920x1080 60 fps
   with a frame count matching the tour, nine beat clips, `events.jsonl`,
   `chapters.md`, and `manifest.json`.
2. The same command works unchanged in a rolled-out game and its `SfxEvent`
   lines appear in the log.
3. The skill, given a game name, ends with a committed `docs/video-notes.md`
   and clips under `build/record/clips/`.
