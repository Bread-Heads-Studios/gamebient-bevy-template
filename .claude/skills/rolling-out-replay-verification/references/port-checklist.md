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

**First, pin the fixed timestep.** Bevy's default `Time<Fixed>` is 64 Hz,
`sim::tick_duration()` is 60, and `Replay::decode` **rejects** a header whose
tick rate is not the sim's — `ReplayError::BadTickRate`. A game that skips
this line records runs that no verifier can decode, and the failure surfaces
as a decode error in the verifier rather than anywhere near the missing line.
It is one line in `GamePlugin::build`, next to the `init_resource` run:
```rust
.insert_resource(Time::<Fixed>::from_duration(sim::tick_duration()))
```
Two of wave 2's three ports (Grand Theft Auto-Reply, Pack The Ripper) lost
time to this as an undiagnosed hand edit, so `tools/rollout-replay.sh` now
prints a HAND EDIT when neither `src/game/mod.rs` nor `src/main.rs` mentions
`Time::<Fixed>`.

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

### Two Bevy limits a mid-sized game's chain will hit

Both produce compiler errors that say nothing useful, so recognise them by
shape rather than by reading the diagnostic.

* **16 system parameters.** Rule 10 adds three to whichever system ends the
  run (`ResMut<RunOver>`, `Option<ResMut<ScreenFade>>`,
  `ResMut<NextState<GameState>>`), and that is often the system that was
  already the busiest. Sundae Shooter's `tick_level` wanted 17. Bundle them
  into a `#[derive(SystemParam)]` struct — theirs is called `RunEnd` — and
  pass that.
* **The `SimSet` tuple.** Bevy's tuple impls stop before 14 elements, and a
  ported chain is `advance_tick` + the game's systems + `checksum_tick` +
  `checksum_<game>` + `record_tick` + `remember_sim_prev`, so a game with
  nine gameplay systems is already over. Nest: two `.chain()`ed tuples
  inside one outer `.chain()`, which preserves the order exactly.

### A run may not leave `Playing`: phases are resources, not states

A `GameState` variant the run passes *through* — an intermission, a shop, a
between-waves tuning screen — looks like good state-machine hygiene and
breaks recording and verification at once. `SimSet` is gated on
`in_state(Playing)`, `replay::recorder::begin_recording` runs on
`OnEnter(Playing)` and `seal_run` on `OnExit(Playing)`, so a run that
bounces `Playing → Shop → Playing` seals a replay per leg, re-seeds per
leg, and the verifier stops at the first one. Gulper did exactly this with
a per-band `Digest` state: `begin_run`, `begin_recording` and `seal_run`
all fired once per band, and the verifier stopped at the first chasm.

Worse, the screen the state existed for is usually where the run is
decided. Gulper's tuning screen — the biggest single influence on the rest
of a run — was an `Update` system reading the per-frame `GameInput`, so
none of its purchases were recorded at all. Dive Rise's opening draft was
the same shape: cursor and card moved in `Update` on `GameInput`, and
since the verifier builds no `UiPlugin`, a draft opened there would never
close and the run would stall behind the cards for ever.

**The fix in both games:** delete the state, make the phase a **sim
resource**, and drive it from `TickInput` inside `SimSet`.

* Gulper: `GameState::Digest` removed; `run::RunPhase::{Hunt, Tune}` is a
  resource, `digest::tune_input` runs in the chain off `TickInput`, and
  `band::advance_band` (chained straight after it) sweeps the old band and
  builds the next one in the same tick and the same command queue.
  `src/ui/tune.rs` keeps only the painting.
* Dive Rise: `levelup::open_draft` at the head of the chain, so the cards
  are up on tick 1; the gameplay group is gated on `levelup::draft_closed`,
  so the world is frozen while they are — **but the ticks keep running and
  recording**, which is exactly what makes the choice replayable. The offer
  comes from `sim::GameRng` (the seed), the picks come from `TickInput`
  (the replay), and between them they determine the run.

**A resource, not a `SubStates`.** `NextState` applies once per *frame*,
and a phase change decided inside `FixedUpdate` may need to take effect on
the next *tick*; at two or three ticks per frame the two disagree, and that
disagreement is frame-rate dependent, which is the whole thing the port is
removing.

