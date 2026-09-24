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

   And a `SimSet` system must not **read** anything `UiPlugin` (or
   `AssetsPlugin`, or `AudioPlugin`) owns. `ScreenFade` is the one that has
   bitten: it is absent in the verifier, it is ticked from `Update` on the
   *frame* delta, and it is busy for the first ~24 ticks of every run because
   entering `Playing` goes through it — so a sim system that waits for it to
   be idle lets the player do one thing and the verifier another. Sundae
   Shooter's `fire_scoop`/`swap_queue` did, and a browser-recorded run came
   back claiming 195 points against 200 re-simulated. `Option<Res<T>>` makes
   such a system *start* headless; it does not make the two builds agree, so
   the read has to go. `sim::end_run`'s `Option<ResMut<ScreenFade>>` is the
   one sanctioned touch (rule 10: it writes a fade request on the tick
   `RunOver` has already frozen the set). `tests/windowed_shape.rs` is the
   test for this one: it records the scripted run in an app carrying
   `ScreenFade`, the windowed build's `Update` systems and the game's own
   `AssetsPlugin` and `verify()`s it in a bare one.

   Resources are not the only way that state reaches the sim. The rule is
   **a sim system may not read anything the windowed build writes and the
   verifier does not**, and that includes a *field of a component* on an
   entity the sim owns: Dive Rise's dash fell back on
   `Transform.scale.x.signum()` to decide which way it was facing, and that
   sign is written by an `AssetsPlugin` system mirroring the sprite — `+1`
   for ever in the verifier, so a dash from a standstill while facing left
   went opposite ways in the two builds. Latch such a thing from
   `TickInput` into a sim-owned component or resource and let presentation
   *paint* it, one-directionally.

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

   *The transcendental rule*, the one exception to "fold floats with
   `to_bits()`": **never fold a value a libm function produced.** `sin`,
   `cos`, `exp`, `powf` and `atan2` are library calls, and macOS, the Linux
   CI runner and wasm are not required to round them identically — so a
   single such value in the fold makes the checksum a referendum on whose
   libm ran the sim rather than on the run. Fold what arithmetic produces
   (`+ - * /` and `sqrt` are pinned exactly by IEEE-754) and leave the libm
   result out; it is almost always a pure function of something already
   folded beside it. Gulper folded `head.facing` (`velocity.y.atan2(...)`)
   and got three checksums for one 5400-tick run from three verifiers —
   same score, same ticks — which turned its own committed native fixture
   red on CI. And **a transcendental whose argument is constant under the
   fixed tick must be a literal**, not a call: `(-K * dt).exp()` with a
   fixed `K` and the fixed 60 Hz `dt` is one number computed sixty times a
   second on a per-platform libm, and its result usually lands straight in
   a folded velocity. Pin each literal with a unit test against the tick
   length and a tolerance (`1e-6`), never a bit-exact assertion — a
   bit-exact one would pin *this* machine's libm, which is the dependency
   the literal exists to remove; what the test catches is a stale literal
   after a tick-rate or constant change. Transcendentals with genuinely
   varying arguments stay, and they are the residual caveat in "Two
   fixtures, and which one is the gate" below.
6. **`sim::checksum_tick` folds only the score and the tick.** It is one of
   the files `tools/rollout-replay.sh` copies verbatim into every game, so
   it must stay game-agnostic. Each game folds its own key run state
   (player/ball position, hole index, lives, ...) in a system of its own,
   registered in `SimSet` right after `sim::checksum_tick` — this
   template's `player::checksum_player` is the example (folds `lives`, then
   the player transform bit-exactly).
