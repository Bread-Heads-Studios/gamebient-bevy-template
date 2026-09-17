# Replay verification

## What it is

Every run records the seed it started from and the exact input of every sim
tick into a compact `GXR1` replay, and posts it to the host as
`{"type":"gx:event","v":1,"event":"run","replay":"<base64>"}`
(`HostEvent::Run`) when the run ends — game over or quit to title. It is
posted on leaving `Playing`, before that transition's `gameover` event. The
ColecoVision GX site re-simulates that replay
headlessly, in a build of **this same crate** compiled for
`wasm32-unknown-unknown` with `MinimalPlugins` (the `verify` module), run
under Node. Only a run whose final score and checksum the re-simulation
reproduces becomes a leaderboard entry.

A mismatch is never treated as cheating. Float behaviour, an untested code
path, or a stale verifier module can all make a legitimate run fail to
reproduce, so the server marks a mismatch `unverified`, not `cheat` — see
"Server contract" below.

## Format

`GXR1`, little-endian (`src/game/replay/mod.rs`):

```text
magic    b"GXR1"
build    u8 len + UTF-8            GX_BUILD_ID of the game that recorded it
tick_hz  u16
seed     [u8; 32]
origin   u8                        0 host, 1 local
ticks    u32                       total sim ticks
score    u64                       claimed
checksum u64                       claimed
runs     u32                       number of RLE runs
run[]    held u16, latched u16, ax i8, ay i8, count u16
```

`build` is `GX_BUILD_ID` (crate version + short git SHA, baked in by
`build.rs`): the server uses it to pick which release's verifier module to
load. Consecutive ticks with identical `(held, latched, ax, ay)` collapse
into one RLE run, so a multi-minute run is a few KB.

## Determinism rules for game code

The migration rule for turning gameplay into something a replay can
reproduce ([spec](superpowers/specs/2026-09-15-replay-verification-design.md)):

1. **`SimSet` only.** Anything that mutates run state runs inside
   `sim::SimSet` (`FixedUpdate`, 60 Hz, chained) — never in `Update`. Bevy's
   `FixedUpdate` runs zero or more times per frame; `Update` code can't be
   replayed tick-for-tick.
   A system left in `Update` must also not **insert or remove components on,
   or spawn or despawn, any entity a `SimSet` system queries.** Inserting a
   presentation component onto a sim entity moves it to a different
   archetype, and archetype order is the order a `Query` iterates in — so the
   windowed game iterates one order and the verifier, which adds no render
   plugins and therefore never performs the insert, iterates another. The
   fixtures never catch it (nothing renders during a selftest); only replays
   of real, rendered runs diverge. Grand Theft Auto-Reply hit exactly this:
   `src/assets/` decorated live `Email` and `Projectile` entities that
   `inbox::tick_emails` and `combat::advance_projectiles` iterate. Put the
   presentation on a child entity, or move the insert into the chain.
   The "delete the system and re-run `--selftest`" check cannot detect this
   — a headless run never decorates, so deleting the decorator changes
   nothing. `tests/archetype_order.rs` is the test that can: it records the
   scripted run with and without the decoration and asserts both that the
   checksums agree and that the decorated recording still `verify()`s in a
   bare app, across several seeds and tick-per-frame cadences. It runs two
   decoration modes, and a port must keep both: `faithful` mirrors the
   game's real decorators (it documents the real partitioning), while
   `split` additionally halves every sim archetype on a key the sim cannot
   see (it is the detector). Measured on Dough.io against a real,
   score-changing instance of this bug: the faithful mode passed 18/18 by
   luck and the split mode failed 2/18.

   *Sorting discipline*, the same hazard from the other end: any sim system
   that iterates a `Query` and accumulates **order-sensitively** — a running
   multiplier, pushing into a `Vec` the sim later reads in order, a
   sequential float threshold, "the first entity within range wins" — must
   sort by a stable per-entity key first. **`sim::SpawnOrder` is that key**,
   and it is the recommended one: a monotonic index the sim stamps on an
   entity as it spawns it, from `sim::SpawnCounter`, which `sim::begin_run`
   rewinds to 0 at the start of every run so a replay sorts by the same
   numbers the recorded run did. It is in `src/game/sim.rs`, so
   `tools/rollout-replay.sh` copies it into every game — there is nothing to
   write. A game-owned equivalent (a slot index, a grid cell, a bake
   counter) is fine where one already exists; what is *not* fine is
   `Entity`, whose value depends on allocation order, which is exactly what
   is in question. Query it non-optionally: an entity spawned without a key
   then stops matching and a playtest says so, where a `Default` of 0 would
   silently hand every un-stamped entity the same key and the tie back to
   archetype order. Query iteration order is an implementation detail of the
   archetype layout even where nothing inserts. The test is not "is the
   operator commutative" — float `+` and `*` are, and still move their last
   bit when reordered, which the checksum folds — but "would reordering the
   operands change the value". Integer sums, maxes, counts and bitwise ORs do
   not care; a `checksum_<game>` system's per-entity folds always do.