**Count the presentation cost honestly, because it is real.** A `ScreenFade`
wipe drives *state* changes (`fade.request(GameState::X)`), so a phase that
is no longer a state no longer gets one: Gulper's hunt → tuning transition
is a hard cut after the port where it used to wipe. That is a behaviour
change to flag for the owner, not a refactor to slip in — and the remedy is
cheap and belongs on the presentation side: react to the phase change from
`Update` (a fade-in on the screen's root node, a cosmetic overlay), never by
routing the phase back through `GameState`.

### Attract mode: systems that run in `Menu` as well as `Playing`

A title screen over a living world (Grand Theft Otto's city keeps driving
behind the menu) puts the same systems in two jobs, and a port that only
looks at `Playing` gets it wrong in both directions: move them wholesale
into `SimSet` and the attract mode dies; leave them in `Update` and the
run's own city is frame-rate dependent.

Classify each one by what it *writes*, exactly as rule 1 asks — and expect
the answer to be "run state". All four of Grand Theft Otto's were:
`tick_ampel` writes the traffic-light clock a red-light infraction is
charged against; `walk_pedestrians` and `drive_cars` move entities the sim
reads **and draw from `GameRng`**, which alone makes every later draw in
the run frame-rate dependent; `drive_tram` moves what `ride_tram` reads.

**The fix is to register the same functions twice**, which is legal and
cheap:

```rust
// The run: inside the chain, on the fixed tick.
.add_systems(FixedUpdate, (.., tick_ampel, walk_pedestrians, drive_cars,
                           drive_tram, ..).chain().in_set(sim::SimSet))
// Attract mode: the same functions, frame-delta, Menu only.
.add_systems(Update, (tick_ampel, walk_pedestrians, drive_cars, drive_tram)
    .run_if(in_state(GameState::Menu)))
```

Gate the `Update` copy on `Menu` **alone** — never `Menu.or(Playing)`,
which is the custom run-condition (`scene_live`, `gameplay_live`) this
replaces and the reason the systems were ambiguous in the first place.

Two things make that safe, and both need checking rather than assuming:

* **RNG.** Attract-mode draws are harmless *because* `sim::begin_run`
  reseeds `GameRng` from `RunSeed` and rewinds `SpawnCounter` on
  `OnEnter(Playing)`. Confirm your game's `begin_run` is chained ahead of
  its own `OnEnter(Playing)` setup; if anything seeds or spawns before it,
  the menu's draws leak into the run.
* **Entities.** Whatever attract mode spawned must be cleaned up on
  `OnExit(Menu)`, or the run starts with a world the verifier never built —
  the same desync as a leaked RNG draw, through entities instead. A
  `GameEntity`-style marker on the attract spawns and one despawn system is
  the whole fix.

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

**Use `sim::SpawnOrder` unless the game already has a key.** It is in
`src/game/sim.rs`, so the rollout copies it in: a `SpawnOrder(u64)` component
stamped from the `SpawnCounter` resource as each sim entity is spawned, with
`sim::begin_run` rewinding the counter to 0 at the start of every run so a
replay sorts by the same numbers the recorded run did. Stamp it at *every*
sim spawn site — `commands.spawn((Crumb, GameEntity, spawn.stamp(), ...))` —
and query it **non-optionally**, so an entity spawned without one stops
matching and the playtest suite notices at once. A `Default`-able or
`#[require]`d version would give every un-stamped entity key 0 and hand the
tie straight back to archetype order, which is worse than the bug. A key the
game already owns (a bake counter, a hole index, an inbox slot) is fine; use
it, and break exact-tie `min_by` searches on it too.

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

### The third trap: a sim system may not read what only the windowed build writes

The two traps above are about which *entities* exist and what *components*
they carry. This one is about the **values** the windowed build writes and
the verifier never does — first as a resource (Sundae Shooter's
`ScreenFade`, the one that shipped), and then, in "The component half of
the same trap" below, as a field of a component on a sim entity (Dive
Rise's sprite mirror, which is the same bug through a hole in the earlier
wording).

Sundae Shooter's `launcher::fire_scoop` and `launcher::swap_queue` refused to
act while `ui::transition::ScreenFade` was mid-transition — a reasonable
"don't shoot during a wipe" gate, and three bugs at once:

