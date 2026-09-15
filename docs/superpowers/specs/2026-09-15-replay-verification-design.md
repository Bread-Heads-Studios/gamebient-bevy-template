# Replay verification — Design

**Date:** 2026-09-15 · **Status:** approved 2026-09-15 (design conversation with the owner)

## Goal

Make a Gamebient game's score checkable by a server it does not trust the
client with. Every run produces a compact **replay** (seed + per-tick inputs).
The ColecoVision GX site re-simulates the replay in a **headless build of the
same game**, compiled to the same `wasm32-unknown-unknown` target the browser
runs, executed under Node in a serverless function, and only a score the
re-simulation reproduces is a leaderboard entry.

This spec covers the template (`gamebient-bevy-template`) and the input crate
(`gamebient-input`). The site's seed endpoint, submit endpoint, verifier hosting
and leaderboard UI are a separate spec; the contract they implement is defined
here in "Server contract".

## Decisions

| Question | Choice | Why |
|---|---|---|
| Verifier runtime | Same crate, `verify` feature, built for `wasm32-unknown-unknown` with `MinimalPlugins`, run under Node (`wasm-bindgen --target nodejs`) | Same target as the shipped game, so float behaviour is identical by construction (wasm arithmetic is spec-deterministic). Runs inside a Vercel Node function: a few MB module, sub-second per replay. No wasip1, no wasmtime, no native binaries on the server. |
| Sim clock | Gameplay on `FixedUpdate` at 60 Hz | Removes wall-clock from the sim; `Res<Time>` inside `FixedUpdate` is already the fixed delta, so existing `time.delta_secs()` code moves unchanged. Matches the recorder's 1/60 s clock. |
| Input to the sim | New `TickInput` resource, sampled once per fixed tick from an accumulator; `GameInput` stays per-frame for UI | Gameplay must see exactly the input the replay carries. Menus and overlays keep the per-frame resource unchanged, so the input crate change is additive. |
| Randomness | `GameRng(Xoshiro256PlusPlus)` resource seeded from the run seed on `OnEnter(Playing)` | The only RNG the sim may use. Not `SmallRng`: it is xoshiro256 on 64-bit and xoshiro128 on wasm32, so native and wasm would diverge. Seed comes from the host when it supplied one, else from the OS with `origin = Local`. |
| Pause | Paused ticks are not recorded | Sim systems are gated by `not_paused`, so a paused tick changes nothing; skipping it in the log makes pause-from-anywhere (player, host) replay-neutral. |
| What is compared | Final `score`, tick count, and a `Checksum` folded each tick from score, lives, tick and the player transform's f32 bits | Enough to reject a forged score, and input- and float-sensitive so the native-vs-wasm fixture check means something. Games fold more state via `Checksum::fold`. |
| Mismatch semantics | The verifier reports; the server marks the run `unverified`, never `cheat` | A determinism bug must not brand players. |
| Rollout to games | Out of scope | The fleet migration is its own plan once the template is proven. |

Out of scope: input interpolation for 120 Hz displays, state-hash snapshots at
intervals, human-vs-bot heuristics, the website.

## Architecture

```
gamebient-input (v0.3.0)
  PreUpdate       accumulate_input   keyboard/gamepad/virtual -> InputAccumulator
  FixedPreUpdate  collect_tick_input InputAccumulator -> TickInput  (replay overwrites the accumulator first)
  PreUpdate       collect_input      unchanged, still feeds GameInput per frame

template
  src/game/sim.rs        GameRng, RunSeed, Checksum, SimSet (the ordered fixed-tick chain)
  src/game/replay/       format + codec (pure), recorder, feeder
  src/bin/verify.rs      native CLI and wasm export around replay::verify()
  src/game/host.rs       HostCommand::Seed in, HostEvent::Run out
```

### Fixed-tick input (`gamebient-input`)

- `GxConfig::tick_input: bool` (default `false`, so existing games see no
  change). The template sets it `true`.
- `InputAccumulator { held: Buttons, latched: Buttons, axis: Vec2 }`, filled
  in `PreUpdate` after `InputSystems` from the same three sources
  `collect_input` uses. `held` and `axis` are the latest values; `latched`
  ORs every press edge since the last tick.
