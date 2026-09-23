//! Determinism scaffolding: the fixed tick, the one RNG, the run seed and
//! the checksum a replay must reproduce. See docs/replay-verification.md.
//!
//! `checksum_tick` (below) folds only the score and the tick — it is copied
//! verbatim into every game by `tools/rollout-replay.sh` and must stay
//! game-agnostic. Each game folds its own key run state (player/ball
//! position, hole index, lives, ...) in a system of its own, placed right
//! after `sim::checksum_tick` in the `SimSet` chain; `player::checksum_player`
//! is this template's example. See docs/replay-verification.md, determinism
//! rule 6.
//!
//! [`RunClock`] is the third: **the sim owns its own clock**. Inside
//! `FixedUpdate`, `Res<Time>` is `Time<Fixed>`, and `Time<Fixed>::elapsed()`
//! counts from *app* start — nothing resets it when a run begins — so a sim
//! system that reads it forks on how long the player watched the studio
//! logo and the menu before pressing Start. In a sim system `Res<Time>` may
//! be used for [`Time::delta_secs`] **only**, which inside `FixedUpdate` is
//! always the fixed timestep; anything phase-like reads [`RunClock`], which
//! [`sync_run_clock`] restates from [`SimTick`] at the head of `SimSet`. See
//! docs/replay-verification.md, determinism rule 7.
//!
//! [`RunOver`] is the other fleet-wide piece here: a windowed game leaves
//! `Playing` through a 0.4 s `ScreenFade`, which runs in `Update` on the
//! *frame* delta, so the sim would keep ticking (and recording) for a
//! frame-rate-dependent tail the headless verifier — which has no fade and
//! leaves on the ending tick itself — can never reproduce. The sim system
//! that ends the run latches [`RunOver`] on the same tick it asks for the
//! fade, and [`run_not_over`] is part of `SimSet`'s run condition, so both
//! paths freeze the sim on exactly the same tick. [`end_run`] does the
//! windowed/headless split in one place. See docs/replay-verification.md,
//! determinism rule 10.

use std::time::Duration;

use bevy::prelude::*;
use gamebient_input::Buttons;
use gamebient_input::input::TickFrame;
use rand::{RngCore, SeedableRng};
use rand_xoshiro::Xoshiro256PlusPlus;

use super::scoring::{GameData, LeaderboardScore};
use super::states::GameState;
use crate::ui::transition::ScreenFade;

pub const TICK_HZ: u32 = 60;

/// One fixed tick. `Time::<Fixed>` and the verifier both step with this.
pub fn tick_duration() -> Duration {
    Duration::from_nanos(16_666_667)
}

/// Sim ticks since the run began (0 before the first tick).
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SimTick(pub u32);

/// Seconds since this run's first sim tick — **the sim's own clock**, and
/// the only clock a `SimSet` system may read.
///
/// Inside `FixedUpdate`, `Res<Time>` is `Time<Fixed>`, and
/// `Time<Fixed>::elapsed()` counts from **app start**. Nothing resets it
/// when a run begins, so its value on the run's first tick is however many
/// fixed ticks the app had already run: the studio logo, the title screen,
/// the how-to-play card, every second the player spent deciding. A sim
/// system that drives a drift, a weave, an orbit or a lunge phase off it
/// therefore gives the player a different world depending on how long they
/// sat in the menu — while `replay::run_verify_app` enters `Playing` on its
/// second `app.update()` under a zero-delta `TimeUpdateStrategy`, so in the
/// verifier that offset is always **zero**.
///
/// That is what cost Dive Rise a production `UNVERIFIED / mismatch`: 11 877
/// ticks claiming score 144, re-simulating to 56 in the deployed verifier,
/// in a locally rebuilt wasm one and natively alike — three verifiers
/// agreeing with each other and disagreeing with the browser. Nine sim
/// systems read `Time::elapsed_secs()`. **One extra tick of menu time is
/// enough to change the whole run**: re-simulating that replay with the
/// pre-run `Time<Fixed>` advanced by 0/1/2/3 ticks gives scores 56/38/36/40
/// and four different checksums.
///
/// Every probe in this repo was blind to it for one reason — they all enter
/// `Playing` in the app's first frames, exactly like the verifier does, so
/// their offset agreed with it by accident: `selftest::record_scripted_run`,
/// both committed `.gxr` fixtures, `tests/windowed_shape.rs`'s
/// `record_windowed` and the `--playtest` harness. The row that is not blind
/// is `tests/windowed_shape.rs::a_run_does_not_depend_on_how_long_the_app_was_up_before_it`,
/// which dwells in `Menu` first.
///
/// So the sim owns the clock. [`sync_run_clock`] derives it from [`SimTick`]
/// at the head of `SimSet`, which makes it exactly reproducible from the
/// replay — the tick index is the one thing a recorded run and its replay
/// always agree on — and impossible to drift from the tick count.
/// `Time::delta_secs()` stays fine and is deliberately not forbidden: inside
/// `FixedUpdate` it is always the fixed timestep.
///
/// `ticks` is the same number as [`SimTick`], carried here so a game whose
/// phases are integer (every 90 ticks, alternate on parity) never has to
/// reach for a float at all.
///
/// The greppable half of this is enforced by the
/// `no_app_lifetime_clock_in_sim_code` test below (`cargo test`) and advised
/// on by `tools/rollout-replay.sh`; see docs/replay-verification.md,
/// determinism rule 7.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq)]
pub struct RunClock {
    /// Seconds since the run's first tick.
    pub secs: f32,
    /// Ticks since the run began — the same number as [`SimTick`].
    pub ticks: u64,
}

