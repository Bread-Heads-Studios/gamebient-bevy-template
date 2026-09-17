# Determinism port checklist

The contract is `docs/replay-verification.md`'s nine determinism rules.
This is that contract applied: for each rule, what a game usually looks
like before the port, what the template's own code looks like after it (the
`after` shapes below are quoted verbatim from this template — they're what
`tools/rollout-replay.sh` ports in, so they're the same in every game), and
what to grep for. Snippets marked `<!-- from pilot -->` are filled in from
the Cannonball Putt port; until then, treat the rule's prose and the
template's own `after` shape as the guidance.

## Rule 1 — `SimSet` only

Everything that mutates run state moves from `Update` into the
`sim::SimSet` chain in `FixedUpdate`, in its existing order. Presentation
systems (particles, trails, aim arrows, HUD text) may stay in `Update`
**only if** they write nothing the sim reads and nothing `Checksum` folds —
see "What may stay in `Update`" below.

**Before** (a typical pre-port game, `Update`-gated, mutating run state):
```rust
<!-- from pilot -->
```

**After** — the template's own `GamePlugin::build` (`src/game/mod.rs`),
the chain shape every game's own gameplay tuple gets folded into:
```rust
.configure_sets(
    FixedUpdate,
    sim::SimSet.run_if(in_state(GameState::Playing).and(states::not_paused)),
)
.add_systems(
    OnEnter(GameState::Playing),
    (
        reset_paused,
        reset_game_data,
        sim::begin_run,
        replay::recorder::begin_recording,
        player::spawn_player, // <- the game's own OnEnter(Playing) setup goes here
    )
        .chain(),
)
.add_systems(
    FixedUpdate,
    toggle_pause
        .run_if(in_state(GameState::Playing))
        .before(sim::SimSet),
)
.add_systems(
    FixedUpdate,
    (
        sim::advance_tick,
        player::move_player,          // <- the game's own gameplay systems, in their
        scoring::handle_score_events, //    existing order, go here
        sim::checksum_tick,
        player::checksum_player,      // <- the game's own checksum_<game> system
        replay::recorder::record_tick,
        sim::remember_sim_prev,
    )
        .chain()
        .in_set(sim::SimSet),
)
.add_systems(
    FixedUpdate,
    sim::restore_tick_frame_while_paused
        .after(sim::SimSet)
        .run_if(in_state(GameState::Playing).and(|p: Res<states::Paused>| p.0)),
)
.add_systems(
    OnExit(GameState::Playing),
    (replay::recorder::seal_run, cleanup_game_entities).chain(),
);
```
The order inside the chained tuple is: `sim::advance_tick` first, the
game's own gameplay systems in their pre-port order, `sim::checksum_tick`,
then the game's own `checksum_<game>` system, then
`replay::recorder::record_tick`, then `sim::remember_sim_prev` last. Every
system in that tuple takes `Res<Time>` (the fixed 60 Hz delta, not the
frame delta) if it needs one.

Grep: `grep -n 'add_systems(Update' src/game/mod.rs` — anything that
mutates a resource `checksum_tick` or a later system reads should be in the
list above, not here.

## Rule 2 — `TickInput`, not `GameInput`, in sim systems

`GameInput` is the per-frame resource menus read; the sim reads the
once-per-tick `TickInput` the replay actually records. This is the swap
`rollout-replay.sh`'s "gameplay reads raw `ButtonInput<KeyCode>`" HAND EDIT
is warning about — the fix is `TickInput`, not the raw button map either.

**Before:**
```rust
<!-- from pilot -->
```

**After** — the template's `player::move_player` (`src/game/player.rs`),
the canonical "sim system reads `TickInput`" example:
```rust
pub fn move_player(
    time: Res<Time>,
    input: Res<gamebient_input::TickInput>,
    mut query: Query<&mut Transform, With<Player>>,
) {
    let Ok(mut tf) = query.single_mut() else {
        return;
    };
    let dir = Vec2::new(input.move_x, input.move_y);
    let delta = dir.normalize_or_zero() * PLAYER_SPEED * time.delta_secs();
    tf.translation.x += delta.x;
    tf.translation.y += delta.y;
}
```
Menu/UI code (`src/ui/*.rs`, anything gated on `GameState::Menu` or
`HowToPlay`) keeps reading `GameInput` — only `SimSet` systems switch.
Never read `input.pause_just_pressed` from a sim system (rule 8 below).

Grep: `grep -rn 'GameInput' src/game/*.rs | grep -v input.rs` — every hit
outside menu/pause code is a candidate to move to `TickInput`.

## Rule 3 — `GameRng` only

