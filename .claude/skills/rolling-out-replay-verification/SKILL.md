---
name: rolling-out-replay-verification
description: Use when a Gamebient game needs replay verification / verified leaderboards ported in, when the user says "roll out replay verification", "make <game> verifiable", or "the leaderboard is empty for <game>", or when a game's info.json lacks verify_url
---

# Rolling Out Replay Verification

## Overview

Every template-derived game can record a `GXR1` replay of its own run — the
seed it started from plus the exact input of every sim tick — and post it to
the host when the run ends. The ColecoVision GX site re-simulates that
replay headlessly, in a `wasm32` build of the game's own crate, to confirm
the score before it becomes a leaderboard entry. None of that verifies until
a game's sim is ported onto the deterministic scaffolding that makes a
replay reproducible: `sim::SimSet` in `FixedUpdate`, `TickInput` instead of
`GameInput`, `GameRng` instead of `rand::rng()`, and `LeaderboardScore` for
games without a plain `score` field. Games are copies of the template, not
dependents, so every game needs this ported in by hand.

`tools/rollout-replay.sh <game-dir>` does the mechanical part: it copies
`src/game/sim.rs`, `src/game/replay/`, `src/bin/verify.rs`, `build.rs`,
`tools/build_verify.sh`, `tools/verify_fixture.mjs` and
`docs/replay-verification.md` from this template into the game verbatim,
and wires `Cargo.toml`, `src/lib.rs`/`main.rs`, `src/game/mod.rs`'s
`GamePlugin { headless }` shape, `build_web.sh`, the CI/release workflows,
`.gitignore` and `assets/info.json`'s `verify_url`. It is idempotent and
never overwrites a file it didn't create — anywhere it can't act safely it
prints a `HAND EDIT:` line instead. This skill drives everything the script
can't do: the per-game determinism port, the selftest script, a verified
real run, the web build, shipping, and the production proof. The feature's
contract lives in `docs/replay-verification.md` (rules 1–10); this skill's
job is to apply it to one more game and prove it end to end.

For a game that was already ported and has since fallen behind, use
`tools/rollout-replay.sh --upgrade <game-dir>` instead — see
"[Upgrading a ported game](#upgrading-a-ported-game)" below.

Supporting files: [port-checklist.md](references/port-checklist.md) (the
determinism rules with before/after code — the reference to keep open while
porting), [catalog-2026-09-16.md](references/catalog-2026-09-16.md) (every
game's survey facts, wave order and status — where you pick the next game
and record what shipped), [irregular-games.md](references/irregular-games.md)
(the seven games whose layout, scoring or randomness need extra steps beyond
the checklist: Hunted, moleman-racing, beat-bender, dough-io, voidrunner,
attic-excavator, grand-theft-otto).

## Process

1. **Pick the game.** Take the next unchecked row in wave order from
   `references/catalog-2026-09-16.md` unless told otherwise. Read that row
   in full (layout, RNG sites, transcendental count, published PDA) and,
   if the game is listed there, its section in `references/irregular-games.md`.