/// Head of `SimSet`, chained right after [`advance_tick`]: restates
/// [`RunClock`] as "ticks so far x the fixed timestep", before anything can
/// read it.
///
/// `secs` is **`ticks as f32 / TICK_HZ as f32`**, exactly that expression and
/// nothing cleverer. It is the formula the fleet's ports had already written
/// by hand (`tick.0 as f32 / sim::TICK_HZ as f32` in Sundae Shooter and
/// Gulper), so a game can migrate its own phase clock onto this one without
/// moving a single folded bit. The obvious alternative,
/// `(tick_duration() * n).as_secs_f32()`, differs from it by one f32 ulp on
/// about a quarter of all tick indices (first at tick 23), which changed a
/// folded bowl position and broke a committed fixture when it was tried. A
/// game whose committed fixtures were recorded against a *different*
/// formula keeps its own clock and leaves this one unread; a new port uses
/// this one. Plain IEEE f32 division, so it is bit-exact on every platform
/// the fleet builds for, and **recomputed rather than accumulated** so it
/// cannot drift from [`SimTick`] however many ticks a run lasts.
/// [`begin_run`] zeroes it, so a second run in the same app starts where
/// the first one did.
pub fn sync_run_clock(tick: Res<SimTick>, mut clock: ResMut<RunClock>) {
    clock.secs = tick.0 as f32 / TICK_HZ as f32;
    clock.ticks = u64::from(tick.0);
}

/// The ordered fixed-tick chain. Everything that mutates run state goes
/// here, in order; nothing in `Update` writes run state.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SimSet;

/// The held set of the last tick that actually reached [`SimSet`] — the last
/// tick the replay recorded.
///
/// `collect_tick_input` runs every fixed tick, paused ones included, so
/// without this the tick that resumes a paused run would derive its press
/// and release edges against a tick nobody recorded. A replay, which only
/// ever sees recorded ticks, would derive them against the last *simulated*
/// tick instead, and the two would disagree: press A during a pause, hold it
/// through the resume, and the live run sees no press edge while its replay
/// sees one. See [`remember_sim_prev`] and [`restore_tick_frame_while_paused`].
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SimPrev(pub Buttons);

/// Latched by the sim system that ends the run, on the very tick it ends;
/// cleared by [`begin_run`]. While it is set, [`run_not_over`] keeps
/// `SimSet` from running, so no further tick is simulated or recorded.
///
/// This exists because the windowed game and the headless verifier leave
/// `Playing` at different moments. The windowed game requests a
/// `ScreenFade` (`DEFAULT_FADE_SECS` = 0.4 s), which is driven from
/// `Update` with the frame delta — roughly 24 more fixed ticks at 60 fps,
/// but however many the frame rate happens to produce. The verifier has no
/// fade and leaves on the ending tick itself. Without the latch those extra
/// ticks are recorded and folded into [`Checksum`], and a real run that
/// reaches the game-over screen seals a checksum its own replay can never
/// reproduce — while a selftest that ends by input exhaustion passes
/// happily. Freezing the sim on the ending tick makes both paths seal the
/// same tick count and the same checksum.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RunOver(pub bool);

/// Run condition: true until the sim system that ends the run has latched
/// [`RunOver`]. Third clause of `SimSet`'s run condition, alongside
/// `in_state(Playing)` and `states::not_paused`.
pub fn run_not_over(over: Res<RunOver>) -> bool {
    !over.0
}

/// Ends the run from a `SimSet` system: latch [`RunOver`] so this is the
/// last simulated and recorded tick, then leave `Playing` — through the
/// fade when there is one (the windowed game), directly through `NextState`
/// when there is not (the headless verifier, which has no UI at all).
///
/// Call it with the fade as `Option<ResMut<ScreenFade>>`; that is the one
/// signature that compiles in both apps. The fade then plays over a frozen
/// sim, which is visually identical since the run is already over.
///
/// ```ignore
/// fn update_flow(
///     mut over: ResMut<sim::RunOver>,
///     fade: Option<ResMut<ScreenFade>>,
///     mut next: ResMut<NextState<GameState>>,
///     /* ... */
/// ) {
///     if lives_are_gone {
///         sim::end_run(&mut over, fade, &mut next);
///         return;
///     }
/// }
/// ```
pub fn end_run(
    over: &mut RunOver,
    fade: Option<ResMut<ScreenFade>>,
    next: &mut NextState<GameState>,
) {
    over.0 = true;
    match fade {
        // One request; `ScreenFade` rejects re-requests while busy anyway.
        Some(mut fade) => {
            fade.request(GameState::GameOver);
        }
        None => next.set(GameState::GameOver),
    }
}

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
        Self {
            bytes,
            origin: SeedOrigin::Local,
        }
    }
}

