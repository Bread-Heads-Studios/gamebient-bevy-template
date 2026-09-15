# Replay Verification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every run of a template-derived game produces a compact replay (seed + per-tick inputs) that a headless build of the same game, compiled to `wasm32-unknown-unknown` and run under Node, re-simulates to confirm the claimed score.

**Architecture:** `gamebient-input` v0.3.0 adds a fixed-tick input path (`InputAccumulator` → `TickInput` in `FixedPreUpdate`), a headless plugin mode, and two protocol additions (`gx:set.seed` in, `gx:event run` out). The template moves run-state systems into an ordered `SimSet` in `FixedUpdate`, seeds one `GameRng` from a `RunSeed`, records each sim tick's `TickInput` into a `GXR1` replay, and gains a `verify` bin that replays a file through `GamePlugin { headless: true }` on `MinimalPlugins`, natively (CLI, `--selftest`) and as a wasm-bindgen `nodejs` module.

**Tech Stack:** Rust 2024, Bevy 0.18 (`FixedUpdate`, `TimeUpdateStrategy::ManualDuration`, `bevy_state::app::StatesPlugin`), `rand` 0.9 + `rand_xoshiro` 0.7, `wasm-bindgen` 0.2.108, Node 20+, bash.

Spec: `docs/superpowers/specs/2026-09-15-replay-verification-design.md`.

## Global Constraints

- Bevy `0.18`, `default-features = false`. New crate deps allowed in the template: `rand_xoshiro = "0.7"`, `wasm-bindgen = "0.2.108"` (wasm32 only, `verify` feature). No JSON/base64 crates; hand-render as the record module does.
- `SmallRng` is forbidden for the sim: it is xoshiro256 on 64-bit targets and xoshiro128 on wasm32, so native and wasm would draw different sequences. Use `rand_xoshiro::Xoshiro256PlusPlus` everywhere.
- Tick rate is 60 Hz: `sim::TICK_HZ = 60`, `sim::tick_duration() = Duration::from_nanos(16_666_667)`. The recorder's manual clock and the verifier step with `tick_duration()`.
- Replay format is `GXR1`, little-endian, exactly as laid out in Task 5.
- Everything that mutates run state runs in `SimSet` (`FixedUpdate`, chained, gated by `in_state(Playing).and(not_paused)`). Nothing in `Update` writes run state.
- `rand::rng()`, `from_os_rng`, `thread_rng`, `SmallRng` and `std::collections::HashMap` are forbidden under `src/game/` except in `sim::RunSeed::local`; a unit test greps for them.
- The `verify` feature gates only `src/bin/verify.rs` and the wasm-bindgen dep. The recorder, `sim.rs`, feeder and codec are always compiled (the shipped game submits replays).
- CI already runs `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-targets --all-features`, `cargo fmt --check`. Keep everything warning-free under `--all-features`.
- `gamebient-input` is a public repo; bump to `0.3.0`, tag `v0.3.0`, and pin the template to that tag. Its wasm-bindgen pin stays `0.2.108`.
- Commits in each repo end with `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`. Do not push; the owner pushes and tags.
- Template repo: `libs/gamebient-bevy-template`. Input crate: `libs/gamebient-input`. Paths below are relative to the repo named in each task.

---

## File map

`gamebient-input`:

| File | Responsibility |
|---|---|
| `Cargo.toml` | version `0.3.0` |
| `src/input.rs` | `InputAccumulator`, `TickInput`, `TickFrame`, `TickInputSet`, `accumulate_input`, `collect_tick_input`, `quantize_axis`, `dequantize_axis` |
| `src/lib.rs` | `GxInputPlugin::headless`, registers the tick path when `config.tick_input`, skips window/web when `config.headless`; re-exports |
| `src/host.rs` | `GxConfig { tick_input, headless }`, `HostCommand::Seed`, `HostEvent::Run`, `base64_encode`, `parse_seed_hex`, encoding |
| `src/web.rs` | `parse_command` handles `seed:<hex>` |
| `js/gx.js` | `gx:set.seed` → `seed:<hex>` command |
| `docs/host-protocol.md` | the two new messages |

Template:

| File | Responsibility |
|---|---|
| `Cargo.toml` | `[lib]` + `[[bin]] verify`, `verify` feature, `rand_xoshiro`, `wasm-bindgen` (wasm32, `verify`), input crate `v0.3.0` |
| `build.rs` | `GX_BUILD_ID` env (`<version>+<short sha>`) |
| `src/lib.rs` | `pub mod assets; pub mod game; pub mod ui;` |
| `src/main.rs` | thin: builds the windowed app from the lib |
| `src/game/sim.rs` | `TICK_HZ`, `tick_duration`, `SimTick`, `SimSet`, `RunSeed`, `SeedOrigin`, `PendingSeed`, `GameRng`, `Checksum`, `begin_run`, `advance_tick`, `checksum_tick` |
| `src/game/mod.rs` | `GamePlugin { headless }`, `FixedUpdate` chain, pause in `FixedUpdate` |
| `src/game/player.rs` | `move_player` reads `TickInput`; `spawn_player` takes optional render handles |
| `src/game/host.rs` | `Seed` → `PendingSeed`; `Run` posted by the recorder |
| `src/game/replay/mod.rs` | `Replay`, `TickRun`, `encode`, `decode`, `DecodeError`, `verify`, `Verdict`, `build_headless_app` |
| `src/game/replay/recorder.rs` | `ReplayRecorder`, `record_tick`, `seal_run` |
| `src/game/replay/feeder.rs` | `ReplayFeeder`, `feed_tick` |
| `src/bin/verify.rs` | native CLI + `--selftest` + wasm export |
| `tools/build_verify.sh` | wasm verifier bundle → `dist-verify/` |
| `tools/verify_fixture.mjs` | Node runner for CI |
| `tests/fixtures/selftest.gxr` | committed fixture |
| `.github/workflows/ci.yml`, `release.yml`, `.gitignore` | build + publish the verifier |
| `docs/replay-verification.md`, `AGENTS.md`, `README.md`, `docs/conventions.md` | contract + conventions |

---

### Task 1: Fixed-tick input path in `gamebient-input`

Repo: `libs/gamebient-input`.

**Files:**
- Modify: `src/input.rs` (append after `collect_input`)
- Modify: `src/host.rs:44-63` (`GxConfig`)
- Modify: `src/lib.rs:53-118`

**Interfaces:**
- Produces: `pub struct InputAccumulator { pub held: Buttons, pub latched: Buttons, pub axis: Vec2 }` (Resource); `pub struct TickInput { pub input: GameInput, pub axis: (i8, i8) }` (Resource, `Deref<Target = GameInput>`; `axis` is the quantized analog stick *before* the D-pad is added, which is what a replay stores); `pub enum TickInputSet { Feed, Collect }` (SystemSet in `FixedPreUpdate`); `pub fn quantize_axis(v: Vec2) -> (i8, i8)`; `pub fn dequantize_axis(x: i8, y: i8) -> Vec2`; `GxConfig::tick_input: bool`.

- [ ] **Step 1: Write the failing tests** (append to the `tests` module at the bottom of `src/input.rs`)

```rust
    #[test]
    fn axis_quantization_is_idempotent_and_clamped() {
        for &(x, y) in &[(0.0, 0.0), (1.0, -1.0), (0.5, 0.25), (1.7, -3.0)] {
            let (qx, qy) = quantize_axis(Vec2::new(x, y));
            let back = dequantize_axis(qx, qy);
            assert_eq!(quantize_axis(back), (qx, qy));
            assert!(back.x.abs() <= 1.0 && back.y.abs() <= 1.0);
        }
        assert_eq!(quantize_axis(Vec2::new(1.0, -1.0)), (127, -127));
    }

    #[test]
    fn tick_input_latches_a_sub_tick_press_and_consumes_it() {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<VirtualInput>()
            .init_resource::<InputAccumulator>()
            .init_resource::<TickFrame>()
            .init_resource::<TickInput>()
            .add_systems(PreUpdate, accumulate_input)
            .add_systems(FixedPreUpdate, collect_tick_input.in_set(TickInputSet::Collect));

        // A tap arrives between ticks: latched but never held.
        app.world_mut().resource_mut::<VirtualInput>().latched = Buttons::A;
        app.world_mut().run_schedule(PreUpdate);
        let acc = *app.world().resource::<InputAccumulator>();
        assert_eq!(acc.latched, Buttons::A);

        app.world_mut().run_schedule(FixedPreUpdate);
        let t = app.world().resource::<TickInput>();
        assert!(t.primary_just_pressed);
        assert!(!t.primary_held);
        assert_eq!(app.world().resource::<InputAccumulator>().latched, Buttons::NONE);

        // The next tick with nothing new sees no edge.
        app.world_mut().run_schedule(FixedPreUpdate);
        assert!(!app.world().resource::<TickInput>().primary_just_pressed);
    }

    #[test]
    fn tick_input_axis_is_the_quantized_axis() {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<VirtualInput>()
            .init_resource::<InputAccumulator>()
            .init_resource::<TickFrame>()
            .init_resource::<TickInput>()
            .add_systems(PreUpdate, accumulate_input)
            .add_systems(FixedPreUpdate, collect_tick_input.in_set(TickInputSet::Collect));
        app.world_mut().resource_mut::<VirtualInput>().axis = Vec2::new(0.333, -0.5);
        app.world_mut().run_schedule(PreUpdate);
        app.world_mut().run_schedule(FixedPreUpdate);
        let (qx, qy) = quantize_axis(Vec2::new(0.333, -0.5));
        let expect = dequantize_axis(qx, qy);
        let t = app.world().resource::<TickInput>();
        assert_eq!(t.axis, (qx, qy));
        assert_eq!(t.move_x, expect.x);
        assert_eq!(t.move_y, expect.y);
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test --lib input::tests`
Expected: compile error, `InputAccumulator` not found.