2. **Branch and roll out.**
   ```bash
   cd games/<game>
   git checkout main && git pull
   git checkout -b feat/replay-verification
   bash ../../libs/gamebient-bevy-template/tools/rollout-replay.sh .
   ```
   Read every `HAND EDIT:` line the script prints and do each one before
   moving on — they cover things it refuses to guess at: an existing
   non-template `sim.rs` (rename it, e.g. `shot_sim.rs`, and re-run), a
   `GameData` with no `score` field, gameplay reading raw
   `ButtonInput<KeyCode>` instead of `TickInput`, an autopilot that writes
   `GameData` fields directly (keep that path out of the selftest script),
   and always the `GamePlugin::build` port itself (the script never rewires
   a game's own systems onto `sim::SimSet`).

3. **Port the determinism rules.** Work through
   `references/port-checklist.md` rule by rule against the game's actual
   systems. Commit once it compiles:
   ```bash
   cargo check --features verify
   git add -A && git commit -m "feat(replay): port <game> onto sim::SimSet"
   ```
   **CI's Node fixture steps (`node tools/verify_fixture.mjs
   tests/fixtures/selftest-wasm.gxr` and the informational native one) are
   red from this commit until step 4 generates both fixtures — that's
   expected, not a regression to chase.**

4. **Write and run the selftest.** `src/game/replay/selftest.rs` was copied
   from the template verbatim; replace its `script` system with one that
   drives the game's own controls through `VirtualInput` for at least 600
   ticks and actually exercises scoring (not just movement) — the
   template's own `script` (sweep right/left, tap A once a second) is the
   skeleton to start from. Nothing in it may touch `GameData` directly;
   only inputs, or a replay could not reproduce it.
   Generate **both** fixtures. The native one is what `cargo test`
   re-simulates; the wasm one is what CI blocks on, and only the wasm module
   can record it (`selftest_record()`), so it needs a verifier build first:
   ```bash
   cargo run --features verify --bin verify -- --selftest --write tests/fixtures/selftest.gxr
   bash tools/build_verify.sh
   node tools/verify_fixture.mjs --record tests/fixtures/selftest-wasm.gxr
   node tools/verify_fixture.mjs tests/fixtures/selftest-wasm.gxr   # matches: true
   cargo test --all-features
   git add tests/fixtures/selftest.gxr tests/fixtures/selftest-wasm.gxr
   ```
   `cargo test` now runs `tests/selftest.rs`: the freshly recorded run
   verifies, a tampered run does not, the native fixture still verifies, and
   the wasm fixture decodes with the right tick count (native deliberately
   does not re-simulate that one — see `docs/replay-verification.md`, "Two
   fixtures, and which one is the gate"). That closes out the red CI step
   from step 3.

5. **Play a real run and verify it two ways.**
   ```bash
   GX_REPLAY_DIR=build/replays cargo run   # play to game over
   cargo run --features verify --bin verify -- build/replays/<ms>.gxr
   tools/build_verify.sh && node tools/verify_fixture.mjs build/replays/<ms>.gxr
   ```
   Both must print `matches: true` with the same `checksum`. A mismatch
   here means nondeterminism, not cheating — re-play and check the
   checklist's usual suspects (a system still in `Update`, a stray
   `rand::rng()`, a `HashMap` iteration, a transcendental function whose
   native and wasm results disagree — see the checklist's last section and
   the Caveat in `docs/replay-verification.md`).

6. **Web build.** Note how long it takes; this is the number the spec's
   follow-up about Vercel build minutes needs.
   ```bash
   bash build_web.sh
   unzip -p dist/verify.zip BUILD
   ```
   The printed line must read `GX_BUILD_ID=<version>+<7-char SHA>` where the
   SHA is `git rev-parse --short=7 HEAD`. Confirm `dist/` still has the game
   wasm (the large file) alongside `verify.zip`.

7. **Ship.**
   ```bash
   cargo test --all-features && cargo clippy --all-targets --all-features -- -D warnings && cargo fmt --check
   ```
   Open the PR, merge, and wait for the Vercel production deploy. Then:
   ```bash
   curl -s <game_url>/verify.zip | unzip -p - BUILD
   curl -s <game_url>/assets/info.json | grep verify_url
   curl -s https://colecovisiongx.com/api/game-meta?pda=<pda> # cached 60s
   ```
   The first must match what step 6 produced; the second confirms
   `verify_url` shipped in the deployed metadata; the third must show
   `verifyUrl` once its cache turns over.

8. **Prove it on production.** As an owner, play a full run on
   `https://colecovisiongx.com/play/<pda>`. The banner must read
   `Verified · <score> · #<rank>`, and the run must appear on
   `/leaderboard/<pda>`. Open the browser's network tab, find the
   `POST /api/runs` response, and copy the run id — that's the proof this
   skill exists to produce.

9. **Tick the catalog row.** In `references/catalog-2026-09-16.md`, change
   the game's status from `[ ]` to the date, PR link, build id and run id,
   then commit the template:
   ```bash
   git add .claude/skills/rolling-out-replay-verification/references/catalog-2026-09-16.md
   git commit -m "docs(skills): <game> shipped with verified replays"
   ```

## Upgrading a ported game

A game ported in an earlier wave keeps the template files it was given, not
the ones the template has now — `sim::RunOver`/`sim::end_run`, the
`NextState`-before-`done` ordering in `run_verify_app`, the wasm-recorded
fixture, a `zip` guard in `build_verify.sh`. Nothing tells the game about
any of it, so pulling it forward is its own job:

```bash
cd games/<game>
git checkout main && git pull
git checkout -b chore/replay-upgrade
bash ../../libs/gamebient-bevy-template/tools/rollout-replay.sh --upgrade .
```

`--upgrade` classifies each copied feature file instead of skipping it:

* **Byte-identical to any committed template version** → a stale verbatim
  copy nobody edited; it is overwritten with the current one and listed in
  the summary at the end.
* **Matches no template version** → the game edited it. It is left exactly
  as it is, with
  `HAND EDIT: <path>: locally modified; merge template changes by hand (git -C <template> diff <sha>:<path> HEAD:<path> …)`.
  That `<sha>` is the last template version this game's own history did
  match, so the diff is precisely the set of template changes it is missing
   — read it and port them, keeping the game-specific parts (its
  `checksum_<game>` system, its `LeaderboardScore`, its `AUTOPILOT_*`
  constants).
* **`src/game/replay/selftest.rs` is never touched.** After the port it is
  the game's own script, not the template's.

`.github/workflows/ci.yml` gets the same treatment structurally: a game
whose Node step still verifies only the native fixture has that step (and
the comment block above it, usually a `continue-on-error` rationale) replaced
with the wasm gate plus the informational native cross-check. A game that
already gained the wasm gate by hand is left alone.

Then, in this order:

1. Do every HAND EDIT. The common one is moving a game off its own
   `states::RunOver`/`run_not_over` onto `sim::RunOver` + `sim::end_run`
   (delete the local copies; the run-ending system calls `sim::end_run`,
   `GamePlugin::build`'s run condition uses `sim::run_not_over`, and
   `sim::begin_run` clears the latch so the game's own `reset_*` system
   should stop doing it).
2. Re-pin `wasm-bindgen` to the fleet's version if the game drifted:
   `install.sh`, both workflows, and `Cargo.lock`
   (`cargo update -p wasm-bindgen --precise <version>`, plus `js-sys`,
   `web-sys`, `wasm-bindgen-futures` and the three `wasm-bindgen-*` crates
   as needed). A CLI/lib skew produces a bundle that fails to load, and the
   fleet's CI installs one pinned CLI.
3. Rebuild the verifier and **regenerate `tests/fixtures/selftest-wasm.gxr`**
   (`bash tools/build_verify.sh` then
   `node tools/verify_fixture.mjs --record tests/fixtures/selftest-wasm.gxr`,
   then verify it) — a refreshed `sim.rs` usually changes the checksum, and
   a wasm-bindgen change changes the module.
4. Gates as in step 7 below, plus `bash build_web.sh` and one autopilot tour.

The build id in the header of already-submitted replays changes, and that
invalidates nothing: the site picks the verifier module by the replay's own
`build` field, so old replays keep verifying against the release they were
recorded against.

## When it fails

| Banner | Cause | Check |
|---|---|---|
| `No verifier for this game build yet` | The site's cached `BUILD` id doesn't match the replay's `build` field | Step 6/7: re-run `unzip -p dist/verify.zip BUILD` against the deployed `verify.zip`; confirm `build_verify.sh` and the game binary were built from the same commit (a dirty tree or a stale Vercel cache breaks this) |
| `Couldn't reproduce this run` | Nondeterminism in the sim | Re-run step 5; walk the port checklist's rule-by-rule "usual suspects" (a system still in `Update`, `rand::rng()`, wall-clock reads, a `HashMap`, a transcendental-function ulp drift between native and wasm) |
| `This run's seed was already used — start a new run` | The player restarted faster than the host could issue a fresh seed, or replayed an already-submitted run | Start a new run from the title screen; not a bug to fix in the game |
| `Verifier unavailable, try again later` | The site failed to load or run the verifier module | Check the site's logs for the `verifier_error` detail (see `docs/replay-verification.md` §Server contract, "Versioning") |
