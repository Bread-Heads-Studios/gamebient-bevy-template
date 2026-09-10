# AGENTS.md

Guidance for coding agents working in this repository. (Claude Code reads this via
`CLAUDE.md`, which points here.) Human-facing usage is in [README.md](README.md).

## Overview

A Bevy 0.18 / Rust 2024 game for the Gamebient ColecoVision GX platform, generated
from the Gamebient Bevy template. Targets native (x86_64 Linux, aarch64 Pi) and web
(wasm32 + WebGL2). Geometry is procedural; the codebase favors small, focused,
one-responsibility files.

## Architecture & conventions

- **One file per system/feature.** A module (`src/game/player.rs`,
  `src/ui/hud.rs`, …) owns one concern and exposes plain `fn` systems.
- **Plugins compose the app.** `GamePlugin`, `AssetsPlugin`, and `UiPlugin` each
  register their own systems/resources. `main.rs` only wires plugins + the window.
- **Enum state machine.** `GameState` (`Menu` → `Playing` → `GameOver`) in
  `src/game/states.rs`. Gameplay systems run with `.run_if(in_state(GameState::Playing))`;
  screen setup/teardown hangs off `OnEnter` / `OnExit`.
- **`GameEntity` cleanup marker.** Entities spawned for a run are tagged
  `GameEntity` and despawned in `cleanup_game_entities` on `OnExit(Playing)`.
- **Extract pure logic and unit-test it.** Game rules live in methods/functions
  (e.g. `GameData::add_score` in `src/game/scoring.rs`) so they can be tested
  without a Bevy `App`. See the `#[cfg(test)] mod tests` there — follow that pattern.
- **Curated Bevy features.** `Cargo.toml` sets `default-features = false` and lists
  features explicitly; native-only features are gated to `cfg(not(target_arch = "wasm32"))`.
  Add features deliberately — they affect the wasm bundle size.

## How to add things

- **A gameplay system:** write a `fn` taking the `Res`/`Query` it needs, register it
  in the owning plugin under `Update` with `.run_if(in_state(GameState::Playing))`
  (or `Startup`/`OnEnter`/`OnExit` as appropriate). `player::move_player` is the model.
- **An asset:** load/create it in `AssetsPlugin` (a `Startup` system inserting a
  resource of handles), then reference that resource where you spawn.
- **A test:** pull the rule into a pure method/function, add a `#[cfg(test)] mod tests`.
  `cargo test` runs in CI.
- **A visual check:** `cargo run --features autopilot` plays a scripted ~60 s
  session (logo → title → how-to-play → bot-played run → pause → game over) and
  saves renderer screenshots `01-studio-logo` … `09-game-over` to
  `/tmp/gamebient-game-shots` (override with `AUTOPILOT_DIR`). No OS capture or
  input permissions needed. The bot and the `06-…` signature beat in
  `src/game/autopilot.rs` are the two game-specific parts; customize them when
  gameplay exists. The feature is dev-only and never ships.
- **Footage:** `tools/record.sh` records the same tour offline (fixed 1/60 s
  clock, one `Screenshot` per frame, `events.jsonl`) into `build/record/`
  and cuts beat clips + `chapters.md` (`src/game/record.rs`,
  `tools/cut_clips.py`). Under `record` the autopilot's `shot()` logs a
  `RecordBeat` instead of taking its own screenshot (Bevy drops a second
  `Screenshot` of the same window in one frame). Add game messages to the log
  with `record::log_messages::<T>(app)` in `GamePlugin`.
  `src/game/record/audio.rs` logs every `AudioPlayer` playback each frame and
  mixes them into `audio.wav` at `AppExit`, which `record.sh` muxes into
  `tour.mp4` (loudness-normalized) and `cut_clips.py` carries into
  `clips/<beat>.mp4`. `cut_clips.py` also renders `banner.png` from
  `tools/vertical-banner.svg` via `rsvg-convert` and reframes the tour and
  clips to 1080x1920 (`tour-vertical.mp4`, `clips/vertical/<beat>.mp4`) for
  vertical-format posting. The `recording-game-footage` skill rolls this out
  and writes `docs/video-notes.md`.
- **The box cover and mint metadata:** `assets/cartridge.png` (768x1024) and
  `assets/info.json` are what the marketplace mints. Both ship as placeholders:
  `make-cartridge.sh` composes the cover from an autopilot shot and
  `tools/cartridge-cover.svg`, which must be redesigned in the game's own voice,
  and `info.json`'s description/genre/hosts must be filled in. Use the
  `designing-cartridge-covers` and `generating-cartridge-metadata` skills.

## Build / CI / release model

- **Scripts:** `build.sh <pi|x86|web>` compiles one target (`web` delegates to
  `build_web.sh`, the Vercel build command); `package.sh <pi|x86>` makes a tarball;
  `fetch-cartridge.sh` pulls the cartridge binary from the latest GitHub release;
  `make-cartridge.sh` regenerates the `assets/cartridge.png` box cover (an
  autopilot gameplay screenshot composed into `tools/cartridge-cover.svg`,
  rendered with `rsvg-convert` — `brew install librsvg`).
- **`ci.yml`** runs on PRs/pushes: `fmt --check`, `clippy -D warnings`, `cargo test`,
  and a `build-web` job mirroring Vercel.
- **`release.yml`** runs on `v*` tags only: a matrix builds web/x86/pi, packages
  tarballs + the flat cartridge, and publishes a GitHub Release. Heavy cross-compiles
  do **not** run per-PR.
- **Cartridge binary is not in git.** It's a release asset, fetched at build time by
  `fetch-cartridge.sh`. Vercel needs `GH_TOKEN` (fine-grained PAT, Contents: Read).

Deeper detail: [docs/build-and-release.md](docs/build-and-release.md) and
[docs/conventions.md](docs/conventions.md).

## Gotchas & hard-won lessons

These cost real debugging time on the project this template was extracted from:

- **`wasm-bindgen-cli` version MUST equal the `wasm-bindgen` lib version** in
  `Cargo.lock`, or the wasm bundle fails to load at runtime. Pinned in `install.sh`
  and both workflows — bump them together.
- **`wasm-opt` needs the feature flags** `--enable-bulk-memory
  --enable-nontrapping-float-to-int --enable-sign-ext` to match the target-features
  in `.cargo/config.toml`. Without them, older binaryen rejects the module
  ("all used features should be allowed"). Already set in `build_web.sh`.
- **The aarch64 (Pi) release build OOM-kills `rustc`** under fat LTO +
  `codegen-units=1`. `release.yml` relaxes the Pi build to thin LTO +
  `codegen-units=16` (forwarded into the cross container via `Cross.toml`
  `[build.env] passthrough`) plus swap. x86/web keep fat LTO.
- **Brotli compression in `build_web.sh` needs Node** (preinstalled on GitHub and
  Vercel runners). Missing Node = uncompressed wasm, warned but non-fatal.
- **Cartridge fetch needs `GH_TOKEN`** in Vercel — fine-grained PAT, **Contents:
  Read** only. A missing/expired token silently 404s `binary_url` (the deploy still
  succeeds, by design).
- **Vercel preview deployments are auth-gated** (deployment protection). You can't
  `curl` a preview URL anonymously — verify the cartridge fetch via build logs or the
  production deploy.
- **Never commit build outputs** (`dist/`, `target/`, compiled binaries, the
  cartridge tarball). They're gitignored; keep `.git` lean.
- **`init-game.sh` is one-shot** and self-deletes. It uses BSD/macOS `sed -i ''`;
  on Linux change to `sed -i`.
