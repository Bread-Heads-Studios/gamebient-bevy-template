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
- **One display module.** `src/display.rs` owns the game's size (`GAME_WIDTH` x
  `GAME_HEIGHT`: 960x720, 720x720 or 720x960), the window (`game_window`), the
  native letterbox (`GameViewport`, applied to every window camera without
  `FrameCamera`) and `UiScale` (viewport short side / 720). Never write a
  resolution anywhere else, never set `UiScale` yourself, and never read display
  state from sim code. See docs/conventions.md, "Screen shape".
- **Deterministic sim.** Run state changes only inside `sim::SimSet`
  (`FixedUpdate`, 60 Hz, chained). Sim systems read `TickInput`, draw
  randomness from `GameRng`, and never touch wall-clock or
  `std::collections::HashMap`. Each run records a replay the site
  re-simulates with `src/bin/verify.rs`; see `docs/replay-verification.md`.

## How to add things

- **A gameplay system:** write a `fn` taking the `Res`/`Query` it needs, register it
  in the owning plugin under `Update` with `.run_if(in_state(GameState::Playing))`
  (or `Startup`/`OnEnter`/`OnExit` as appropriate) — or in `GamePlugin` inside the
  `SimSet` chain (not `Update`) if it mutates run state; `player::move_player` is
  the model.
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
  and cuts beat clips + `chapters.md` (`src/game/record/`,
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
  vertical-format posting. `tools/store-assets.sh` turns a `record.sh` capture
  into the store screenshots, trailer and v2 metadata the website renders
  (see `docs/build-and-release.md`). The `recording-game-footage` skill rolls this out
  and writes `docs/video-notes.md`.
- **The box cover and mint metadata:** `assets/cartridge.png` (768x1024) and
  `assets/info.json` are what the marketplace mints. Both ship as placeholders:
  `make-cartridge.sh` composes the cover from an autopilot shot and
  `tools/cartridge-cover.svg`, which must be redesigned in the game's own voice,
  and `info.json`'s description/genre/hosts must be filled in. Use the
  `designing-cartridge-covers` and `generating-cartridge-metadata` skills.
- **Sizes come from one place:** `tools/game-size.sh` prints the game's size
  from `src/display.rs`. `record.sh`, `make-cartridge.sh` and
  `store-assets.sh` ask it; never write 1280, 720, 1920 or 1080 into a tool.
- **Cabinet frame art:** `tools/frame-art.sh` renders `tools/marquee.svg` to
  `assets/marquee.png` (1080x360) and `tools/bezel.svg` to `assets/bezel.png`
  (1920x1920) and refuses a bezel with detail in its central 1080x1080 or
  with high contrast. The SVGs ship as placeholders; the PNGs do not ship. A
  game with no `assets/marquee.png` or `assets/bezel.png` falls back to the
  shared ColecoVision GX art on the cabinet, so run `frame-art.sh` only once
  the SVGs are designed. `frame-art.md` in the `designing-cartridge-covers`
  skill says how to make them from the cover.
- **Replay verification:** `cargo test --all-features` runs the codec tests and
  the native `--selftest`; regenerate `tests/fixtures/selftest.gxr` whenever the
  sim changes — `cargo run --features verify --bin verify -- --selftest --write
  tests/fixtures/selftest.gxr` — `cargo test` tells you when. Check it under
  wasm too: `tools/build_verify.sh && node tools/verify_fixture.mjs
  tests/fixtures/selftest.gxr`. See `docs/replay-verification.md`.
- **Rolling replay verification into a game:** `tools/rollout-replay.sh <game>`
  then the `rolling-out-replay-verification` skill.
- **Converting a game to 4:3 / 1:1 / 3:4 with the cabinet frame:**
  `tools/rollout-aspect.sh <game> <4:3|1:1|3:4>` copies the verbatim files,
  pins the size, writes `tests/display_contract.rs` and prints `HAND EDIT:`
  lines for the rest. `tools/frame-accept.sh --check-copies <game>` proves the
  copies are still verbatim; `tools/frame-accept.sh --shots <dir> --window WxH
  --ratio <r>` checks kept screenshots. Neither is copied into a game.
- **A marquee highlight:** write `frame::Highlight::rgb(r, g, b)` with a
  `MessageWriter<frame::Highlight>`. The frame (`src/frame/`, native only,
  added in `main.rs`) and web hosts both react. See "Cabinet frame" in
  `docs/conventions.md`.

### Demo and full bundles

Every web deploy carries two bundles from one commit: the **demo** at `/`
(built with `--features demo`) and the **full game** at `/full/`, which
`middleware.ts` serves only to a request carrying a valid ColecoVision GX
entitlement (a 12 h Ed25519 token the website signs for an owner; public key
in the Vercel env `GX_ENTITLEMENT_PUBKEY`). Native, Pi, `verify`, `autopilot`
and `record` builds are always the full game.

The cut lives in `src/game/demo.rs` and nowhere else:

- `DEMO_CAP` and `demo_reached(..)` express the cap in the game's own unit
  ("after level 2" is `level >= 3`, checked when the next unit begins).
  Change the `end_demo` parameter to whatever resource holds that unit.
- `end_demo` runs in `FixedUpdate` after `SimSet`, latches `sim::RunOver`
  (the sim freezes, as on `sim::end_run`), posts `HostEvent::Custom
  { name: "demo_end" }`, and fades into `GameState::DemoEnd`.
- `src/ui/demo_end.rs` draws `DEMO OVER` / `OWN <TITLE> TO KEEP PLAYING`;
  set `FULL_GAME_LINE` to name what the full game has more of. The title
  card shows a `DEMO` chip in the demo bundle (`demo::DEMO`).
- `assets/info.json`: `demo_url` is the host root, `game_url` is
  `<host>/full/`.

Port checklist: feature in `Cargo.toml`; `DemoEnd` in `states.rs` plus every
`match` on `GameState`; `demo.rs` with the game's cut and tests;
`ui/demo_end.rs`, `ui/mod.rs`, `ui/menu.rs` edits; `build_web.sh`,
`middleware.ts`, `tools/entitlement.mjs`, `tools/test_entitlement.mjs`,
`vercel.json` headers and the CI step copied from the template; `game_url`;
`GX_ENTITLEMENT_PUBKEY` on the game's Vercel project; then release and run the
production checks below.

Production checks after a release: `/` plays and stops at the cut with the
card; `curl -sI https://<host>/full/` is 403; `/play/<pda>` as an owner on
colecovisiongx.com runs past the cut; the cabinet paired to that wallet does
too; a `/play` run still lands on the verified leaderboard.

## Typography and cards

Full rules: "Typography and cards" in `docs/conventions.md`.

- **Two faces, one optional accent**, embedded in `src/ui/fonts.rs`:
  `DISPLAY` (titles, headlines, big numbers) and `BODY` (prompts, HUD,
  labels), plus the shared studio faces `STUDIO` / `STUDIO_ITALIC`. Each
  is a `Face` (`include_bytes!` TTF, fixed `uuid_handle!`, real weight,
  measured `advance_em`) with its licence beside it in `src/ui/fonts/`.
  Set a face on every UI `Text` (`DISPLAY.font(size)`,
  `BODY.fitted(text, max)`); nothing in the UI uses Fira Mono.
- **Never touch the default font slot**: the cabinet frame's marquee and
  caption use it.
- **Fitting is per face.** `fit_font_size(face, text, max, view_w)` uses
  `face.advance_em`; `fonts::tests` parses the TTFs and fails if a string
  in `tests::fitted()` is wider than the declared metric. Add every string
  you fit to that list.
- **Text spawned from `src/game/` takes `Option<Res<UiFonts>>`** and
  `fonts::body_or_default` / `display_or_default`: the verifier has no
  `UiPlugin`. Never a required `Res`.
- **Never animate `font_size`** (each size leaks a glyph atlas); animate
  `UiTransform` or alpha.
- **Title anatomy:** animated on-theme backdrop, display lockup with depth
  in the upper ~45%, a prompt rail on a plate (pulsing verb prompt naming
  the button, one controls line, best chip), settled by t = 1.0 s.
- **Cards:** one `CardStyle` set per game in `src/ui/theme.rs`, built with
  `card::card` / `card_with`, entering with `card::intro()` (<= 250 ms,
  `UiTransform`), spaced on the 8 px scale (`card::S1..S6`). Game over and
  pause are cards.
- `fit.rs`, `card.rs` and `studio_logo.rs` are copied into games verbatim;
  `fonts.rs`, `theme.rs`, `backdrop.rs` and the screens are the game's own.

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
- **`build_web.sh` runs `bundle` twice** (demo, then full); `dist/verify.zip` is
  built once from the full sim. **CI runs** `node --test tools/test_entitlement.mjs`.
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
- **`SmallRng` differs on wasm32** (xoshiro128 vs 256): the sim uses
  `rand_xoshiro::Xoshiro256PlusPlus` explicitly.
- **The wasm verifier must not call `App::run`**: Bevy's wasm runner wants
  `window.setTimeout`, which Node lacks; `verify()` steps `app.update()` itself.
- **`GX_BUILD_ID` must be unique per release**: `build.rs` and
  `tools/build_verify.sh` derive the sha from `git rev-parse --short=7 HEAD`,
  falling back to `VERCEL_GIT_COMMIT_SHA`; a build with neither fails on
  purpose (never ships `+unknown`), because the site caches a verifier per
  build id — every release must carry a new one.
- **`world_to_viewport` includes the letterbox offset.** It returns window
  pixels; UI nodes are placed relative to the camera viewport. On a desktop
  window the offset is zero and the bug is invisible; on a cabinet every
  projected label lands a bar's width off. Subtract
  `camera.logical_viewport_rect().min` first.
- **Linux native opens borderless fullscreen**; autopilot and record builds
  stay windowed so captures are the game's own size. The check is
  `cfg!(feature = "capture")` in `display::game_window`; `autopilot` implies
  it, and a game with any other capture feature makes it imply `capture` in
  `Cargo.toml` (docs/conventions.md, "Capture builds stay windowed"). `GX_WINDOW_SIZE=WxH`
  forces a window of that size on any native build.
- **`GX_MUTE=1` (native) or `?mute=1` (web) starts a run silent.** It mutes
  sinks only (`game::host::LocalMute`), so `record` footage keeps full audio
  and a host `unmute` can't undo it. Set it whenever an agent launches a game.
- **`/full/` is open when served locally** (`python3 -m http.server`): only
  Vercel runs `middleware.ts`. Never mirror `dist/full/` anywhere else.
- **`cargo test --all-features` includes `demo`.** A test that scores past
  `DEMO_CAP` in a windowed or headless `GamePlugin` app lands in `DemoEnd`,
  not `GameOver`; keep fixtures below the cap or run them without the feature.
- **`fonts::tests::fitted()` must list `demo_end::own_line(menu::TITLE)`**
  (it does in the template); a game with a longer title re-checks that row.
