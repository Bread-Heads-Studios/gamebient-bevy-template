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
/// reseed the RNG, zero the tick and checksum.
pub fn begin_run(
    mut pending: ResMut<PendingSeed>,
    mut seed: ResMut<RunSeed>,
    mut rng: ResMut<GameRng>,
    mut tick: ResMut<SimTick>,
    mut sum: ResMut<Checksum>,
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

// NOTE for `no_forbidden_randomness_or_hashmaps_in_game_code` below: this
// file is necessarily the one place the forbidden-strings list itself
// contains those strings as data (and the doc comments above name them).
// The walker stops scanning a file at its `#[cfg(test)]` marker (test
// modules are always the last item in a `src/game/` file, by convention),
// so the list literal and these very words never reach the grep.
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
        let forbidden = [
            "rand::rng()",
            "from_os_rng",
            "thread_rng",
            "SmallRng",
            "std::collections::HashMap",
        ];
        let mut hits = Vec::new();
        for entry in walk(std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/game"
        ))) {
            let text = std::fs::read_to_string(&entry).unwrap();
            // The test module itself (this file, from `#[cfg(test)]` down)
            // necessarily contains the forbidden strings as data (this list)
            // and is exempt; test modules are always the last item in a
            // `src/game/` file, so stop scanning once we reach one.
            let scannable = match text.find("#[cfg(test)]") {
                Some(idx) => &text[..idx],
                None => &text[..],
            };
            for (n, line) in scannable.lines().enumerate() {
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