/// A host-supplied seed waiting for the next run.
///
/// **This, not `GameRng`, is what a test harness must set.** `begin_run`
/// overwrites `GameRng` on every `OnEnter(Playing)` from `RunSeed`, so a
/// harness that inserts its own seeded RNG resource before entering `Playing`
/// has it silently thrown away and runs on a locally drawn seed instead —
/// deterministic-looking code that is not. Stage the seed here and let
/// `begin_run` do the reseeding, exactly as a host-issued seed does; see
/// [`seed_bytes`] for widening a `u64` harness seed to the 32 bytes this
/// takes.
#[derive(Resource, Debug, Default)]
pub struct PendingSeed(pub Option<[u8; 32]>);

/// Widens a harness/CLI `u64` seed into the 32 bytes [`PendingSeed`] takes.
///
/// Test harnesses and balance sweeps are seeded with a plain integer; the
/// replay format carries 32 bytes. This is the one conversion, so a game's
/// `--seed 7` means the same run everywhere. It is a fixed, documented
/// expansion (the `u64` little-endian, repeated across all four 8-byte
/// lanes with a per-lane counter mixed in so the lanes differ), not a hash:
/// it must never change, or every recorded harness seed means something
/// different afterwards.
pub fn seed_bytes(seed: u64) -> [u8; 32] {
    let mut out = [0u8; 32];
    // Indexed rather than `chunks_exact_mut(8)`: newer clippy rejects a
    // constant chunk size in favour of `as_chunks_mut`, which is not stable
    // on every toolchain the fleet builds with. This compiles everywhere.
    for lane in 0..4usize {
        let mixed = seed
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            .wrapping_add(lane as u64);
        out[lane * 8..lane * 8 + 8].copy_from_slice(&mixed.to_le_bytes());
    }
    out
}

/// Monotonic spawn index: the canonical **stable per-entity key** determinism
/// rule 1's sorting clause asks for. Stamp it on every sim entity whose
/// iteration order could change an outcome, and sort by it instead of
/// trusting the order a `Query` happens to yield.
///
/// It exists because query iteration order is archetype order, and an
/// entity's archetype changes the moment a component is inserted on it. The
/// windowed game decorates sim entities from `Update` (meshes, materials,
/// sparkles, eye children); the headless verifier builds none of that, so
/// the two apps hand the same entities to the same sim systems in different
/// orders. `Entity` is no help — its value depends on allocation order,
/// which is exactly what is in question. This key is assigned by the sim, in
/// the order the sim spawns things, so both paths agree on it.
///
/// Query it **non-optionally**. An entity spawned without one then stops
/// matching, which a playtest notices at once — far better than a `Default`
/// of 0 on every un-stamped entity, which collides with every other default
/// and hands the tie straight back to archetype order.
///
/// ```ignore
/// let mut live: Vec<(Entity, &Crumb)> = crumbs.iter().collect();
/// live.sort_unstable_by_key(|(e, _)| *order.get(*e).unwrap());
/// ```
#[derive(Component, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct SpawnOrder(pub u64);

/// The counter behind [`SpawnOrder`]. Reset by [`begin_run`] before anything
/// a run spawns exists, so the same seed stamps the same keys every time and
/// a replay sorts its entities by the same numbers the recorded run did.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SpawnCounter(pub u64);