2. **`TickInput`, not `GameInput`, in sim systems.** `GameInput` is a
   per-frame resource for menus/UI; the sim reads the once-per-tick
   `TickInput` the replay actually records.
3. **`GameRng` only.** All randomness in sim code draws from the
   `GameRng` resource (`Xoshiro256PlusPlus`, reseeded from the run seed on
   `OnEnter(Playing)`) — nothing else.
4. **No `std::collections::HashMap` in sim state.** Its iteration order
   isn't deterministic across builds; use `bevy::platform::collections::HashMap`
   or `BTreeMap` instead.
5. **Fold extra state into `Checksum`** via `Checksum::fold(&mut self, u64)`
   whenever a score could be reached through different in-game states — the
   checksum is what catches a replay that reproduces the score by accident.
6. **`sim::checksum_tick` folds only the score and the tick.** It is one of
   the files `tools/rollout-replay.sh` copies verbatim into every game, so
   it must stay game-agnostic. Each game folds its own key run state
   (player/ball position, hole index, lives, ...) in a system of its own,
   registered in `SimSet` right after `sim::checksum_tick` — this
   template's `player::checksum_player` is the example (folds `lives`, then
   the player transform bit-exactly).
7. **No `Instant`/`SystemTime`/frame count in sim logic.** Wall-clock and
   frame-count reads aren't reproducible by a headless re-simulation driven
   by `TimeUpdateStrategy::ManualDuration`.
8. **Never read `pause_just_pressed` in a sim system.** The recorder masks
   `Buttons::PAUSE` out of every recorded tick (a replayed pause would
   freeze the sim it is meant to reproduce), so it is the one bit a replay
   cannot carry. Pause belongs in `toggle_pause`, which runs before
   `SimSet`.
9. **`LeaderboardScore`, not `GameData.score`, for the replay/host score.**
   games without `score` implement `LeaderboardScore`; the HUD, host `score`
   event and replay must agree.
10. **Freeze the sim on the tick the run ends.** The sim system that ends
   the run sets `RunOver(true)` on the same tick it requests the fade — or,
   headless, sets `NextState(GameOver)` — and `sim::run_not_over` is the
   third clause of `SimSet`'s run condition:
   `in_state(Playing).and(not_paused).and(sim::run_not_over)`.
   `sim::end_run(&mut over, fade, &mut next)` does both halves in one place;
   call it with the fade as `Option<ResMut<ScreenFade>>`, the one signature
   that compiles in the windowed game and in the headless verifier alike.
   Without the latch the two paths leave `Playing` at different ticks: the
   windowed game keeps ticking for the length of the fade, which
   `ScreenFade::tick` drives from `Update` on the *frame* delta
   (`DEFAULT_FADE_SECS` = 0.4 s ≈ 24 ticks, but however many the frame rate
   produces), while the verifier leaves on the ending tick itself. Those
   extra ticks are recorded and folded into `Checksum`, so a real run that
   reaches the game-over screen seals a checksum its own replay can never
   reproduce — and a selftest that ends by input exhaustion never notices.

Paused ticks are skipped by `SimSet` and so are never recorded, but
`collect_tick_input` still runs on them and would leave `TickFrame.prev`
holding input the replay never saw — the tick that resumes would then
derive different press and release edges live than on replay. `SimPrev`
(`src/game/sim.rs`) closes that: `remember_sim_prev` stores the held set of
every tick that reaches `SimSet`, and `restore_tick_frame_while_paused`
rewinds `TickFrame` to it on every skipped tick, so the resuming tick
derives its edges against the last *recorded* tick, exactly as the replay's
feeder does. Host pauses (`gx:set`) are covered by the same rule.