* the fade lives in `UiPlugin`, which the verifier never builds, so it blocks
  nothing there;
* it is ticked from `Update` on the **frame** delta, so how many sim ticks it
  covers depends on the frame rate;
* it is busy for the first ~24 ticks of **every** run, because entering
  `Playing` goes through it.

A run recorded in the browser came back claiming 195 points against 200
re-simulated: the verifier took an opening shot the player's game had
refused. Nothing local could see it — `--selftest` and both `.gxr` fixtures
run in bare apps with no fade, so the selftest's verdict was byte-for-byte
identical before and after the fix, and `tests/archetype_order.rs` adds
components, not resources. Only a browser recording exposed it, and every
template-derived game has `ScreenFade` in `UiPlugin` and reaches `Playing`
through it.

**`Option<Res<T>>` is a compile fix, not a determinism fix.** The table in the
next section tells you to make `UiPlugin`/`AudioPlugin`/`AssetsPlugin`
resources optional so parameter validation passes headless. That gets the
verifier running; it does not make the two builds agree. If the sim *branches*
on whether the resource was there, or on what it said, the run forks. Reading
it optionally and ignoring it is fine; reading it and acting on it is the bug.

#### The two tolerated reads, and why they are tolerated

Everything else that reads the fade from `src/game/` is a bug. These two are
not, and the rollout script's advisory excludes both by name so it stops
being noise on every upgrade.

**`sim::end_run`'s `Option<ResMut<ScreenFade>>`** (rule 10). It only *writes*
a fade request, on the tick `sim::RunOver` has already frozen `SimSet`.
Nothing the sim does afterwards depends on what the fade said, because the
sim does nothing afterwards.

**`game::toggle_pause`'s `Option<Res<ScreenFade>>`** (rule 8). This one is a
genuine *read* that a genuine *branch* hangs off — the template's own shape,
shared verbatim by four ported games:

```rust
// FixedUpdate, .before(sim::SimSet), registered in BOTH modes
fn toggle_pause(
    input: Res<gamebient_input::TickInput>,
    mut paused: ResMut<states::Paused>,
    fade: Option<Res<crate::ui::transition::ScreenFade>>,
    ...
) {
    if fade.as_ref().is_some_and(|f| !f.is_idle()) || !input.pause_just_pressed {
        return;
    }
    paused.0 = !paused.0;
```

The windowed build has a fade and refuses the pause while it is busy; the
verifier has none and would accept it. That is exactly the Sundae Shooter
shape, and it is harmless for one reason only: **rule 8 keeps pause off the
replay entirely.** `replay::recorder::push` masks `Buttons::PAUSE` out of
`held` and `latched` before a tick is recorded, and `replay::feeder` masks it
again out of what it writes back into the accumulator. So in the verifier
`input.pause_just_pressed` is false on every tick of every replay, and the
`!input.pause_just_pressed` clause on its own returns early on all of them —
whatever the fade would have said. The fade can only change the outcome on a
tick where pause was really pressed, and no replay carries one. The two
builds disagree about a branch neither run can take.

Read that as the narrow licence it is. It holds while all three remain true:

1. the system runs **outside** `SimSet`;
2. the pause edge alone is sufficient to make it a no-op — the fade only
   ever strengthens a guard the masked pause already forces, never a
   condition that could let something *through* headless;
3. pause stays masked at both ends (recorder and feeder).

Move the read into a sim system, or let the fade decide anything on a
non-pause tick, and it is the shipped bug again. Cannonball Putt is where
this got written down; `tests/windowed_shape.rs` watches it, since inserting
`ScreenFade` is what makes the two apps disagree in the first place.

#### The component half of the same trap: the sprite mirror

The rule above says *resources*, and Dive Rise found the hole in that
wording by desyncing through a **component field** instead.

`behaviors::swim_burst::dash` has a fallback: a dash thrown with the stick
at zero has no direction of its own, so it dashes the way you face — and
the way it read the facing was

```rust
Vec2::new(tf.scale.x.signum(), 0.0)   // the bug
```

