# Replay verification

## What it is

Every run records the seed it started from and the exact input of every sim
tick into a compact `GXR1` replay, and posts it to the host as
`{"event":"run","replay":"<base64>"}` (`HostEvent::Run`) when the run ends —
game over or quit to title. The ColecoVision GX site re-simulates that replay
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
   publishes `verify-<version>.zip`; replays from a build the site has no
   module for are `unverified`.

Module API, called once per verification:

```js
const { verify } = require("verify.js");
const json = verify(new Uint8Array(bytes));
```

`json` parses to `{score, checksum, ticks, ended: "gameover"|"input_exhausted"|"cap", matches}`
or `{"error": "..."}` if the bytes don't decode as `GXR1`. Cache the required
module at module scope — loading it is the expensive part, `verify()` itself
is sub-second. The replay header's `build` field is what selects which
release's `gamebient-game-verify.zip` to load.

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