- `collect_tick_input` in `FixedPreUpdate`: quantizes `axis` to two `i8`
  (`-127..=127`), runs the existing `edges` / `derive` against a tick-scoped
  previous-held, writes `TickInput(GameInput)`, clears `latched`. The
  quantized axis is what the sim sees, so the log is exact.
- `TickInput { input: GameInput, axis: (i8, i8) }` derefs to `GameInput` and
  also carries the quantized stick on its own, which is what the replay
  stores (the D-pad is in `held`). Gameplay systems swap `Res<GameInput>`
  for `Res<TickInput>` and nothing else.
- Any system registered in `FixedPreUpdate` before `collect_tick_input` may
  overwrite the accumulator: that is the replay hook. It is a public set
  `TickInputSet::{Feed, Collect}` so the template does not depend on system
  names.
- Host protocol: `gx:set` gains `"seed": "<64 hex chars>"`, parsed into
  `HostCommand::Seed([u8; 32])`; `gx:event` gains
  `{"event":"run","replay":"<base64>"}` from `HostEvent::Run(Vec<u8>)`.
  Documented in `docs/host-protocol.md` (version stays 1; unknown fields are
  ignored by old hosts).

### Template sim scaffolding (`src/game/sim.rs`)

- `RunSeed { bytes: [u8; 32], origin: SeedOrigin::{Host, Local} }`. A host
  `Seed` command stores `PendingSeed`; `OnEnter(Playing)` takes it (or draws
  a local one) into `RunSeed` and reseeds `GameRng`.
- `GameRng(rand_xoshiro::Xoshiro256PlusPlus)`. The one RNG resource.
  `rand::rng()` / `from_os_rng` are forbidden in `src/game/` except in
  `RunSeed::local()`; a unit test greps for them.
- `Checksum(u64)`: FNV-1a folded each tick from `(score, lives, tick,
  player.x bits, player.y bits)`. Games may fold more via
  `Checksum::fold(&mut self, u64)`.
- `SimSet`: one `SystemSet` in `FixedUpdate`, `.chain()`ed, gated by
  `in_state(Playing).and(not_paused)`. `GamePlugin` registers
  `move_player`, `handle_score_events`, `checksum_tick` in it. The rule for
  games: **everything that mutates run state goes in `SimSet`, in order.**
  Bevy's `FixedUpdate` runs zero or more times per frame; nothing in
  `Update` may write run state.
- `Time::<Fixed>::from_duration(sim::tick_duration())` (16 666 667 ns)
  inserted by `GamePlugin`; the verifier steps with the same duration.
- Pause toggling moves into `FixedUpdate` (before `SimSet`, reading
  `TickInput`), so the "was this tick paused" decision is tick-aligned. The
  overlay sync stays in `Update`.

### Replay module (`src/game/replay/`)

`mod.rs` — format and codec, pure, unit-tested.

```
magic     b"GXR1"
build     u8 len + UTF-8 (crate version + short git sha, from build.rs env)
tick_hz   u16 (60)
seed      [u8; 32]
origin    u8 (0 host, 1 local)
ticks     u32     total sim ticks
score     u64     claimed
checksum  u64     claimed
runs      u32     count of RLE runs
run[]     held u16, latched u16, ax i8, ay i8, count u16
```

Little-endian throughout. `Replay::encode` / `Replay::decode` with explicit
errors (bad magic, truncated, run count mismatch). A 3-minute run is ~10 800
ticks and compresses to a few KB.

`recorder.rs` — `ReplayRecorder` resource, cleared on `OnEnter(Playing)`.
A system at the end of `SimSet` appends the tick's `TickInput` raw
`(held, latched, ax, ay)` (RLE merging equal consecutive ticks). On
`OnExit(Playing)` (game over or quit to title) it seals the replay with `RunSeed`, `GameData.score`,
`Checksum`, tick count, and:

- posts `HostEvent::Run(bytes)`;
- on native, if `GX_REPLAY_DIR` is set, writes `<dir>/<unix-ms>.gxr`.

Always compiled in; it is how the shipped game submits.