7. **A sim system may read only what the replay carries**: `TickInput`,
   `RunSeed`/`GameRng`, its own run state, and the tick-derived clock
   (`sim::RunClock`). Never `Time::elapsed*`, never a wall clock
   (`Instant::now`, `SystemTime`, `Date.now`), never an app-lifetime
   counter (frame count, a resource that only ever increments in `Update`).
   A replay carries a seed and a stream of ticks; anything else a sim system
   reads is a number the verifier has to guess, and it guesses whatever a
   fresh app happens to hold.

   **`Time::elapsed_secs()` is the one that has shipped.** Inside
   `FixedUpdate`, `Res<Time>` is `Time<Fixed>`, and `Time<Fixed>::elapsed()`
   counts from **app** start — nothing resets it when a run begins. So a
   system that drives a drift, a weave, an orbit or a lunge phase off it
   gives the player a different world depending on how long they watched the
   studio logo and the menu, while `replay::run_verify_app` enters `Playing`
   on its second `app.update()` under a zero-delta strategy and so always
   re-simulates at offset **zero**. `Time::delta_secs()` stays fine and is
   the reason the whole `Res<Time>` parameter is still allowed: inside
   `FixedUpdate` it is always the fixed timestep. Use `sim::RunClock`
   (`secs`, `ticks`) for anything phase-like; `sim::sync_run_clock` restates
   it from `SimTick` at the head of `SimSet`, so it is a pure function of
   the tick index and cannot drift.

   The worked example is Dive Rise, 2026-09-23. A real run submitted from
   colecovisiongx.com came back `UNVERIFIED / mismatch`: 11 877 ticks
   claiming score 144, re-simulating to 56 in the deployed `verify.zip`, in
   a locally rebuilt wasm one and natively — three verifiers agreeing with
   each other and disagreeing with the browser, which is what says the
   *recording* side is the outlier. Nine sim systems read
   `Time::elapsed_secs()`: `food::move_food`,
   `hunters::comb_jelly::jelly_drift`, `hunters::amphipod::amphipod_motion`,
   `hunters::cutlassfish::cutlass_motion`, `hunters::viperfish::viper_attack`,
   `companions::krill_swarm::update`, `companions::jellyfish::update`,
   `behaviors::schooling::orbit_mates` and `events::squid_pack::update`.
   **One extra tick of menu time is enough**: re-simulating that replay with
   the pre-run `Time<Fixed>` advanced by 0/1/2/3 ticks gives scores
   56/38/36/40 and four different checksums (60 → 50, 600 → 54, 1800 → 43).

   Every probe the fleet had was blind to it, and all for the same reason:
   `selftest::record_scripted_run`, both committed `.gxr` fixtures,
   `tests/windowed_shape.rs`'s `record_windowed` and the `--playtest`
   harness all enter `Playing` in the app's first frames, exactly like the
   verifier, so their offset agreed with it by accident. The row that is not
   blind is
   `tests/windowed_shape.rs::a_run_does_not_depend_on_how_long_the_app_was_up_before_it`,
   which dwells 1 319 fixed ticks in `Menu` first and requires the recorded
   run to equal the cold one and to verify bare.

   **And the probe has to be long and busy.** A short, quiet run does not
   reproduce this class of bug: a 1 720-tick browser run of the *broken*
   Dive Rise build that ate nothing and took no damage verified `matches:
   true`, and re-simulating it at eight different clock offsets gave the
   same checksum every time — nothing the clock drove had reached anything
   the checksum folds. A green after-menu row on an idle bot means nothing;
   point the dwell at a run that scores, spawns and collides.

   **And run state may not outlive the run.** The clock above is what the
   app did *before* the run; this is what the app kept *from the last one*,
   and it is the same rule: the verifier is always a **fresh app that plays
   exactly one run**, and the browser is not. So a run's state lives in a
   resource or a component that `OnEnter(Playing)` resets — never in a
   `Local<_>`, never in a `static` or a `thread_local!`, never in a cache a
   plugin computed once at build time, and never in a resource nothing
   rewinds. A `Local<T>` belongs to the *system instance*: it lives as long
   as the `App`, and no run start can reach it — not `OnEnter(Playing)`,
   not `begin_run`, not a `reset_run` however careful.

   The worked example is Grand Theft Auto-Reply, 2026-09-23. Its
   `selection::handle_input` — a `SimSet` system — kept the cursor's and the
   crime wheel's held-direction auto-repeat in two `Local<Repeat>`s. A
   second run played in one browser session therefore began holding
   whatever direction the player was holding on run 1's last tick, with
   that direction's repeat timer already past `REPEAT_DELAY`, and swallowed
   the cursor step a fresh app emits on its first tick. It is now a
   resource `reset_run` clears. Nothing in the fleet could see it: every
   probe in every repo recorded exactly **one run per `App`**, so they all
   agreed with the verifier by accident, exactly as they all agreed with it
   about the clock.
   `tests/windowed_shape.rs::a_second_run_in_the_same_app_reproduces_the_first`
   is the row that is not blind — it plays run 1, leaves through the real
   `GameOver` → `Menu` path, dwells, then plays run 2 with the same seed
   and script and requires it to equal a cold recording and to verify bare
   — and `sim::tests::no_local_state_in_sim_systems` is the greppable half.

   **And a choice made before the run is not carried either.** The clock
   above is what the app did *before* the run and the `Local` is what it
   kept *from the last one*; this is what the **player** decided before tick
   1. A replay carries a seed and a stream of ticks and no title screen, so
   anything `OnEnter(Playing)` reads that the player chose or earned outside
   the run — a song, a difficulty, a starting kit, a loadout, a saved
   profile, a `localStorage` blob — is a number the verifier has to guess,
   and a fresh app guesses the `Default`. Beat Bender's title screen wrote
   `SelectedSong`/`SelectedDifficulty` and the verifier therefore
   re-simulated every run on song 0 at NORMAL: 4 705 recorded ticks against
   4 477 re-simulated, which is a different song's length. Dive Rise's
   `MetaSave` decided the draft pool and the starting kit off a save file,
   so no replay of a real run could ever have verified. **Anything that
   configures a run must be a build constant or an in-run phase inside
   `Playing`** — offered from `GameRng`, chosen through `TickInput`, with
   its ticks recorded like any other — and a persisted profile may hold
   stats but nothing a `SimSet` system reads. No probe in the kit can see
   this class: every fixture and every probe starts from a fresh `App` and
   so picks the same default the verifier does. Only a real recording made
   with a **non-default** choice can, which is why the rollout plays one.
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
`src/game/sim.rs` (`cargo test`), and rule 7's clock family
(`.elapsed()`, `.elapsed_secs()`, `.elapsed_secs_f64()`) by
`sim::tests::no_app_lifetime_clock_in_sim_code` beside it — `allow-app-clock`
on the line is the opt-out for a genuine non-sim read, and
`tools/rollout-replay.sh` prints an advisory for the wider family
(`elapsed_wrapping`, `Instant::now`, `SystemTime`, `Date.now`) that the Rust
test leaves to review. Rule 7's other half, run state that outlives the run,
is enforced by `sim::tests::no_local_state_in_sim_systems`, which flags
`Local<` in any file whose systems `SimSet` runs (the files that register a
chain `.in_set(..SimSet)`, plus the modules those registrations name);
`allow-local: <reason>` on the line is its opt-out, for a `Local` in an
`Update` system that happens to share the file. `tools/rollout-replay.sh`
advises on the same thing during a port. Both scans are greps, and both have
a behavioural row in `tests/windowed_shape.rs` that can see what a grep
cannot — a clock reached through a helper, run state carried in a resource
nothing resets. The rest — `SimSet` placement, `TickInput` usage,
checksum folding, run-end latching — aren't mechanically checkable and need
review.

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
   publishes `<package>-verify.zip` as a release asset — the release asset
   named after the crate, so it is `<name>-verify.zip` for whatever
   `Cargo.toml`'s `name` is in that game (`.github/workflows/release.yml`'s
   "Package replay verifier" step is the definition; this document is copied
   verbatim into every game and so cannot name one). The replay's
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
`<package>-verify.zip` to load.

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

