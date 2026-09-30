# Gamebient Game

A Bevy game for the Gamebient ColecoVision GX platform, built from the
[Gamebient Bevy template](#about-this-template). Runs natively and in the browser
(WebGL2), and ships as a Pi cartridge binary.

## Quick start

```bash
./init-game.sh "My Game" OWNER/REPO   # rename the template to your game, then self-deletes
cargo run                             # native debug build — a window opens
```

Controls: **WASD / arrow keys** move, **ENTER** starts (and restarts after game over).

## Building

### All targets

```bash
./build.sh web     # wasm32 bundle -> dist/ (the path Vercel serves)
./build.sh x86     # x86_64-unknown-linux-gnu (native on Linux, cross elsewhere)
./build.sh pi      # aarch64-unknown-linux-gnu (Raspberry Pi), via cross
```

Bundle a compiled Linux target (binary + assets + launcher) into a tarball:

```bash
./package.sh x86   # -> build/gamebient-game-x86.tar.gz
./package.sh pi    # -> build/gamebient-game-pi.tar.gz
```

**Requirements:**
- Web: `rustup target add wasm32-unknown-unknown`, `cargo install wasm-bindgen-cli@0.2.108 --locked`, and `wasm-opt` (binaryen).
- Pi / cross builds: `cargo install cross --locked` and a running Docker daemon.

> The `wasm-bindgen-cli` version **must** match the `wasm-bindgen` library version in `Cargo.lock`. If you bump the dependency, bump the CLI (in `install.sh` and the workflows) in lockstep, or the bundle fails to load at runtime.

### Desktop (quick run)

```bash
cargo run                    # debug, native host
cargo run --release          # release, native host
```

A native window also draws the cabinet frame (dim bezel art, and a marquee on
portrait displays) around the game. `GX_FRAME=off` turns it off,
`GX_WINDOW_SIZE=1080x1920` opens a window of that size, and the score to beat
is kept in `$GX_DATA_DIR/best-score` (else
`~/.local/share/gamebient/<package>/best-score`). See "Cabinet frame" in
`docs/conventions.md`.

### Visual check (autopilot)

```bash
cargo run --features autopilot                       # scripted tour, shots in /tmp/gamebient-game-shots
AUTOPILOT_DIR=shots AUTOPILOT_SCALE=1.5 cargo run --features autopilot   # 1440x1080 captures at 4:3
GX_WINDOW_SIZE=450x800 cargo run --features autopilot                    # letterboxed in a portrait window
```

A bot plays the game through the real input path and saves a screenshot at
each beat (`01-studio-logo` … `09-game-over`). The bot policy and the
signature-moment beat live in `src/game/autopilot.rs`; replace them once the
game has gameplay. Dev-only; never compiled into shipping builds.

### Footage (record)

```bash
tools/record.sh                       # 60 fps tour video + beat clips -> build/record/
AUTOPILOT_SCALE=1.0 tools/record.sh   # the game's pinned size (960x720) instead of 1.5x
tools/record.sh --keep-frames         # keep the PNG frames after encoding
```

Runs the autopilot tour on a fixed 1/60 s clock, captures every frame, and
encodes `build/record/tour.mp4` with ffmpeg. `events.jsonl` logs state
changes, beats, score, pause and every `SfxEvent`; `tools/cut_clips.py` cuts
`clips/<beat>.mp4` (2 s before to 4 s after each beat), `shots/<beat>.png`,
and `chapters.md`. Needs `ffmpeg` and `python3`. A tour is ~3,700 frames of
PNG (several GB) until the script deletes them. Dev-only; never ships.

When the game plays any sound during the tour, `tour.mp4` and every
`clips/<beat>.mp4` carry the mixed audio, normalized to −16 LUFS. `cut_clips.py`
also renders `clips/vertical/<beat>.mp4` and `tour-vertical.mp4` (1080x1920,
the gameplay at full width over a blurred fill of itself, with a title plate
above it and a "Play the demo at colecovisiongx.com" CTA plate below it; the
plates move and shrink with the game's ratio so they never cover the
gameplay) — needs `rsvg-convert`
(`brew install librsvg`); without it these outputs are skipped.

Run the cutter's own unit tests with `cd tools && python3 -m unittest test_cut_clips`.

### Box cover (`assets/cartridge.png`)

```bash
./make-cartridge.sh                    # capture gameplay (autopilot) + compose
./make-cartridge.sh --skip-capture     # reuse the last capture
./make-cartridge.sh --beat 07-late-play
```

Composes the 768x1024 cover that `assets/info.json` points at from a real
gameplay shot and `tools/cartridge-cover.svg`. The shipped SVG is a labelled
placeholder: redesign it in the game's own voice (typeface, words, drawings,
palette) before release. Needs `rsvg-convert` (`brew install librsvg`).

