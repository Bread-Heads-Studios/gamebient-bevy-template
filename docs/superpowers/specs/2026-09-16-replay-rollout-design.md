# Replay verification rollout — Design

**Date:** 2026-09-16 · **Status:** draft for owner review

## Goal

Get verified leaderboards live for real games. The template
(`gamebient-bevy-template` PRs #9, #10) and the site
(`ColecoVisionGXWebsite` #33, deployed) are done; nothing verifies until a
game carries the feature. Games are copies of the template, not dependents,
so every game needs the feature ported in. This spec covers:

1. `tools/rollout-replay.sh` — the mechanical port, in the style of
   `tools/rollout-record.sh`.
2. A workspace skill, `rolling-out-replay-verification`, that drives one
   game from "not ported" to "first verified run on production", including
   the per-game determinism work the script cannot do.
3. The pilot: Cannonball Putt, end to end on production.
4. The fleet catalog: every other game with its survey facts, difficulty and
   order, as a checklist the skill consumes later.

Out of scope: porting any game other than the pilot; the two Bevy 0.15
games (`delivery-dash`, `pizza-rush`); `voidrunner-authoritative` and
`voidrunner-streamed`; site changes (one is recorded as a follow-up below).

## Decisions

| Question | Choice | Why |
|---|---|---|
| Pilot | Cannonball Putt | Published, zero RNG, 14 gameplay systems already chained, no shared-state hazards. Cleanest game in the survey. |
| Score for golf | Games without a "higher is better" `GameData.score` implement `fn leaderboard_score(&self) -> u32`; Cannonball Putt scores each completed hole `max(0, 2·par − strokes) × 100` (hole-in-one on a par 3 = 500, par = 300, +3 or worse = 0), summed | The site ranks by a single higher-is-better integer. Strokes-as-points reads naturally on the board ("2 700") and rewards finishing the round. A site-side per-game `score_order` is a follow-up, not a blocker. |
| Where the port's per-game surface lives | One function per game, `leaderboard_score`, plus an optional `Checksum::fold` call in the game's own tick system | Keeps the template's `sim.rs`/`replay/` verbatim-copyable; the game touches only its own files. |
| Module clash handling | The script never overwrites a game file it did not author; it prints `HAND EDIT` lines instead (Cannonball Putt's test-only `src/game/sim.rs` is renamed `shot_sim.rs` by hand) | Same rule `rollout-record.sh` uses; games have drifted from the template. |
| Metadata update | Deploy only | Each collection's on-chain `uri` is `<game_url>/assets/info.json`, so adding `verify_url` to the file and redeploying updates the metadata the site reads. No transaction. |
| Proof of done | A run played on `colecovisiongx.com/play/<pda>` by an owner shows `Verified · <points> · #1` and appears on `/leaderboard/<pda>` | Nothing short of production proves the cross-repo path. |
| Fleet order | Published S → published M → published L → unpublished, per the catalog | Value first (published games have owners who can rank); learn on small games. |

## `tools/rollout-replay.sh <game-dir>`

Copies the feature from the template into a game checkout and wires it.
Idempotent; prints `HAND EDIT:` lines for anything it cannot do safely.

Copies verbatim (refusing to overwrite a file that exists and differs from
the template's previous version, except where noted):
`src/game/sim.rs` (HAND EDIT if a different `sim.rs` exists),
`src/game/replay/{mod,recorder,feeder,selftest}.rs`, `src/bin/verify.rs`,
`build.rs`, `tools/build_verify.sh`, `tools/verify_fixture.mjs`,
`tests/selftest.rs`, `docs/replay-verification.md`.

Edits in place (perl, like `rollout-record.sh`):
- `Cargo.toml`: `[lib] name = "<snake>"` + `[[bin]]` for the game and for
  `verify` (`required-features = ["verify"]`), feature
  `verify = ["dep:wasm-bindgen"]`, deps `rand_xoshiro = "0.7"` and the
  wasm32 `wasm-bindgen = { version = "0.2.108", optional = true }`,
  `gamebient-input` tag bumped to `v0.3.0`.
- `src/lib.rs` created (`pub mod assets; pub mod game; pub mod ui;` plus any
  other top-level `mod` the game's `main.rs` declares); `main.rs` rewritten
  to `use <snake>::{…}` and `game::GamePlugin::default()`.
- `src/game/mod.rs`: `pub mod replay; pub mod sim;` declarations; `GamePlugin`
  becomes `#[derive(Default)] pub struct GamePlugin { pub headless: bool }`
  (HAND EDIT if the struct is not the template's unit struct).
- `build_web.sh`: the verifier build + `dist/verify.zip` copy and the
  `find … ! -name verify.wasm` guard (HAND EDIT if the script diverged).
- `.github/workflows/ci.yml` / `release.yml`: the `setup-node` + fixture
  steps and the `gamebient-game-verify.zip` asset (HAND EDIT if the
  workflows diverged; the game's binary name replaces `gamebient-game`).
- `.gitignore`: `/dist-verify`, `/dist-verify.zip`, `/build/replays`.
- `assets/info.json`: `properties.verify_url = "<game_url>/verify.zip"`.
- `cargo fmt --all` at the end.

Prints `HAND EDIT` for: an existing non-template `src/game/sim.rs`; a
`GameData` without `score: u32` (needs `leaderboard_score`); `autopilot.rs`
writing `GameData` fields (dev-only, allowed, but the selftest must not use
that path); gameplay reading `ButtonInput<KeyCode>` or a custom input
resource (moleman-racing); a flat layout (Hunted); missing `record/`
(beat-bender, moleman-racing — run `rollout-record.sh` first).

The script is tested by running it on a scratch copy of the template itself
(must be a no-op) and on a scratch copy of Cannonball Putt (must compile
after the documented hand edits).

## The determinism port (per game, by hand, skill-guided)

The skill's `references/port-checklist.md` is the contract; in brief:

1. Every system that mutates run state moves from `Update` into the
   `sim::SimSet` chain in `FixedUpdate`, in its existing order; `Res<Time>`
   there is the fixed 60 Hz delta. Presentation-only systems (particles,
   trails, aim arrow, HUD) may stay in `Update` **only** if they write
   nothing the sim reads and nothing the checksum folds.
2. Sim systems read `Res<TickInput>` instead of `Res<GameInput>`; menus keep
   `GameInput`. No `pause_just_pressed` in the sim (the recorder masks it).
3. Randomness comes only from `Res<GameRng>` (Xoshiro256++); every
   `rand::rng()` / `SmallRng` / `StdRng` seeded from wall-clock goes. Games
   with their own seeded resource (dive-rise, dough-io, GTAR, GTO, gulper,
   voidrunner) rename or wrap it so `sim::begin_run` reseeds it from
   `RunSeed`. Presentation RNG (dough-io's `FxRng`) stays separate.
4. Run end goes through `ScreenFade::request(GameOver)` from a sim system,
   never `NextState::set` inside `FixedUpdate` (dough-io must change).
5. `leaderboard_score()` if there is no `score` field; the recorder seals
   with it and the HUD/host score report use it too so the three agree.
6. `checksum_tick` folds the game's own key state (ball position and hole
   index for golf; player position and wave for shooters) so the selftest is
   sensitive to inputs.
7. `verify --selftest` records a scripted run through `VirtualInput` (the
   autopilot's bot logic is the starting point; it must not poke
   `GameData`), replays it, and the verdict matches; a tampered input does
   not. The fixture is committed.
8. A real run recorded with `GX_REPLAY_DIR` verifies natively and under Node
   (`tools/verify_fixture.mjs`) with the same checksum.
9. `cargo test --all-features`, clippy, fmt, `bash build_web.sh` (game wasm
   plus `dist/verify.zip` with a `BUILD` id equal to `git rev-parse
   --short=7 HEAD`).

## Ship and prove (per game)

1. PR → merge → Vercel deploys `dist/` including `verify.zip`; the on-chain
   metadata now carries `verify_url`.
2. `curl <game_url>/verify.zip | unzip -p - BUILD` equals the id the deployed
   game embeds (`GX_BUILD_ID` in `dist/*.wasm` via `strings`).
3. On `colecovisiongx.com/play/<pda>` as an owner: play a round; the banner
   shows Verified with points and a rank; `/leaderboard/<pda>` lists it.
   The site's `game_runs` row is `VERIFIED`, `ranked = true`.
4. If the banner says "No verifier for this game build yet": the site's
   cached `BUILD` disagrees with the replay's build — check step 2, then the
   site's `verifier_bundles` table.

## Fleet catalog (from the 2026-09-16 survey; details in the skill's `references/catalog-2026-09-16.md`)

| Wave | Game | Published | Size | Notes |
|---|---|---|---|---|
| 0 | Cannonball Putt | Y | S | pilot; golf points; rename `sim.rs` |
| 1 | Dough.io | Y | S | has `SimRng`/`FxRng`; game over via `NextState` must move to the fade |
| 1 | Grand Theft Auto-Reply | Y | S | has `GameRng` |
| 2 | Pack The Ripper | Y | M | 3 `rand::rng()` sites |
| 2 | Sundae Shooter | Y | M | 7 RNG sites |
| 2 | Attic Excavator | Y | M | board seed drawn from `rand::rng()` → derive from `RunSeed` |
| 3 | Gulper | Y | L | 46 systems |
| 3 | Ladder Legend | Y | L | 28 systems, chart generation RNG |
| 3 | Grand Theft Otto | Y | L | ~42 systems behind custom run-conditions incl. attract mode |
| 3 | Dive Rise | Y | L | largest surface |
| 4 | Tire Stack, BeerPong | N | S | |
| 4 | Gravestone Gauntlet, Voidrunner, Beat Bender | N | M | voidrunner bypasses its `GameRng` twice; beat-bender lacks `record/` |
| 5 | Pizza Pinball, Hunted, Moleman Racing | N | L | pinball tick-sensitive; Hunted flat layout; moleman custom input |

## Follow-ups recorded, not planned

- Site: per-game `score_order` / score label from metadata, so golf can show
  strokes instead of points.
- Site: `NEXT_PUBLIC_GX_GAME_PDAS` needs a redeploy when a game is added.
- Template: `build_web.sh` now builds two wasm targets on Vercel; measure the
  added minutes on the pilot and, if painful, cache `target/` or build the
  verifier only when `src/` changed.

## Testing

- Script: no-op on the template; compiles on a Cannonball Putt scratch copy.
- Skill: exercised once by the pilot; `references/` are the artifacts.
- Pilot: the nine port steps above plus the production proof.