impl SpawnCounter {
    /// Takes the next key. Call it once per spawned sim entity, in the order
    /// the sim decides to spawn them. (Named `stamp`, not `next`: clippy's
    /// `should_implement_trait` reserves an inherent `next` for `Iterator`.)
    pub fn stamp(&mut self) -> SpawnOrder {
        let order = SpawnOrder(self.0);
        self.0 += 1;
        order
    }
}

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
/// reseed the RNG, zero the tick, checksum and [`SpawnCounter`], reset the
/// tick-input press-edge tracker, zero [`RunClock`], and clear the
/// [`RunOver`] latch the previous run may have left set. Without the
/// tick-input reset, `collect_tick_input` would
/// derive tick 1's `just_pressed` against whatever was held in the menu
/// (live play) or nothing at all (`verify()`'s fresh `App`) — two different
/// starting points that would make the same first tick reproduce different
/// press edges.
pub fn begin_run(
    mut pending: ResMut<PendingSeed>,
    mut seed: ResMut<RunSeed>,
    mut rng: ResMut<GameRng>,
    mut tick: ResMut<SimTick>,
    mut sum: ResMut<Checksum>,
    mut frame: ResMut<TickFrame>,
    mut sim_prev: ResMut<SimPrev>,
    mut over: ResMut<RunOver>,
    mut spawn: ResMut<SpawnCounter>,
    mut clock: ResMut<RunClock>,
) {
    *seed = match pending.0.take() {
        Some(bytes) => RunSeed {
            bytes,
            origin: SeedOrigin::Host,
        },
        None => RunSeed::local(),
    };
    *rng = GameRng::from_seed(&seed.bytes);
    *tick = SimTick(0);
    // Zeroed here as well as restated by `sync_run_clock`, so a system that
    // reads it between `OnEnter(Playing)` and the run's first tick sees this
    // run's zero rather than the previous run's last value.
    *clock = RunClock::default();
    *sum = Checksum::default();
    *frame = TickFrame::default();
    *sim_prev = SimPrev::default();
    *over = RunOver::default();
    // Before anything this run spawns exists: `begin_run` is chained ahead
    // of every OnEnter(Playing) spawn system, so the run's first stamped
    // entity is always key 0.
    *spawn = SpawnCounter::default();
}

/// First in `SimSet`, with [`sync_run_clock`] chained straight after it.
pub fn advance_tick(mut tick: ResMut<SimTick>) {
    tick.0 += 1;
}

/// Last in `SimSet`: remembers the held set this tick was simulated with, so
/// a later paused stretch can restore it (see [`SimPrev`]). Read straight off
/// `TickFrame`, which `collect_tick_input` has already set to this tick's
/// held set.
pub fn remember_sim_prev(frame: Res<TickFrame>, mut sim_prev: ResMut<SimPrev>) {
    sim_prev.0 = frame.prev();
}

/// `FixedUpdate`, after `SimSet`, only while `Playing` and paused: rewinds
/// `TickFrame` to the last simulated tick's held set, so the tick that
/// resumes derives its edges against exactly what the replay's feeder will.
///
/// Placed after `SimSet` rather than in `FixedPreUpdate` on purpose. On the
/// tick that *starts* the pause, `toggle_pause` (which runs before `SimSet`)
/// has already flipped `Paused`, so that tick is rewound too — which matters
/// when the player resumes on the very next tick, whose edges would
/// otherwise be derived against a pause tick the replay never recorded. On
/// the tick that *ends* the pause, `toggle_pause` has already cleared
/// `Paused`, so this system correctly does nothing and `TickFrame` keeps the
/// held set `remember_sim_prev` just stored.
///
/// A host pause (`gx:set`, applied from `Update`) is covered by the same run
/// condition.
///
/// `Buttons::PAUSE` is exempt from the rewind and keeps its live value.
/// Every recorded tick has PAUSE masked out (`ReplayRecorder::push`), so the
/// bit can never affect anything a replay reproduces — there is nothing to
/// keep in sync. It *is* what un-pauses the game: `toggle_pause` reads
/// `pause_just_pressed`, which `collect_tick_input` derives as
/// `(held | latched) \ prev`, and every real pause path (keyboard
/// `pressed(Escape)`, gamepad button 9, the touch overlay) holds PAUSE for
/// as long as the button is down rather than only latching it. Rewinding
/// PAUSE out of `prev` would therefore re-fire the press on the very next
/// tick and drop the pause after one tick, leaving the game running under
/// its own pause overlay.
pub fn restore_tick_frame_while_paused(sim_prev: Res<SimPrev>, mut frame: ResMut<TickFrame>) {
    let live_pause = Buttons(frame.prev().0 & Buttons::PAUSE.0);
    frame.set_prev(sim_prev.0.difference(Buttons::PAUSE).union(live_pause));
}