- [ ] **Step 3: Implement** (append to `src/input.rs`, before `#[cfg(test)]`)

```rust
/// Input folded since the last fixed tick. `held` / `axis` are the latest
/// values; `latched` ORs every press edge so a sub-tick tap still counts.
/// Written by [`accumulate_input`] every frame, or by a replay feeder.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq)]
pub struct InputAccumulator {
    pub held: Buttons,
    pub latched: Buttons,
    pub axis: Vec2,
}

/// The sim's input for the current fixed tick. Gameplay systems in
/// `FixedUpdate` read this instead of [`GameInput`]; `GameInput` stays the
/// per-frame view for menus and overlays.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq)]
pub struct TickInput {
    pub input: GameInput,
    /// The quantized analog stick alone (no D-pad folded in): what the
    /// replay stores and the feeder writes back.
    pub axis: (i8, i8),
}

impl core::ops::Deref for TickInput {
    type Target = GameInput;
    fn deref(&self) -> &GameInput {
        &self.input
    }
}

/// Previous tick's held set.
#[derive(Resource, Debug, Default)]
pub struct TickFrame {
    prev: Buttons,
}

/// Ordering hooks in `FixedPreUpdate`: a replay feeder runs in `Feed`,
/// [`collect_tick_input`] in `Collect`.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TickInputSet {
    Feed,
    Collect,
}

/// Clamps to -1..1 and maps to -127..127.
pub fn quantize_axis(v: Vec2) -> (i8, i8) {
    let q = |x: f32| (x.clamp(-1.0, 1.0) * 127.0).round() as i8;
    (q(v.x), q(v.y))
}

pub fn dequantize_axis(x: i8, y: i8) -> Vec2 {
    Vec2::new(f32::from(x) / 127.0, f32::from(y) / 127.0)
}

/// `PreUpdate`, after Bevy's input systems: folds keyboard, gamepads and the
/// virtual source into [`InputAccumulator`]. Does not consume the virtual
/// latch, so [`collect_input`] still sees it for the per-frame view.
pub fn accumulate_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    virt: Res<VirtualInput>,
    mut acc: ResMut<InputAccumulator>,
) {
    let mut held = map_keys(|k| keyboard.pressed(k));
    let mut latched = map_keys(|k| keyboard.just_pressed(k));
    let mut analog = Vec2::ZERO;
    for pad in &gamepads {
        held |= map_gamepad(|b| pad.pressed(b));
        latched |= map_gamepad(|b| pad.just_pressed(b));
        analog += apply_deadzone(pad.left_stick());
    }
    held |= virt.held;
    latched |= virt.latched;
    analog += virt.axis;
    acc.held = held;
    acc.latched |= latched;
    acc.axis = analog;
}

/// `FixedPreUpdate`: consumes the accumulator into [`TickInput`] with the
/// axis quantized, so what the sim sees is exactly what a replay carries.
pub fn collect_tick_input(
    mut acc: ResMut<InputAccumulator>,
    mut frame: ResMut<TickFrame>,
    mut tick: ResMut<TickInput>,
) {
    let (qx, qy) = quantize_axis(acc.axis);
    let e = edges(frame.prev, acc.held, core::mem::take(&mut acc.latched));
    frame.prev = e.held;
    tick.input = derive(e, dequantize_axis(qx, qy));
    tick.axis = (qx, qy);
}
```

Add to `GxConfig` in `src/host.rs` (both the struct and `Default`):

```rust
    /// Also maintain the fixed-tick input path (`TickInput`) for games whose
    /// sim runs in `FixedUpdate`. Off by default.
    pub tick_input: bool,
    /// No window, no web glue: for headless verifiers and tests.
    pub headless: bool,
```

with `tick_input: false, headless: false` in `Default`.

In `src/lib.rs`, after `.init_resource::<input::InputFrame>()` add:

```rust
        if self.config.tick_input {
            app.init_resource::<input::InputAccumulator>()
                .init_resource::<input::TickFrame>()
                .init_resource::<input::TickInput>()
                .configure_sets(
                    FixedPreUpdate,
                    input::TickInputSet::Feed.before(input::TickInputSet::Collect),
                )
                .add_systems(
                    PreUpdate,
                    input::accumulate_input.after(bevy::input::InputSystems),
                )
                .add_systems(
                    FixedPreUpdate,
                    input::collect_tick_input.in_set(input::TickInputSet::Collect),
                );
        }
```

and re-export: `pub use input::{GameInput, InputAccumulator, TickInput, TickInputSet, VirtualInput};` plus add `TickInput` to `prelude`.

- [ ] **Step 4: Run tests**

Run: `cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check`
Expected: all pass, including the three new tests.

- [ ] **Step 5: Commit**

```bash
git add src/input.rs src/host.rs src/lib.rs
git commit -m "feat: fixed-tick input path (InputAccumulator -> TickInput)

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 2: Headless plugin mode, seed/run protocol, v0.3.0

Repo: `libs/gamebient-input`.

**Files:**
- Modify: `src/host.rs` (`HostCommand`, `HostEvent`, `encode_event`, helpers, tests)
- Modify: `src/lib.rs` (`GxInputPlugin::headless`, skip window/web)
- Modify: `src/web.rs:118-131` (`parse_command`)
- Modify: `js/gx.js:100-103`
- Modify: `docs/host-protocol.md`
- Modify: `Cargo.toml` (version)

**Interfaces:**
- Produces: `HostCommand::Seed([u8; 32])`, `HostEvent::Run(Vec<u8>)`, `pub fn base64_encode(bytes: &[u8]) -> String`, `pub fn parse_seed_hex(s: &str) -> Option<[u8; 32]>`, `GxInputPlugin::headless(name) -> Self` (sets `tick_input: true, headless: true`).

- [ ] **Step 1: Write the failing tests** (append to `tests` in `src/host.rs`)

```rust
    #[test]
    fn base64_matches_rfc4648() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn encodes_run_event_as_base64() {
        assert_eq!(
            encode_event(&HostEvent::Run(b"foo".to_vec())),
            r#"{"type":"gx:event","v":1,"event":"run","replay":"Zm9v"}"#
        );
    }

    #[test]
    fn parses_seed_hex() {
        let hex = "00".repeat(31) + "ff";
        let seed = parse_seed_hex(&hex).unwrap();
        assert_eq!(seed[31], 0xff);
        assert_eq!(seed[0], 0);
        assert!(parse_seed_hex("abc").is_none());
        assert!(parse_seed_hex(&"zz".repeat(32)).is_none());
    }