All randomness in sim code draws from `Res<GameRng>`
(`Xoshiro256PlusPlus`, reseeded from `RunSeed` in `sim::begin_run`) —
nothing else. A game with its own seeded resource (Dive Rise, Dough.io,
Grand Theft Auto-Reply, Grand Theft Otto, Gulper, Voidrunner all already
have one, see `references/catalog-2026-09-16.md`) renames or wraps it so
`sim::begin_run` reseeds it from `RunSeed`; presentation-only RNG (e.g.
Dough.io's particle `FxRng`) stays separate and unseeded.

**Before** — the shape the forbidden-names test in `src/game/sim.rs`
(`cargo test`) catches, e.g. Voidrunner's `space.rs`:
```rust
let mut rng = rand::rng();
let count = rng.random_range(min_count..=max_count);
```

**After** — draw from the sim's own `GameRng`:
```rust
let count = rng.0.random_range(min_count..=max_count);
```
where the system takes `mut rng: ResMut<sim::GameRng>` (or the game's
renamed wrapper around it) instead of calling `rand::rng()` locally. A
game with a `StdRng`/`SmallRng` seeded once at startup (Attic Excavator's
board seed, Beat Bender's chart generator) needs its seed itself to derive
from `RunSeed` or from a draw off `GameRng`, not from `rand::rng()` or
`from_os_rng()` — see `references/irregular-games.md` for Attic Excavator's
specific case.

Grep: the forbidden-names test already fails the build on
`rand::rng()`/`from_os_rng`/`thread_rng`/`SmallRng` inside `src/game/`
(`cargo test sim::tests::no_forbidden_randomness_or_hashmaps_in_game_code`);
run it after the port, don't just grep by hand.

## Rule 4 — no `std::collections::HashMap` in sim state

Iteration order isn't deterministic across builds. Use
`bevy::platform::collections::HashMap` or `BTreeMap` in anything the sim
iterates. The survey found no game iterating a `HashMap`/`HashSet` in
gameplay (lookup-only uses are fine — see
`references/catalog-2026-09-16.md`'s HashMap/Set column), so this is
usually a no-op; the forbidden-names test (rule 3's grep) covers the
literal `std::collections::HashMap` name too.

## Rule 5 — fold extra state into `Checksum`

Whenever a score could be reached through different in-game states, fold
that state into `Checksum` too — it's what catches a replay that
reproduces the score by accident rather than by faithfully replaying the
inputs. Register the game's own system directly after `sim::checksum_tick`
in the chain (rule 1's `after` block).

**Before:**
```rust
<!-- from pilot -->
```

**After** — the template's `player::checksum_player`
(`src/game/player.rs`), the model for a game's `checksum_<game>` system:
```rust
pub fn checksum_player(
    data: Res<GameData>,
    player: Query<&Transform, With<Player>>,
    mut sum: ResMut<Checksum>,
) {
    sum.fold(u64::from(data.lives));
    if let Ok(tf) = player.single() {
        sum.fold(u64::from(tf.translation.x.to_bits()));
        sum.fold(u64::from(tf.translation.y.to_bits()));
    }
}
```
Fold float state with `.to_bits()` (bit-exact, not `as u64`) so the
checksum is sensitive to float behaviour — that's what the native-vs-wasm
fixture check in step 5 of the skill relies on catching. Fold in a fixed
order every tick; `Checksum::fold` is order-sensitive by design
(`sim.rs`'s `checksum_folds_order_sensitively` test).

## Rule 6 — `sim::checksum_tick` folds only score and tick

It's one of the files the rollout script copies verbatim, so it must stay
game-agnostic — never edit it. It already folds
`data.leaderboard_score()` and the tick:
```rust
pub fn checksum_tick(data: Res<GameData>, tick: Res<SimTick>, mut sum: ResMut<Checksum>) {
    sum.fold(u64::from(data.leaderboard_score()));
    sum.fold(u64::from(tick.0));
}
```
Everything game-specific goes in the game's own system from rule 5,
registered immediately after this one.

## Rule 7 — no wall-clock reads in sim logic

No `Instant::now`, `SystemTime`, or frame-count reads inside `SimSet` —
they aren't reproducible under the verifier's
`TimeUpdateStrategy::ManualDuration`. The survey found none in any game's
gameplay code (`src/` excluding `ui/`, `record/`, `autopilot.rs`, tests,
dev harnesses), so this is usually already satisfied; the games with large
`Timer`/`delta_secs`/`elapsed_secs` counts in the catalog use Bevy's
`Time`/`Timer` API, which is fine — `Res<Time>` inside `FixedUpdate` is the
fixed delta, not wall-clock.

## Rule 8 — never read `pause_just_pressed` in a sim system

The recorder masks `Buttons::PAUSE` out of every recorded tick, so it's the
one bit a replay cannot carry — reading it inside `SimSet` would make a
live run and its own replay disagree the instant either one pauses. Pause
handling belongs in a system that runs **before** `SimSet`, exactly like
the template's `toggle_pause`:
```rust
fn toggle_pause(
    input: Res<gamebient_input::TickInput>,
    mut paused: ResMut<states::Paused>,
    fade: Option<Res<crate::ui::transition::ScreenFade>>,
    mut sfx: MessageWriter<audio::SfxEvent>,
) {
    if fade.as_ref().is_some_and(|f| !f.is_idle()) || !input.pause_just_pressed {
        return;
    }
    paused.0 = !paused.0;
    sfx.write(audio::SfxEvent::Pause);
}
```
registered `.before(sim::SimSet)`, not inside the chained tuple.

## Rule 9 — `LeaderboardScore`, not `GameData.score`

Games without a plain `score` field (golf strokes, a race time, Hunted's
survival timer) implement the trait instead of relying on the blanket impl:
```rust
pub trait LeaderboardScore {
    fn leaderboard_score(&self) -> u32;
}

impl LeaderboardScore for GameData {
    fn leaderboard_score(&self) -> u32 {
        self.score
    }
}
```
`sim::checksum_tick` folds `data.leaderboard_score()`;
`replay::recorder::seal_run` seals `u64::from(data.leaderboard_score())`;
`host::report_score`/`report_game_over` report `leaderboard_score()` too —
grep `grep -rn "data.score\b" src/game/sim.rs src/game/replay src/game/host.rs`
after the port; every remaining hit should read `leaderboard_score()`
except the HUD, which may keep showing the game's native unit (strokes,
time) as long as the number it reports to the host and folds into the
replay is the leaderboard integer.

**Before / after (game-specific scoring function):**
```rust
<!-- from pilot -->
```

## Run end always through `ScreenFade::request`

Not one of the nine numbered rules, but load-bearing: a run ends by calling
`ScreenFade::request(GameState::GameOver)` from a sim system, never
`next_state.set(GameState::GameOver)` directly inside `FixedUpdate` — a
direct `NextState::set` from inside `SimSet` changes state mid-chain, which
the replay's headless re-simulation (driven by the same chain) can't
reproduce in lockstep. Dough.io is the one game in the fleet that does this
today (`eating.rs`, `next_state.set(GameState::GameOver)`); see
`references/irregular-games.md`.

**After** — the template's own game-over sites all read the same way, e.g.
`hole_flow.rs`'s pattern in Cannonball Putt or this template's own
`sim::begin_run`/`OnExit` shape: request the fade, let the fade's own
`OnEnter`/`OnExit` transition drive `NextState` once the fade completes.

## The selftest script skeleton

`src/game/replay/selftest.rs` is copied verbatim by the rollout script;
only its `script` system is game-specific. The template's own version —
the skeleton every game's selftest starts from:
```rust
pub const SELFTEST_SEED: [u8; 32] = [0x5e; 32];
pub const SELFTEST_TICKS: u32 = 600;

/// Sweeps right for 2 s, left for 2 s, taps A every second. Written before
/// the accumulator folds the virtual source. Nothing here may write score or
/// state directly: only inputs, or the replay could not reproduce it.
fn script(tick: Res<SimTick>, mut virt: ResMut<VirtualInput>) {
    let t = tick.0;
    let held = if (t / 120).is_multiple_of(2) {
        Buttons::RIGHT
    } else {
        Buttons::LEFT
    };
    virt.set_held(held);
    if t.is_multiple_of(60) {
        virt.latched |= Buttons::A;
    }
}
```
`record_scripted_run` (unchanged, copied verbatim) drives `PendingSeed`,
enters `Playing`, runs `SELFTEST_TICKS` ticks, then transitions to
`GameOver` and returns the sealed replay. A game's own `script` must drive
enough of the game's real controls (not just movement — steering, shooting,
placing, whatever the game's `TickInput` fields are) that the checksum
would visibly differ if a ported system regressed. ≥ 600 ticks (10 s at
60 Hz); longer if the game's first scoring event doesn't happen that fast.

**Game-specific script:**
```rust
<!-- from pilot -->
```

## What may stay in `Update`

A system may stay in `Update`, ungated by `SimSet`, only if it writes
nothing the sim reads and nothing `Checksum` folds — pure presentation:
particle trails, an aim-arrow gizmo, HUD text, camera shake. Read the
system before trusting this: if it writes a component the sim also reads
(e.g. a `Transform` the physics also moves) or a resource `checksum_tick`
or the game's own checksum system folds, it must move into the chain.

**The test that proves a system is safe to leave in `Update`:** remove it
entirely (comment it out) and re-run
`cargo run --features verify --bin verify -- --selftest`. If the printed
checksum is unchanged, the system's output isn't part of what the replay
reproduces and it's safe where it is. If the checksum changes, the system
was folding into game state after all and belongs in the chain.

**Before / after (the pilot's specific Update-vs-SimSet split):**
```rust
<!-- from pilot -->
```
