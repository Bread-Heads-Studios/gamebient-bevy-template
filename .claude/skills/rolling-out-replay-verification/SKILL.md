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
`tools/build_verify.sh`, `tools/verify_fixture.mjs`,
`docs/replay-verification.md`, `tests/archetype_order.rs` and
`tests/windowed_shape.rs` from this template into the game verbatim,
and wires `Cargo.toml`, `src/lib.rs`/`main.rs`, `src/game/mod.rs`'s
`GamePlugin { headless }` shape, `build_web.sh` (including the
`tools/build_verify.sh` + `cp dist-verify.zip dist/verify.zip` step that
produces what `verify_url` serves), the CI/release workflows, `.gitignore`
and `assets/info.json`'s `verify_url`. It also writes the common
`impl LeaderboardScore for GameData` when `GameData` has a `pub score: u32`
and no impl, and reorders `drive_autopilot` before `accumulate_input`. It is
idempotent and never overwrites a file it didn't create — anywhere it can't
act safely it prints a `HAND EDIT:` line instead (some of them advisory, and
labelled as such). This skill drives everything the script
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
   non-template `sim.rs` (rename it, e.g. `shot_sim.rs`, and re-run),
   `tests/archetype_order.rs` (the copied probe queries the template's own
   `Player` entity, so it does not compile until step 4 adapts it), a
   `GameData` whose leaderboard score isn't a plain `pub score: u32`,
   gameplay reading raw `ButtonInput<KeyCode>` instead of `TickInput`, an
   autopilot that writes `GameData` fields directly (keep that path out of
   the selftest script), and always the `GamePlugin::build` port itself
   (the script never rewires a game's own systems onto `sim::SimSet`).

   One line is labelled `HAND EDIT (advisory)` and lists
   `<file>:<Component>` pairs: a file under `src/` that pulls an entity out
   of a query and inserts, removes or despawns on it, where that component is
   also declared and queried in `src/game/`. That is the archetype-order trap
   — see the checklist's "The two order traps". It is a reading list, not a
   verdict: sim code decorating its own entities from inside the chain shows
   up too and is fine. Read the files, then let `tests/archetype_order.rs`
   answer it.

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

   **Adapt `tests/archetype_order.rs` in the same pass** — it is not
   optional, and it does not compile as copied: the template's version
   queries the template's own `Player` sim entity. Replace its marker
   components and `decorate_*` systems with stand-ins for this game's real
   `Update` decorators, reproducing their *branching* (different entities
   getting different component sets is what splits the archetype) and keeping
   the child spawn, and point `trace_order` at the queries the game's
   order-sensitive sim systems iterate. It is the only test that catches the
   archetype-order trap — the "delete the system and re-run `--selftest`"
   check provably cannot — so a run of it that passes with the template's
   markers still in place has tested nothing.

   **`tests/windowed_shape.rs` is its resource-level sibling**, and unlike
   the probe it compiles as copied: it records the scripted run in an app
   carrying `ScreenFade::boot()` and verifies it in a bare one, which is the
   production question (the browser records with the windowed shape present,
   the site re-simulates with none of it). Extend `record_windowed` with this
   game's own `!headless` `Update` systems and any `UiPlugin`/`AssetsPlugin`
   resource a sim system might read. It exists because Sundae Shooter's
   `fire_scoop`/`swap_queue` gated on the fade and no fixture could see it —
   a browser recording claimed 195 points against 200 re-simulated. The file's doc comment and the
   checklist's "What may stay in `Update`" have the detail.

   **Then measure that the probe's script actually reaches the mechanics
   presentation could touch** — adding the systems is half the job, proving
   the bot runs them is the other half. Add a counter resource per
   windowed-only system (the file ships `FadeBusyFrames` as the pattern) and
   look at the totals. **If the fixture bot is a careful router, write a
   reckless second script and assert it reaches them (non-zero counter):**
   the probe carries a two-row bot table (`enum Bot`) with `reckless_script`
   in the second row as a documented placeholder, so this is a replacement,
   not new scaffolding. Attic Excavator is why the step exists — under its
   fixture bot `heavy::wobble_shake` fired zero times in 1800 ticks and
   `cat::cat_touch` never connected, and two planted bugs passed the probe
   until a heavy-seeking second bot went in. Checklist rule 1, "Then measure
   that the probe's script actually reaches the mechanics", has the
   measurement and the two constraints on the bot (pure input; no RNG).

5. **Play a real run and verify it two ways.**
   ```bash
   GX_REPLAY_DIR=build/replays cargo run   # play to game over
   cargo run --features verify --bin verify -- build/replays/<ms>.gxr
   tools/build_verify.sh && node tools/verify_fixture.mjs build/replays/<ms>.gxr
   ```
   Both must print `matches: true` with the same `checksum`. A mismatch
   here means nondeterminism, not cheating — re-play and check the
   checklist's usual suspects (a system still in `Update`, a stray
   `rand::rng()`, a `HashMap` iteration, an `Update` system decorating a sim
   entity, a transcendental function whose native and wasm results disagree
   — see the checklist's last section and the Caveat in
   `docs/replay-verification.md`).

   If the answer turns out to be native-vs-wasm ulp drift, write the
   measurement into a **game-local `docs/replay-notes.md`**, never into
   `docs/replay-verification.md`: that file is copied verbatim from the
   template and `--upgrade` refreshes it only while it is byte-identical to a
   committed template version, so appending to it costs the game every future
   contract update.

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
   `cargo test` includes `tampered_inputs_do_not_verify`. Before trusting
   it, confirm it fails for the right reason on this game — see the
   checklist's "Verify the tamper test fails for the right reason"; a game
   with an intro card can pass it vacuously.
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
* **`src/game/replay/selftest.rs` and `tests/archetype_order.rs` are the
  narrow case** — both are files the port *replaces* rather than keeps. Each
  is refreshed only while it is still byte-identical to a committed template
  version —
  i.e. nobody ever replaced the skeleton, so there is nothing to lose. Once
  the port has replaced it, it is left alone and, unlike every other file,
  prints **no** HAND EDIT: it is supposed to differ, so a line about it would
  be noise on every upgrade of every game for ever.
  `tests/archetype_order.rs` has one extra rule: if it is **absent**,
  `--upgrade` does *not* create it. The template's version references the
  template's own `Player` entity, which most games do not have, so
  materialising it would turn a green checkout red at `cargo test` with
  nothing in the output to explain where the file came from — and not
  breaking a working game is the whole promise of `--upgrade`. You get a HAND
  EDIT and a summary line asking for it instead; copy it from the template
  and adapt it as step 4 describes.