```

Also in `src/host.rs` tests (`parse_command` moves from `web.rs` to `host.rs` in Step 3 so it is testable natively):

```rust
    #[test]
    fn parses_seed_command() {
        let hex = "ab".repeat(32);
        assert_eq!(
            parse_command(&format!("seed:{hex}")),
            Some(HostCommand::Seed([0xab; 32]))
        );
        assert_eq!(parse_command("seed:nope"), None);
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test --lib host::tests`
Expected: compile error, `base64_encode` / `HostEvent::Run` not found.

- [ ] **Step 3: Implement**

`src/host.rs`: add variants

```rust
    /// The sealed replay of the run that just ended (`GXR1` bytes). Posted
    /// base64-encoded as `{"event":"run","replay":"..."}`.
    Run(Vec<u8>),
```

to `HostEvent`, and

```rust
    /// A server-issued run seed (32 bytes) for the next run.
    Seed([u8; 32]),
```

to `HostCommand`. Add the `encode_event` arm:

```rust
        HostEvent::Run(bytes) => format!("\"event\":\"run\",\"replay\":\"{}\"", base64_encode(bytes)),
```

Add helpers:

```rust
/// Standard base64 with padding (RFC 4648), no dependency.
pub fn base64_encode(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

/// 64 hex chars → 32 bytes.
pub fn parse_seed_hex(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, pair) in s.as_bytes().chunks(2).enumerate() {
        let hi = (pair[0] as char).to_digit(16)?;
        let lo = (pair[1] as char).to_digit(16)?;
        out[i] = (hi * 16 + lo) as u8;
    }
    Some(out)
}

/// Decodes the short command strings the JS side queues:
/// `hello:1|0`, `pause`, `resume`, `mute:1|0`, `seed:<64 hex>`.
pub fn parse_command(s: &str) -> Option<HostCommand> {
    if let Some(hex) = s.strip_prefix("seed:") {
        return parse_seed_hex(hex).map(HostCommand::Seed);
    }
    match s {
        "pause" => Some(HostCommand::Pause),
        "resume" => Some(HostCommand::Resume),
        "mute:1" => Some(HostCommand::Mute(true)),
        "mute:0" => Some(HostCommand::Mute(false)),
        "hello:1" => Some(HostCommand::Hello { host_has_controls: true }),
        "hello:0" => Some(HostCommand::Hello { host_has_controls: false }),
        _ => None,
    }
}
```

Delete `parse_command` from `src/web.rs` and import it: `use crate::host::{..., parse_command};`. Update the `gx_take_commands` doc comment at `src/web.rs:47-48` to list `seed:<hex>`.

`js/gx.js` in the `gx:set` case, after the `muted` line:

```js
      if (typeof data.seed === 'string' && /^[0-9a-fA-F]{64}$/.test(data.seed)) state.commands.push('seed:' + data.seed.toLowerCase());
```

`src/lib.rs`: add the constructor and gate the window/web parts:

```rust
    /// Headless: no window, no web glue, fixed-tick input on. For the
    /// replay verifier and headless tests.
    pub fn headless(name: impl Into<String>) -> Self {
        Self {
            config: GxConfig {
                name: name.into(),
                tick_input: true,
                headless: true,
                ..Default::default()
            },
        }
    }
```

In `build`, wrap the canvas-policy block so headless inserts `CanvasPolicy::Fit` without querying the window, and change the web line to:

```rust
        #[cfg(target_arch = "wasm32")]
        if !self.config.headless {
            app.add_plugins(web::WebPlugin);
        }
```

`docs/host-protocol.md`: add to the Game → host block

```jsonc
{ "type": "gx:event", "v": 1, "event": "run", "replay": "<base64 GXR1 bytes>" } // a run ended; its replay
```

and to Host → game

```jsonc
{ "type": "gx:set", "v": 1, "seed": "<64 hex chars>" }   // server-issued seed for the next run
```

with a paragraph: "`seed` is stored and consumed by the next run; a run that starts without one seeds itself and marks the replay `origin = local`. `run` carries the sealed replay of the run that just ended; hosts forward it to their verifier. The format is defined by the game template's `docs/replay-verification.md`."

`Cargo.toml`: `version = "0.3.0"`.

- [ ] **Step 4: Run tests**

Run: `cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check`
Expected: pass.

- [ ] **Step 5: Commit** (the owner tags `v0.3.0` and pushes)

```bash
git add -A
git commit -m "feat: headless mode, gx:set seed and gx:event run (v0.3.0)

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

Tell the owner: `git tag v0.3.0 && git push origin main v0.3.0` is required before Task 3's `cargo update` resolves. Until then, Task 3 may use `path = "../gamebient-input"` locally and switch to the tag before its commit.

---

### Task 3: Template lib split, `sim.rs`, and the fixed-tick chain

Repo: `libs/gamebient-bevy-template`.

**Files:**
- Create: `src/lib.rs`, `src/game/sim.rs`
- Modify: `src/main.rs`, `src/game/mod.rs`, `src/game/player.rs`, `src/game/host.rs`, `Cargo.toml`

**Interfaces:**
- Produces: `sim::TICK_HZ: u32`, `sim::tick_duration() -> Duration`, `sim::SimTick(pub u32)`, `sim::SimSet` (SystemSet), `sim::SeedOrigin { Host = 0, Local = 1 }`, `sim::RunSeed { bytes: [u8; 32], origin: SeedOrigin }` (Resource, Default = zero/Local), `sim::PendingSeed(pub Option<[u8; 32]>)` (Resource), `sim::GameRng(pub Xoshiro256PlusPlus)` (Resource), `sim::Checksum(pub u64)` (Resource) with `fn fold(&mut self, v: u64)`; `sim::checksum_tick` folds score, lives, tick and the player `Transform` x/y bits; `GamePlugin { pub headless: bool }`.

- [ ] **Step 1: Cargo and lib split**

`Cargo.toml`: replace the `gamebient-input` line with `tag = "v0.3.0"`; add `rand_xoshiro = "0.7"` under `[dependencies]`; add after `[package]`:

```toml
[lib]
name = "gamebient_game"
path = "src/lib.rs"

[[bin]]
name = "gamebient-game"
path = "src/main.rs"
```

`src/lib.rs`:

```rust
//! Library half of the game so a second entry point (`src/bin/verify.rs`)
//! can build the headless sim from the same modules as the windowed game.
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

pub mod assets;
pub mod game;
pub mod ui;
```

`src/main.rs`: delete the `mod` lines and the `#![allow]`, add `use gamebient_game::{assets, game, ui};` and pass `game::GamePlugin::default()` in `add_plugins`.

Run: `cargo build`
Expected: builds. (`init-game.sh` renames the package; check it also rewrites `name = "gamebient_game"` in `[lib]` and the `use gamebient_game` line: add a `sed` for `gamebient_game` next to its existing `gamebient-game` substitutions.)

- [ ] **Step 2: Write the failing tests** for `src/game/sim.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_duration_is_sixty_hz() {
        assert_eq!(TICK_HZ, 60);
        assert_eq!(tick_duration().as_nanos(), 16_666_667);
    }

    #[test]
    fn same_seed_same_stream() {
        let mut a = GameRng::from_seed(&[7u8; 32]);
        let mut b = GameRng::from_seed(&[7u8; 32]);
        let xs: Vec<u32> = (0..8).map(|_| a.0.next_u32()).collect();
        let ys: Vec<u32> = (0..8).map(|_| b.0.next_u32()).collect();
        assert_eq!(xs, ys);
        let mut c = GameRng::from_seed(&[8u8; 32]);
        assert_ne!(xs[0], c.0.next_u32());
    }

    #[test]
    fn checksum_folds_order_sensitively() {
        let mut a = Checksum::default();
        a.fold(1);
        a.fold(2);
        let mut b = Checksum::default();
        b.fold(2);
        b.fold(1);
        assert_ne!(a.0, b.0);
        assert_ne!(a.0, Checksum::default().0);
    }

    #[test]
    fn no_forbidden_randomness_or_hashmaps_in_game_code() {
        let forbidden = ["rand::rng()", "from_os_rng", "thread_rng", "SmallRng", "std::collections::HashMap"];
        let mut hits = Vec::new();
        for entry in walk(std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/src/game"))) {
            let text = std::fs::read_to_string(&entry).unwrap();
            for (n, line) in text.lines().enumerate() {
                if line.contains("allow-forbidden-rng") {
                    continue;
                }
                for f in forbidden {
                    if line.contains(f) && !line.trim_start().starts_with("//") {
                        hits.push(format!("{}:{}: {f}", entry.display(), n + 1));
                    }
                }
            }
        }
        assert!(hits.is_empty(), "forbidden in sim code:\n{}", hits.join("\n"));
    }

    fn walk(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
        let mut out = Vec::new();
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                out.extend(walk(&p));
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
        out
    }
}
```

Run: `cargo test sim::` — Expected: compile error, module missing.

- [ ] **Step 3: Implement `src/game/sim.rs`**

```rust
//! Determinism scaffolding: the fixed tick, the one RNG, the run seed and
//! the checksum a replay must reproduce. See docs/replay-verification.md.

use std::time::Duration;

use bevy::prelude::*;
use rand::{RngCore, SeedableRng};
use rand_xoshiro::Xoshiro256PlusPlus;

use super::scoring::GameData;

pub const TICK_HZ: u32 = 60;

/// One fixed tick. `Time::<Fixed>` and the verifier both step with this.
pub fn tick_duration() -> Duration {
    Duration::from_nanos(16_666_667)
}

/// Sim ticks since the run began (0 before the first tick).
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SimTick(pub u32);

/// The ordered fixed-tick chain. Everything that mutates run state goes
/// here, in order; nothing in `Update` writes run state.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SimSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum SeedOrigin {
    Host = 0,
    #[default]
    Local = 1,
}

impl SeedOrigin {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Host),
            1 => Some(Self::Local),
            _ => None,
        }
    }
}

/// The seed the current run was started with.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RunSeed {
    pub bytes: [u8; 32],
    pub origin: SeedOrigin,
}

impl RunSeed {
    /// The only OS randomness in `src/game/`: used when no host seed is
    /// pending. Such runs are `origin = Local`.
    pub fn local() -> Self {
        let mut bytes = [0u8; 32];
        rand::rng().fill_bytes(&mut bytes); // allow-forbidden-rng
        Self { bytes, origin: SeedOrigin::Local }
    }
}

/// A host-supplied seed waiting for the next run.
#[derive(Resource, Debug, Default)]
pub struct PendingSeed(pub Option<[u8; 32]>);

/// The one RNG the sim may use. Xoshiro256++ explicitly: `SmallRng` picks a
/// different algorithm on wasm32, which would break native-vs-wasm replay.
#[derive(Resource)]
pub struct GameRng(pub Xoshiro256PlusPlus);

impl GameRng {
    pub fn from_seed(seed: &[u8; 32]) -> Self {
        Self(Xoshiro256PlusPlus::from_seed(*seed))
    }
}

impl Default for GameRng {
    fn default() -> Self {
        Self::from_seed(&[0u8; 32])
    }
}

/// FNV-1a over whatever the sim folds in each tick.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Checksum(pub u64);

impl Default for Checksum {
    fn default() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }
}

impl Checksum {
    pub fn fold(&mut self, v: u64) {
        for b in v.to_le_bytes() {
            self.0 ^= u64::from(b);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
}

/// `OnEnter(Playing)`: take the pending host seed (or draw a local one),
/// reseed the RNG, zero the tick and checksum.
pub fn begin_run(
    mut pending: ResMut<PendingSeed>,
    mut seed: ResMut<RunSeed>,
    mut rng: ResMut<GameRng>,
    mut tick: ResMut<SimTick>,
    mut sum: ResMut<Checksum>,
) {
    *seed = match pending.0.take() {
        Some(bytes) => RunSeed { bytes, origin: SeedOrigin::Host },
        None => RunSeed::local(),
    };
    *rng = GameRng::from_seed(&seed.bytes);
    *tick = SimTick(0);
    *sum = Checksum::default();
}

/// First in `SimSet`.
pub fn advance_tick(mut tick: ResMut<SimTick>) {
    tick.0 += 1;
}

/// Last in `SimSet` before the recorder: folds the state a replay must
/// reproduce. The player transform is folded bit-exactly so the checksum
/// is sensitive to inputs and to float behaviour, which is what the
/// native-vs-wasm fixture check relies on. Games fold more via
/// `Checksum::fold` from their own systems.
pub fn checksum_tick(
    data: Res<GameData>,
    tick: Res<SimTick>,
    player: Query<&Transform, With<super::player::Player>>,
    mut sum: ResMut<Checksum>,
) {
    sum.fold(u64::from(data.score));
    sum.fold(u64::from(data.lives));
    sum.fold(u64::from(tick.0));
    if let Ok(tf) = player.single() {
        sum.fold(u64::from(tf.translation.x.to_bits()));
        sum.fold(u64::from(tf.translation.y.to_bits()));
    }
}
```

- [ ] **Step 4: Rewire `GamePlugin`** in `src/game/mod.rs`

Add `pub mod sim;`. Replace `pub struct GamePlugin;` with:

```rust
/// `headless: true` builds the sim only (no scene, audio, overlay or fade)
/// for the replay verifier. The windowed game uses `GamePlugin::default()`.
#[derive(Default)]
pub struct GamePlugin {
    pub headless: bool,
}
```

Rewrite `build` (keep the record/autopilot blocks as they are):

```rust
    fn build(&self, app: &mut App) {
        let input = if self.headless {
            gamebient_input::GxInputPlugin::headless("Gamebient Game")
        } else {
            let mut p = gamebient_input::GxInputPlugin::named("Gamebient Game");
            p.config.tick_input = true;
            p
        };
        app.init_state::<GameState>()
            .add_plugins(input)
            .add_plugins(gamebient_input::StateEvents::<GameState>::default())
            .add_plugins(host::HostBridgePlugin)
            .insert_resource(Time::<Fixed>::from_duration(sim::tick_duration()))
            .init_resource::<scoring::GameData>()
            .init_resource::<states::Paused>()
            .init_resource::<sim::SimTick>()
            .init_resource::<sim::RunSeed>()
            .init_resource::<sim::PendingSeed>()
            .init_resource::<sim::GameRng>()
            .init_resource::<sim::Checksum>()
            .add_message::<scoring::ScoreEvent>()
            .add_message::<audio::SfxEvent>()
            .configure_sets(
                FixedUpdate,
                sim::SimSet.run_if(in_state(GameState::Playing).and(states::not_paused)),
            )
            .add_systems(
                OnEnter(GameState::Playing),
                (reset_paused, reset_game_data, sim::begin_run, player::spawn_player),
            )
            .add_systems(
                FixedUpdate,
                toggle_pause.run_if(in_state(GameState::Playing)).before(sim::SimSet),
            )
            .add_systems(
                FixedUpdate,
                (
                    sim::advance_tick,
                    player::move_player,
                    scoring::handle_score_events,
                    sim::checksum_tick,
                )
                    .chain()
                    .in_set(sim::SimSet),
            )
            .add_systems(OnExit(GameState::Playing), cleanup_game_entities);

        if !self.headless {
            app.init_resource::<audio::CurrentTrack>()
                .add_systems(Startup, (setup_scene, audio::setup_sfx))
                .add_systems(
                    Update,
                    (audio::play_sfx, audio::music_director, audio::update_music_fades),
                )
                .add_systems(
                    Update,
                    (pause_quit, sync_pause_overlay)
                        .chain()
                        .run_if(in_state(GameState::Playing)),
                )
                .add_systems(OnExit(GameState::Playing), cleanup_pause_overlay);
        }
        #[cfg(feature = "autopilot")]
        app.add_plugins(autopilot::AutopilotPlugin);
        #[cfg(feature = "record")]
        { /* unchanged */ }
    }
```

Change `toggle_pause` to read `Res<gamebient_input::TickInput>` and take `fade: Option<Res<crate::ui::transition::ScreenFade>>`, guarding with `if fade.as_ref().is_some_and(|f| !f.is_idle()) || !input.pause_just_pressed { return; }`.

`src/game/player.rs`: `move_player` takes `input: Res<gamebient_input::TickInput>` (body unchanged; `Res<Time>` inside `FixedUpdate` is the fixed delta). `spawn_player` takes `meshes: Option<ResMut<Assets<Mesh>>>, materials: Option<ResMut<Assets<StandardMaterial>>>`; spawn `(Player, GameEntity, Transform::from_xyz(0.0, 0.0, 0.0))` and, only when both are `Some`, insert `Mesh3d` / `MeshMaterial3d` on the same entity.

`src/game/host.rs`: in `apply_host_commands` add `mut pending: ResMut<sim::PendingSeed>` and the arm `HostCommand::Seed(bytes) => pending.0 = Some(*bytes),`.

`src/game/autopilot.rs`: no change; it writes `VirtualInput`, which `accumulate_input` folds into the tick path.

- [ ] **Step 5: Run everything**

Run: `cargo test --all-features && cargo clippy --all-targets --all-features -- -D warnings && cargo fmt --check && cargo run --features autopilot`
Expected: tests pass; the autopilot tour still plays (cube moves in `05-mid-play.png`, score on the HUD).

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat(sim): fixed-tick SimSet, RunSeed, GameRng, Checksum; lib split; headless GamePlugin

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 4: Build id

Repo: template.

**Files:**
- Create: `build.rs`
- Modify: `Cargo.toml` (`build = "build.rs"` is implicit; nothing to add)

**Interfaces:**
- Produces: `env!("GX_BUILD_ID")` = `"<CARGO_PKG_VERSION>+<7-char sha>"` or `"<version>+unknown"`.

- [ ] **Step 1: Write `build.rs`**

```rust
use std::process::Command;

fn main() {
    let sha = Command::new("git")
        .args(["rev-parse", "--short=7", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".into());
    println!(
        "cargo:rustc-env=GX_BUILD_ID={}+{sha}",
        std::env::var("CARGO_PKG_VERSION").unwrap()
    );
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs");
}
```

- [ ] **Step 2: Verify**

Run: `cargo build && strings target/debug/gamebient-game | grep -m1 '0.1.0+'`
Expected: prints `0.1.0+<sha>`.

- [ ] **Step 3: Commit**

```bash
git add build.rs
git commit -m "build: GX_BUILD_ID env for replay headers

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 5: Replay format and codec

Repo: template.

**Files:**
- Create: `src/game/replay/mod.rs`
- Modify: `src/game/mod.rs` (`pub mod replay;`)

**Interfaces:**
- Produces:

```rust
pub const MAGIC: &[u8; 4] = b"GXR1";
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TickRun { pub held: u16, pub latched: u16, pub ax: i8, pub ay: i8, pub count: u16 }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replay {
    pub build: String, pub tick_hz: u16, pub seed: [u8; 32], pub origin: SeedOrigin,
    pub ticks: u32, pub score: u64, pub checksum: u64, pub runs: Vec<TickRun>,
}
#[derive(Debug, PartialEq, Eq)]
pub enum DecodeError { BadMagic, Truncated, BadOrigin(u8), BadBuild, RunCountMismatch { header: u32, sum: u32 } }
impl Replay { pub fn encode(&self) -> Vec<u8>; pub fn decode(bytes: &[u8]) -> Result<Replay, DecodeError>; pub fn push_tick(&mut self, held: u16, latched: u16, ax: i8, ay: i8); }
```

`push_tick` merges into the last run when `(held, latched, ax, ay)` are equal and `count < u16::MAX`, else appends `count: 1`. It also increments `ticks`.

- [ ] **Step 1: Write the failing tests** (in `src/game/replay/mod.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Replay {
        let mut r = Replay {
            build: "0.1.0+abc1234".into(),
            tick_hz: 60,
            seed: [9u8; 32],
            origin: SeedOrigin::Host,
            ticks: 0,
            score: 1500,
            checksum: 0xdead_beef,
            runs: Vec::new(),
        };
        for _ in 0..3 {
            r.push_tick(8, 0, 0, 0);
        }
        r.push_tick(8, 16, 0, 0);
        r.push_tick(0, 0, 127, -127);
        r
    }

    #[test]
    fn push_tick_run_length_encodes() {
        let r = sample();
        assert_eq!(r.ticks, 5);
        assert_eq!(r.runs.len(), 3);
        assert_eq!(r.runs[0], TickRun { held: 8, latched: 0, ax: 0, ay: 0, count: 3 });
        assert_eq!(r.runs[2].ax, 127);
    }

    #[test]
    fn round_trips() {
        let r = sample();
        let bytes = r.encode();
        assert_eq!(&bytes[..4], MAGIC);
        assert_eq!(Replay::decode(&bytes).unwrap(), r);
    }

    #[test]
    fn rejects_bad_input() {
        let r = sample();
        let bytes = r.encode();
        assert_eq!(Replay::decode(b"GXR0"), Err(DecodeError::BadMagic));
        assert_eq!(Replay::decode(&bytes[..bytes.len() - 1]), Err(DecodeError::Truncated));
        let mut bad = bytes.clone();
        let origin_at = 4 + 1 + r.build.len() + 2 + 32;
        bad[origin_at] = 7;
        assert_eq!(Replay::decode(&bad), Err(DecodeError::BadOrigin(7)));
        let mut mism = r.clone();
        mism.ticks = 99;
        assert_eq!(
            Replay::decode(&mism.encode()),
            Err(DecodeError::RunCountMismatch { header: 99, sum: 5 })
        );
    }

    #[test]
    fn run_count_saturates_at_u16() {
        let mut r = sample();
        r.runs.clear();
        r.ticks = 0;
        for _ in 0..70_000 {
            r.push_tick(1, 0, 0, 0);
        }
        assert_eq!(r.ticks, 70_000);
        assert_eq!(r.runs.len(), 2);
        assert_eq!(r.runs[0].count, u16::MAX);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test replay::` — Expected: compile error.

- [ ] **Step 3: Implement**

```rust
//! Replay format `GXR1` (little-endian):
//!
//! ```text
//! magic    b"GXR1"
//! build    u8 len + UTF-8            GX_BUILD_ID of the game that recorded it
//! tick_hz  u16
//! seed     [u8; 32]
//! origin   u8                        0 host, 1 local
//! ticks    u32                       total sim ticks
//! score    u64                       claimed
//! checksum u64                       claimed
//! runs     u32                       number of RLE runs
//! run[]    held u16, latched u16, ax i8, ay i8, count u16
//! ```

pub mod feeder;
pub mod recorder;

use super::sim::SeedOrigin;

pub const MAGIC: &[u8; 4] = b"GXR1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TickRun {
    pub held: u16,
    pub latched: u16,
    pub ax: i8,
    pub ay: i8,
    pub count: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replay {
    pub build: String,
    pub tick_hz: u16,
    pub seed: [u8; 32],
    pub origin: SeedOrigin,
    pub ticks: u32,
    pub score: u64,
    pub checksum: u64,
    pub runs: Vec<TickRun>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum DecodeError {
    BadMagic,
    Truncated,
    BadOrigin(u8),
    BadBuild,
    RunCountMismatch { header: u32, sum: u32 },
}

impl Replay {
    pub fn push_tick(&mut self, held: u16, latched: u16, ax: i8, ay: i8) {
        self.ticks += 1;
        if let Some(last) = self.runs.last_mut()
            && last.held == held
            && last.latched == latched
            && last.ax == ax
            && last.ay == ay
            && last.count < u16::MAX
        {
            last.count += 1;
            return;
        }
        self.runs.push(TickRun { held, latched, ax, ay, count: 1 });
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(64 + self.runs.len() * 8);
        out.extend_from_slice(MAGIC);
        let build = self.build.as_bytes();
        out.push(build.len().min(255) as u8);
        out.extend_from_slice(&build[..build.len().min(255)]);
        out.extend_from_slice(&self.tick_hz.to_le_bytes());
        out.extend_from_slice(&self.seed);
        out.push(self.origin as u8);
        out.extend_from_slice(&self.ticks.to_le_bytes());
        out.extend_from_slice(&self.score.to_le_bytes());
        out.extend_from_slice(&self.checksum.to_le_bytes());
        out.extend_from_slice(&(self.runs.len() as u32).to_le_bytes());
        for r in &self.runs {
            out.extend_from_slice(&r.held.to_le_bytes());
            out.extend_from_slice(&r.latched.to_le_bytes());
            out.push(r.ax as u8);
            out.push(r.ay as u8);
            out.extend_from_slice(&r.count.to_le_bytes());
        }
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Replay, DecodeError> {
        let mut c = Cursor { b: bytes, i: 0 };
        if c.take(4)? != MAGIC {
            return Err(DecodeError::BadMagic);
        }
        let n = c.u8()? as usize;
        let build = core::str::from_utf8(c.take(n)?).map_err(|_| DecodeError::BadBuild)?.to_string();
        let tick_hz = c.u16()?;
        let mut seed = [0u8; 32];
        seed.copy_from_slice(c.take(32)?);
        let o = c.u8()?;
        let origin = SeedOrigin::from_u8(o).ok_or(DecodeError::BadOrigin(o))?;
        let ticks = c.u32()?;
        let score = c.u64()?;
        let checksum = c.u64()?;
        let n_runs = c.u32()? as usize;
        let mut runs = Vec::with_capacity(n_runs.min(1 << 16));
        let mut sum: u32 = 0;
        for _ in 0..n_runs {
            let r = TickRun {
                held: c.u16()?,
                latched: c.u16()?,
                ax: c.u8()? as i8,
                ay: c.u8()? as i8,
                count: c.u16()?,
            };
            sum = sum.saturating_add(u32::from(r.count));
            runs.push(r);
        }
        if sum != ticks {
            return Err(DecodeError::RunCountMismatch { header: ticks, sum });
        }
        Ok(Replay { build, tick_hz, seed, origin, ticks, score, checksum, runs })
    }
}

struct Cursor<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], DecodeError> {
        let end = self.i.checked_add(n).ok_or(DecodeError::Truncated)?;
        let s = self.b.get(self.i..end).ok_or(DecodeError::Truncated)?;
        self.i = end;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, DecodeError> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, DecodeError> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32, DecodeError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64, DecodeError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
}
```

Create empty `src/game/replay/feeder.rs` and `recorder.rs` (a `//!` line each) so the module compiles; Tasks 6 and 7 fill them. Add `pub mod replay;` to `src/game/mod.rs`.

- [ ] **Step 4: Run tests**

Run: `cargo test replay:: && cargo clippy --all-targets --all-features -- -D warnings`
Expected: 4 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/game/replay src/game/mod.rs
git commit -m "feat(replay): GXR1 format and codec

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 6: Recorder and submission

Repo: template.

**Files:**
- Create: `src/game/replay/recorder.rs`
- Modify: `src/game/mod.rs` (register), `src/game/host.rs` (doc table)

**Interfaces:**
- Consumes: `TickInput` (Task 1), `sim::*` (Task 3), `Replay` (Task 5), `HostEvent::Run` (Task 2).
- Produces: `pub struct ReplayRecorder { pub replay: Replay, pub sealed: bool }` (Resource); `pub fn record_tick(...)` (last in `SimSet`); `pub fn seal_run(...)` (`OnExit(Playing)`); `pub fn last_run(&self) -> Option<&Replay>`; `ReplayRecorder::begin(seed: RunSeed)`.

- [ ] **Step 1: Write the failing tests** (in `recorder.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::sim::{RunSeed, SeedOrigin};
    use gamebient_input::{GameInput, TickInput};

    fn tick(held: Buttons) -> TickInput {
        let mut g = GameInput::default();
        g.edges.held = held;
        TickInput { input: g, axis: (0, 0) }
    }

    #[test]
    fn records_raw_tick_input_and_seals() {
        let mut rec = ReplayRecorder::default();
        rec.begin(RunSeed { bytes: [1u8; 32], origin: SeedOrigin::Host });
        rec.push(&tick(Buttons::RIGHT));
        rec.push(&tick(Buttons::RIGHT));
        rec.push(&tick(Buttons::NONE));
        let r = rec.seal(1500, 42);
        assert_eq!(r.ticks, 3);
        assert_eq!(r.runs.len(), 2);
        assert_eq!(r.runs[0].held, Buttons::RIGHT.0 as u16);
        assert_eq!(r.score, 1500);
        assert_eq!(r.checksum, 42);
        assert_eq!(r.origin, SeedOrigin::Host);
        assert_eq!(r.build, env!("GX_BUILD_ID"));
        assert!(rec.sealed);
    }

    #[test]
    fn begin_resets_a_previous_run() {
        let mut rec = ReplayRecorder::default();
        rec.begin(RunSeed::default());
        rec.push(&tick(Buttons::A));
        rec.seal(1, 1);
        rec.begin(RunSeed::default());
        assert_eq!(rec.replay.ticks, 0);
        assert!(!rec.sealed);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test recorder::` — Expected: compile error.

- [ ] **Step 3: Implement**

```rust
//! Records each sim tick's `TickInput` into a `Replay`; seals it when the
//! run ends and posts it to the host as `HostEvent::Run`. Always compiled:
//! this is how the shipped game submits a run.

use bevy::prelude::*;
use gamebient_input::{Buttons, HostEvent, TickInput};

use super::Replay;
use crate::game::scoring::GameData;
use crate::game::sim::{Checksum, RunSeed, TICK_HZ};

#[derive(Resource)]
pub struct ReplayRecorder {
    pub replay: Replay,
    pub sealed: bool,
}

impl Default for ReplayRecorder {
    fn default() -> Self {
        Self { replay: empty(RunSeed::default()), sealed: false }
    }
}

fn empty(seed: RunSeed) -> Replay {
    Replay {
        build: env!("GX_BUILD_ID").to_string(),
        tick_hz: TICK_HZ as u16,
        seed: seed.bytes,
        origin: seed.origin,
        ticks: 0,
        score: 0,
        checksum: 0,
        runs: Vec::new(),
    }
}

impl ReplayRecorder {
    pub fn begin(&mut self, seed: RunSeed) {
        self.replay = empty(seed);
        self.sealed = false;
    }

    /// Raw tick record: the held set, the sub-tick taps (pressed but not
    /// held) as a latch, and the quantized stick — exactly what the feeder
    /// writes back into `InputAccumulator`.
    pub fn push(&mut self, t: &TickInput) {
        let held = t.edges.held.sanitized().0 as u16;
        let latched = t.edges.just_pressed.difference(t.edges.held).sanitized().0 as u16;
        let (ax, ay) = t.axis;
        self.replay.push_tick(held, latched, ax, ay);
    }

    pub fn seal(&mut self, score: u64, checksum: u64) -> Replay {
        self.replay.score = score;
        self.replay.checksum = checksum;
        self.sealed = true;
        self.replay.clone()
    }

    pub fn last_run(&self) -> Option<&Replay> {
        self.sealed.then_some(&self.replay)
    }
}

/// `OnEnter(Playing)`, after `sim::begin_run`.
pub fn begin_recording(seed: Res<RunSeed>, mut rec: ResMut<ReplayRecorder>) {
    rec.begin(*seed);
}

/// Last in `SimSet`: only sim ticks are recorded, so paused ticks are absent.
pub fn record_tick(t: Res<TickInput>, mut rec: ResMut<ReplayRecorder>) {
    rec.push(&t);
}

/// `OnExit(Playing)`: seal, post to the host, and on native optionally
/// write `$GX_REPLAY_DIR/<unix-ms>.gxr`.
pub fn seal_run(
    data: Res<GameData>,
    sum: Res<Checksum>,
    mut rec: ResMut<ReplayRecorder>,
    mut host: MessageWriter<HostEvent>,
) {
    if rec.sealed {
        return;
    }
    let replay = rec.seal(u64::from(data.score), sum.0);
    let bytes = replay.encode();
    host.write(HostEvent::Run(bytes.clone()));
    #[cfg(not(target_arch = "wasm32"))]
    if let Ok(dir) = std::env::var("GX_REPLAY_DIR") {
        let ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let path = std::path::Path::new(&dir).join(format!("{ms}.gxr"));
        if let Err(e) = std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&path, &bytes)) {
            warn!("replay: could not write {}: {e}", path.display());
        } else {
            info!("replay: wrote {} ({} ticks, origin {:?})", path.display(), replay.ticks, replay.origin);
        }
    }
}
```

Register in `GamePlugin::build` (both modes): `.init_resource::<replay::recorder::ReplayRecorder>()`, add `replay::recorder::begin_recording` to the `OnEnter(Playing)` tuple after `sim::begin_run` (make the tuple `.chain()`), append `replay::recorder::record_tick` after `sim::checksum_tick` in the `SimSet` chain, and add `replay::recorder::seal_run` to `OnExit(Playing)` before `cleanup_game_entities`.

Update the table in `src/game/host.rs`'s module doc with a row `| game → host | run + replay | leaving Playing (sealed GXR1, base64) |` and change the last doc paragraph to say the `run` event is what a verifier consumes.

- [ ] **Step 4: Run tests and a real run**

Run: `cargo test && GX_REPLAY_DIR=build/replays cargo run --features autopilot && ls build/replays`
Expected: tests pass; one `.gxr` file exists after the tour.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(replay): record sim ticks, seal on run end, post gx:event run

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 7: Feeder, headless app, and `verify()`

Repo: template.

**Files:**
- Create: `src/game/replay/feeder.rs`
- Modify: `src/game/replay/mod.rs` (append `verify`, `Verdict`, `build_headless_app`)

**Interfaces:**
- Consumes: `InputAccumulator`, `TickInputSet::Feed`, `dequantize_axis` (Task 1); `GamePlugin { headless: true }` (Task 3); `Replay` (Task 5); `ReplayRecorder` (Task 6).
- Produces:

```rust
pub struct ReplayFeeder { pub runs: Vec<TickRun>, pub run: usize, pub left: u16, pub remaining: u32, pub done: bool }  // Resource; done flips on the tick that consumes the last record
impl ReplayFeeder { pub fn new(replay: &Replay) -> Self; pub fn next(&mut self) -> Option<TickRun>; }
pub fn feed_tick(feeder: ResMut<ReplayFeeder>, acc: ResMut<InputAccumulator>)             // TickInputSet::Feed
#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum Ended { GameOver, InputExhausted, Cap }
#[derive(Debug, Clone, PartialEq, Eq)] pub struct Verdict { pub score: u64, pub checksum: u64, pub ticks: u32, pub ended: Ended, pub matches: bool }
impl Verdict { pub fn to_json(&self) -> String; }
pub fn build_headless_app() -> App
pub fn verify(replay: &Replay) -> Verdict
```

- [ ] **Step 1: Write the failing tests**

In `feeder.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walks_runs_then_is_done() {
        let mut r = Replay {
            build: String::new(), tick_hz: 60, seed: [0; 32], origin: SeedOrigin::Local,
            ticks: 0, score: 0, checksum: 0, runs: Vec::new(),
        };
        r.push_tick(1, 0, 0, 0);
        r.push_tick(1, 0, 0, 0);
        r.push_tick(2, 4, 127, 0);
        let mut f = ReplayFeeder::new(&r);
        assert_eq!(f.next().unwrap().held, 1);
        assert_eq!(f.next().unwrap().held, 1);
        assert!(!f.done);
        assert_eq!(f.next().unwrap().latched, 4);
        assert!(f.done, "done flips on the tick that consumes the last record");
        assert!(f.next().is_none());
    }
}
```

In `mod.rs` tests:

```rust
    #[test]
    fn headless_app_runs_a_replay_to_a_verdict() {
        // A 120-tick run holding RIGHT: the template scores nothing, so the
        // verdict is score 0 with the checksum the sim produces; encode the
        // sim's own answer as the claim and it must match.
        let mut r = sample();
        r.runs.clear();
        r.ticks = 0;
        for _ in 0..120 {
            r.push_tick(8, 0, 0, 0);
        }
        r.score = 0;
        let first = verify(&r);
        assert_eq!(first.ticks, 120);
        assert_eq!(first.ended, Ended::InputExhausted);
        r.checksum = first.checksum;
        let second = verify(&r);
        assert!(second.matches, "{second:?}");
        assert_eq!(second.checksum, first.checksum);
        r.score = 999;
        assert!(!verify(&r).matches);
    }

    #[test]
    fn verdict_json_is_flat() {
        let v = Verdict { score: 1, checksum: 2, ticks: 3, ended: Ended::Cap, matches: false };
        assert_eq!(
            v.to_json(),
            r#"{"score":1,"checksum":2,"ticks":3,"ended":"cap","matches":false}"#
        );
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test replay::` — Expected: compile error.

- [ ] **Step 3: Implement `feeder.rs`**

```rust
//! Drives the fixed-tick input path from a decoded replay. Only the
//! verifier inserts this resource.

use bevy::prelude::*;
use gamebient_input::{Buttons, InputAccumulator, input::dequantize_axis};

use super::{Replay, TickRun};

#[derive(Resource, Debug)]
pub struct ReplayFeeder {
    pub runs: Vec<TickRun>,
    pub run: usize,
    pub left: u16,
    /// Ticks not yet handed out; `done` flips when this reaches zero, i.e.
    /// on the tick that consumes the last record, so the verifier stops
    /// after exactly `replay.ticks` ticks.
    pub remaining: u32,
    pub done: bool,
}

impl ReplayFeeder {
    pub fn new(replay: &Replay) -> Self {
        let left = replay.runs.first().map_or(0, |r| r.count);
        Self {
            runs: replay.runs.clone(),
            run: 0,
            left,
            remaining: replay.ticks,
            done: replay.ticks == 0,
        }
    }

    pub fn next(&mut self) -> Option<TickRun> {
        if self.remaining == 0 {
            self.done = true;
            return None;
        }
        while self.run < self.runs.len() && self.left == 0 {
            self.run += 1;
            self.left = self.runs.get(self.run).map_or(0, |r| r.count);
        }
        let Some(r) = self.runs.get(self.run) else {
            self.done = true;
            return None;
        };
        self.left -= 1;
        self.remaining -= 1;
        if self.remaining == 0 {
            self.done = true;
        }
        Some(*r)
    }
}

/// `FixedPreUpdate`, `TickInputSet::Feed`: overwrite the accumulator with
/// this tick's record. Past the end, feed nothing (the accumulator is
/// cleared so a stale hold cannot leak).
pub fn feed_tick(mut feeder: ResMut<ReplayFeeder>, mut acc: ResMut<InputAccumulator>) {
    match feeder.next() {
        Some(r) => {
            acc.held = Buttons(u32::from(r.held));
            acc.latched = Buttons(u32::from(r.latched));
            acc.axis = dequantize_axis(r.ax, r.ay);
        }
        None => *acc = InputAccumulator::default(),
    }
}
```

- [ ] **Step 4: Implement `verify` in `mod.rs`** (append)

```rust
use std::time::Duration;

use bevy::app::App;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy::time::TimeUpdateStrategy;
use gamebient_input::TickInputSet;

use super::GamePlugin;
use super::scoring::GameData;
use super::sim::{Checksum, PendingSeed, RunSeed, SimTick, tick_duration};
use super::states::GameState;
use feeder::{ReplayFeeder, feed_tick};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ended {
    GameOver,
    InputExhausted,
    Cap,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    pub score: u64,
    pub checksum: u64,
    pub ticks: u32,
    pub ended: Ended,
    pub matches: bool,
}

impl Verdict {
    pub fn to_json(&self) -> String {
        let ended = match self.ended {
            Ended::GameOver => "gameover",
            Ended::InputExhausted => "input_exhausted",
            Ended::Cap => "cap",
        };
        format!(
            "{{\"score\":{},\"checksum\":{},\"ticks\":{},\"ended\":\"{ended}\",\"matches\":{}}}",
            self.score, self.checksum, self.ticks, self.matches
        )
    }
}

/// The sim with no window, render, audio or UI, stepping one tick per
/// `app.update()`.
pub fn build_headless_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(StatesPlugin)
        .insert_resource(TimeUpdateStrategy::ManualDuration(tick_duration()))
        .add_plugins(GamePlugin { headless: true });
    app
}

/// Re-simulates `replay` and reports what the sim produced.
pub fn verify(replay: &Replay) -> Verdict {
    let mut app = build_headless_app();
    app.insert_resource(ReplayFeeder::new(replay))
        .add_systems(FixedPreUpdate, feed_tick.in_set(TickInputSet::Feed));
    // Seed the run exactly as the recording game did.
    app.world_mut().resource_mut::<PendingSeed>().0 = Some(replay.seed);
    // First update: Time's first frame has zero delta and runs no fixed tick.
    app.update();
    app.world_mut().resource_mut::<NextState<GameState>>().set(GameState::Playing);
    app.update(); // applies the transition; OnEnter(Playing) runs begin_run
    if replay.origin != super::sim::SeedOrigin::Host {
        app.world_mut().resource_mut::<RunSeed>().origin = replay.origin;
    }
    let cap = replay.ticks.saturating_add(60);
    let ended = loop {
        app.update();
        let tick = app.world().resource::<SimTick>().0;
        if *app.world().resource::<State<GameState>>().get() != GameState::Playing {
            break Ended::GameOver;
        }
        if app.world().resource::<ReplayFeeder>().done {
            break Ended::InputExhausted;
        }
        if tick >= cap {
            break Ended::Cap;
        }
    };
    let score = u64::from(app.world().resource::<GameData>().score);
    let checksum = app.world().resource::<Checksum>().0;
    let ticks = app.world().resource::<SimTick>().0;
    Verdict {
        score,
        checksum,
        ticks,
        ended,
        matches: score == replay.score && checksum == replay.checksum,
    }
}
```

If `headless_app_runs_a_replay_to_a_verdict` reports `ticks == 0`, the fixed schedule did not run: check that `Time::<Fixed>` was inserted (Task 3) and that `MinimalPlugins` brought `TimePlugin`. `ticks` must equal `replay.ticks` exactly: the feeder flips `done` on the tick that consumes the last record and the loop checks `done` right after that update, so no extra empty tick runs. If it is off by one, the schedule order differs from `First → PreUpdate → StateTransition → RunFixedMainLoop → Update`; fix the loop, not the assertion.

- [ ] **Step 5: Run tests**

Run: `cargo test replay:: && cargo clippy --all-targets --all-features -- -D warnings`
Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add src/game/replay
git commit -m "feat(replay): feeder, headless app and verify()

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 8: `verify` bin: native CLI, `--selftest`, wasm export

Repo: template.

**Files:**
- Create: `src/bin/verify.rs`, `tests/fixtures/selftest.gxr`, `tests/selftest.rs`
- Modify: `Cargo.toml`

**Interfaces:**
- Consumes: `build_headless_app`, `verify`, `Replay`, `ReplayRecorder`, `GameState`, `PendingSeed`.
- Produces: `gamebient_game::game::replay::selftest::{record_scripted_run, SELFTEST_SEED, SELFTEST_TICKS}` (put in `src/game/replay/selftest.rs`, always compiled so `tests/` can use it); binary `verify` (native: `verify <file.gxr>`, `verify --selftest [--write <file>]`; wasm: `export function verify(bytes: Uint8Array): string`).

- [ ] **Step 1: Cargo**

```toml
[features]
autopilot = []
record = ["autopilot"]
# Replay verifier entry point (src/bin/verify.rs): native CLI and the
# wasm-bindgen module the site runs under Node. See docs/replay-verification.md.
verify = ["dep:wasm-bindgen"]

[[bin]]
name = "verify"
path = "src/bin/verify.rs"
required-features = ["verify"]

[target.'cfg(target_arch = "wasm32")'.dependencies]
getrandom = { version = "0.3", features = ["wasm_js"] }
wasm-bindgen = { version = "0.2.108", optional = true }
```

`dep:wasm-bindgen` under a non-target feature: Cargo allows an optional target-specific dep to be named by a feature; on native the feature enables nothing. Verify with `cargo build --features verify` on native.

- [ ] **Step 2: Write `src/game/replay/selftest.rs`** (add `pub mod selftest;` in `replay/mod.rs`)

```rust
//! A scripted headless run used as the determinism test: record it, replay
//! it, the verdicts must agree. Native `cargo test` runs it; the wasm CI
//! job replays the committed fixture it wrote.

use bevy::prelude::*;
use gamebient_input::{Buttons, VirtualInput};

use super::recorder::ReplayRecorder;
use super::{Replay, build_headless_app};
use crate::game::sim::{PendingSeed, SimTick};
use crate::game::states::GameState;

pub const SELFTEST_SEED: [u8; 32] = [0x5e; 32];
pub const SELFTEST_TICKS: u32 = 600;

/// Sweeps right for 2 s, left for 2 s, taps A every second. Written before
/// the accumulator folds the virtual source. Nothing here may write score or
/// state directly: only inputs, or the replay could not reproduce it.
fn script(tick: Res<SimTick>, mut virt: ResMut<VirtualInput>) {
    let t = tick.0;
    let held = if (t / 120) % 2 == 0 { Buttons::RIGHT } else { Buttons::LEFT };
    virt.set_held(held);
    if t % 60 == 0 {
        virt.latched |= Buttons::A;
    }
}

/// Records the scripted run and returns the sealed replay.
pub fn record_scripted_run() -> Replay {
    let mut app = build_headless_app();
    app.add_systems(PreUpdate, script.before(gamebient_input::input::accumulate_input));
    app.world_mut().resource_mut::<PendingSeed>().0 = Some(SELFTEST_SEED);
    app.update();
    app.world_mut().resource_mut::<NextState<GameState>>().set(GameState::Playing);
    app.update();
    while app.world().resource::<SimTick>().0 < SELFTEST_TICKS {
        app.update();
    }
    app.world_mut().resource_mut::<NextState<GameState>>().set(GameState::GameOver);
    app.update();
    app.world()
        .resource::<ReplayRecorder>()
        .last_run()
        .cloned()
        .expect("run sealed on OnExit(Playing)")
}
```

- [ ] **Step 3: Write the failing integration test** `tests/selftest.rs`

```rust
use gamebient_game::game::replay::selftest::{SELFTEST_TICKS, record_scripted_run};
use gamebient_game::game::replay::{Ended, Replay, verify};

#[test]
fn recorded_run_replays_to_the_same_verdict() {
    let replay = record_scripted_run();
    assert_eq!(replay.ticks, SELFTEST_TICKS);
    let v = verify(&replay);
    assert!(v.matches, "{v:?} vs claimed score {} checksum {}", replay.score, replay.checksum);
    assert_eq!(v.ended, Ended::InputExhausted);
    assert_eq!(v.ticks, SELFTEST_TICKS);
}

#[test]
fn tampered_inputs_do_not_verify() {
    // Flip LEFT on in the first run: the cube stops moving, the folded
    // transform differs, and the claimed checksum no longer reproduces.
    let mut replay = record_scripted_run();
    replay.runs[0].held ^= 4;
    assert!(!verify(&replay).matches);
}

#[test]
fn committed_fixture_still_verifies() {
    let bytes = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/selftest.gxr"))
        .expect("run `cargo run --features verify --bin verify -- --selftest --write tests/fixtures/selftest.gxr`");
    let replay = Replay::decode(&bytes).unwrap();
    let v = verify(&replay);
    assert!(v.matches, "sim changed: regenerate the fixture with --selftest --write (see docs/replay-verification.md)\n{v:?}");
}
```

Run: `cargo test --test selftest` — Expected: first test fails to compile until Step 2 is in; second fails on the missing fixture.

- [ ] **Step 4: Write `src/bin/verify.rs`**

```rust
//! Replay verifier. Native: `verify <file.gxr>` prints a verdict JSON and
//! exits 0 (matches) / 1 (mismatch) / 2 (decode error);
//! `verify --selftest [--write <file>]` records the scripted run, replays
//! it, and optionally writes the fixture. wasm (`--target nodejs`):
//! `verify(bytes)` returns the same JSON.

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use gamebient_game::game::replay::selftest::record_scripted_run;
    use gamebient_game::game::replay::{Replay, verify};
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--selftest") => {
            let replay = record_scripted_run();
            let v = verify(&replay);
            println!("{}", v.to_json());
            if let Some(i) = args.iter().position(|a| a == "--write") {
                let path = &args[i + 1];
                std::fs::write(path, replay.encode()).expect("write fixture");
                eprintln!("wrote {path} ({} bytes)", replay.encode().len());
            }
            std::process::exit(if v.matches { 0 } else { 1 });
        }
        Some(path) => {
            let bytes = std::fs::read(path).expect("read replay");
            match Replay::decode(&bytes) {
                Ok(r) => {
                    let v = verify(&r);
                    println!("{}", v.to_json());
                    std::process::exit(if v.matches { 0 } else { 1 });
                }
                Err(e) => {
                    eprintln!("decode error: {e:?}");
                    std::process::exit(2);
                }
            }
        }
        None => {
            eprintln!("usage: verify <file.gxr> | verify --selftest [--write <file>]");
            std::process::exit(2);
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn verify(bytes: &[u8]) -> String {
    use gamebient_game::game::replay::{Replay, verify};
    match Replay::decode(bytes) {
        Ok(r) => verify(&r).to_json(),
        Err(e) => format!("{{\"error\":\"{e:?}\"}}"),
    }
}
```

- [ ] **Step 5: Generate the fixture and run tests**

Run:

```bash
cargo run --features verify --bin verify -- --selftest --write tests/fixtures/selftest.gxr
cargo test --all-features
cargo clippy --all-targets --all-features -- -D warnings && cargo fmt --check
```

Expected: selftest prints `"matches":true`; all tests pass; the fixture is a few hundred bytes.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock src/bin/verify.rs src/game/replay tests
git commit -m "feat(verify): native CLI, --selftest determinism check, wasm export

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 9: wasm verifier build, Node runner, CI and release

Repo: template.

**Files:**
- Create: `tools/build_verify.sh`, `tools/verify_fixture.mjs`
- Modify: `.github/workflows/ci.yml` (build-web job), `.github/workflows/release.yml` (web matrix leg + artifact), `.gitignore`

**Interfaces:**
- Produces: `dist-verify/verify.js` + `dist-verify/verify_bg.wasm` (wasm-bindgen `nodejs` target); `node tools/verify_fixture.mjs <file.gxr>` prints the verdict JSON and exits 0 only when `matches` is true; release asset `gamebient-game-verify.zip`.

- [ ] **Step 1: `tools/build_verify.sh`**

```bash
#!/usr/bin/env bash
# Builds the replay verifier as a wasm-bindgen *nodejs* module into
# dist-verify/. Same target, profile and wasm-opt flags as build_web.sh so the
# sim's float behaviour is the shipped game's. Usage: tools/build_verify.sh
set -euo pipefail
cd "$(dirname "$0")/.."
rustup target add wasm32-unknown-unknown 2>/dev/null || true
cargo build --profile wasm-release --target wasm32-unknown-unknown \
    --bin verify --features verify
command -v wasm-bindgen >/dev/null || { echo "wasm-bindgen-cli missing (see install.sh)" >&2; exit 1; }
command -v wasm-opt >/dev/null || { echo "wasm-opt missing (binaryen)" >&2; exit 1; }
rm -rf dist-verify && mkdir -p dist-verify
wasm-bindgen --out-dir dist-verify --out-name verify --target nodejs \
    target/wasm32-unknown-unknown/wasm-release/verify.wasm
wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
    dist-verify/verify_bg.wasm -o dist-verify/verify_bg.wasm
echo "GX_BUILD_ID=$(strings dist-verify/verify_bg.wasm | grep -m1 -E '^[0-9]+\.[0-9]+\.[0-9]+\+' || true)" > dist-verify/BUILD
ls -la dist-verify
```

`chmod +x tools/build_verify.sh`.

- [ ] **Step 2: `tools/verify_fixture.mjs`**

```js
#!/usr/bin/env node
// Runs the wasm verifier on a .gxr file. Exit 0 only when matches === true.
// Usage: node tools/verify_fixture.mjs tests/fixtures/selftest.gxr
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { resolve } from "node:path";

const require = createRequire(import.meta.url);
const file = process.argv[2];
if (!file) {
  console.error("usage: verify_fixture.mjs <file.gxr>");
  process.exit(2);
}
const { verify } = require(resolve("dist-verify/verify.js"));
const t0 = performance.now();
const out = JSON.parse(verify(new Uint8Array(readFileSync(file))));
out.ms = Math.round(performance.now() - t0);
console.log(JSON.stringify(out));
process.exit(out.matches === true ? 0 : 1);
```

- [ ] **Step 3: Local check**

Run: `tools/build_verify.sh && node tools/verify_fixture.mjs tests/fixtures/selftest.gxr`
Expected: `{"score":...,"matches":true,"ms":<small>}`. If Node throws on `performance` or `crypto` at instantiation, Bevy's `TimePlugin`/`getrandom` are reaching for browser globals: Node 20+ has both; check `node --version`. If `wasm-bindgen` complains about `main`, the wasm `main` must stay an empty `fn main() {}` (Task 8).

- [ ] **Step 4: CI** — in `.github/workflows/ci.yml`, `build-web` job, after "Build web bundle":

```yaml
      - name: Build replay verifier (wasm, nodejs)
        run: bash tools/build_verify.sh

      - name: Verify the committed fixture under Node
        run: node tools/verify_fixture.mjs tests/fixtures/selftest.gxr
```

- [ ] **Step 5: Release** — in `.github/workflows/release.yml`, after "Zip web bundle":

```yaml
      - name: Build and zip replay verifier
        if: matrix.target == 'web'
        run: |
          bash tools/build_verify.sh
          (cd dist-verify && zip -r ../build/gamebient-game-verify.zip .)
```

and change the upload step's `path` to a list:

```yaml
          path: |
            build/gamebient-game-${{ matrix.target }}.*
            build/gamebient-game-verify.zip
```

(`if-no-files-found: error` still holds because the first pattern matches on every leg.)

- [ ] **Step 6: `.gitignore`** — add

```
# Replay verifier module — regenerated by tools/build_verify.sh
/dist-verify
# Local replays written when GX_REPLAY_DIR points here
/build/replays
```

- [ ] **Step 7: Commit**

```bash
git add tools/build_verify.sh tools/verify_fixture.mjs .github .gitignore
git commit -m "ci: build the wasm verifier, run the fixture under Node, publish verify.zip

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 10: Documentation

Repo: template.

**Files:**
- Create: `docs/replay-verification.md`
- Modify: `AGENTS.md`, `README.md`, `docs/conventions.md`, `init-game.sh`

- [ ] **Step 1: `docs/replay-verification.md`**

Write these sections, with the content below:

**What it is.** Two paragraphs: every run records seed + per-tick inputs into a `GXR1` replay and posts it to the host as `gx:event run`; the site re-simulates it with this game's `verify` module (same crate, `wasm32-unknown-unknown`, `MinimalPlugins`, under Node) and only a reproduced score is a leaderboard entry. A mismatch means `unverified`, never `cheat`.

**Format.** Copy the `GXR1` layout table from `src/game/replay/mod.rs`.

**Determinism rules for game code** (the migration rule from the spec, numbered): SimSet only; `TickInput` not `GameInput` in sim systems; `GameRng` only; no `std::collections::HashMap` in sim state; fold extra state into `Checksum` if a score can be reached by different states; no `Instant`/`SystemTime`/frame count in sim logic; the forbidden-names test in `sim.rs` enforces the greppable ones.

**Commands.**

```bash
cargo test --all-features                                   # codec, selftest, fixture
cargo run --features verify --bin verify -- --selftest --write tests/fixtures/selftest.gxr   # regenerate after a sim change
GX_REPLAY_DIR=build/replays cargo run                        # play; each run writes build/replays/<ms>.gxr
cargo run --features verify --bin verify -- build/replays/<ms>.gxr
tools/build_verify.sh && node tools/verify_fixture.mjs tests/fixtures/selftest.gxr
```

**Server contract.** Copy the four numbered points from the spec's "Server contract" verbatim, then the module API: `const { verify } = require("verify.js"); const json = verify(new Uint8Array(bytes));` → `{score, checksum, ticks, ended: "gameover"|"input_exhausted"|"cap", matches}` or `{error}`; cache the required module at module scope; the `build` header field selects which release's `gamebient-game-verify.zip` to load.

**Caveat.** Native and wasm agree for the template because its sim uses only basic IEEE ops. A game that calls `sin`/`cos`/`powf` in sim code may see the native `--selftest` and the Node fixture disagree by an ulp that snowballs. The shipped check is browser-vs-Node (both wasm) and is unaffected; in that case mark the CI Node step with `continue-on-error: true` and rely on `cargo test`'s native selftest for regressions.

- [ ] **Step 2: `AGENTS.md`** — add to "Architecture & conventions":

```markdown
- **Deterministic sim.** Run state changes only inside `sim::SimSet`
  (`FixedUpdate`, 60 Hz, chained). Sim systems read `TickInput`, draw
  randomness from `GameRng`, and never touch wall-clock or
  `std::collections::HashMap`. Each run records a replay the site
  re-simulates with `src/bin/verify.rs`; see `docs/replay-verification.md`.
```

Add to "How to add things": "**A gameplay system:** … register it in `GamePlugin` inside the `SimSet` chain (not `Update`) if it mutates run state; `player::move_player` is the model." Add a "Replay verification" bullet with the `--selftest`/fixture commands and the rule "regenerate `tests/fixtures/selftest.gxr` whenever the sim changes; `cargo test` tells you when."

Add to "Gotchas": "**`SmallRng` differs on wasm32** (xoshiro128 vs 256): the sim uses `rand_xoshiro::Xoshiro256PlusPlus` explicitly. **The wasm verifier must not call `App::run`**: Bevy's wasm runner wants `window.setTimeout`, which Node lacks; `verify()` steps `app.update()` itself."

- [ ] **Step 3: `README.md`** — a short "Replay verification" section pointing at the doc with the three commands. **`docs/conventions.md`** — a "Sim determinism" subsection repeating the numbered rules. **`init-game.sh`** — confirm the `[lib] name` and `use gamebient_game` substitutions from Task 3 are present.

- [ ] **Step 4: Final check and commit**

Run: `cargo test --all-features && cargo clippy --all-targets --all-features -- -D warnings && cargo fmt --check && bash build_web.sh && tools/build_verify.sh && node tools/verify_fixture.mjs tests/fixtures/selftest.gxr`
Expected: all green; `dist/` still serves the game (`cd dist && python3 -m http.server 8080`, play a run, and the browser console shows a `gx:event` with `"event":"run"` when the run ends — open devtools, filter on `gx:event`).

```bash
git add -A
git commit -m "docs: replay verification contract and sim determinism rules

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

## Self-review notes

- Spec coverage: fixed-tick input (T1), headless + protocol (T2), sim scaffolding + SimSet + pause in FixedUpdate + headless GamePlugin (T3), build id (T4), format (T5), recorder + `GX_REPLAY_DIR` + `Run` event (T6), feeder + `verify()` (T7), bin + selftest + wasm export (T8), build script + Node runner + CI fixture + release asset (T9), docs + server contract + migration rule (T10). Cabinet-signed scores and the website are out of scope by the spec.
- Spec updated alongside this plan: the recorder seals on `OnExit(Playing)` (quit-to-title runs also post); `TickInput` carries the quantized stick separately from the D-pad so the record is exact; the checksum folds the player transform so the selftest is input-sensitive.
- Type names used across tasks: `InputAccumulator`, `TickInput`, `TickInputSet::{Feed, Collect}`, `quantize_axis`/`dequantize_axis` (T1 → T6, T7); `HostCommand::Seed`, `HostEvent::Run` (T2 → T3, T6); `SimTick`, `SimSet`, `RunSeed`, `SeedOrigin`, `PendingSeed`, `GameRng`, `Checksum`, `tick_duration` (T3 → T5–T8); `Replay`, `TickRun`, `DecodeError` (T5 → T6–T8); `ReplayRecorder::{begin, push, seal, last_run}` (T6 → T8); `ReplayFeeder`, `feed_tick`, `Verdict`, `Ended`, `verify`, `build_headless_app` (T7 → T8, T9).
