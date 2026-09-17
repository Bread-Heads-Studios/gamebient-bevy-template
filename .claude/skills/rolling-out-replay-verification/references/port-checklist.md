# Determinism port checklist

The contract is `docs/replay-verification.md`'s ten determinism rules.
This is that contract applied: for each rule, what a game usually looks
like before the port, what the template's own code looks like after it (the
`after` shapes below are quoted verbatim from this template — they're what
`tools/rollout-replay.sh` ports in, so they're the same in every game), and
what to grep for. The game-specific `before`/`after` snippets are from the
Cannonball Putt port (the pilot, `games/cannonball-putt`, PR "deterministic
sim + replay verification") — a game with no `score` field, floats everywhere
in its physics, and a screenshot tour that writes run state. Read them as one
worked example, not a template: the shapes to copy are the rule's prose and
this template's own `after` blocks.

## Rule 1 — `SimSet` only

Everything that mutates run state moves from `Update` into the
`sim::SimSet` chain in `FixedUpdate`, in its existing order. Presentation
systems (particles, trails, aim arrows, HUD text) may stay in `Update`
**only if** they write nothing the sim reads and nothing `Checksum` folds —
see "What may stay in `Update`" below.

**Before** (a typical pre-port game, `Update`-gated, mutating run state) —
Cannonball Putt's whole gameplay tuple before the port, presentation and sim
chained together in one `Update` block:
```rust
.add_systems(
    Update,
    (
        // Order matters within the frame: input drives the shot,
        // physics moves the ball, flow reacts to the outcome, and
        // the visuals read the final state.
        shot::update_shot,
        ball::integrate,
        ball::animate_sink,
        hole_flow::update_hole_flow,
        shot::update_aim_arrow,
        hazards::advance_tilt_clock,
        hazards::animate_flow_particles,
        hazards::animate_tilt_indicators,
        ball::spawn_trail,
        ball::fade_trail,
    )
        .chain()
        .run_if(in_state(GameState::Playing).and(states::not_paused)),
)
.add_systems(OnExit(GameState::Playing), cleanup_game_entities)
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

### The two order traps

Rule 1 is usually read as "does this system write sim state?". Two ways of
breaking a replay pass that test and still desync, and both were found by
reviewing a wave-1 port rather than by any fixture:

**A system left in `Update` must not insert or remove components on, or
spawn or despawn, any entity a `SimSet` system queries.** Inserting a
presentation component onto a sim entity moves that entity to a different
archetype, and archetype order is the order a `Query` iterates in. The
windowed game performs the insert and iterates one order; the verifier adds
no render plugins, never performs it, and iterates another. Nothing in the
selftest catches this — nothing renders during a selftest — so the fixtures
stay green and only replays of real, rendered runs diverge.

Grand Theft Auto-Reply is the worked example. Its `src/assets/` visuals
decorate live `Email` and `Projectile` entities — the same entities
`inbox::tick_emails` and `combat::advance_projectiles` iterate inside
`SimSet`. Three of six seeds diverged under a two-ticks-per-frame schedule
while every fixture passed. The fixes, in order of preference:

1. Put the presentation on a **child** entity the sim never queries.
2. Move the insert into the chain, so both paths do it.
3. Make the sim system's iteration order not depend on the archetype —
   which is the second trap.

**Sorting discipline: any sim system that iterates a `Query` and accumulates
order-sensitively must sort by a stable per-entity key first.**
Order-sensitive means a running multiplier, pushing into a `Vec` the sim
later reads in order, a sequential float threshold, "the first entity within
range wins" — anything where swapping two entities changes the result. The
key must be something the *sim* assigns and both paths agree on: an inbox
slot, a spawn sequence number, a grid index. **Not `Entity`** — its value
depends on allocation order, which is exactly what is in question.

```rust
// before: whatever order the archetype happens to give
for email in &emails {
    combo *= email.multiplier;   // f32: not associative, so order IS the result
}

// after: a stable key the sim owns
let mut live: Vec<_> = emails.iter().collect();
live.sort_by_key(|email| email.slot);
for email in live {
    combo *= email.multiplier;
}
```

Note what that example is *not*: `*` is commutative, and it still matters,
because f32 multiplication is not **associative** — reordering the operands
moves the last bit, and the checksum folds bits. So "the operator is
commutative" is not the test; "would reordering the operands change the
value" is. Integer sums, maxes, counts and bitwise ORs genuinely do not care,
and neither does a system that only reads.

The per-entity folds in a `checksum_<game>` system are the most
order-sensitive code in the game by construction — a hash is order-dependent
by design — so those must **always** be sorted before folding, commutative
accumulator or not. That is rule 5's own advice; this clause is the rest of
the sim.

`tools/rollout-replay.sh` prints an **advisory** HAND EDIT for this, as
`<file>:<Component>` pairs: any file under `src/` that takes an entity out of
a query and calls `.insert(`/`.remove::<`/`.despawn(` on it, naming a
component the game declares *and* queries in `src/game/`. Three filters keep
it readable — only game-declared components (so `Transform` and `Sprite` never
appear), not a file that spawns the component itself ("decorate what I just
spawned" is benign: both paths do it), and minus whatever the same scan finds
in the template (every game inherits the same cleanup/pause/audio
boilerplate). On Grand Theft Auto-Reply:

```
src/assets/inbox_view.rs:Email  src/assets/projectiles.rs:Email
src/assets/projectiles.rs:Projectile  src/game/audio/mod.rs:RaidSiren
src/game/combat.rs:Email  src/game/combat.rs:Projectile
src/game/inbox.rs:Email
```

The first three are the real bug. The rest are sim code decorating its own
entities from inside the chain, which is fine — both paths do it — and is
what "advisory" means: it is a reading list, not a verdict. Expect false
negatives too (an entity reached through a resource rather than a query, or a
file that both spawns and decorates the same type). The thing that *answers*
the question is `tests/archetype_order.rs`.

### What the headless app does not have

`build_headless_app` is `MinimalPlugins` + `StatesPlugin` + `InputPlugin` and
nothing else — no `AssetPlugin`, no `AudioPlugin`, no render, no `UiPlugin`.
Any system the plugin still registers in headless mode that takes a `Res<T>`
from one of those fails parameter validation, and Bevy reports it as

```
Encountered an error in system `Enable the debug feature to see the name`:
Parameter `Enable the debug feature to see the name` failed validation:
Resource does not exist
```

with no system name in a release-ish test build, so budget for a bisect unless
you know the list. The three that bit the pilot:

| Resource | Owner | Fix |
|---|---|---|
| `GlobalVolume` | `AudioPlugin` | `host::apply_host_commands` takes `Option<ResMut<GlobalVolume>>` (the template already does; games predating that change do not) |
| the game's `GameAssets` | the game's `AssetsPlugin` | hole/level setup takes `Option<Res<GameAssets>>` and spawns only the entities the sim reads |
| `ScreenFade` | `UiPlugin` | `Option<Res<…>>` / `Option<ResMut<…>>` in `toggle_pause` and the game-over site |
| **messages**, not resources: `HostEvent`, `TickInput`, `TickFrame` | `GxInputPlugin` | see below — a harness that builds the sim plugin standalone must `add_message::<HostEvent>()` and init the tick-input resources, or use `build_headless_app` |

That last row is a different failure with the same nameless error, and it
bites test harnesses rather than the verifier. `replay::recorder::seal_run`
takes a `MessageWriter<HostEvent>`, and `HostEvent`'s message queue is
registered by `GxInputPlugin`. `build_headless_app` adds the whole
`GamePlugin`, so the verifier is fine — but a game whose own test harness
builds just its sim plugin (Dough.io's `SimCorePlugin`, a balance sweep, a
playtest) has no `GxInputPlugin`, and the game-over path dies with Bevy's

```
Parameter `Enable the debug feature to see the name` failed validation:
Message not initialized
```

`TickInput` and `TickFrame` are the same class: the sim reads them, the
input crate inits them. Either add them in the sim plugin —

```rust
app.add_message::<gamebient_input::HostEvent>()
    .init_resource::<gamebient_input::TickInput>()
    .init_resource::<gamebient_input::TickFrame>();
```

— or build harness apps through `replay::build_headless_app` so they get the
same shape the verifier does. Prefer the second where the harness can take it.

The second one is the interesting one: a game that builds its level out of
meshes has to split "spawn the thing the sim moves" from "spawn what it looks
like". Cannonball Putt's `build_hole` spawns the ball unconditionally and
returns early before the felt, walls, hazards, cup and aim arrow when there
are no assets — the ball is the only entity any `SimSet` system queries.

Also: adding `src/bin/verify.rs` gives the crate a second binary, so plain
`cargo run` (and `cargo run --features autopilot`) becomes ambiguous. Add
`default-run = "<kebab-name>"` under `[package]`.

## Rule 2 — `TickInput`, not `GameInput`, in sim systems

`GameInput` is the per-frame resource menus read; the sim reads the
once-per-tick `TickInput` the replay actually records. This is the swap
`rollout-replay.sh`'s "gameplay reads raw `ButtonInput<KeyCode>`" HAND EDIT
is warning about — the fix is `TickInput`, not the raw button map either.

**Before** — Cannonball Putt's `shot::update_shot`, the system that reads the
two-tap shot's press edges (`src/game/shot.rs`):
```rust
use super::input::GameInput;

/// Per-frame driver for the Intro/Aiming/Power phases.
pub fn update_shot(
    time: Res<Time>,
    input: Res<GameInput>,
    // ...
```
After the port the `use` line goes away entirely and the parameter reads
`input: Res<gamebient_input::TickInput>`; the body is untouched, because
`TickInput` derefs to the same `GameInput` fields (`primary_just_pressed`,
`secondary_just_pressed`, `move_x`). That is the whole of rule 2 in most
games — a parameter type, not a rewrite.

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

**The harness trap, and it is silent.** `sim::begin_run` **overwrites**
`GameRng` on every `OnEnter(Playing)`, reseeding it from `RunSeed`. So an
existing test harness or balance sweep that inserts its own seeded RNG
resource before entering `Playing` — the normal pre-port way to make a
playtest deterministic — has it thrown away, and the run proceeds on a
locally drawn seed. Nothing errors; the harness simply stops being
deterministic, and a CSV of "seeded" results quietly becomes noise. Dough.io
hit this.

Stage `sim::PendingSeed` instead and let `begin_run` do the reseeding,
exactly as a host-issued seed does. `sim::seed_bytes(u64) -> [u8; 32]`
(unit-tested in `sim.rs`) widens a harness/CLI `u64` seed to the 32 bytes the
resource takes, so `--seed 7` means the same run in the harness, the sweep
and the replay:

```rust
// before: undone by begin_run, silently
app.insert_resource(SimRng::seeded(seed));

// after
app.world_mut().resource_mut::<sim::PendingSeed>().0 = Some(sim::seed_bytes(seed));
```

Grep: the forbidden-names test already fails the build on
`rand::rng()`/`from_os_rng`/`thread_rng`/`SmallRng` inside `src/game/`
(`cargo test sim::tests::no_forbidden_randomness_or_hashmaps_in_game_code`);
run it after the port, don't just grep by hand. It cannot catch the harness
trap — that code is correct Rust doing the wrong thing — so check by hand
that every harness entry point stages `PendingSeed`.

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

**Before** — nothing. A pre-port game folds nothing, so all
`sim::checksum_tick` has to go on is the leaderboard score, and in Cannonball
Putt the same points total is reachable from wildly different rounds
(`max(0, 2·par − strokes) × 100` per hole, so a bogey on a par 4 and a par on
a par 3 both score 300). What the pilot added, in `src/game/ball.rs`,
registered straight after `sim::checksum_tick`:
```rust
pub fn checksum_golf(
    data: Res<GameData>,
    ball_q: Query<&Transform, With<Ball>>,
    mut sum: ResMut<super::sim::Checksum>,
) {
    if let Ok(tf) = ball_q.single() {
        sum.fold(u64::from(tf.translation.x.to_bits()));
        sum.fold(u64::from(tf.translation.y.to_bits()));
    }
    sum.fold(data.hole_index as u64);
    sum.fold(u64::from(data.strokes));
}
```
The ball's position is what makes the checksum sensitive to the physics
actually being reproduced; `hole_index` and `strokes` pin where in the round
the sim thinks it is.

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

The trait and its impl live in each game's **own** `src/game/scoring.rs` —
`rollout-replay.sh` does not copy that file — while `sim.rs`,
`replay/mod.rs`, `replay/recorder.rs` and `host.rs` all `use` it. A game
without one simply does not compile after the rollout.

The common shape (`pub score: u32` on `GameData`, no impl) the script now
writes for you, verbatim from the template:
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
Anything else is a HAND EDIT, because it is a judgement call about polarity
and units rather than a copy — games without a plain `score` field (golf
strokes, a race time, Hunted's survival timer) write their own, as in the
before/after below.

Once the impl exists, the plumbing is the same either way:
`sim::checksum_tick` folds `data.leaderboard_score()`;
`replay::recorder::seal_run` seals `u64::from(data.leaderboard_score())`;
`host::report_score`/`report_game_over` report `leaderboard_score()` too —
grep `grep -rn "data.score\b" src/game/sim.rs src/game/replay src/game/host.rs`
after the port; every remaining hit should read `leaderboard_score()`
except the HUD, which may keep showing the game's native unit (strokes,
time) as long as the number it reports to the host and folds into the
replay is the leaderboard integer.

**Before / after (game-specific scoring function)** — Cannonball Putt has no
`score`; its `GameData` carries strokes, which are lower-is-better and reset
per hole:
```rust
// before: the only round-wide number, and the wrong polarity for a leaderboard
impl GameData {
    /// Sum of recorded holes plus the in-progress hole.
    pub fn total_strokes(&self) -> u32 {
        self.results.iter().flatten().map(|&s| s as u32).sum::<u32>() + self.strokes as u32
    }
}
```
```rust
// after: golf points, higher is better, incomplete holes contribute nothing
impl LeaderboardScore for GameData {
    fn leaderboard_score(&self) -> u32 {
        self.results
            .iter()
            .zip(course::HOLES.iter())
            .filter_map(|(result, hole)| {
                result.map(|strokes| {
                    let par = u32::from(hole.par);
                    let strokes = u32::from(strokes);
                    (2 * par).saturating_sub(strokes) * 100
                })
            })
            .sum()
    }
}
```
`saturating_sub` is doing real work: it is the `max(0, …)` floor, and without
it a hole played past `2 × par` would wrap to a colossal score. The HUD still
shows strokes; only the host event, the replay seal and the checksum switch to
points.

## Rule 10 — end the run through `sim::end_run`

A run ends by calling `sim::end_run` from a sim system, never by a bare
`next_state.set(GameState::GameOver)` inside `FixedUpdate`. Two things have to
happen on that one tick, and `end_run` (copied verbatim into every game with
`sim.rs`) does both:

```rust
pub fn end_run(
    over: &mut RunOver,
    fade: Option<ResMut<ScreenFade>>,
    next: &mut NextState<GameState>,
) {
    over.0 = true;
    match fade {
        // One request; ScreenFade rejects re-requests while busy anyway.
        Some(mut fade) => {
            fade.request(GameState::GameOver);
        }
        None => next.set(GameState::GameOver),
    }
}
```

1. It latches `sim::RunOver`, the third clause of `SimSet`'s run condition, so
   this is the last tick the sim simulates, checksums and records.
2. It leaves `Playing`: through the fade when there is one (the windowed
   game), straight through `NextState` when there is not (the headless
   verifier, which has no UI at all). Take the fade as
   `Option<ResMut<ScreenFade>>` — the one signature that compiles in both.

Dough.io is the one game in the fleet that ends a run with a bare
`next_state.set(GameState::GameOver)` (`eating.rs`); see
`references/irregular-games.md`.

**After** — the game's own game-over site. Cannonball Putt's
`hole_flow::update_hole_flow`, with the pilot's hand-rolled version reduced to
the call:
```rust
if shot.round_over {
    data.finalize_round();
    sim::end_run(&mut over, fade, &mut next);
    return;
}
```
where the system takes `mut over: ResMut<sim::RunOver>`,
`fade: Option<ResMut<ScreenFade>>` and
`mut next: ResMut<NextState<GameState>>`.

### Why the latch: the fade tail

**Every game that ends a run this way needs this, and it is not obvious.** The
two paths above do not leave `Playing` at the same sim tick. Headless leaves
on the ending tick itself; the windowed game keeps ticking for the length of
the fade (`DEFAULT_FADE_SECS` = 0.4 s ≈ 24 ticks) because `ScreenFade::tick`
runs in `Update` on the *frame* delta. Those extra ticks are recorded, fold
into `Checksum`, and their count depends on the frame rate — so a real run
that reaches the game-over screen seals a checksum its own replay can never
reproduce, while the selftest (which leaves `Playing` from outside the sim)
passes happily. It is a mismatch you only see in step 5, on the exact path
players use.

The latch stops the sim on the ending tick in both modes. `sim.rs` declares
it and clears it in `begin_run`:
```rust
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RunOver(pub bool);

/// Run condition: true until the sim system that ends the run has latched
/// [`RunOver`].
pub fn run_not_over(over: Res<RunOver>) -> bool {
    !over.0
}
```
and `GamePlugin::build` puts it in the set's run condition — all three
clauses, in every game:
```rust
.configure_sets(
    FixedUpdate,
    sim::SimSet.run_if(
        in_state(GameState::Playing)
            .and(states::not_paused)
            .and(sim::run_not_over),
    ),
)
```
plus `.init_resource::<sim::RunOver>()`. The fade then plays over a frozen
sim — visually identical, since the run is already over — and both paths seal
the same tick count and the same checksum.

> **Ported before this landed?** Cannonball Putt (the pilot) declares its own
> `RunOver` / `run_not_over` in `src/game/states.rs` and latches the field by
> hand in `hole_flow.rs`, because `sim::end_run` did not exist yet. That still
> works and is not urgent to change — but don't copy it into a new port, and
> expect `states::run_not_over` (not `sim::run_not_over`) in that game's run
> condition.

Cover it with a unit test per path (`the_windowed_path_ends_the_run_through_the_fade`
/ `the_headless_path_ends_the_run_without_a_fade` in the pilot's
`hole_flow.rs`); a run that ends by input exhaustion, as the selftest does,
will not catch it. The template's own regression tests for the shape are
`sim::tests::end_run_latches_*` and
`replay::tests::a_run_ended_from_the_sim_seals_on_that_tick_despite_the_windowed_fade`.

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

**Game-specific script** — Cannonball Putt's golf bot
(`src/game/replay/selftest.rs`). The decision function is pure and unit-tested
on its own; the system's only job is to read what a player could see and write
`VirtualInput`:
```rust
pub const SELFTEST_TICKS: u32 = 3600; // 60 s: four of nine holes complete

/// What the bot presses on one tick, given only what a player can see.
/// Returns `(held, tapped)`; `tapped` is a one-frame latch.
pub fn bot_input(
    phase: ShotPhase,
    aim_angle: f32,
    cup_angle: f32,
    power: f32,
    strokes: u8,
) -> (Buttons, Buttons) {
    let i = usize::from(strokes) % AIM_OFFSETS.len();
    match phase {
        ShotPhase::Intro => (Buttons::NONE, Buttons::A), // skip the plaque
        ShotPhase::Aiming => {
            let delta = angle_delta(aim_angle, cup_angle + AIM_OFFSETS[i]);
            if delta.abs() > AIM_TOLERANCE {
                // Right sweeps clockwise (decreasing angle).
                let held = if delta > 0.0 { Buttons::LEFT } else { Buttons::RIGHT };
                (held, Buttons::NONE)
            } else {
                (Buttons::NONE, Buttons::A) // lock the aim
            }
        }
        // The meter rises from zero: fire on its first crossing.
        ShotPhase::Power if power >= POWER_TARGETS[i] => (Buttons::NONE, Buttons::A),
        ShotPhase::Power | ShotPhase::Rolling | ShotPhase::Sunk => (Buttons::NONE, Buttons::NONE),
    }
}

fn script(
    shot: Res<Shot>,
    data: Res<GameData>,
    ball_q: Query<&Transform, With<Ball>>,
    mut virt: ResMut<VirtualInput>,
) {
    let Some(def) = HOLES.get(data.hole_index) else { return };
    let ball = ball_q.single().map(|tf| tf.translation.truncate()).unwrap_or(def.tee);
    let cup_angle = (def.cup - ball).to_angle();
    let (held, tapped) = bot_input(shot.phase, shot.aim_angle, cup_angle, shot.power, data.strokes);
    virt.set_held(held);
    virt.latched |= tapped;
}
```
Three things worth copying.

**Vary the plan per attempt.** `AIM_OFFSETS` and `POWER_TARGETS` are indexed
by the stroke number, so a hole the bot can't sink gets eight different shots
instead of the same one eight times — that is what moves `hole_index`,
`strokes` and the score far enough for the checksum to have something to
catch.

**Read state, never write it.** The script may look at `Shot` and `GameData`
the way a player looks at the screen; the moment it assigns to one, the replay
stops reproducing. That is exactly why a game's autopilot/screenshot tour is
not a selftest — Cannonball Putt's writes `shot.aim_angle`, `data.hole_index`
and `shot.celebrate(…)` directly to fit a 90 s screenshot budget.

**Pick `SELFTEST_TICKS` so the run does not finish.** `record_scripted_run`
loops `while SimTick < SELFTEST_TICKS`, and a run that ends early stops
advancing the tick and hangs that loop for ever.

Check what the script actually reached before committing the fixture — a
throwaway integration test that runs `build_verify_app`/`run_verify_app` and
prints `GameData` is enough:
```
verdict Verdict { score: 200, checksum: 5538231768564850416, ticks: 3600,
                  ended: InputExhausted, matches: true }
hole_index 3 strokes 8 results [Some(2), Some(8), Some(8), Some(8), None, ...]
```
A verdict that matches with `score: 0` and `hole_index: 0` is a bot that never
scored, and a checksum that proves nothing.

### Verify the tamper test fails for the right reason

`tests/selftest.rs`'s `tampered_inputs_do_not_verify` is copied in, and a
test that passes vacuously is worse than no test. The template's version
tampers with the **back half** of the runs (strip every `latched` bit, flip
the D-pad bits in `held`), asserts the untampered replay verifies first, and
asserts the re-simulated **checksum differs** from the claimed one — not
just `matches == false`, which a decode error or a score-only difference
would also produce.

The reason it is shaped that way: the original flipped one bit in
`runs[0]`, which assumes tick-1 input moves the sim. Any game that opens on
an intro card, a title plaque or a countdown ignores tick-1 input entirely,
so the tampered replay reproduced the claimed checksum and the test reported
"tampering detected" when it had detected nothing.

After the port, prove it on this game:

```bash
cargo test --all-features tampered_inputs_do_not_verify -- --nocapture
```
then break it on purpose — narrow the tamper to `runs[0].held ^= 4` — and
confirm it now **fails** for your game. If the one-bit version still passes,
the back-half version is the one doing the work and you have just learned
that your game's opening ticks are inert. If neither fails, the selftest
script is not exercising anything the checksum folds; fix the script, not
the test.

## The autopilot and the accumulator

Two facts about `VirtualInput` that only start to matter once the sim reads
`TickInput`. Both cost a wave-1 port a debugging session.

**Order the bot before `accumulate_input`, not before `collect_input`.**
`gamebient-input` registers `accumulate_input.before(collect_input)`, so
ordering a bot only against `collect_input` leaves the bot and the
accumulator unordered relative to each other and Bevy's schedule builder
picks. A tap written after the accumulator has run is folded into the
per-frame `GameInput` and never reaches a fixed tick — so the bot presses
buttons the sim, and therefore the replay, never sees. `rollout-replay.sh`
rewrites this in a game's `src/game/autopilot.rs` automatically; check any
other system that writes `VirtualInput` (a `--sim` harness bot, a demo
attract mode) by hand:

```rust
// before — insufficient once the sim reads TickInput
drive_autopilot.before(gamebient_input::input::collect_input)
// after
drive_autopilot.before(gamebient_input::input::accumulate_input)
```

**A one-frame direction hold can miss a tick entirely.**
`TickInput.move_x`/`move_y` derive from the accumulator's *held* set, which
`accumulate_input` **overwrites every frame**; only press edges ride the
`latched` bits through to the next tick. So a bot that holds a direction for
exactly one frame lands on a tick only when that frame happens to contain
one — on a 120 Hz display roughly half its steps vanish, and the failure is
silent and frame-rate-dependent. Grand Theft Auto-Reply's first autopilot
tour after the port quietly lost its `06-mission-passed` beat to this.

The pattern: re-emit the direction for at least two frames (a `MOVE_SECS`
cooldown works, and still reads as a single step as long as it stays inside
the 0.32 s auto-repeat delay). Taps are unaffected — that is what `latched`
is for.

## What may stay in `Update`

A system may stay in `Update`, ungated by `SimSet`, only if it writes
nothing the sim reads and nothing `Checksum` folds — pure presentation:
particle trails, an aim-arrow gizmo, HUD text, camera shake. Read the
system before trusting this: if it writes a component the sim also reads
(e.g. a `Transform` the physics also moves) or a resource `checksum_tick`
or the game's own checksum system folds, it must move into the chain.

**The test that proves a system is *value*-safe:** remove it entirely
(comment it out) and re-run
`cargo run --features verify --bin verify -- --selftest`. If the printed
checksum is unchanged, the system's output isn't part of what the replay
reproduces. If the checksum changes, the system was folding into game state
after all and belongs in the chain.

**That test proves value-safety only, and CANNOT detect the archetype
trap.** Deleting a decorator changes nothing about a headless checksum,
because there is no decoration in a headless run — the selftest never
renders, so the system under test never ran in the first place. A decorator
can pass this check perfectly and still desync every rendered run, by moving
the entities it touches into a different archetype (see "The two order traps"
above). It is exactly the check Grand Theft Auto-Reply's port passed.

**The test that does detect it:** `tests/archetype_order.rs`, copied into
every game by the rollout script. It records the selftest script twice — once
in a plain headless app, once in a headless app that also runs `Update`
systems inserting inert markers onto the entities the sim queries — and
asserts the two agree on score, checksum and ticks, and that the decorated
recording still `verify()`s in a plain app (which is the production question:
the browser records decorated, Node verifies bare). Repeated over several
seeds at 1, 2 and 3 sim ticks per frame, because at one tick per frame the
`Update` decoration and the `FixedUpdate` sim interleave one-to-one and a
reorder can stay hidden.

**Extend its markers to your game's real decorators during the port** — as
shipped it is a smoke test, since the template's sim queries one entity and
one entity has no order to get wrong. Two things matter when you do:

* **Reproduce the branching.** A decorator that attaches a different
  component set to different entities (golden crumbs get a sparkle, rivals
  get a mood) is what splits one archetype into several. Markers that are
  identical for every entity cannot reproduce the bug.
* **Keep the child spawn.** `ChildOf` puts `Children` on the *parent*, so a
  decorator that only spawns children still moves its parent's archetype.

The file's own doc comment carries both, plus how to extend `trace_order` for
the queries your order-sensitive sim systems iterate.

**Before / after (the pilot's specific Update-vs-SimSet split)** — Cannonball
Putt's ten chained `Update` systems split six/four. The six that moved into
`SimSet`, with the scaffolding around them:
```rust
.add_systems(
    FixedUpdate,
    (
        sim::advance_tick,
        shot::update_shot,           // two-tap state machine, power meter
        ball::integrate,             // physics, hazards, cup capture
        ball::animate_sink,          // writes the ball Transform -> folded
        hole_flow::update_hole_flow, // hole advance, round end
        hazards::advance_tilt_clock, // the deck-tilt clock integrate reads
        sim::checksum_tick,
        ball::checksum_golf,
        replay::recorder::record_tick,
        sim::remember_sim_prev,
    )
        .chain()
        .in_set(sim::SimSet),
)
```
and the four that stayed, now registered only in the windowed branch:
```rust
// Presentation only: these read `Shot`/`Ball`/`TiltClock` and write nothing
// the sim reads or the checksum folds, so they keep the frame delta.
.add_systems(
    Update,
    (
        shot::update_aim_arrow,           // AimArrow/ArrowShaft/ArrowTip transforms
        hazards::animate_flow_particles,  // FlowParticle transforms
        hazards::animate_tilt_indicators, // TiltIndicator transforms
        ball::spawn_trail,                // spawns TrailDot entities
        ball::fade_trail,                 // TrailDot life + scale
    )
        .run_if(in_state(GameState::Playing).and(states::not_paused)),
)
```
The judgement call that is easy to get wrong is `ball::animate_sink`. It is
named like an animation and reads like presentation, but it writes the
**ball's own** `Transform` during the sink celebration — the exact component
`checksum_golf` folds — so it belongs in the chain. The five that stayed write
transforms too, but only on entities of their own (`AimArrow`, `FlowParticle`,
`TiltIndicator`, `TrailDot`) that nothing in the chain queries. Read the query
filters, not the system name.

## Native vs Node mismatch: telling ulp drift from a real bug

Step 5 of the skill compares the native verifier against the Node one, and
`docs/replay-verification.md` ("Two fixtures, and which one is the gate")
says a game whose sim calls `sin`/`cos`/`powf` may see them disagree because
native libm and wasm are not required to round transcendental functions
identically. CI already treats that comparison as informational, so nothing
turns red on its own — which makes it a very convenient excuse. Prove which
one you are looking at before you write "it's just libm" in a PR, because
the same symptom is what a genuine desync looks like from a distance.

**Read the verdicts first.** Ulp drift shows up as *identical* `score` and
`ticks` with a different `checksum` — the sim played exactly the same game,
and only the folded float bits moved. Cannonball Putt's fixture:

```
native  {"score":200,"checksum":"5538231768564850416","ticks":3600,…,"matches":true}
node    {"score":200,"checksum":"6557314733352226416","ticks":3600,…,"matches":false}
```

and its 6720-tick real run likewise agreed on `score: 3000` and on every
tick. A system left in `Update`, a stray `rand::rng()` or a `HashMap`
iteration does not behave like that: it desynchronises the *run*, so the
score and usually the tick count move too.

**Then bisect the real replay by truncated prefix.** The question to answer
is *where* the two sides part company, and the cheapest instrument is the
failing replay itself, cut short. Decode it, keep only the first N ticks of
its runs, fix the header's `ticks` to match (the claimed `score` and
`checksum` are now meaningless and both verifiers will say `matches: false` —
ignore that and read the **printed `checksum`**), then run both verifiers on
the truncated file and compare. Binary-search N for the first tick at which
the two checksums differ.

Save this as `tools/truncate_gxr.py` for the duration of the hunt:

```python
import struct, sys
src, dst, n = sys.argv[1], sys.argv[2], int(sys.argv[3])
b = bytearray(open(src, "rb").read())
hdr = 4 + 1 + b[4] + 2 + 32 + 1        # magic, build, tick_hz, seed, origin -> `ticks`
runs_at = hdr + 4 + 8 + 8              # ticks, score, checksum -> run count
(nruns,) = struct.unpack_from("<I", b, runs_at)
out, left = bytearray(), n
for r in range(nruns):
    if left == 0:
        break
    o = runs_at + 4 + r * 8            # held u16, latched u16, ax i8, ay i8, count u16
    take = min(struct.unpack_from("<H", b, o + 6)[0], left)
    left -= take
    out += b[o:o + 6] + struct.pack("<H", take)
assert left == 0, f"replay has fewer than {n} ticks"
struct.pack_into("<I", b, hdr, n)      # header `ticks` must equal the run-count sum
open(dst, "wb").write(bytes(b[:runs_at]) + struct.pack("<I", len(out) // 8) + bytes(out))
```
(the layout it walks is the `GXR1` table in `docs/replay-verification.md`.)

```bash
N=1800
python3 tools/truncate_gxr.py build/replays/<ms>.gxr /tmp/p.gxr $N
cargo run -q --features verify --bin verify -- /tmp/p.gxr   # native checksum
node tools/verify_fixture.mjs /tmp/p.gxr                    # wasm checksum
```

Read only the `checksum` field, and halve or double N until you have the
first N where they disagree.

What ulp drift looks like under this bisect: the two sides stay
**bit-identical for a long stretch**, often many hundreds of ticks *after*
the first `sin`/`cos`/`powf` call, and then diverge at one specific tick —
because libm implementations agree on most arguments and differ on a few. So
"the first transcendental runs at tick 40 and the checksums first differ at
tick 1173" is the expected shape, not a contradiction. What a real bug looks
like: the divergence point tracks something structural (the first RNG draw,
the first spawn, the tick a `HashMap` is iterated) and the run's `score` or
`ticks` move with it.

Having the exact tick also tells you what to read. Diff the folded state at
that tick — add a temporary `eprintln!` in the game's own `checksum_<game>`
system for `tick == N` and run both verifiers again — and you will see which
value moved and, usually, which call produced it.

When the bisect confirms drift, no workflow change is needed — the
native-vs-Node step (`Cross-check the native fixture under Node`) is already
`continue-on-error: true` — but write the measurement down: the two
checksums, the agreeing `score` and `ticks`, the first tick at which they
part, and the call site responsible. The next person should not have to redo
the bisect.

**Write it in the game's own `docs/replay-notes.md`, never in
`docs/replay-verification.md`.** That second file is copied verbatim from the
template into every game, and `--upgrade` refreshes it *only while it is
still byte-identical to a committed template version*. Appending a per-game
section to it permanently converts it to "locally modified": the game stops
receiving fleet-wide contract changes automatically and gets a HAND EDIT
about it on every future upgrade instead. Dough.io did exactly this and
called it out in its own PR.

So: `docs/replay-notes.md`, a new file the game owns outright, with a pointer
to it from the PR body. One line in the game's README or its
`docs/conventions.md` naming the file is enough for discoverability.

What does **not** get an excuse is `Verify the wasm-recorded fixture under
Node`. Both sides of that one are wasm arithmetic, which is
spec-deterministic, so a failure there is a real determinism regression no
matter how many transcendentals the sim calls.
