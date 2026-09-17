# Games needing extra steps beyond the port checklist

`tools/rollout-replay.sh` and `references/port-checklist.md` assume the
template's shape: `src/game/mod.rs` with a `GamePlugin`, a `GameData` with
a plain `score: u32`, gameplay reading `Res<GameInput>`/`TickInput`, and
one seeded RNG resource if any. These seven games depart from that in a way
the script can only flag, not fix. Read the game's own current code before
acting on this — it's a 2026-09-16 snapshot; verify line numbers and
specifics against `references/catalog-2026-09-16.md`'s survey columns and
the file itself.

## Hunted

Flat layout: no `src/game/` at all. `autopilot.rs`, `audio.rs`,
`GameState`, `Paused` live directly under `src/`, declared and registered
from a bare `fn main() { let mut app = App::new(); ... }` — not a
`GamePlugin`. There is no `score` field anywhere; `run_timer.rs`'s
`RunTimer` resource (`tick_run_timer`, gated
`in_state(Playing).run_if(not_paused)`, currently in `Update`) tracks
survival time instead, and a *lower* time is better (`beats_best`).

`rollout-replay.sh` still copies `src/game/sim.rs` and `src/game/replay/`
to those paths unconditionally — it has no flat-layout branch the way
`rollout-record.sh` does — so after running it you'll have an orphaned
`src/game/` directory with no `mod.rs` to declare it from `main.rs`. Move
both to the crate root to match where `rollout-record.sh` already put the
recorder (`src/record/`): `git mv src/game/sim.rs src/sim.rs` and
`git mv src/game/replay src/replay`, fix the `use super::...`/`use
crate::game::...` paths those files reference (`sim.rs` imports
`super::scoring::{GameData, LeaderboardScore}` — becomes `crate::scoring`
or wherever `GameData` ends up), then `rmdir src/game`.

Declare `mod sim; mod replay;` in `main.rs` next to `mod run_timer;`. Wire
`SimSet`/`begin_run`/`checksum_tick` directly into `main()`'s `app` — it's
a local value here, not `&mut App` (same gotcha `references/irregular-games.md`
in the `recording-game-footage` skill documents for this exact game), so
every `app.add_systems(...)`/`app.init_resource::<...>()` call for the sim
scaffolding is unaffected (those already take `&mut self`), but anything
written as a free function expecting `&mut App` needs `&mut app`
explicitly.