* **`tests/windowed_shape.rs` is the opposite case.** Everything it names is
  template-owned, so the copy compiles in any ported game — it is therefore
  written whenever it is absent, `--upgrade` included, and called out in the
  summary. Expect it to be able to fail on a checkout that was green a
  minute ago: that is a finding (a sim system is reading presentation state),
  not a regression. A game that already adapted it keeps its copy and gets
  the usual "locally modified" HAND EDIT; when you merge the template's
  changes in, the second bot row is the part worth taking — step 4's
  measurement paragraph says why. It is skipped, with a HAND EDIT, only when the game's
  `src/game/replay/selftest.rs` does not export `pub fn script` or its
  `ScreenFade` has lost `boot()` — the two things the copy needs.

Under `--upgrade`, a game whose `src/game/mod.rs` already names
`sim::SimSet` also stops getting the two HAND EDITs the script prints
unconditionally on a first rollout (the `GamePlugin::build` port, and
"autopilot writes GameData fields directly"). The work is demonstrably done;
re-printing it devalues the rest of the list.

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
2. Re-pin `wasm-bindgen` to the fleet's version (`0.2.108`) if the game
   drifted. A CLI/lib skew produces a bundle that fails to load at runtime,
   and the fleet's CI installs one pinned CLI, so this is a hard blocker.
   Three places plus the lock:
   `install.sh`, `.github/workflows/ci.yml`, `.github/workflows/release.yml`
   (all say `wasm-bindgen-cli@<version>`), and `Cargo.toml`, which the
   rollout script pins as `wasm-bindgen = { version = "=0.2.108", optional
   = true }` — the `=` is deliberate, so a later `cargo update` fails to
   resolve rather than drifting silently.

   **Do not reach for `cargo update -p wasm-bindgen --precise`.** It cannot
   get there: `js-sys 0.3.103` requires `wasm-bindgen =0.2.126` and
   `wasm-bindgen-futures 0.4.76` requires `js-sys =0.3.103`, so every
   single-package downgrade is blocked by one of the others, in a cycle.
   Take the lock from before the drift instead, and prove it:
   ```bash
   # the last commit whose Cargo.lock still had the fleet's version
   git log --oneline -- Cargo.lock
   git checkout <sha> -- Cargo.lock
   cargo check --locked --features verify   # must pass: the lock is self-consistent
   ```
   `--locked` is the proof, not a formality — it fails rather than silently
   re-resolving, which is the whole question being asked.

   **Do this before the next deploy, not after.** `tools/build_verify.sh` is
   copied verbatim into every game and refreshed by `--upgrade`, and it
   builds `--locked`; `build_web.sh` (the Vercel build command, which also
   builds `--locked`) invokes it. So from the moment a game is rolled out or
   upgraded, a stale or drifted `Cargo.lock` **fails that game's Vercel
   deploy** rather than quietly re-resolving to whatever is current. That is
   the point — a silent re-resolve is how the pilot's wasm-bindgen drifted to
   0.2.126 against a 0.2.108 CLI — but it means a drifted game's first red
   build after an upgrade is this, and the recipe above is the fix.
