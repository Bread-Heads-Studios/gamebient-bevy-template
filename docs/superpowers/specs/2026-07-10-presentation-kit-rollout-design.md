# Presentation Kit + Five-Repo Rollout

**Date:** 2026-07-10
**Status:** Approved
**Reference implementation:** `../voidrunner` PR #8 (`feat/presentation-polish`, verified end-to-end)

## Goal

Make every Gamebient cartridge boot and flow like a professionally released
game, identically across the catalog: studio logo → title → (first run) how to
play → gameplay, with fade transitions on every flow edge, a pulsing start
prompt, an upgraded pause overlay, and a web boot where the engine starts on
the player's "Click to Start" gesture. The kit lands first in
`gamebient-bevy-template` (canonical source), then is applied to `Hunted`,
`pizza-pinball`, `BeerPong`, and `Gravestone_Gauntlet`.

All five targets are Bevy 0.18 / Rust 2024 (the monorepo CLAUDE.md's "Bevy
0.15" note is stale), so the Voidrunner modules port near-verbatim.

## The canonical kit (template)

Every item below is copied per-game with only content adapted — no behavioral
divergence between repos.

### 1. States & flow

`GameState`: `StudioLogo` (boot default) → `Menu` → `HowToPlay` → `Playing` →
`GameOver` (→ `Menu`). `HowToPlay` is the generic rename of Voidrunner's
`ItemKey`. Games with extra states (Hunted's `Victory`) keep them; their exits
also route through the fade.

### 2. `src/ui/transition.rs` — verbatim port from Voidrunner

`ScreenFade` resource (`request`/`request_with` with non-positive-duration
clamp, `is_idle`, `tick` emitting the target exactly once), persistent
full-screen overlay at `GlobalZIndex(1000)` with change-detection-friendly
writes, `Pulse { speed, min, max }` + `pulse_text`, `BOOT_FADE_SECS = 0.6`,
`DEFAULT_FADE_SECS = 0.4`, and the 4 unit tests. Input systems that request
transitions gate on `fade.is_idle()`.

### 3. `src/ui/studio_logo.rs` — verbatim port

Black screen, `assets/breadheads_logo.png` (512×512 RGBA, committed to every
repo) at 220px, "BREAD HEADS STUDIOS" / "PRESENTS", fade in 0.6 s → hold →
auto-advance at 2.2 s, any key/gamepad button skips via the latch helper
(`latch_and_should_advance` + its 3 unit tests). Timer resource removed on
exit. Silent (menu music starts on `OnEnter(Menu)` where the game has music).

### 4. Title screen

- **With artwork** (Hunted `hunted.png`, Gravestone Gauntlet
  `gravestone_gauntlet.png`; both contain baked-in logotypes): full-bleed
  cover-fit `ImageNode` (width 100 %, aspect preserved, root `Overflow::clip()`),
  existing text-title nodes removed, artwork handle preloaded at `Startup` in a
  `TitleArtwork` resource so it never pops in.
- **Without artwork** (template, pizza-pinball, BeerPong): keep the styled
  text title.
- Both variants: bottom scrim strip (black 0.55 alpha, vertical padding 24)
  holding the pulsing start prompt (`Pulse { 3.0, 0.25, 1.0 }`) and the
  existing controls line. The template ships the artwork variant as a
  commented example.
- Start input keeps each game's existing convention (Enter/Space vs
  any-button) but routes through `fade.request(start_target(seen.0))`.

### 5. How-to-play screen (once per session)

`SeenHowToPlay(bool)` resource + `start_target` pure fn (+ test). Headline
("KNOW THE VOID" becomes a per-game line, e.g. "HOW TO PLAY" default), real
gameplay meshes spawned from each game's existing asset/mesh code, slow spin,
labels pinned via `Camera::world_to_viewport` ÷ `UiScale` (works for 3D and
2D cameras), pulsing launch prompt, own light where the gameplay light is
state-scoped (3D games). Entered only through the fade (label projection's
first-frame invariant). Rosters:

| Repo | Roster |
|---|---|
| template | one placeholder cuboid ("YOUR ITEM — describe it here") |
| Hunted | Creature (red/orange/crimson eye variants), Staff, Key, Exit Door, Projectile |
| Gravestone_Gauntlet | Drifter, Bouncer, Tracker, Gunner, Splitter (2D: `Mesh2d` showcase) |
| pizza-pinball | Meatball, Pepperoni Bumper, Olive Bumper, Pepper Bumper, Flipper, Launcher |
| BeerPong | Ball, Red Cup, Blue Cup, Aim Arc |

### 6. Pause

Esc / gamepad Start toggle (kept where it exists; **added** to pizza-pinball
and BeerPong), dimmed overlay: "PAUSED" + "ESC: RESUME" + "ENTER: QUIT TO
TITLE" (gamepad East also quits). Quit routes through the fade; toggle and
quit gate on `fade.is_idle()`; `(toggle, quit).chain()` for same-frame
determinism. Games whose pause lives in a sub-state (Gravestone Gauntlet's
`PlayState::Paused`) keep that mechanism and adopt the overlay styling +
quit-to-title behavior.

### 7. Death/game-over flow

Every `next_state.set(GameState::GameOver)` (and Hunted's `Victory`) call site
becomes `fade.request(...)`, with once-only side effects (death SFX etc.)
gated behind the request's `bool`. Phase/wave-advance systems that set
sub-states gate on `fade.is_idle()` so nothing advances under a death fade.

### 8. `ClearColor`

Near-black per-game tint inserted where missing (template, BeerPong).
Gravestone Gauntlet, Hunted, and pizza-pinball already set one — keep theirs.

### 9. Web boot (index.html)

Port Voidrunner's rework to every repo's `index.html`: wasm fetched and
compiled behind the progress bar (`WebAssembly.compile`), download/compile
failures reach the error overlay (try/catch → `showError`), "Click to Start"
shown **before** `init()`, and the async unlock handler (re-entry guard,
synchronous listener removal) starts the engine — so the studio logo plays
while the player is watching. Audio-context patching preserved as-is.

### 10. Tests & gates

The 8 kit unit tests (4 fade, 3 logo latch, 1 gating) port to every repo.
Every repo must pass `cargo test`, `cargo fmt --all -- --check`, and
`cargo clippy --all-targets -- -D warnings` (BeerPong gains these gates via
new CI). Runtime verification per repo: the headless-Chrome recipe from
Voidrunner (serve `dist/`, drive unlock → logo → title → how-to-play →
gameplay → pause → quit → second start skips how-to-play; wasm-404 probe).

## Per-repo extra work

- **BeerPong** (not currently a git repo): `git init` on `main`, then template
  infra parity — `.gitignore`, `index.html`, `build_web.sh`, `package.sh`,
  CI workflows (fmt/clippy/test/wasm with `wasm-bindgen-cli@0.2.108` pinned),
  `assets/info.json`, `rust-toolchain.toml` — adapted to the name "Beer Pong"
  following the template's token conventions. No GitHub remote yet: commits
  stay local on a feature branch.
- **Gravestone_Gauntlet**: execute its existing approved infra plan
  (`docs/superpowers/plans/2026-06-26-production-readiness.md`, 70 steps:
  source reorg into `game/`/`assets/`/`ui/`, CI, cartridge de-git, docs,
  starter tests) **first**, then apply the presentation kit to the
  reorganized layout. Same branch, sequential.
- **Hunted / pizza-pinball**: no extra work; branch from current HEAD
  (`production-ready`, `gameplay-harness-tuning`).

## Out of scope (flagged in PRs, not fixed)

- Gamepad support for pizza-pinball and BeerPong (gameplay work; kiosk gap).
- High-score persistence, attract modes.
- Refreshing GG's infra plan document itself (it executes as written; file
  references it contains are validated during execution).

## Branching & delivery

`feat/presentation-polish` per repo, branched from current HEAD. PR to the
repo's default branch where a GitHub remote exists (Hunted, pizza-pinball,
Gravestone_Gauntlet, template if it has a remote); local branch otherwise
(BeerPong). Execution: template first (canonical kit), then Hunted /
pizza-pinball / BeerPong in parallel (independent repos), Gravestone_Gauntlet
last (infra plan first, largest).

## Error handling

Same invariants as Voidrunner: missing images never block boot (timer-driven
advance, text fallbacks); web boot failures surface in the error overlay; a
soft-locked fade is impossible (duration clamp + monotonic fade-in).

## Testing summary

Per repo: 8 ported unit tests + existing tests; fmt/clippy/test gates; scripted
headless-Chrome flow walkthrough with screenshots as the acceptance evidence.