`LeaderboardScore` needs a real implementation, not the blanket
`GameData` one — there's no `GameData` to blanket-impl it on. The natural
choice is survival time in a fixed-point integer form the leaderboard can
rank as "higher is better" (e.g. tenths of a second survived, since the
leaderboard is higher-is-better but a lower raw time is the in-game win —
either negate/invert consistently or (preferred) rank ascending on the
site side as a documented follow-up; for this rollout, expose
`leaderboard_score()` as `u32::MAX - tenths_survived` or similar so the
existing higher-is-better leaderboard ranks correctly, and say so clearly
in a doc comment because it's surprising). No `Paused`-gated `ScreenFade`
usage was confirmed in this pass — check `screens.rs`/`transition.rs` for
the game-over path before assuming it already calls `ScreenFade::request`.

## Moleman Racing

No `GameInput` use anywhere in gameplay (0 sites) — throttle/brake/steer
plus build actions all come from `src/game/controls.rs`'s own `Controls`
resource, populated straight from `Res<ButtonInput<KeyCode>>` and gamepads
in `PreUpdate` (`collect_controls`, `game/controls.rs:33-44`). This is
exactly the "gameplay reads raw `ButtonInput<KeyCode>`" case
`rollout-replay.sh`'s pre-flight HAND EDIT warns about, and it's the whole
control scheme, not an incidental pause read.

`TickInput` doesn't carry steer/throttle/brake/recipe-select — only the
canon `move_x`/`move_y`/button bits. Two ways to close the gap, in order of
preference: (a) extend `gamebient_input`'s recorded tick shape isn't an
option from inside a game (it's a separate crate, pinned by tag); the
practical path is (b) derive `Controls` deterministically from `TickInput`
inside a `SimSet` system — `move_x`/`move_y` become steer/throttle-or-brake
by sign, and the recipe/place/reset actions map onto the primary/secondary/
start buttons — replacing raw-key reads with a `TickInput`-driven
`collect_controls_from_tick` that runs inside the chain. This is a genuine
control-scheme redesign, not a mechanical swap; flag it to the human before
picking this game off the catalog, since it changes how the game feels to
play. `record/` is also missing (`rollout-record.sh` hasn't been run here)
— `references/catalog-2026-09-16.md` notes it, and neither script requires
the other, but check whether recording was skipped on purpose before
assuming it's just not gotten to yet.

## Beat Bender

`record/` is missing — the survey couldn't confirm layout details
downstream of it, so treat every claim here as needing a fresh read before
acting. `driver.rs` seeds two things from non-`GameRng` sources:
`gen_chart` seeds a local `StdRng::seed_from_u64(song_seed.wrapping_add(seg
as u64))` per segment (deterministic given `song_seed`, fine as long as
`song_seed` itself is derived from `RunSeed`), but `setup_match` mixes in
session entropy from `rand::rng()`:
```rust
let session: u64 = rand::rng().random();
commands.insert_resource(BeatClock::new(s.bpm));
commands.insert_resource(MatchState::new(song.0, difficulty.0, s.seed ^ session));
```
The comment ("AI rolls share the stream, so runs still vary via the
session-entropy mixin") is the non-determinism itself — a replay can't
reproduce a `rand::rng()` draw. Replace `session` with a value derived from
`Res<sim::GameRng>` (drawn once in `setup_match`, which needs to run inside
or right after `sim::begin_run` so the draw happens after the RNG is
reseeded from `RunSeed`) so the same seed always produces the same session
mix. `MatchState::new`'s own `StdRng::seed_from_u64(seed)` field
(`rng: StdRng::seed_from_u64(seed)`) then stays deterministic without
further changes, since it's seeded from that same derived value.

## Dough.io

Two RNG resources: `SimRng` (`arena.rs:14-24`, `StdRng`, defaults via
`StdRng::from_os_rng()`) drives spawn points, rival wander and ingredient
choice — this is the one that must reseed from `RunSeed` in
`sim::begin_run` (rename it, or wrap it the way the port checklist's rule 3
describes, so it becomes the game's `GameRng`). The separate `FxRng`
(`particles.rs:12-18`, unseeded) is presentation-only and stays as-is.

Game over is the one confirmed direct-`NextState` site in the fleet:
```rust
// eating.rs
next_state.set(GameState::GameOver); // line 282, inside a Playing-gated system
```
This must change to `ScreenFade::request(GameState::GameOver)` per the port
checklist's "Run end always through `ScreenFade::request`" section — check
whether `eating.rs`'s system already has fade access; if not, add
`mut fade: ResMut<crate::ui::transition::ScreenFade>` (or wherever this
game's `ScreenFade` lives) to its params.

## Voidrunner

Has its own `GameRng` (`game/rng.rs`, wraps `SmallRng`, seeded once from
the OS at `Default::default()`) but two hot paths bypass it and call
`rand::rng()` directly instead — confirmed:
```rust
// space.rs:48, inside manage_space_combat
let mut rng = rand::rng();
let count = rng.random_range(min_count..=max_count);
```
```rust
// enemies.rs:51, inside spawn_wraith_wave
let mut rng = rand::rng();
let roll: f32 = rng.random();
```
Both need `mut rng: ResMut<sim::GameRng>` (or this game's renamed wrapper)
threaded into the function instead, replacing `rand::rng()` with `rng.0`.
`spawn_wraith_wave` is a plain function called from a system, not a system
itself — its signature needs to grow a `&mut GameRng` (or `&mut
Xoshiro256PlusPlus`) parameter and its caller (`manage_space_combat`)
passes its own `rng.0` through. `starfield.rs:25`'s RNG use is
presentation-only (FX, not gameplay state) and is fine to leave on its
current source. Also note: this template's `GameRng` is
`Xoshiro256PlusPlus`; Voidrunner's existing `GameRng` wraps `SmallRng`,
which the port checklist's rule 3 explicitly calls out — `SmallRng` picks a
different algorithm on wasm32 than native, so replaying across native/wasm
would disagree even with a correct seed. Rename Voidrunner's `GameRng` to
avoid the collision with `sim::GameRng` and switch its inner type to
`Xoshiro256PlusPlus` (matching the template's `rand_xoshiro` dependency the
rollout script already adds to `Cargo.toml`), not just reseed the existing
`SmallRng`.

## Attic Excavator (board seed)

`level.rs:69`'s `board_seed` draws non-deterministic randomness outside the
`harness` feature:
```rust
fn board_seed(level: u32) -> u64 {
    #[cfg(feature = "harness")]
    {
        42 + level as u64
    }
    #[cfg(not(feature = "harness"))]
    {
        rand::rng().random::<u64>().wrapping_add(level as u64)
    }
}
```
This is called from `spawn_board`, itself called from an `OnEnter`-style
level-start path (not confirmed to run inside `SimSet` — check
`level.rs`/`game/mod.rs` before moving it). The fix is not a third `#[cfg]`
branch: replace the non-harness arm with a draw from `Res<sim::GameRng>` (a
`u64` built from two `GameRng::next_u32()` calls, or via
`rand::Rng::random::<u64>()` called on `&mut rng.0`) so board generation
becomes a function of the run seed instead of the OS. Keep the `harness`
arm as-is (fixed 42+level, used for scripted playtest runs) — it doesn't
need to change, since harness runs aren't what gets replayed/verified.
`grid.rs:230`'s `try_generate(level, seed)` and its retry loop
(`seed.wrapping_add(attempt)`, up to 32 attempts for a reachable board) stay
unchanged: they're already a pure function of whatever `seed` they're
given, which is exactly the property the fix above needs to hold once that
seed comes from `GameRng` instead of `rand::rng()`. `treasure.rs:42` and
`effects.rs:23`'s RNG sites are gameplay-during-a-run, not level setup —
port them the same way as any other game's rule-3 sites (thread
`GameRng` in), not specially.

## Grand Theft Otto (attract-mode run conditions)

Gameplay doesn't run under a plain `in_state(Playing)` condition — it runs
behind two custom run conditions defined in `game/mod.rs`:
```rust
/// Run condition: gameplay is live (Playing and not paused).
pub fn gameplay_live(state: Res<State<GameState>>, paused: Res<states::Paused>) -> bool {
    *state.get() == GameState::Playing && !paused.0
}

/// Run condition: the city is on screen and its ambient life should move
/// (playing, or the title screen's attract mode).
pub fn scene_live(state: Res<State<GameState>>, paused: Res<states::Paused>) -> bool {
    match state.get() {
        GameState::Playing => !paused.0,
        GameState::Menu => true,
        _ => false,
    }
}
```
`gameplay_live`-gated systems (the ~42 the survey counted) are the ones
that move into `SimSet` — `gameplay_live` and `sim::SimSet`'s own run
condition (`in_state(Playing).and(not_paused)`) are equivalent, so this is
a straightforward swap. `scene_live`-gated systems are a genuine third
category the template has no precedent for: they run during `Menu`'s
attract mode too (pedestrians, traffic, tram — ambient city life, not
scored gameplay), which `SimSet` never does (it's gated to `Playing`
alone). These must **not** move into `SimSet` even though some of them
mutate world state, because a replay only ever re-simulates `Playing` — an
attract-mode system inside `SimSet` would never run during verification
and the live game's attract-mode visuals would silently diverge from
nothing (harmless) but any `scene_live` system that *also* writes state
`gameplay_live` systems read would break rule 1 by writing run state from
outside the chain. Read each `scene_live` system before deciding: if it
only writes its own ambient entities (pedestrian/car/tram positions) that
no `gameplay_live` system reads, it's presentation and can stay exactly as
it is, gated on `scene_live`, outside `SimSet`, for both `Menu` attract
mode and `Playing`. If any `scene_live` system writes something a
`gameplay_live` system also reads, split it: the `Playing`-only mutation
moves into `SimSet`, the `Menu`-attract-mode-only visual stays in `Update`
under `scene_live`. `game/rng.rs`'s `GameRng` here already forbids
`rand::rng()` by its own doc comment and needs no port; confirm the doc's
claim holds (`grep -rn 'rand::rng()' src/game`) rather than trusting the
comment alone.
