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
pub mod selftest;

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
    /// Merges into the last run when `(held, latched, ax, ay)` are equal and
    /// its count hasn't saturated; else appends a new run. Always increments
    /// `ticks`.
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
        self.runs.push(TickRun {
            held,
            latched,
            ax,
            ay,
            count: 1,
        });
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
        let build = core::str::from_utf8(c.take(n)?)
            .map_err(|_| DecodeError::BadBuild)?
            .to_string();
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
        Ok(Replay {
            build,
            tick_hz,
            seed,
            origin,
            ticks,
            score,
            checksum,
            runs,
        })
    }
}

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
        .add_plugins(bevy::input::InputPlugin)
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
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Playing);
    // Apply the transition with zero elapsed time: `StateTransition` (which
    // runs `OnEnter(Playing)` / `begin_run`, resetting `SimTick`) happens
    // before `RunFixedMainLoop` within the same `app.update()`. If this
    // update also carried a full tick's worth of accumulated time, that
    // first fixed tick would run right here — before the loop below gets a
    // chance to check `done`/cap for a 0- or 1-tick replay, over-running it
    // by one tick. Feeding zero delta this update means no fixed tick can
    // fire yet, so every tick the loop below observes came from its own
    // update.
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
    app.update(); // applies the transition; OnEnter(Playing) runs begin_run
    app.insert_resource(TimeUpdateStrategy::ManualDuration(tick_duration()));
    if replay.origin != super::sim::SeedOrigin::Host {
        app.world_mut().resource_mut::<RunSeed>().origin = replay.origin;
    }
    let cap = replay.ticks.saturating_add(60);
    // Checked right after every update that can run a fixed tick, including
    // the transition update above, so a 0- or 1-tick replay is never made
    // to run one tick further than it claims.
    let check_ended = |app: &App| -> Option<Ended> {
        if *app.world().resource::<State<GameState>>().get() != GameState::Playing {
            return Some(Ended::GameOver);
        }
        if app.world().resource::<ReplayFeeder>().done {
            return Some(Ended::InputExhausted);
        }
        if app.world().resource::<SimTick>().0 >= cap {
            return Some(Ended::Cap);
        }
        None
    };
    let ended = loop {
        if let Some(ended) = check_ended(&app) {
            break ended;
        }
        app.update();
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
        assert_eq!(
            r.runs[0],
            TickRun {
                held: 8,
                latched: 0,
                ax: 0,
                ay: 0,
                count: 3
            }
        );
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
        assert_eq!(
            Replay::decode(&bytes[..bytes.len() - 1]),
            Err(DecodeError::Truncated)
        );
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

    #[test]
    fn headless_app_runs_a_replay_to_a_verdict() {
        // A 120-tick run holding RIGHT: the template scores nothing, so the
        // verdict is score 0 with the checksum the sim produced; encode the
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
    fn verify_zero_tick_replay_reports_zero_ticks() {
        // A 0-tick replay (transitioned into Playing and immediately out,
        // or simply never advanced) must not have a phantom tick folded in
        // by the transition update that applies `OnEnter(Playing)`.
        let mut r = sample();
        r.runs.clear();
        r.ticks = 0;
        r.score = 0;
        let v = verify(&r);
        assert_eq!(v.ticks, 0, "{v:?}");
        assert_eq!(v.ended, Ended::InputExhausted);
    }

    #[test]
    fn verify_one_tick_replay_reports_one_tick_and_matches_its_own_checksum() {
        let mut r = sample();
        r.runs.clear();
        r.ticks = 0;
        r.push_tick(8, 0, 0, 0);
        r.score = 0;
        let first = verify(&r);
        assert_eq!(first.ticks, 1, "{first:?}");
        assert_eq!(first.ended, Ended::InputExhausted);
        r.checksum = first.checksum;
        let second = verify(&r);
        assert!(second.matches, "{second:?}");
    }

    #[test]
    fn verify_masks_a_replayed_pause_bit_so_the_sim_never_freezes() {
        // A replay whose runs still carry `Buttons::PAUSE` (256) in `held`
        // -- e.g. an older file recorded before `ReplayRecorder::push`
        // started stripping it -- must not re-press pause on replay: that
        // would freeze `SimSet` and strand the verifier well short of
        // `replay.ticks`.
        const PAUSE: u16 = 1 << 8;
        let mut r = sample();
        r.runs.clear();
        r.ticks = 0;
        r.push_tick(PAUSE, PAUSE, 0, 0); // looks like a "pause just pressed" tick
        for _ in 0..9 {
            r.push_tick(PAUSE, 0, 0, 0); // and stays "held" every tick after
        }
        r.score = 0;
        let v = verify(&r);
        assert_eq!(v.ticks, 10, "{v:?}");
        assert_eq!(v.ended, Ended::InputExhausted);
    }

    #[test]
    fn verdict_json_is_flat() {
        let v = Verdict {
            score: 1,
            checksum: 2,
            ticks: 3,
            ended: Ended::Cap,
            matches: false,
        };
        assert_eq!(
            v.to_json(),
            r#"{"score":1,"checksum":2,"ticks":3,"ended":"cap","matches":false}"#
        );
    }
}