/// Folds the two things every game has: the leaderboard score and the tick.
/// Game-agnostic on purpose (see the module doc) — this is one of the files
/// `tools/rollout-replay.sh` copies verbatim into every game, so it must
/// compile and mean the same thing regardless of what a game's own run
/// state looks like. Each game folds its own key state (player/ball
/// position, hole index, lives, ...) in its own system placed right after
/// this one in the `SimSet` chain, bit-exactly (`f32::to_bits`) so the
/// checksum is sensitive to float behaviour, which is what the
/// native-vs-wasm fixture check relies on.
pub fn checksum_tick(data: Res<GameData>, tick: Res<SimTick>, mut sum: ResMut<Checksum>) {
    sum.fold(u64::from(data.leaderboard_score()));
    sum.fold(u64::from(tick.0));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_bytes_is_deterministic_distinct_and_stable() {
        // Same input, same output — the whole point.
        assert_eq!(seed_bytes(7), seed_bytes(7));
        // Different inputs must not collide, or two harness seeds would be
        // the same run.
        assert_ne!(seed_bytes(7), seed_bytes(8));
        assert_ne!(seed_bytes(0), seed_bytes(u64::MAX));
        // The four 8-byte lanes must differ, or the RNG is seeded with one
        // value repeated and loses entropy.
        let b = seed_bytes(7);
        let lanes: Vec<&[u8]> = (0..4).map(|l| &b[l * 8..l * 8 + 8]).collect();
        for (i, a) in lanes.iter().enumerate() {
            for (j, other) in lanes.iter().enumerate().skip(i + 1) {
                assert_ne!(a, other, "lanes {i} and {j} are identical");
            }
        }
        // Pinned: changing this expansion silently changes what every
        // recorded harness seed means.
        assert_eq!(
            seed_bytes(1),
            [
                0x15, 0x7c, 0x4a, 0x7f, 0xb9, 0x79, 0x37, 0x9e, 0x16, 0x7c, 0x4a, 0x7f, 0xb9, 0x79,
                0x37, 0x9e, 0x17, 0x7c, 0x4a, 0x7f, 0xb9, 0x79, 0x37, 0x9e, 0x18, 0x7c, 0x4a, 0x7f,
                0xb9, 0x79, 0x37, 0x9e,
            ]
        );
        // And it really does drive the RNG apart.
        let mut x = GameRng::from_seed(&seed_bytes(7));
        let mut y = GameRng::from_seed(&seed_bytes(8));
        assert_ne!(x.0.next_u64(), y.0.next_u64());
    }

    #[test]
    fn spawn_keys_are_monotonic_and_totally_ordered() {
        // The key has to be a total order the sim owns: sorting by it must
        // put entities back into spawn order regardless of what order they
        // came out of a query in.
        let mut c = SpawnCounter::default();
        let a = c.stamp();
        let b = c.stamp();
        assert!(a < b);
        assert_eq!(c.0, 2);
        let mut v = vec![b, a];
        v.sort_unstable();
        assert_eq!(v, vec![a, b]);
    }

    #[test]
    fn begin_run_rewinds_the_spawn_counter_so_every_run_stamps_the_same_keys() {
        use bevy::ecs::system::RunSystemOnce;
        // Whatever the previous run left behind, the next run's first spawn
        // is key 0 again — otherwise a replay would sort its entities by
        // different numbers than the run it is reproducing, which is exactly
        // the desync the key exists to prevent.
        let mut world = World::new();
        world.init_resource::<PendingSeed>();
        world.init_resource::<RunSeed>();
        world.init_resource::<GameRng>();
        world.init_resource::<SimTick>();
        world.init_resource::<Checksum>();
        world.init_resource::<TickFrame>();
        world.init_resource::<SimPrev>();
        world.init_resource::<RunOver>();
        world.init_resource::<RunClock>();
        world.insert_resource(SpawnCounter(17));
        world.run_system_once(begin_run).unwrap();
        assert_eq!(world.resource_mut::<SpawnCounter>().stamp(), SpawnOrder(0));
    }

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
    fn begin_run_resets_tick_frame_so_a_fresh_run_reproduces_press_edges() {
        use bevy::prelude::*;
        use gamebient_input::input::{TickFrame, collect_tick_input};
        use gamebient_input::{Buttons, InputAccumulator, TickInput, TickInputSet};

        let mut app = App::new();
        app.init_resource::<InputAccumulator>()
            .init_resource::<TickFrame>()
            .init_resource::<TickInput>()
            .init_resource::<PendingSeed>()
            .init_resource::<RunSeed>()
            .init_resource::<GameRng>()
            .init_resource::<SimTick>()
            .init_resource::<Checksum>()
            .init_resource::<SimPrev>()
            .init_resource::<RunOver>()
            .init_resource::<SpawnCounter>()
            .init_resource::<RunClock>()
            .add_systems(
                FixedPreUpdate,
                collect_tick_input.in_set(TickInputSet::Collect),
            )
            .add_systems(Update, begin_run);

        // Menu-time input: A is already held (e.g. a stray keypress) before
        // the run starts, so `TickFrame.prev` carries it by the time
        // `Playing` begins.
        app.world_mut().resource_mut::<InputAccumulator>().held = Buttons::A;
        app.world_mut().run_schedule(FixedPreUpdate);
        assert!(app.world().resource::<TickInput>().primary_just_pressed);
        // Held continuously into a second tick: no new edge, as expected.
        app.world_mut().resource_mut::<InputAccumulator>().held = Buttons::A;
        app.world_mut().run_schedule(FixedPreUpdate);
        assert!(
            !app.world().resource::<TickInput>().primary_just_pressed,
            "sanity: A held across two ticks should not re-edge"
        );

        // `begin_run` (`OnEnter(Playing)`) must reset `TickFrame` so tick 1
        // of the run - live or replayed - reproduces the same press edge
        // regardless of what was held a moment earlier in the menu.
        app.world_mut().run_schedule(Update); // runs begin_run

        app.world_mut().resource_mut::<InputAccumulator>().held = Buttons::A;
        app.world_mut().run_schedule(FixedPreUpdate);
        assert!(
            app.world().resource::<TickInput>().primary_just_pressed,
            "begin_run should have reset TickFrame so a still-held A \
             reproduces a press edge on the run's first tick"
        );
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
    fn end_run_latches_and_falls_back_to_next_state_without_a_fade() {
        // The headless verifier has no `ScreenFade` at all, so `end_run`
        // must both freeze the sim and drive the transition itself.
        let mut over = RunOver::default();
        let mut next = NextState::<GameState>::default();
        end_run(&mut over, None, &mut next);
        assert!(over.0, "the latch must freeze SimSet on this very tick");
        assert!(
            matches!(next, NextState::Pending(GameState::GameOver)),
            "no fade means end_run owns the transition"
        );
    }

    #[test]
    fn end_run_latches_and_leaves_the_transition_to_the_fade() {
        // The windowed game keeps its fade-to-black: `end_run` asks for it
        // and leaves `NextState` alone, so the fade's own completion drives
        // the transition — over a sim that is already frozen.
        use bevy::ecs::system::RunSystemOnce;

        fn ends_the_run(
            mut over: ResMut<RunOver>,
            fade: Option<ResMut<ScreenFade>>,
            mut next: ResMut<NextState<GameState>>,
        ) {
            end_run(&mut over, fade, &mut next);
        }

        let mut world = World::new();
        world.init_resource::<RunOver>();
        world.init_resource::<NextState<GameState>>();
        world.insert_resource(ScreenFade::default());
        world.run_system_once(ends_the_run).unwrap();

        assert!(world.resource::<RunOver>().0);
        assert!(!world.resource::<ScreenFade>().is_idle(), "fade requested");
        assert!(matches!(
            *world.resource::<NextState<GameState>>(),
            NextState::Unchanged
        ));
    }

    #[test]
    fn run_clock_is_the_tick_index_not_the_app_lifetime_clock() {
        use bevy::ecs::system::RunSystemOnce;
        // The whole point: the same tick number always means the same
        // seconds, whatever the app happened to be doing beforehand.
        let mut world = World::new();
        world.init_resource::<RunClock>();
        world.insert_resource(SimTick(0));
        world.run_system_once(sync_run_clock).unwrap();
        assert_eq!(world.resource::<RunClock>().secs, 0.0);
        assert_eq!(world.resource::<RunClock>().ticks, 0);
        world.insert_resource(SimTick(60));
        world.run_system_once(sync_run_clock).unwrap();
        let one_second = world.resource::<RunClock>().secs;
        assert!(
            (one_second - 1.0).abs() < 1e-4,
            "60 ticks should be ~1 s, got {one_second}"
        );
        assert_eq!(world.resource::<RunClock>().ticks, 60);
        // And it is a restatement of the tick, not an accumulator: running
        // it twice on the same tick must not advance it. An accumulator
        // would drift from `SimTick` the moment a tick ran it twice or not
        // at all, which is the failure mode this shape removes.
        world.run_system_once(sync_run_clock).unwrap();
        assert_eq!(world.resource::<RunClock>().secs, one_second);
        assert_eq!(world.resource::<RunClock>().ticks, 60);
        // The formula is a contract, not an implementation detail: ports
        // migrate hand-written `tick as f32 / TICK_HZ as f32` clocks onto
        // this resource on the promise that every bit stays the same. And
        // the doc comment's ulp warning must stay true: the Duration-based
        // alternative really does disagree somewhere in a normal run.
        let mut differs = false;
        for n in 0..=3600u32 {
            world.insert_resource(SimTick(n));
            world.run_system_once(sync_run_clock).unwrap();
            let secs = world.resource::<RunClock>().secs;
            assert_eq!(secs.to_bits(), (n as f32 / TICK_HZ as f32).to_bits());
            differs |= secs.to_bits() != (tick_duration() * n).as_secs_f32().to_bits();
        }
        assert!(
            differs,
            "the Duration formula now agrees everywhere; the doc's ulp warning is stale"
        );
    }

    #[test]
    fn no_app_lifetime_clock_in_sim_code() {
        // `Time::elapsed_secs()` inside `FixedUpdate` is `Time<Fixed>`'s
        // elapsed, which counts from app start and not from run start — see
        // [`RunClock`]. A sim system that reads it forks on how long the
        // player sat in the menu, which no fixture can see (they all begin
        // their run in the app's first frames, exactly as the verifier
        // does) and which cost Dive Rise a production `UNVERIFIED`. Read
        // `sim::RunClock` instead.
        //
        // `delta_secs()` is fine and deliberately not listed: inside
        // `FixedUpdate` it is always the fixed timestep.
        //
        // Each needle below is itself an instance of what it forbids, so
        // this line carries the marker the per-line skip honours — which
        // exempts exactly this list and nothing else. `allow-app-clock` is
        // also the opt-out for a genuine non-sim read (a dev harness, a
        // wall-clock stamp on the replay header); write why on the line.
        let forbidden = [".elapsed_secs()", ".elapsed_secs_f64()", ".elapsed()"]; // allow-app-clock
        let mut hits = Vec::new();
        for entry in walk(std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/game"
        ))) {
            let text = std::fs::read_to_string(&entry).unwrap();
            for (n, line) in text.lines().enumerate() {
                let code = line.trim_start();
                if code.starts_with("//") || line.contains("allow-app-clock") {
                    continue;
                }
                for needle in forbidden {
                    if code.contains(needle) {
                        hits.push(format!("{}:{}: {}", entry.display(), n + 1, code.trim()));
                    }
                }
            }
        }
        assert!(
            hits.is_empty(),
            "app-lifetime clock read in sim code (use sim::RunClock):\n{}",
            hits.join("\n")
        );
    }

    #[test]
    fn no_local_state_in_sim_systems() {
        // The sibling of `no_app_lifetime_clock_in_sim_code`, and the same
        // bug class from the other end: state the recording app has and the
        // verifier does not.
        //
        // A `Local<T>` belongs to the **system instance**, so it lives as
        // long as the `App` and no run start can reach it — not
        // `OnEnter(Playing)`, not `begin_run`, not a `reset_run` however
        // careful. The verifier is always a fresh app that plays exactly
        // one run, so every `Local` it owns starts at `Default`. A browser
        // is not: run 2 begins with whatever run 1 left in there, and
        // re-simulates into a different game.
        //
        // That is not hypothetical. Grand Theft Auto-Reply's
        // `selection::handle_input` — a `SimSet` system — kept the cursor's
        // and the crime wheel's held-direction auto-repeat in two
        // `Local<Repeat>`s, so a second run in one browser session started
        // holding run 1's last direction with its repeat timer already
        // past the delay, and swallowed a cursor step the verifier emits.
        // Found by the fleet clock audit (grand-theft-auto-reply#8) because
        // nothing in the fleet recorded two runs in one `App`;
        // `tests/windowed_shape.rs::a_second_run_in_the_same_app_reproduces_the_first`
        // is the behavioural half of this test and does.
        //
        // The rule: **run state lives in a resource or a component that
        // `OnEnter(Playing)` resets.** Never a `Local<_>`, never a
        // `static`, never something a plugin computed once at build time.
        //
        // Scope is the files whose systems `SimSet` runs —
        // presentation and `Update` systems may keep `Local`s freely, and
        // that is most of what the parameter is for (a change detector, a
        // "have I spawned the overlay yet", a dedupe). `// allow-local:
        // <reason>` on the line is the opt-out for a `Local` in a sim file
        // that genuinely cannot carry run state across runs (one in an
        // `Update` system that happens to live in the same file, a
        // `#[cfg(test)]` helper); write why on the line.
        //
        // The needle is itself an instance of what it forbids, so this line
        // carries the marker, exactly like the two scans beside it.
        let needle = "Local<"; // allow-local: the needle itself
        let root = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/src/game"));
        let sim_files = sim_set_files(root);
        // Vacuity guard. This scan derives its scope from the
        // registrations, so a game that writes them in a shape it does not
        // recognise would be scanning nothing at all and passing for it —
        // the one failure mode a source scan cannot report as a hit. If
        // this fires during a port, teach `sim_set_files` that game's
        // registration shape rather than deleting the test.
        assert!(
            !sim_files.is_empty(),
            "no file was found registering systems into SimSet, so this scan \
             covered nothing. It looks for `.add_systems(..)` carrying \
             `.in_set(..SimSet)`; if this game registers its chain some other \
             way, extend sim_set_files to match."
        );
        let mut hits = Vec::new();
        for entry in &sim_files {
            let text = std::fs::read_to_string(entry).unwrap();
            for (n, line) in text.lines().enumerate() {
                let code = line.trim_start();
                if code.starts_with("//") || line.contains("allow-local") {
                    continue;
                }
                if code.contains(needle) {
                    hits.push(format!("{}:{}: {}", entry.display(), n + 1, code.trim()));
                }
            }
        }
        assert!(
            hits.is_empty(),
            "a sim system's file keeps state in a system-local parameter, \
             which outlives the \
             run (and the whole App) — the verifier's fresh app starts it at \
             Default and the browser's second run does not. Move it into run \
             state OnEnter(Playing) resets; see docs/replay-verification.md \
             rule 7 and tests/windowed_shape.rs::\
             a_second_run_in_the_same_app_reproduces_the_first. Mark a \
             genuinely run-free one `// allow-local: <reason>`.\n{}",
            hits.join("\n")
        );
    }

    /// Every file under `src/game/` whose systems `SimSet` runs: the files
    /// that *register* a chain into it, plus the files the registered
    /// system paths name.
    ///
    /// Derived from the registrations rather than from a hand-kept list, so
    /// a port cannot forget to add its new gameplay module to it. A chunk
    /// counts when it registers `.in_set(..SimSet)`; `.before(SimSet)` and
    /// `.after(SimSet)` deliberately do not, since those systems are
    /// outside the chain (`toggle_pause` is the template's own).
    ///
    /// It over-approximates on purpose: every `a::b` path in the
    /// registration's text — comments included — that resolves to a file
    /// under `src/game/` is scanned. A module named in a comment beside the
    /// chain is one somebody thought belonged there.
    fn sim_set_files(root: &std::path::Path) -> Vec<std::path::PathBuf> {
        let mut out: Vec<std::path::PathBuf> = Vec::new();
        let push = |p: std::path::PathBuf, out: &mut Vec<std::path::PathBuf>| {
            if !out.contains(&p) {
                out.push(p);
            }
        };
        for entry in walk(root) {
            let text = std::fs::read_to_string(&entry).unwrap();
            let mut registers = false;
            // One chunk per `add_systems(` call: the call's own text plus
            // whatever follows it up to the next call. Splitting here is
            // what keeps a `.before(sim::SimSet)` in a *different*
            // registration from pulling that registration's systems in.
            for chunk in text.split("add_systems(").skip(1) {
                if !registers_into_sim_set(chunk) {
                    continue;
                }
                registers = true;
                for path in module_paths(chunk) {
                    if let Some(file) = resolve_module(root, &path) {
                        push(file, &mut out);
                    }
                }
            }
            if registers {
                push(entry, &mut out);
            }
        }
        out
    }

    /// Does this `add_systems` chunk put something **in** `SimSet`?
    fn registers_into_sim_set(chunk: &str) -> bool {
        chunk.match_indices("in_set(").any(|(i, _)| {
            let rest = &chunk[i..];
            rest[..rest.len().min(64)].contains("SimSet")
        })
    }

    /// Every `a::b(::c)*` path in `chunk`, as its segments.
    fn module_paths(chunk: &str) -> Vec<Vec<String>> {
        let bytes = chunk.as_bytes();
        let mut out = Vec::new();
        let mut i = 0;
        while i < bytes.len() {
            let c = bytes[i] as char;
            if !(c.is_ascii_alphabetic() || c == '_') {
                i += 1;
                continue;
            }
            let start = i;
            while i < bytes.len() {
                let c = bytes[i] as char;
                if c.is_ascii_alphanumeric() || c == '_' {
                    i += 1;
                } else if c == ':' && bytes.get(i + 1) == Some(&b':') {
                    i += 2;
                } else {
                    break;
                }
            }
            let path = &chunk[start..i];
            if path.contains("::") {
                let segs: Vec<String> = path
                    .split("::")
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .collect();
                if segs.len() > 1 {
                    out.push(segs);
                }
            }
        }
        out
    }

    /// The file a system path's module lives in, if it is one of ours.
    ///
    /// `player::move_player` is `src/game/player.rs`,
    /// `replay::recorder::record_tick` is `src/game/replay/recorder.rs`,
    /// `hunters::viperfish::attack` is `src/game/hunters/viperfish.rs` (or
    /// its `mod.rs`). A path whose module segments are not lowercase is an
    /// associated item (`GameState::Playing`, `Buttons::A`), and a path
    /// that resolves to no file is another crate's
    /// (`gamebient_input::input::accumulate_input`).
    fn resolve_module(root: &std::path::Path, segs: &[String]) -> Option<std::path::PathBuf> {
        let mods: Vec<&str> = segs[..segs.len() - 1]
            .iter()
            .map(String::as_str)
            .filter(|s| !matches!(*s, "crate" | "self" | "super" | "game"))
            .collect();
        let (last, rest) = mods.split_last()?;
        if mods
            .iter()
            .any(|s| !s.starts_with(|c: char| c.is_ascii_lowercase() || c == '_'))
        {
            return None;
        }
        let mut dir = root.to_path_buf();
        for seg in rest {
            dir = dir.join(seg);
        }
        let flat = dir.join(format!("{last}.rs"));
        if flat.is_file() {
            return Some(flat);
        }
        let nested = dir.join(last).join("mod.rs");
        nested.is_file().then_some(nested)
    }

    #[test]
    fn no_forbidden_randomness_or_hashmaps_in_game_code() {
        // Each literal below is itself an instance of what it forbids, so it
        // carries the same `allow-forbidden-rng` marker `RunSeed::local`
        // uses above — the per-line skip below already honours it, which
        // exempts exactly this list (and nothing else) without needing to
        // special-case this file or truncate the scan.
        let forbidden = [
            "rand::rng()",               // allow-forbidden-rng
            "from_os_rng",               // allow-forbidden-rng
            "thread_rng",                // allow-forbidden-rng
            "SmallRng",                  // allow-forbidden-rng
            "std::collections::HashMap", // allow-forbidden-rng
        ];
        let mut hits = Vec::new();
        for entry in walk(std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/game"
        ))) {
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
        assert!(
            hits.is_empty(),
            "forbidden in sim code:\n{}",
            hits.join("\n")
        );
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