3. Rebuild the verifier and **regenerate `tests/fixtures/selftest-wasm.gxr`**
   (`bash tools/build_verify.sh` then
   `node tools/verify_fixture.mjs --record tests/fixtures/selftest-wasm.gxr`,
   then verify it) — a refreshed `sim.rs` usually changes the checksum, and
   a wasm-bindgen change changes the module.
4. Gates as in step 7 below, plus `bash build_web.sh` and one autopilot tour.
   Confirm `dist/verify.zip` actually appeared — a game rolled out between
   template commits `a9aa6ed` and the guard fix shipped **without** the
   `bash tools/build_verify.sh` step in its `build_web.sh` and with no HAND
   EDIT about it, so its `verify_url` 404s in production while everything
   local looks fine:
   ```bash
   grep -n 'build_verify.sh' build_web.sh   # must show the `bash ...` line, not only the comment
   ```

The build id in the header of already-submitted replays changes, and that
invalidates nothing: the site picks the verifier module by the replay's own
`build` field, so old replays keep verifying against the release they were
recorded against.

## Changing the rollout script itself

`tools/test_rollout_replay.sh` is the regression suite for
`rollout-replay.sh`, and every part of it exists because something reached a
game (or production) silently. Run it before and after any change:

```bash
bash tools/test_rollout_replay.sh              # locally: parts (a)-(h)
bash tools/test_rollout_replay.sh --no-game    # what CI runs: skips (b)
```

`--no-game` skips part (b), the only part that needs a sibling
`games/cannonball-putt` checkout at its pre-rollout commit — the template's
own workflow has no token for that private repo. Part (b) is also the only
end-to-end "an unported game compiles after the rollout" coverage there is,
so run the suite **without** the flag before pushing a script change.

## When it fails

| Banner | Cause | Check |
|---|---|---|
| `No verifier for this game build yet` | The site's cached `BUILD` id doesn't match the replay's `build` field | Step 6/7: re-run `unzip -p dist/verify.zip BUILD` against the deployed `verify.zip`; confirm `build_verify.sh` and the game binary were built from the same commit (a dirty tree or a stale Vercel cache breaks this) |
| `Couldn't reproduce this run` | Nondeterminism in the sim | Re-run step 5; walk the port checklist's rule-by-rule "usual suspects" (a system still in `Update`, `rand::rng()`, wall-clock reads, a `HashMap`, a transcendental-function ulp drift between native and wasm) |
| `This run's seed was already used — start a new run` | The player restarted faster than the host could issue a fresh seed, or replayed an already-submitted run | Start a new run from the title screen; not a bug to fix in the game |
| `Verifier unavailable, try again later` | The site failed to load or run the verifier module | Check the site's logs for the `verifier_error` detail (see `docs/replay-verification.md` §Server contract, "Versioning") |
