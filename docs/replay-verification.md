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
6. **No `Instant`/`SystemTime`/frame count in sim logic.** Wall-clock and
   frame-count reads aren't reproducible by a headless re-simulation driven
   by `TimeUpdateStrategy::ManualDuration`.
7. **Never read `pause_just_pressed` in a sim system.** The recorder masks
   `Buttons::PAUSE` out of every recorded tick (a replayed pause would
   freeze the sim it is meant to reproduce), so it is the one bit a replay
   cannot carry. Pause belongs in `toggle_pause`, which runs before
   `SimSet`.
8. **`LeaderboardScore`, not `GameData.score`, for the replay/host score.**
   games without `score` implement `LeaderboardScore`; the HUD, host `score`
   event and replay must agree.

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
usage, checksum folding, wall-clock reads — aren't mechanically checkable and
need review.

## Commands

```bash
cargo test --all-features                                   # codec, selftest, fixture
cargo run --features verify --bin verify -- --selftest --write tests/fixtures/selftest.gxr   # regenerate after a sim change
GX_REPLAY_DIR=build/replays cargo run                        # play; each run writes build/replays/<ms>.gxr
cargo run --features verify --bin verify -- build/replays/<ms>.gxr
tools/build_verify.sh && node tools/verify_fixture.mjs tests/fixtures/selftest.gxr
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

## Caveat

Native and wasm agree for the template because its sim uses only basic IEEE
ops (add/sub/mul, no transcendental functions). A game whose sim code calls
`sin`/`cos`/`powf` may see the native `--selftest` and the Node fixture check
disagree by an ulp that snowballs over enough ticks — different platforms'
libm implementations aren't required to round transcendental functions
identically. This doesn't affect what's actually shipped: the real check is
wasm-in-browser vs. wasm-in-Node, and wasm arithmetic is spec-deterministic
for both. If a game hits this, mark the CI Node step
(`node tools/verify_fixture.mjs …`) with `continue-on-error: true` and rely
on `cargo test`'s native `--selftest` for regression coverage instead.