Note what that leaves guaranteed and what it does not. **Only a
wasm-recorded run is guaranteed to verify**, and that is the shipping path:
the browser records in a wasm build of the game's crate and the site
re-simulates in another, and wasm arithmetic is spec-deterministic. A run
recorded **natively** — a cabinet run, or a local `GX_REPLAY_DIR` recording
— reproduces natively on the same machine, but the *site* re-simulates it
in wasm, so it crosses the libm boundary and a long enough run may fail to
reproduce through no fault of the sim. Rule 5's
transcendental rule removes the systematic half of this (a folded libm
result, a per-tick `exp`); what remains is a varying-argument
transcendental laundered into a folded number by some in-game event, and it
is a matter of whether a particular run happens to hit one. Measured: Dive
Rise's two same-length rendered runs landed on opposite sides of it, one
reproducing bit-for-bit under both runtimes and the other parting at tick
44 083 — about **twelve sim-minutes** — three ticks after a hunter bite
folded a `sin`/`cos`-driven hunter position into the player's knockback;
Sundae Shooter's parts at tick **1056**. So: do not read a native-vs-Node
mismatch on a long native recording as a port defect without the bisect,
and know that **verified leaderboards accepting cabinet submissions is an
open question**, not a solved one.

```bash
# regenerate both after a sim change
cargo run --features verify --bin verify -- --selftest --write tests/fixtures/selftest.gxr
bash tools/build_verify.sh
node tools/verify_fixture.mjs --record tests/fixtures/selftest-wasm.gxr
node tools/verify_fixture.mjs tests/fixtures/selftest-wasm.gxr   # must be matches: true
```
