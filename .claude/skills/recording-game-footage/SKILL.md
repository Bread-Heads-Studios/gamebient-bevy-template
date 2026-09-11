---
name: recording-game-footage
description: Use when a Gamebient game needs play-video footage, trailer clips, or video notes; when the user says "record footage", "make the play video", "video notes", or "roll out the recorder"; or when a game lacks docs/video-notes.md or its notes predate a gameplay change
---

# Recording Game Footage

## Overview

Every template-derived game can record its own autopilot tour offline:
`tools/record.sh` runs the game with `--features record` on a fixed 1/60 s
clock, captures every frame, encodes `build/record/tour.mp4` (60 fps,
1920x1080), cuts `clips/<beat>.mp4` and `shots/<beat>.png` around each
autopilot beat, and writes `events.jsonl`, `manifest.json`, `chapters.md`.
Recordings also carry a mixed audio track (`audio.wav`, muxed into
`tour.mp4` at −16 LUFS) and 9:16 versions of everything (`clips/vertical/
<beat>.mp4`, `tour-vertical.mp4`): the full 16:9 frame over a blurred fill
with title/CTA banners. The skill's job is to get a
game to that point, verify it, and turn the result plus a code read into
`docs/video-notes.md` for whoever edits the video. The recorder lives in the
template (`libs/gamebient-bevy-template/src/game/record/`, design specs
`docs/superpowers/specs/2026-09-09-recording-harness-design.md` and
`docs/superpowers/specs/2026-09-10-recording-audio-vertical-reel-design.md`). The
skill's canonical home is the template's `.claude/skills/`, symlinked from
the workspace root.

Supporting files: [notes-template.md](references/notes-template.md) (the
section skeleton), [irregular-games.md](references/irregular-games.md)
(games whose layout differs from the template),
[catalog-2026-09-09.md](references/catalog-2026-09-09.md) (per-game beats
already researched for the 16 published games; a starting corpus, verify
against the code before reuse).

## Process

1. **Roll out if missing.** `grep -q '^record = ' games/<g>/Cargo.toml` or
   run `libs/gamebient-bevy-template/tools/rollout-record.sh games/<g>`.
   Read every `HAND EDIT` line it prints and fix them (see
   irregular-games.md). Then in the game:
   `cargo fmt --all -- --check && cargo check --features record && cargo clippy --all-targets --all-features -- -D warnings && cargo test --all-features` (CI runs the fmt check; the rollout script already formats).
   Commit: `feat(record): offline footage recorder from the template`.
2. **Record.** `tools/record.sh` in the game dir. First release build takes
   minutes; the tour itself takes 3-6 minutes because every frame is read
   back from the GPU. Do not run two recordings at once.
3. **Verify** before writing anything:
   - `manifest.json` `frames` is within 5 of
     `ffprobe -v error -select_streams v -show_entries stream=nb_frames -of csv=p=0 build/record/tour.mp4`;
   - every beat in `manifest.json` has `clips/<beat>.mp4` and `shots/<beat>.png`;
   - `grep -c SfxEvent build/record/events.jsonl` is greater than zero (or the
     game's equivalent message);
   - `ffprobe` shows an `aac` stream on `tour.mp4` and on every clip; manifest
     `audio.peak > 0` (a `null` audio field means no `AudioPlayer` played —
     check the game isn't muting itself under the harness);
   - `ls build/record/clips/vertical | wc -l` equals the beat count;
   - Read `shots/06-*.png` and `shots/05-mid-play.png` as images: gameplay
     is visible, HUD shows a score or progress, no letterbox band. Also
     extract and Read one frame from `clips/vertical/<beat>.mp4` to confirm
     the full 16:9 frame sits centered over the blurred fill, between the
     title/CTA banners. If the bot idles or the
     signature beat never fires, fix `src/game/autopilot.rs` first (the
     `designing-cartridge-covers` skill's autopilot.md covers this) and
     re-record.
4. **Write `docs/video-notes.md`** from notes-template.md. Paste
   `chapters.md`'s table verbatim. Fill the editorial sections by reading
   the README, `docs/superpowers/specs/*-design.md`, the gameplay `src/game/*.rs`
   files that define scoring, waves, levels, bosses, upgrades, and any
   copy/flavor-text file. Every claim cites a file. Quote flavor text
   verbatim. Numbers (thresholds, counts, timings) come from constants in
   the code, not from memory.
5. **Report.** Clip paths, the lead clip you'd cut first, and everything the
   tour did not reach (a boss, a fever mode, a rare drop) as candidates for
   a custom recording scenario. Commit `docs/video-notes.md`:
   `docs: video notes for <Game>`.

## Sizzle reel

Once several games have recordings with audio and vertical clips, cut a
catalog trailer that leads with each game's title card and its best beat.
From the workspace root:

```bash
python3 .claude/skills/recording-game-footage/tools/reel.py --init
# writes docs/reel.json from docs/video-notes-index.md; reorder, drop, or
# retime entries there before building
python3 .claude/skills/recording-game-footage/tools/reel.py
```

Outputs `build/reel/reel-16x9.mp4` and `build/reel/reel-9x16.mp4`. Verify
with `ffprobe` durations (≈ 16 × (1.5 + 4.5) + 3 s for a 16-game reel) and by
reading one frame each from a title card, a clip, and the end card.

## Common mistakes

| Mistake | Fix |
|---------|-----|
| Recording with `cargo run --features autopilot` | That saves nine stills only. Use `tools/record.sh`. |
| Two `Screenshot`s in one frame | Under `record`, `shot()` must write `RecordBeat`, never spawn a Screenshot; Bevy drops the duplicate. |
| Notes written from the catalog file without checking the code | The catalog is a 2026-09-09 snapshot. Re-verify constants and names in `src/`. |
| Frame count far below expected | The run crashed or the bot skipped a state; read the cargo output, check `events.jsonl`'s last lines. |
| Committing `build/record` | It is gitignored; never force-add it. Deliver clips by path or SendUserFile. |
| Running on the web build | The recorder is native-only; web builds never include it. |
| Rolling out over an old `record.rs` | The script deletes it before copying the folder module in; don't hand-restore a stray `record.rs`. |
| Hunted's flat layout | No `src/game/`; the recorder lands at `src/record/` (see irregular-games.md). |
| Manifest `audio.peak` is 0 | The game muted itself under the autopilot (grand-theft-otto honours `AUTOPILOT_SOUND`, which `record.sh` sets); look for `GlobalVolume` inserts gated on harness/autopilot flags. |