The greppable ones (`rand::rng()`, `from_os_rng`, `thread_rng`, `SmallRng`,
`std::collections::HashMap`) are enforced by the forbidden-names test in
`src/game/sim.rs` (`cargo test`); the rest — `SimSet` placement, `TickInput`
usage, checksum folding, wall-clock reads, run-end latching — aren't
mechanically checkable and need review.

## Keeping a ported game current

Games are copies of this template, not dependents, so a game ported in an
earlier wave keeps the version of `sim.rs`, `replay/`, `verify.rs`,
`build_verify.sh` and this document that it was given — including whatever
the template has fixed since (rule 10's `RunOver` latch, the
`NextState`-before-`done` ordering in `run_verify_app`, the wasm-recorded
fixture). `tools/rollout-replay.sh --upgrade <game-dir>` pulls it forward:
each copied file that is byte-identical to *any* committed template version
is a stale verbatim copy and is overwritten with the current one, while a
file that matches no template version was edited in the game and is left
untouched with a `HAND EDIT:` naming the template commit to diff against.
`src/game/replay/selftest.rs` is narrower: it is refreshed while it is
still byte-identical to a committed template version (nobody replaced the
skeleton), and once it is the game's own script it is left alone with no
`HAND EDIT:`, since there is nothing to act on. Refreshing `sim.rs` generally changes the checksum, so
regenerate both fixtures afterwards; the resulting build id change
invalidates nothing, since the site selects a verifier module by each
replay's own `build` field.

**Keep game-specific notes out of this file.** It is copied verbatim into
every game, and `--upgrade` refreshes it only while the game's copy is still
byte-identical to a committed template version. A per-game addition — a
native-vs-wasm drift measurement, a "this game ships only the wasm fixture"
caveat — permanently marks it locally modified, so the game stops receiving
fleet-wide contract changes and gets a HAND EDIT about it on every upgrade
instead. Those belong in a game-local `docs/replay-notes.md`.

## Commands

```bash
cargo test --all-features                                   # codec, selftest, fixtures
cargo run --features verify --bin verify -- --selftest --write tests/fixtures/selftest.gxr   # regenerate after a sim change
bash tools/build_verify.sh                                   # -> dist-verify/ (the wasm module)
node tools/verify_fixture.mjs --record tests/fixtures/selftest-wasm.gxr   # regenerate the CI gate's fixture
node tools/verify_fixture.mjs tests/fixtures/selftest-wasm.gxr
GX_REPLAY_DIR=build/replays cargo run                        # play; each run writes build/replays/<ms>.gxr
cargo run --features verify --bin verify -- build/replays/<ms>.gxr
node tools/verify_fixture.mjs build/replays/<ms>.gxr
```

## Server contract

What the site must do; defined here so the template and the site cannot
drift.

1. **Seed issuance.** Before a run, the host posts `gx:set {"seed": hex}`
   where `seed` is 32 random bytes the server generated and stored with
   `(user, game, expires_at)`. Runs with `origin = local` are accepted for
   personal bests only.
2. **Submission.** On `run`, the host forwards the base64 replay, with the
   user's session, to the submit endpoint.
3. **Verification.** The endpoint decodes the header, looks up the seed
   (must exist, be unexpired, unused, and belong to the user and game),
   loads the verifier module for `build` (from the release asset, cached
   at module scope), calls `verify`, and stores
   `status: verified | unverified | rejected` with the verdict. `rejected`
   is for structural failures only (bad seed, bad build, decode error);
   a score mismatch is `unverified`.
4. **Versioning.** The module for a `build` is immutable. A game release
   publishes `gamebient-game-verify.zip` as a release asset. The replay's
   `build` field is `<version>+<short sha>`, so the site maps that sha to
   the release whose asset it must load; replays from a build the site has
   no module for are `unverified`. Games publish their verifier at
   `properties.verify_url` (the web deploy's `/verify.zip`); the site
   fetches it on first sight of a build and caches it per build. The zip's
   `BUILD` file carries `GX_BUILD_ID=<version>+<sha>`, matching the replay
   header's `build` field exactly (`tools/build_verify.sh` computes it the
   same way `build.rs` bakes it into the binary); the bundle resolver reads
   this to confirm it loaded the right module before trusting a verdict. The
   sha comes from `git rev-parse --short=7 HEAD`, falling back to
   `VERCEL_GIT_COMMIT_SHA` when there's no `.git` checkout (a Vercel build);
   a build with neither fails on purpose rather than shipping a non-unique
   id, since the site caches a verifier per build id and a reused id makes
   every honest run after the next redeploy mismatch.