`tf` is the **player's own** `Transform`, and its `scale.x` is written by
`assets::creature_visuals::face_movement`: an `Update` system, inside
`AssetsPlugin`, that mirrors the sprite when the player swims left. The
verifier builds no `AssetsPlugin`, so nothing ever writes that field there
and it reads `+1.0` from `spawn_player` to the end of the run. A dash
thrown from a standstill while facing left went **left in the browser and
right in the verifier** — 0.4667 m, one tick at dash speed — and the run
never re-converged. It fired several times a run in practice, because
taking a draft card *is* A-with-no-stick.

Every existing probe was green, each for its own reason: the fixtures run
in bare apps, so `scale.x` is `+1` on both sides and the fallback agrees by
accident; `archetype_order` adds components and children but never writes
an existing component's **value**; and `windowed_shape` carried the
resources and deliberately left `AssetsPlugin` out.

**So the rule is wider than it was written.** Not "a sim system may not
read a resource `UiPlugin` owns" but:

> A sim system may not read **any state the windowed build writes and the
> verifier does not** — a resource, or a field of a component on an entity
> the sim itself owns.

In practice that means `Transform.scale`, `Transform.rotation`,
`Visibility`, and any component a decorator in `src/assets/` or `src/ui/`
writes on a sim entity. **Latch it from input into sim-owned state
instead**: Dive Rise's fix is a `player::Facing` resource holding `±1.0`,
latched by `move_player` from `TickInput.move_x` (so a replay carries it
like everything else), reset per run, and read by `dash`; `face_movement`
now *paints* `facing.0` rather than deciding it, so presentation follows
the sim and the two can no longer drift apart. Keep the write on the
presentation side one-directional and the whole class goes away.

The audit that closes it out is two greps: the mutable `Transform` queries
in `src/assets/` and `src/ui/` that can match a sim entity, and then
`grep -rn 'scale\|rotation' src/game/` for reads of them. Survivors are
fine as long as nothing in `src/game/` reads them back.

**And carry the real `AssetsPlugin` in the probe**, on hand-made asset
stores, so the decorators run for real:

```rust
app.add_plugins(bevy::asset::AssetPlugin::default());   // the AssetServer
app.init_asset::<Shader>();                             // + Mesh / ColorMaterial /
app.init_asset::<Mesh>();                               //   StandardMaterial — whichever
app.init_asset::<ColorMaterial>();                      //   stores the plugin bakes from
app.add_plugins(AssetsPlugin);
```

`build_headless_app` is `MinimalPlugins`, so none of that is present by
default — that is why the earlier fleet-upgrade note reached for
`insert_resource(Assets::<Mesh>::default())`. Adding `AssetPlugin` itself
is the stronger version and the one to prefer: it brings an `AssetServer`,
which a plugin that loads anything (Dive Rise's `ShaderKitPlugin` loads two
internal WGSL assets) needs in order to build at all. Insert any resource
the plugin expects but does not create — `main.rs` usually inserts one or
two — and delete whatever hand-written stand-in the probe was carrying
instead. Then **mutation-check it**: re-plant the read (`tf.scale.x.signum()`
in the movement system) and confirm a test now fails. If none does, the
plugin is in but inert, and the next paragraph is why.

**Carrying the plugin is not enough on its own** — Dive Rise's passed with
`AssetsPlugin` in and two bots, because the forager was facing right on
every one of its six blind dashes and the diver held `DOWN` every tick so
its stick was never zero. Each bot reached one half of the bug. What
reaches both is a bot that **releases the stick**, and the reason neither
shipped row does is structural rather than particular to that game: the
fixture script and `reckless_script` both hold a direction on *every* tick,
so between them they never produce an idle-stick tick, and a
"do it the way you're facing" fallback fires on exactly those.

