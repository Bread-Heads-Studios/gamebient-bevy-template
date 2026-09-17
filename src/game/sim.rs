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
/// reseed the RNG, zero the tick and checksum, and reset the tick-input
/// press-edge tracker, and clear the [`RunOver`] latch the previous run may
/// have left set. Without the tick-input reset, `collect_tick_input` would
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
    *sum = Checksum::default();
    *frame = TickFrame::default();
    *sim_prev = SimPrev::default();
    *over = RunOver::default();
}

/// First in `SimSet`.
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