`feeder.rs` — `ReplayFeeder` resource holding a decoded replay and a cursor.
A system in `TickInputSet::Feed` writes the next tick's `(held, latched,
axis)` into `InputAccumulator`; past the end it feeds nothing and sets
`ReplayFeeder::done`. Only the verifier inserts it.

`verify()` (in `mod.rs`): builds a headless `App` (`MinimalPlugins`,
`StatesPlugin`, `GxInputPlugin` with `tick_input`, `GamePlugin` minus
anything that needs assets, audio or a window; see "Headless GamePlugin"),
sets `GameState::Playing` directly, installs `RunSeed` from the header and
the `ReplayFeeder`, then calls `app.update()` per tick under
`TimeUpdateStrategy::ManualDuration(1/60 s)` until the state leaves
`Playing`, the feeder is done, or a cap of `ticks + 60` is reached. Returns
`Verdict { score, checksum, ticks, ended: GameOver | InputExhausted | Cap,
matches: bool }`. `matches` is `score == claimed && checksum == claimed`.

### Headless `GamePlugin`

`GamePlugin` gains a `headless: bool`. Headless skips `setup_scene`, the
audio systems, `spawn_player`'s mesh/material (the player is spawned with a
`Transform` only), the pause overlay and the `ScreenFade` dependency. The
split is a `cfg`-free runtime flag so one crate builds both the game and
the verifier; the render/audio features remain in `Cargo.toml` (features
are additive) and LTO drops what the verifier never references.

### `src/bin/verify.rs`

Needs `src/lib.rs` exposing `pub mod game; pub mod assets; pub mod ui;`;
`main.rs` becomes a thin caller. `[[bin]] name = "verify"` with
`required-features = ["verify"]`.

- Native: `verify <replay.gxr>` prints the verdict as JSON, exit 0 if
  `matches`, 1 otherwise, 2 on decode error. `verify --selftest` drives the
  headless app with a scripted bot for 600 ticks (recording), then replays
  the recording and asserts the verdicts agree; this is the determinism
  unit test and it runs in `cargo test`.
- wasm: `#[wasm_bindgen] pub fn verify(replay: &[u8]) -> String` returning
  the same JSON. `tools/build_verify.sh` runs `cargo build --bin verify
  --features verify --profile wasm-release --target wasm32-unknown-unknown`,
  `wasm-bindgen --target nodejs --out-name verify`, `wasm-opt` with the
  existing flags, into `dist-verify/`. `release.yml` uploads
  `verify-<version>.zip` as a release asset beside the cartridge tarball.

Feature `verify = []` on the template gates the bin and adds
`wasm-bindgen = "0.2"` under the wasm32 target (already in `Cargo.lock`
through `gamebient-input`, so the CLI pin is unchanged).

### CI

- `cargo test --all-features` runs the codec tests, the forbidden-RNG grep,
  and `--selftest`.
- The `build-web` job also builds the verifier module and runs
  `node tools/verify_fixture.mjs tests/fixtures/selftest.gxr`, which must
  print `matches: true`. The fixture is produced once by `--selftest
  --write tests/fixtures/selftest.gxr` and committed; it pins native and
  wasm to the same answer. Regenerate it whenever the sim changes (the test
  says so when it fails).

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

## Migration rule for games (recorded here, executed later)

1. Register run-state systems in `SimSet` instead of `Update`.
2. Swap `Res<GameInput>` for `Res<TickInput>` in those systems.
3. Route every random draw through `GameRng`.
4. Replace `std::collections::HashMap` in sim state with
   `bevy::platform::collections::HashMap` or `BTreeMap`.
5. Run `verify --selftest`; then record a real run under
   `GX_REPLAY_DIR` and verify it.

## Testing

- Codec: round-trip, truncation, magic, RLE boundaries.
- Fixed-tick input: accumulator latches a sub-tick press; axis quantization
  is idempotent (quantize(dequantize(x)) == x).
- Determinism: `--selftest` (native, in `cargo test`) and the wasm fixture
  (CI web job).
- Recorder: paused ticks absent; a run seals with the right counts.
- Host: `Seed` command sets `PendingSeed`; `Run` event carries a decodable
  replay.
