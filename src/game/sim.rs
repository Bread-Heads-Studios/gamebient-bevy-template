//! Determinism scaffolding: the fixed tick, the one RNG, the run seed and
//! the checksum a replay must reproduce. See docs/replay-verification.md.

use std::time::Duration;

use bevy::prelude::*;
use gamebient_input::Buttons;
use gamebient_input::input::TickFrame;
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
/// press-edge tracker. Without that last reset, `collect_tick_input` would
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
pub fn restore_tick_frame_while_paused(sim_prev: Res<SimPrev>, mut frame: ResMut<TickFrame>) {
    frame.set_prev(sim_prev.0);
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