5. **Limits.** `decode` rejects a header claiming a tick rate other than
   60 Hz (`BadTickRate`) or more than `MAX_TICKS` = 216 000 ticks — one hour
   of play (`TooManyTicks`) — before it reads a single run, so a few crafted
   header bytes cannot buy unbounded verifier CPU. Both are `rejected`, not
   `unverified`.
6. **Runtime.** The verifier module is wasm-bindgen's `nodejs` target and
   `require()`s an ESM snippet: Node 20.19+ or 22.12+ (CI pins Node 22).

Module API, called once per verification:

```js
const { verify } = require("verify.js");
const json = verify(new Uint8Array(bytes));
```

`json` parses to
`{score, checksum, ticks, ended: "gameover"|"input_exhausted"|"cap", matches}`
or `{"error": "..."}` if the bytes don't decode as `GXR1`. `score` and
`ticks` are numbers; **`checksum` is a decimal string**
(`"checksum":"15390901594743611022"`) because it is a u64 and JSON numbers
are doubles in JavaScript — parsing it as a number would round it. Compare
it verbatim, or as a `BigInt`. Cache the required module at module scope —
loading it is the expensive part, `verify()` itself is sub-second. The
replay header's `build` field is what selects which release's
`gamebient-game-verify.zip` to load.

`ended` says why the re-simulation stopped, and is diagnostic — `matches` is
the verdict. A run the sim ended itself (rule 10: `sim::end_run` latches
`RunOver` and queues `NextState(GameOver)`) reports **`gameover`**, which is
what every real run that reaches the game-over screen produces. That takes a
deliberate ordering in `run_verify_app`: such a run's replay carries exactly
the ticks it simulated, so the feeder runs dry on the very tick the
transition is queued, and a queued `NextState` is therefore checked *before*
`ReplayFeeder::done`. `input_exhausted` means the input ran out with the run
still live — a quit-to-title, or a selftest replay whose script simply
stopped. `cap` means the sim outran `ticks + 60` without ending, which is a
determinism failure dressed as a timeout.

## Two fixtures, and which one is the gate

`tests/fixtures/` holds two recordings of the same scripted selftest run:

| File | Recorded by | Verified by | Role |
|---|---|---|---|
| `selftest-wasm.gxr` | the wasm verifier module | wasm, under Node (CI) | **the gate** |
| `selftest.gxr` | the native build | native (`cargo test`), and informationally by Node | native regression check |

Both files' header `build` lags `HEAD` by construction: it is the
`GX_BUILD_ID` of the commit that recorded them, and any commit after that
leaves it behind. Nothing in verification reads it — `verify()` re-simulates
from the seed and inputs alone, and `build` only matters to the *site*, which
uses it to pick which release's verifier module to load. A fixture whose
`build` disagrees with `HEAD` is normal, not stale.

`selftest-wasm.gxr` is the one CI blocks on, because both sides of that
comparison are wasm arithmetic — which is exactly what ships: the browser
records a run in a wasm build of this crate and Node re-simulates it in
another wasm build of the same crate, and wasm arithmetic is
spec-deterministic. It is produced by the module's own `selftest_record()`
export; native code cannot write it, which is the point.

`selftest.gxr` is recorded and verified natively by `cargo test`, so it stays
the fast local regression check on the sim. CI *also* runs it through Node,
but with `continue-on-error: true`: that step compares native arithmetic
against wasm, and native libm and wasm are not required to round
transcendental functions (`sin`, `cos`, `powf`) identically. A game whose sim
calls them may see the two disagree by an ulp that snowballs over enough
ticks — identical `score` and `ticks`, a different `checksum`. That is drift,
not nondeterminism, and it does not affect what is shipped. Before accepting
it as drift, prove it: the port checklist's "Native vs Node mismatch" section
has the bisection procedure.

```bash
# regenerate both after a sim change
cargo run --features verify --bin verify -- --selftest --write tests/fixtures/selftest.gxr
bash tools/build_verify.sh
node tools/verify_fixture.mjs --record tests/fixtures/selftest-wasm.gxr
node tools/verify_fixture.mjs tests/fixtures/selftest-wasm.gxr   # must be matches: true
```