So the template's probe now ships a third row, `coaster_script` — hold LEFT
for 90 ticks, release for 30, tap A in the middle of the release — and a
port should keep it and point it at whatever its own direction-less action
is (a dash, a swing, a drop, a fire button with no aim). Measured on the
template itself while adopting it: a planted `tf.scale.x.signum()` fallback
in `move_player` passes all three rows on its own (nothing writes that
field — the template's `AssetsPlugin` is a stub), and fails **the coaster
row, and only it**, as soon as that plugin is given the `face_movement`
system every real game has. That pair of mutations is the check to re-run
in a game after adapting the file.

Counters are what turn this from guesswork into a measurement. Dive Rise's
are the pattern: `facing_flip_frames`, asserted non-zero for *every* bot so
the asset layer can never go inert unnoticed, and `left_facing_dash_starts`,
asserted non-zero for the new row so it cannot decay into a copy of
another.

**The test:** `tests/windowed_shape.rs`, copied into every game by the
rollout script alongside `archetype_order.rs`. It records the selftest script
in an app carrying `ScreenFade::boot()` and the windowed build's `Update`
systems, then `verify()`s it in a bare one — the production question exactly
(the browser records with all of this present, the site re-simulates with none
of it). As shipped it covers `ScreenFade` and the asset-store lines above;
**extend `record_windowed` during the port** with every `Update` system
`GamePlugin::build` registers behind `!self.headless`, every
`UiPlugin`/`AssetsPlugin` resource a sim system might read, and the game's
own `AssetsPlugin` so its decorators write real component values. The script
also prints an advisory naming `src/game/`
functions that take a fade resource and do not hand it to `sim::end_run` or
gate it behind `pause_just_pressed`; like the archetype advisory it is a grep,
not a verdict.

**Then measure that the probe's script actually reaches the mechanics
presentation could touch.** Adding the systems is half the job; the other
half is proving the bot runs them. Count the frames on which each
windowed-only system had a live entity to write — a `#[derive(Resource)]`
counter and a one-line `Update` system beside the ones you just added is the
whole technique — and print or assert the totals before trusting a green
probe. **If the fixture bot is a careful router, write a reckless second
script and assert it reaches them (non-zero counter).** The probe ships a
three-row bot table (`enum Bot`) for exactly this: `reckless_script` as a
documented placeholder in the second row, and `coaster_script` — which
releases the stick, the one thing neither other row ever does — in the
third.

Attic Excavator is the worked example, and the measurement is why this
paragraph exists: over the selftest script's full 1800 ticks
`heavy::wobble_shake` fired **zero** times and `cat::cat_touch` never
connected — the fixture's router costs +60 for a cell under heavy junk and
+25 near an awake cat, so it avoids by construction the two
windowed-vs-headless divergences worth probing (and `archetype_order`'s
reckless staircase does no better there: it sinks a narrow shaft that misses
all six heavies). Its replacement bot seeks the shallowest idle heavy, digs
down beside it and steps underneath: 66 frames of teetering, 50 of falling
junk, a death — and the test asserts the wobble count is non-zero so the row
cannot decay into a duplicate of the first. Two planted bugs prove the point:
a *sim* system filtering its query `With<Visibility>`, and the checksum
folding the `translation.x` that only `wobble_shake` writes. Both **passed**
under the fixture bot and **failed** under the reckless one.

Two constraints on whatever you write: it may read the world but may only
write `VirtualInput` (a recording that is not pure input cannot be replayed),
and it may not draw from any RNG — not `rand::rng()`, which makes the run
differ every time, and not `sim::GameRng`, which desynchronises every later
draw in the sim's own stream. Hash the tick index instead.

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
| `ScreenFade` | `UiPlugin` | `Option<Res<…>>` / `Option<ResMut<…>>` in `toggle_pause` and the game-over site — the two tolerated reads, see rule 1 |
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

### Input the canon cannot express: bridge it, do not read it

`TickInput` carries the canon alphabet and nothing else, because that is
all a `.gxr` records: four directions, A, B, START, SELECT, PAUSE and the
stick. **A sim system that reads any other input source is unreplayable**,
however legitimate the device.

Ladder Legend is the case. Its `input::read_input` probed
`Query<&Gamepad>` for the four *face* buttons directly and ORed them into
the lane mask, because the canon folds South/North into `A` and West/East
into `B` and cannot express a dance-mat panel. So a run played on a mat —
the platform's *canonical* controller for that game — recorded lane steps
no verifier could reconstruct.

The fix is a bridge, not an extension of the format. A `PreUpdate` system
ordered `.before(gamebient_input::input::accumulate_input)` latches the
extra device's state onto `VirtualInput` as ordinary canon bits:

```rust
// PreUpdate, .before(accumulate_input): the mat's four panels become
// ordinary direction bits, so GameInput and TickInput both see a normal
// edge and the sim reads no raw input at all.
fn mat_panel_bridge(pads: Query<&Gamepad>, mut virt: ResMut<VirtualInput>) {
    // ... virt.latched |= Buttons::LEFT; etc.
}
```

After that the sim reads `TickInput` like every other game, the recorder
sees the press, and a mat run replays in a browser with a keyboard. The
same shape covers any non-canon source a game grows: a steering axis
quantised into LEFT/RIGHT, a light gun, a second player's pad. If the
device genuinely cannot be expressed in the canon bits, that is a
`gamebient-input` change, not a game-local read.

Grep: `grep -rn 'ButtonInput<\|Query<&Gamepad\|KeyCode\|GamepadButton' src/game/`.
The rollout script's HAND EDIT about raw `ButtonInput<KeyCode>` is the same
finding one device narrower.

### The dev autopilot must end its runs through input too

Rule 2's other half, and it is the one that quietly breaks a port's
end-to-end proof. An autopilot that finishes its tour by writing
`GameData` — `data.health = 0.0`, `data.lives = 0` — is fine as a
screenshot harness and useless as a recording: the replay carries the
inputs, not the write, so the re-simulation plays on past the point the
recording ended. Ladder Legend's `drive_autopilot` pinned `health = 0.0`;
it now stops stepping and lets missed notes drain HP through the real
game-over path, which is what made a verified autopilot tour possible at
all.

So: a dev path may write run state **only** if no recording is ever made
through it. The moment you want `GX_REPLAY_DIR=... cargo run --features
autopilot` to produce a verifiable `.gxr` — and step 5 of the skill does —
the tour has to end the run the way a player does. Keep it out of the
selftest script either way (the rollout script prints a HAND EDIT naming
the file).

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

**The scan is textual, and `#[cfg(test)]` is not spared.** It reads whole
files, so a `rand::rng()` inside a test module fails it exactly as one in a
system would — and the failure names the file, not the test module, which
reads like a port mistake for a minute or two. That is deliberate (a scan that
parsed cfgs would miss randomness behind a feature the verifier does build),
so fix the tests rather than the scan: draw from `sim::GameRng`, or build a
`Xoshiro256PlusPlus` from a fixed seed. Both Attic Excavator (one site) and
Sundae Shooter (five) had to. A seeded test is better anyway; a test that
called `rand::rng()` was only ever flaky on a slow day.

```rust
// before, in #[cfg(test)]: fails the forbidden-names scan
let mut rng = rand::rng();

// after
use rand::SeedableRng;
let mut rng = rand_xoshiro::Xoshiro256PlusPlus::seed_from_u64(7);
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

### The transcendental rule: never fold what libm produced

The paragraph above is right for the floats *arithmetic* produces and wrong
for the ones `libm` does, and the difference is the whole of this rule.
`+ - * /` and `sqrt` are pinned bit-exactly by IEEE-754 on every platform
the fleet runs on; `sin`, `cos`, `exp`, `powf` and `atan2` are library
calls, and macOS, the Linux CI runner and wasm are each free to round them
a different way. **One such value in the fold makes the checksum a
referendum on whose libm ran the sim.**

Gulper is the measurement (59 transcendental call sites, the fleet's
highest). Its `checksum_gulper` folded `head.facing`, which is
`velocity.y.atan2(velocity.x)`, and one 5400-tick fixture gave three
verifiers three answers:

```
native (macOS aarch64) checksum 15088799176327886924  score 366  ticks 5400
node   (wasm)          checksum 7126891345786100575   score 366  ticks 5400
native (CI, Linux x64) checksum 11654525614748877228  score 366  ticks 5400
```

Identical score, identical tick count, three checksums. That is not a
tolerable informational mismatch: `tests/selftest.rs`'s
`committed_fixture_still_verifies` re-simulates the committed **native**
fixture **natively**, so a fixture recorded on the porter's Mac could not
verify on the CI runner and the job went red. Bisected by truncated prefix,
ticks 1–7 were bit-identical and they parted at tick 8 — the two libms
agree on `atan2` for the first six arguments the run produces and round the
seventh one ulp apart.

Two halves, and both are cheap:

1. **No libm result is folded bit-exactly.** Audit the game's
   `checksum_<game>` system value by value and ask, for each, which
   operation produced it. A transcendental's output is almost always a pure
   function of something already folded beside it (Gulper's `facing` is a
   function of the velocity in the next line), so dropping it costs the
   checksum almost no sensitivity. Where a transcendental only ever feeds a
   *comparison* — an aggro radius, a regen gate — it never reaches the fold
   and is fine as it is; Dive Rise's `depth::light_at` is an `exp` in that
   position.
2. **A transcendental whose argument is constant under the fixed tick
   becomes a literal.** `(-K * dt).exp()` with a fixed rate and the fixed
   60 Hz `dt` is one number, recomputed sixty times a second on a
   per-platform libm, and its result typically lands straight in a folded
   velocity. Gulper's `eel::movement` drag became `DRAG_PER_TICK`; Dive
   Rise turned eight of them into `*_PER_TICK` literals (knockback decay,
   an orbit ease, six companion follow rates) and changed the signatures
   that took a rate and a `dt` to take the per-tick ease instead.

**Unit-test each literal against the tick length, with a tolerance — never
bit-exactly.**

```rust
#[test]
fn exp_drag_literal_matches_the_formula() {
    let dt = sim::tick_duration().as_secs_f32();
    assert!((DRAG_PER_TICK - (-EEL_DRAG * dt).exp()).abs() < 1e-6);
}
```

A bit-exact assertion would pin *this* machine's libm, which is precisely
the dependency the literal exists to remove — it would go red on the CI
runner for the reason the literal fixed. What the tolerance version catches
is the thing that actually happens: a stale or mistyped literal after
someone changes the rate constant or the tick rate.

Measured on Dive Rise, the change is **bit-neutral**: the committed
fixture's checksum was the same before and after the literals, natively and
under Node. What the literals buy is that a third platform cannot disagree
later.

**What the rule does not remove**, and do not claim it does: a
transcendental whose argument genuinely varies (a wander phase, a drift
angle, `Vec2::from_angle`) has no literal to write, and its result can still
reach a folded number indirectly. Dive Rise's rendered run parts from Node
at tick 44 083 — twelve sim-minutes — three ticks after a hunter bite folds
a `sin`/`cos`-driven hunter position into the player's knockback; Sundae
Shooter's parts at 1056. That is the fleet-wide caveat in
`docs/replay-verification.md`, and the shape of what it costs is: **only a
wasm-recorded run is guaranteed to verify.** Production is safe (the
browser records in wasm, the site re-simulates in wasm), and a
native-recorded run — a cabinet run, or your own `GX_REPLAY_DIR` recording
in step 5 — may legitimately fail under the Node verifier after long
enough. Record the measurement in the game's `docs/replay-notes.md`, never
in `docs/replay-verification.md`.

**Greppable:** `grep -rnE '\.sin\(\)|\.cos\(\)|\.exp\(\)|\.powf\(|\.atan2\(' src/game/`.
`tools/rollout-replay.sh` runs that scan for you and prints a
`HAND EDIT (advisory)` naming the files; like the other two advisories it is
a reading list, not a verdict — most hits are presentation or comparisons.
The two things to look for are a hit whose result reaches
`checksum_<game>`, and a hit whose argument is constant under the tick.

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

That `fade` parameter is the one *tolerated* read of a `UiPlugin` resource
from `src/game/` — the masking above is precisely what makes it safe, since
`pause_just_pressed` is false on every tick of every replay and the fade's
answer is therefore never reached in the verifier. Rule 1's "The two
tolerated reads, and why they are tolerated" spells out the three conditions
it depends on; keep all three when you port this system.

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
every game by the rollout script. It records the selftest script three times
per case — once in a plain headless app and once in each of two decoration
modes, both of which run `Update` systems inserting inert markers onto the
entities the sim queries — and asserts each decorated recording agrees with
the plain one on score, checksum and ticks, and still `verify()`s in a plain
app (which is the production question: the browser records decorated, Node
verifies bare). Repeated over several seeds at 1, 2 and 3 sim ticks per
frame, because at one tick per frame the `Update` decoration and the
`FixedUpdate` sim interleave one-to-one and a reorder can stay hidden.

**The two modes are not redundant. Keep both.** `Decoration { on, split }`:

* **`faithful`** (`on`) mirrors the game's real decorators — same
  components, same entities, same branching. It is what makes a failure a
  statement about the shipping game rather than about a synthetic worst
  case: *the faithful mode documents the real partitioning*.
* **`split`** additionally halves every sim archetype on the parity of
  `sim::SpawnOrder`, a key the sim's own queries cannot see. It is
  deliberately more aggressive than any real decorator: *the split mode is
  the detector*.

Measured, not assumed. Dough.io's reviewer reproduced a genuine,
score-changing instance of this bug (18/18 recordings with the ECS order
provably different), then deleted the two sorts that fixed it: the faithful
mode passed all 18 by luck, the split mode failed 2 of 18 with
`verify(decorated).matches=false`. A port that keeps only the faithful mode
has a test that agrees with the bug.

**Extend both modes to your game's real entities during the port** — as
shipped it is a smoke test, since the template's sim queries one entity and
one entity has no order to get wrong (its one entity is `SpawnOrder(0)`; the
split marker lands on even keys, so the insert path does run, against nothing). It also does not *compile*
unadapted in most games: the template's version references the template's own
`Player`. (That is why `rollout-replay.sh --upgrade` refuses to create the
file when a game does not already have one, and asks for it instead.) Three
things matter when you adapt it:

* **Reproduce the branching** in the faithful mode. A decorator that attaches
  a different component set to different entities (golden crumbs get a
  sparkle, rivals get a mood) is what splits one archetype into several.
  Markers that are identical for every entity cannot reproduce the bug.
* **Point the split mode at every kind of sim entity**, not just the ones the
  faithful mode emphasises. It is the detector; give it everything.
* **Keep the child spawn.** `ChildOf` puts `Children` on the *parent*, so a
  decorator that only spawns children still moves its parent's archetype.

The file's own doc comment carries all three, plus how to extend
`trace_order` for the queries your order-sensitive sim systems iterate.

**A green probe is not proof the sorts are unnecessary — measure what the
recorded runs actually do.** The probe can only reorder interactions that
happen. Attic Excavator's fixture bot routes *around* heavy junk and sleeping
cats, so one to four cats and six to ten heavies on a 15x34 board never
collided in 1800 ticks, and deleting either of the two `SpawnOrder` sorts
still passed 24/24 in both modes. Pack The Ripper's sim never held more than
two packs at once, so reordering two IEEE floats under a commutative op was
exact and its float mutations only bit at three ticks per frame. After
adapting the probe, check what the runs do (how many of each entity kind are
live at once? do the multi-entity interactions occur at all?), and when they
do not, write the dependency down as a targeted unit test instead — Attic's
`a_burrowed_cell_changes_the_next_cats_route_cost` and
`a_vacated_cell_changes_where_the_next_heavy_lands` are the pattern — and say
so in `docs/replay-notes.md`, so nobody later reads the green probe as
evidence the sorts can go.

**`tests/windowed_shape.rs` is the other half of this test, and it asks the
resource question.** `archetype_order` adds components; `windowed_shape`
records the same script in an app carrying `ScreenFade`, the windowed
build's `Update` systems and the game's own `AssetsPlugin` (on hand-made
asset stores) and verifies it in a bare one. It is copied in by the rollout
script and, unlike the archetype probe, compiles unadapted — the one thing
it needs from the game is a `pub struct AssetsPlugin` under `src/assets/`,
and the script refuses to write the file rather than hand over one that
cannot compile. Adapt it anyway with your `!headless` systems and your
`UiPlugin`-owned resources. See "The third trap" under rule 1 for the bug it exists to catch.

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

**Then check whether it is the removable half of the drift before writing it
off.** Rule 5's transcendental rule is the fix for two of the three shapes
this bisect turns up, and both were found this way: a libm result folded
into the checksum bit-exactly (Gulper's `atan2` facing — and note that this
one can turn `cargo test` itself red on a different machine, because the
committed *native* fixture is re-simulated *natively*), and a
transcendental whose argument is constant under the fixed tick (an
`exp(-k·dt)` decay) whose result lands in a folded value. Both are removed
rather than documented. Only the third shape — a varying-argument
transcendental laundered into a folded number by an in-game event — is the
caveat, and that is the one to write up.

When the bisect confirms drift of that third kind, no workflow change is
needed — the native-vs-Node step (`Cross-check the native fixture under
Node`) is already `continue-on-error: true` — but write the measurement
down: the two checksums, the agreeing `score` and `ticks`, the first tick at
which they part, and the call site responsible. The next person should not
have to redo the bisect.

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
