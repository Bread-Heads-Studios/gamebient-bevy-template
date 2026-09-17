# Replay Verification Rollout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A script and a skill that port replay verification from the template into any game, proven end to end by shipping Cannonball Putt with a verified score on `colecovisiongx.com`, with the other 17 games catalogued in order for later.

**Architecture:** `tools/rollout-replay.sh` copies the template's `sim.rs`, `replay/`, `verify` bin, build script and workflow changes into a game and rewires `Cargo.toml`/`lib.rs`/`main.rs`/`GamePlugin`, printing `HAND EDIT` lines for anything it cannot do safely (same shape as `tools/rollout-record.sh`). The skill `rolling-out-replay-verification` (template `.claude/skills/`, symlinked at the workspace root like the others) drives the human/agent part: the determinism port checklist, the selftest, the real-run verification, deploy and the production proof. The pilot executes the skill on Cannonball Putt.

**Tech Stack:** bash + perl (script), Rust 2024 / Bevy 0.18 (game port), Node 22 (`tools/verify_fixture.mjs`), Vercel (game deploy), the site's `/api/runs` + `/leaderboard` (already live on Railway).

Spec: `docs/superpowers/specs/2026-09-16-replay-rollout-design.md`. Feature docs: `docs/replay-verification.md`.

## Global Constraints

- Template feature files are copied verbatim from this repo's `main` (57b53a0 or later): `src/game/sim.rs`, `src/game/replay/{mod,recorder,feeder,selftest}.rs`, `src/bin/verify.rs`, `build.rs`, `tools/build_verify.sh`, `tools/verify_fixture.mjs`, `tests/selftest.rs`, `docs/replay-verification.md`. The script never edits a copied file's contents; game-specific behaviour lives in the game's own files.
- The script never overwrites a game file that exists and is not byte-identical to the template's version of that file; it prints `HAND EDIT: <path>: <what to do>` instead. Idempotent: a second run changes nothing.
- Game `Cargo.toml` after the script: `[lib] name = "<snake>"`; `[[bin]] name = "<kebab>"` (the existing binary name — read it from `package.name`), `[[bin]] name = "verify" path = "src/bin/verify.rs" required-features = ["verify"]`; `verify = ["dep:wasm-bindgen"]`; `rand_xoshiro = "0.7"`; wasm32 `wasm-bindgen = { version = "0.2.108", optional = true }`; `gamebient-input … tag = "v0.3.0"`.
- Determinism rules for game code are rules 1–10 in `docs/replay-verification.md` §"Determinism rules"; the forbidden-names test in `sim.rs` is the mechanical check.
- The sealed replay's `score` is `GameData.score` when it exists, else `GameData::leaderboard_score() -> u32`; the recorder (`replay/recorder.rs`) is not edited — instead `sim::begin_run`/`checksum_tick`/`seal_run` read a `LeaderboardScore` trait implemented in the game's `scoring.rs` (see Task 1 for the exact template change).
- Cannonball Putt golf points: per completed hole `max(0, 2·par − strokes) × 100`, summed over `results`; incomplete holes add 0.
- Every game keeps `cargo test --all-features && cargo clippy --all-targets --all-features -- -D warnings && cargo fmt --check` green; `bash build_web.sh` must produce the game wasm plus `dist/verify.zip` whose `BUILD` equals `git rev-parse --short=7 HEAD`.
- Commit messages end with `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`. Push and PR only when a task says so.
- Repos: template `libs/gamebient-bevy-template` (branch `feat/replay-rollout`), pilot `games/cannonball-putt` (branch `feat/replay-verification`). Workspace root is not a repo; the skill symlink lives in its `.claude/skills/`.

---

## File map

Template:

| File | Responsibility |
|---|---|
| `src/game/scoring.rs`, `src/game/sim.rs`, `src/game/replay/recorder.rs` | `LeaderboardScore` trait so games without `score` plug in |
| `tools/rollout-replay.sh` | copy + wire + HAND EDIT report |
| `tools/test_rollout_replay.sh` | runs the script on scratch copies (template no-op; Cannonball Putt compiles) |
| `.claude/skills/rolling-out-replay-verification/SKILL.md` | the procedure |
| `…/references/port-checklist.md` | determinism port contract with code patterns |
| `…/references/catalog-2026-09-16.md` | survey table + waves |
| `…/references/irregular-games.md` | Hunted (flat), moleman-racing (custom input), beat-bender (no record), dough-io (NextState game over), voidrunner (GameRng bypass) |
| `AGENTS.md`, `README.md` | pointers |
| workspace `.claude/skills/rolling-out-replay-verification` | symlink |