### Mint metadata (`assets/info.json`)

The marketplace mints from `assets/info.json`: `name`, a real one-to-two
sentence `description`, `image` → the cover, `attributes` (Genre, Platform,
Players), and `properties` with `game_url`/`demo_url` (the live Vercel host),
`verify_url` (the web deploy's `/verify.zip`, built by `build_web.sh`),
`binary_url` (the flat cartridge tarball the release workflow produces) and
`binary_type: "bevy-tar"`. Fill in the placeholder description and check every
URL resolves before publishing.

## Replay verification

Every run records its seed and per-tick input into a `GXR1` replay and posts
it to the host when the run ends; the ColecoVision GX site re-simulates it
headlessly to confirm the score before it becomes a leaderboard entry.

```bash
cargo test --all-features                                   # codec, selftest, fixture
cargo run --features verify --bin verify -- --selftest --write tests/fixtures/selftest.gxr   # regenerate after a sim change
tools/build_verify.sh && node tools/verify_fixture.mjs tests/fixtures/selftest.gxr
```

See [docs/replay-verification.md](docs/replay-verification.md) for the format,
the determinism rules gameplay code must follow, and the server contract.
Porting this into a game (not the template) is `tools/rollout-replay.sh
<game-dir>` followed by the `rolling-out-replay-verification` skill.

## Deploying (web)

**Vercel** runs `build_web.sh` to build `dist/` and serves it as a static site.

> **Required env var:** set `GH_TOKEN` in the Vercel project — a GitHub
> **fine-grained PAT** with **Contents: Read** on this repo. The build uses it to
> fetch the Pi cartridge binary (`gamebient-game.tar.gz`) from the latest GitHub
> release into `dist/assets/`, so `assets/info.json`'s `binary_url` resolves
> without keeping the ~45 MB blob in git. Without the token the web game still
> deploys, but `binary_url` returns 404. See [docs/build-and-release.md](docs/build-and-release.md).

## Releases

Push a `v*` tag (e.g. `v0.0.1`) to trigger `.github/workflows/release.yml`, which
builds all three targets and publishes a GitHub Release with:
`gamebient-game-pi.tar.gz`, `gamebient-game-x86.tar.gz`, `gamebient-game-web.zip`,
and the flat cartridge `gamebient-game.tar.gz`.

## Screen shape

The game renders at one pinned size, set in `src/display.rs`:

| Ratio | `GAME_WIDTH` x `GAME_HEIGHT` |
|---|---|
| 4:3 (default) | 960 x 720 |
| 1:1 | 720 x 720 |
| 3:4 | 720 x 960 |

On the web the canvas is that size and the page letterboxes it. On a cabinet
(Linux) the window is fullscreen and the game is letterboxed inside it. UI is
authored against a short side of 720 and scales with the game. Details:
[docs/conventions.md](docs/conventions.md), "Screen shape".

## Project structure

```
src/
  main.rs            App + curated DefaultPlugins, window and plugin wiring
  display.rs         Game size, window, native letterbox, UI scale (DisplayPlugin)
  game/
    mod.rs           GamePlugin: state machine, resources, gameplay systems, cleanup
    states.rs        GameState (Menu / Playing / GameOver)
    player.rs        WASD-movable player (the "add a system" example)
    scoring.rs       GameData + add_score(), with unit tests
  assets/mod.rs      AssetsPlugin (procedural; add asset loading here)
  ui/
    mod.rs / menu.rs / hud.rs   Title + game-over screens, score/lives HUD
    fit.rs           Text sizing against the game's width
```

Geometry is procedural — no external model files. See
[docs/conventions.md](docs/conventions.md) for the patterns to follow and
[AGENTS.md](AGENTS.md) for agent-oriented guidance.

## About this template

This repository was generated from the Gamebient Bevy template: a minimal runnable
skeleton plus a full production stack (multi-target build, CI, tag-driven releases,
and the Gamebient cartridge pipeline). Run `./init-game.sh` once to make it yours.

## License

Copyright © 2026 Bread Heads Studios. All rights reserved. Proprietary software —
see [LICENSE](LICENSE). Built on open-source libraries (including
[Bevy](https://bevyengine.org/), MIT/Apache-2.0) under their own terms.