Cannonball Putt: everything the script copies/edits, plus `src/game/shot_sim.rs` (renamed), `src/game/scoring.rs` (`LeaderboardScore`), `src/game/mod.rs` (SimSet chain), `src/game/{shot,ball,hole_flow,hazards}.rs` (`TickInput`, fixed tick), `src/game/replay/selftest.rs` script for golf, `tests/fixtures/selftest.gxr`, `assets/info.json` (`verify_url`).

---

### Task 1: `LeaderboardScore` trait in the template

**Files:**
- Modify: `src/game/scoring.rs`, `src/game/sim.rs`, `src/game/replay/recorder.rs`, `src/game/host.rs`, `docs/replay-verification.md`

**Interfaces:**
- Produces:

```rust
// src/game/scoring.rs
/// The single higher-is-better integer the leaderboard ranks. The template's
/// GameData has `score`; games without one (golf, racing) compute it here.
pub trait LeaderboardScore {
    fn leaderboard_score(&self) -> u32;
}
impl LeaderboardScore for GameData { fn leaderboard_score(&self) -> u32 { self.score } }
```

`sim::checksum_tick` folds `data.leaderboard_score()` (not `data.score`); `recorder::seal_run` seals `u64::from(data.leaderboard_score())`; `host::report_score`/`report_game_over` report `leaderboard_score()`. The template folds `lives` and the player transform in `player::checksum_player`; games fold their own state in their own system after `sim::checksum_tick`.

- [ ] **Step 1: Failing test** in `src/game/scoring.rs` tests: `leaderboard_score_is_score_for_the_template` (`GameData { score: 42, .. }` → 42). Run `cargo test scoring::` → compile error.
- [ ] **Step 2: Implement** the trait and the three call-site changes. Grep: `grep -rn "data.score\b" src/game/sim.rs src/game/replay src/game/host.rs` must show only `leaderboard_score()` uses afterwards (the HUD in `src/ui/` may keep reading `score`).
- [ ] **Step 3: Verify** `cargo test --all-features && cargo clippy --all-targets --all-features -- -D warnings && cargo fmt --check`. The selftest fixture is unaffected (template score is unchanged). Add two sentences to `docs/replay-verification.md` §Determinism rules: rule 8, "games without `score` implement `LeaderboardScore`; the HUD, host `score` event and replay must agree".
- [ ] **Step 4: Commit** `feat(replay): LeaderboardScore trait for games without a score field`.

---

### Task 2: `tools/rollout-replay.sh`

**Files:**
- Create: `tools/rollout-replay.sh`, `tools/test_rollout_replay.sh`

**Interfaces:**
- Produces: `tools/rollout-replay.sh <game-dir>` (exit 0; prints `HAND EDIT:` lines; ends with `rollout-replay: files in place for <game>` and the `next:` command line like `rollout-record.sh`).

- [ ] **Step 1: Read the pattern.** `tools/rollout-record.sh` in full (perl in-place edits, flat-layout detection, HAND EDIT convention, final `cargo fmt`).

- [ ] **Step 2: Write the script** with these sections, in order:

1. Resolve `TEMPLATE` (script dir's parent) and `GAME`; `PKG=$(grep -m1 '^name' "$GAME/Cargo.toml" | sed -E 's/.*"([^"]+)".*/\1/')`, `SNAKE=${PKG//-/_}`.
2. Pre-flight `HAND EDIT`s (do not stop): no `src/game/mod.rs` → flat layout; no `src/game/record` → "run tools/rollout-record.sh first"; `src/game/sim.rs` exists and differs from the template's → "rename your sim.rs (e.g. shot_sim.rs) and re-run"; `grep -q 'pub score' src/game/scoring.rs` fails → "implement LeaderboardScore in scoring.rs"; `grep -rn 'ButtonInput<KeyCode>' src/game --include=*.rs | grep -v autopilot` non-empty → "gameplay reads raw keys; route through TickInput"; `grep -n 'data\.\w* = ' src/game/autopilot.rs` non-empty → "autopilot writes GameData (dev-only; keep it out of the selftest script)".
3. Copy verbatim (skip + HAND EDIT when a differing file exists): the nine feature files from the Global Constraints list; `mkdir -p src/game/replay src/bin tests/fixtures tools`.
4. `Cargo.toml` edits (perl, each guarded by a `grep -q` so re-runs are no-ops): insert `[lib]`/`[[bin]]` blocks after `edition = "2024"`; `verify = ["dep:wasm-bindgen"]` after `record = …`; `rand_xoshiro = "0.7"` after `rand = "0.9"`; the optional wasm-bindgen line after the `getrandom` wasm32 line; `tag = "v0.2.x"` → `tag = "v0.3.0"` on the `gamebient-input` line.
5. `src/lib.rs`: if absent, write `#![allow(clippy::too_many_arguments, clippy::type_complexity)]` plus `pub mod X;` for every `^mod X;` in `main.rs` (excluding `#[cfg]`-gated ones, which are copied with their cfg line); then in `main.rs` replace the `mod` lines with `use <snake>::{…};` and `game::GamePlugin` → `game::GamePlugin::default()`. HAND EDIT if `main.rs` has no `game::GamePlugin` literal.
6. `src/game/mod.rs`: add `pub mod replay;` and `pub mod sim;` (sorted, next to `pub mod scoring;`); replace `pub struct GamePlugin;` with the `#[derive(Default)] pub struct GamePlugin { pub headless: bool }` block + doc comment; HAND EDIT for the `build()` body ("move run-state systems into sim::SimSet; see the skill's port-checklist").
7. `build_web.sh`: insert the `! -name 'verify.wasm'` guard into the `find` line and the verifier build + copy after the wasm-opt step (HAND EDIT if the `find` line is not the template's).
8. Workflows: `ci.yml` add `setup-node@v4` (22) + fixture step after "Build web bundle"; `release.yml` add the verify zip package step and the two-line upload `path` with `<kebab>` substituted for `gamebient-game` (HAND EDIT if the anchor lines are missing).
9. `.gitignore` lines; `assets/info.json`: insert `"verify_url": "<game_url>/verify.zip"` after the `"demo_url"` line using the file's `game_url` value (HAND EDIT if no `game_url`).
10. `cargo fmt --all`; final summary lines.

- [ ] **Step 3: `tools/test_rollout_replay.sh`**: (a) copies the template to a scratch dir, runs the script, asserts `git status --porcelain` in the copy is empty except `Cargo.lock` (no-op on the template itself — it already has everything); (b) copies `../../games/cannonball-putt` to a scratch dir, renames `src/game/sim.rs` → `src/game/shot_sim.rs` (and fixes its `mod` line) to simulate the one documented hand edit, runs the script, and asserts `cargo check --features verify` succeeds and the HAND EDIT output names exactly: `LeaderboardScore` (no `score` field) and the `GamePlugin::build` port. Uses `$TMPDIR`; never touches the real game checkout.

- [ ] **Step 4: Run** `bash tools/test_rollout_replay.sh` → both parts pass. `shellcheck tools/rollout-replay.sh` if available (report if not).

- [ ] **Step 5: Commit** `feat(tools): rollout-replay.sh ports replay verification into a game`.

---

### Task 3: The skill

**Files:**
- Create: `.claude/skills/rolling-out-replay-verification/SKILL.md`, `references/port-checklist.md`, `references/catalog-2026-09-16.md`, `references/irregular-games.md`
- Create: workspace symlink `/Users/kelliott/Gamebient/colecovisiongx/.claude/skills/rolling-out-replay-verification -> ../../libs/gamebient-bevy-template/.claude/skills/rolling-out-replay-verification` (not in any repo; the other three skills are symlinked the same way)
- Modify: `AGENTS.md` (one bullet under "How to add things"), `README.md` (one line)

- [ ] **Step 1: `SKILL.md`** — frontmatter `name: rolling-out-replay-verification`, `description: Use when a Gamebient game needs replay verification / verified leaderboards ported in, when the user says "roll out replay verification", "make <game> verifiable", or "the leaderboard is empty for <game>", or when a game's info.json lacks verify_url`. Body, in the voice of `recording-game-footage/SKILL.md`:
  1. **Pick the game** from `references/catalog-2026-09-16.md` (next unchecked in wave order unless told otherwise); read its row and any entry in `references/irregular-games.md`.
  2. **Branch** `feat/replay-verification` from the game's `main`; run `tools/rollout-replay.sh games/<game>`; do every `HAND EDIT` it prints.
  3. **Port** per `references/port-checklist.md`; commit after `cargo check --features verify` passes.
  4. **Selftest**: write the game's `replay/selftest.rs` script (inputs only, via `VirtualInput`, ≥ 600 ticks, exercising scoring); `cargo run --features verify --bin verify -- --selftest --write tests/fixtures/selftest.gxr`; `cargo test --all-features` includes `tests/selftest.rs` (fixture verifies; tampered inputs do not).
  5. **Real run**: `GX_REPLAY_DIR=build/replays cargo run` (play a run to game over), then `cargo run --features verify --bin verify -- build/replays/<ms>.gxr` and `tools/build_verify.sh && node tools/verify_fixture.mjs build/replays/<ms>.gxr` → both `matches: true` with equal checksums.
  6. **Web build**: `bash build_web.sh`; `unzip -p dist/verify.zip BUILD` equals `GX_BUILD_ID=<version>+$(git rev-parse --short=7 HEAD)`; the game wasm in `dist/` is the big one. Note the build time.
  7. **Ship**: `cargo test/clippy/fmt`, PR, merge; wait for the Vercel production deploy; `curl -s <game_url>/verify.zip | unzip -p - BUILD`; `curl -s <game_url>/assets/info.json | grep verify_url`; `curl -s https://colecovisiongx.com/api/game-meta?pda=<pda>` (cached 60 s) shows `verifyUrl`.
  8. **Prove on production**: as an owner, play a full run on `https://colecovisiongx.com/play/<pda>`; the banner reads `Verified · <score> · #<rank>`; `/leaderboard/<pda>` lists it. Record the run id from the network tab (`POST /api/runs` response) in the catalog row.
  9. **Tick the catalog row** (date, PR, build id, run id) and commit the template.
  Plus a "When it fails" table: `No verifier for this game build yet` → BUILD/replay id mismatch (step 6/7); `Couldn't reproduce this run` → nondeterminism (re-run step 5; check the checklist's usual suspects); `This run's seed was already used` → restarted too fast; `Verifier unavailable` → site logs (`verifier_error` detail).

- [ ] **Step 2: `references/port-checklist.md`** — rules 1–9 from `docs/replay-verification.md` plus `LeaderboardScore`, each with a before/after Rust snippet from the Cannonball Putt port (Task 4 supplies them; write the checklist with placeholders marked `<!-- from pilot -->` now and fill them in Task 6). Include: the `SimSet` chain registration shape; `Res<TickInput>` swap; `GameRng` replacement for `rand::rng()` (`rng.0.random_range(..)`); the `ScreenFade::request(GameOver)` rule; folding game state into `Checksum` inside the game's own last system; the selftest script skeleton; what may stay in `Update` (pure presentation) and the test that proves it (`verify --selftest` unchanged after removing the system).

- [ ] **Step 3: `references/catalog-2026-09-16.md`** — the survey table from the spec expanded with the per-game columns from the 2026-09-16 survey (layout, record, Update systems, RNG sites with file:line, wall-clock, HashMap iter, transcendental count, published PDA, difficulty, notes) and a status column (`[ ]` / date + PR + run id). `references/irregular-games.md` — one section each for Hunted, moleman-racing, beat-bender, dough-io, voidrunner, attic-excavator (board seed), grand-theft-otto (attract-mode run conditions) with the specific extra steps.

- [ ] **Step 4: Symlink + docs.** `ln -s ../../libs/gamebient-bevy-template/.claude/skills/rolling-out-replay-verification /Users/kelliott/Gamebient/colecovisiongx/.claude/skills/rolling-out-replay-verification`; AGENTS.md bullet: "**Rolling replay verification into a game:** `tools/rollout-replay.sh <game>` then the `rolling-out-replay-verification` skill."; README line.

- [ ] **Step 5: Commit** `docs(skills): rolling-out-replay-verification`.

---

### Task 4: Pilot — mechanical port of Cannonball Putt

Repo: `games/cannonball-putt`, branch `feat/replay-verification` from `main`.

- [ ] **Step 1:** `git mv src/game/sim.rs src/game/shot_sim.rs`; update `pub mod sim;` → `pub mod shot_sim;` in `src/game/mod.rs` and every `sim::` use (`grep -rn "sim::" src tests`). `cargo test` still passes.
- [ ] **Step 2:** `bash ../../libs/gamebient-bevy-template/tools/rollout-replay.sh .` — the script prints exactly three HAND EDITs for this game: `src/game/scoring.rs` (no `score` field on `GameData`; implement `LeaderboardScore`), `src/game/autopilot.rs` (writes `GameData` fields directly — dev-only, keep that path out of the selftest script), and `src/game/mod.rs` (the `GamePlugin::build` port, which is always printed). Nothing about `src/game/sim.rs`, because Step 1 renamed it. Confirm `Cargo.toml` (`[lib]`/`[[bin]]`, `default-run`, `verify` feature, `rand_xoshiro`, `wasm-bindgen`, `gamebient-input` at `v0.3.0`), `src/lib.rs`, `src/main.rs`, `src/game/mod.rs` (`GamePlugin { headless }`), `src/game/host.rs` (`Option<ResMut<GlobalVolume>>` + the `HostCommand::Seed` arm), `build_web.sh`, workflows, `.gitignore`, `assets/info.json` (`verify_url: https://cannonball-putt.vercel.app/verify.zip`).
- [ ] **Step 3:** `src/game/scoring.rs`: implement `LeaderboardScore` with golf points and unit tests (`hole_in_one_on_par_3_is_500`, `par_is_300`, `triple_bogey_is_0`, `incomplete_hole_adds_nothing`, `nine_pars_sum`). Make `host::report_score` and the HUD's round total use the same function where a "score" is shown (the HUD keeps strokes; the host `score` event switches to points so the site's `score` event and the replay agree — note this in `host.rs`'s doc table).
- [ ] **Step 4:** `harness` feature: `src/harness.rs` imports `ScreenFade` from `crate::ui` — with the lib split it must import via the crate name or keep `crate::` (it is inside the lib now, so `crate::` still works). `cargo check --features harness`, `--features autopilot`, `--features record`, `--features verify` all compile.
- [ ] **Step 5:** `cargo fmt`, commit `chore: rollout-replay.sh + LeaderboardScore for golf`.

---

### Task 5: Pilot — determinism port

Repo: `games/cannonball-putt`.

- [ ] **Step 1: SimSet chain.** In `GamePlugin::build`: `insert_resource(Time::<Fixed>::from_duration(sim::tick_duration()))`; init `sim::{SimTick, RunSeed, PendingSeed, GameRng, Checksum, SimPrev}` and `replay::recorder::ReplayRecorder`; `configure_sets(FixedUpdate, sim::SimSet.run_if(in_state(Playing).and(not_paused)))`; `OnEnter(Playing)` chain becomes `(reset_paused, reset_game_data, reset_tilt_clock, sim::begin_run, replay::recorder::begin_recording, hole_flow::enter_playing).chain()`; the existing chained `Update` gameplay tuple moves to `FixedUpdate` inside `SimSet` as `(sim::advance_tick, shot::update_shot, ball::integrate, ball::animate_sink, hole_flow::update_hole_flow, hazards::advance_tilt_clock, sim::checksum_tick, replay::recorder::record_tick, sim::remember_sim_prev).chain()`; the presentation systems (`shot::update_aim_arrow`, `hazards::animate_flow_particles`, `hazards::animate_tilt_indicators`, `ball::spawn_trail`, `ball::fade_trail`) stay in `Update` gated `in_state(Playing)` — they read `Ball`/`Shot` and write only their own visuals (confirm by reading them; if one writes `Ball` or `Shot`, it moves into the chain). `toggle_pause` moves to `FixedUpdate` before `SimSet` reading `TickInput`; `restore_tick_frame_while_paused` registered as in the template; `OnExit(Playing)` = `(replay::recorder::seal_run, cleanup_game_entities).chain()`. Headless branch: skip `setup_scene`, audio, overlays, `harness`.
- [ ] **Step 2: Inputs.** `shot::update_shot` (and any other chain system reading input) takes `Res<gamebient_input::TickInput>`; `pause_quit` keeps `GameInput`.
- [ ] **Step 3: Time.** `ball::integrate`'s `dt = time.delta_secs().min(1.0/30.0)` becomes the fixed delta (drop the clamp: inside `FixedUpdate` it is always 1/60). `hazards::advance_tilt_clock` and `hole_flow`'s timer likewise run in the chain. Ball trail/particles use `Update`'s delta — fine.
- [ ] **Step 4: Checksum.** In `hole_flow::update_hole_flow` (or a new `checksum_golf` system placed right after `sim::checksum_tick`): `sum.fold(ball.translation.x.to_bits() as u64)`, `.y`, `u64::from(data.hole_index as u32)`, `u64::from(data.strokes)`.
- [ ] **Step 5: Game over.** Already `fade.request(GameState::GameOver)` from `hole_flow` (a sim system) — keep. In headless there is no `ScreenFade`: `hole_flow::update_hole_flow` must take `Option<ResMut<ScreenFade>>` and, when `None`, set `NextState(GameOver)` directly (the verifier's `Ended::GameOver` path). Test that both paths end the run.
- [ ] **Step 6: Selftest script** (`src/game/replay/selftest.rs` replaces the template's cube script): a golf bot — on each tick, if `shot.phase` is aiming, hold nothing and tap `A` when the aim angle points at the cup (reuse the harness's aiming math from `src/harness.rs` via a pure helper), then tap `A` again when the power meter is in the good window; `SELFTEST_TICKS = 3600` (60 s) so at least a few holes complete. Inputs only — no `GameData` writes. `--selftest --write tests/fixtures/selftest.gxr`; `cargo test --all-features` (the two `tests/selftest.rs` tests: matches; tampered `held ^= 16` on the first `A` tap does not).
- [ ] **Step 7: Real run.** `GX_REPLAY_DIR=build/replays cargo run`, play to the scorecard; native and Node verdicts match; `pnpm`-free: `bash tools/build_verify.sh && node tools/verify_fixture.mjs build/replays/*.gxr`.
- [ ] **Step 8:** `cargo test --all-features && cargo clippy --all-targets --all-features -- -D warnings && cargo fmt --check && cargo run --features autopilot` (tour still plays) && `bash build_web.sh` (`unzip -p dist/verify.zip BUILD`; `ls -la dist/*.wasm`). Commit `feat: deterministic sim + replay verification`.
- [ ] **Step 9:** Fill the `<!-- from pilot -->` snippets in the template's `references/port-checklist.md` with real before/after code from this port (template repo, same branch as Tasks 1–3); commit there.

---

### Task 6: Pilot — ship and prove

- [ ] **Step 1:** Push `feat/replay-verification`, `gh pr create` (summary: deterministic sim, verify bin, `verify_url`; test plan from Task 5), merge after CI is green (the game's CI now runs the Node fixture step).
- [ ] **Step 2:** Watch the Vercel production deploy for `cannonball-putt`; note the build duration vs the previous deploy. `curl -s https://cannonball-putt.vercel.app/verify.zip -o /tmp/v.zip && unzip -p /tmp/v.zip BUILD`; `curl -s https://cannonball-putt.vercel.app/assets/info.json | grep verify_url`; `curl -s "https://colecovisiongx.com/api/game-meta?pda=BrgzzBTfV2soCeZxWRKz2LBKNfCPdjwoCUqk6PwfCZcL"` includes `verifyUrl` (after ≤ 60 s cache).
- [ ] **Step 3 (owner at the keyboard):** on `https://colecovisiongx.com/play/BrgzzBTfV2soCeZxWRKz2LBKNfCPdjwoCUqk6PwfCZcL`, signed in with the wallet holding edition #1, play a round to the scorecard. Expected banner: `Verified · <points> · #1 · New personal best`; `https://colecovisiongx.com/leaderboard/BrgzzBTfV2soCeZxWRKz2LBKNfCPdjwoCUqk6PwfCZcL` shows the row. If not, follow the skill's "When it fails" table; the site's `game_runs.reason`/`verdict` for the run is the diagnostic.
- [ ] **Step 4:** Tick the catalog row (date, PR, build id, run id); template commit; push the template branch and open its PR (`feat/replay-rollout`: Tasks 1–3 + the filled checklist).

---

## Self-review notes

- Spec coverage: trait (T1), script + its test (T2), skill + references + symlink + docs (T3), pilot mechanical (T4), pilot port + selftest + real run (T5), ship + production proof + catalog tick (T6). Fleet beyond the pilot is deliberately a checklist (the catalog), per the owner's scope choice.
- The one template code change (T1) must land before the script copies `sim.rs`/`recorder.rs` (T2 copies from the working tree, so the order holds within the branch).
- Cannonball Putt's `harness` and `autopilot` features poke `GameData`/`ScreenFade`; they remain dev-only and never enter the headless verifier (`GamePlugin { headless: true }` skips them, as in the template).
