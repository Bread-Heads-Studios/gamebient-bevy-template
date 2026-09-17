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

/// Hard ceiling on a replay's claimed tick count: one hour at 60 Hz.
///
/// `ticks` drives how long `verify()` re-simulates, so an attacker who can
/// post a handful of bytes could otherwise ask a verifier to run for days.
/// `decode` rejects anything above this before it reads a single run.
pub const MAX_TICKS: u32 = 216_000;

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
    /// The header claims a tick rate this build does not simulate at, so its
    /// sim steps would not line up with the recorded ones.
    BadTickRate(u16),
    /// The header claims more than [`MAX_TICKS`] ticks.
    TooManyTicks(u32),
    RunCountMismatch {
        header: u32,
        sum: u32,
    },
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
        if tick_hz != super::sim::TICK_HZ as u16 {
            return Err(DecodeError::BadTickRate(tick_hz));
        }
        let mut seed = [0u8; 32];
        seed.copy_from_slice(c.take(32)?);
        let o = c.u8()?;
        let origin = SeedOrigin::from_u8(o).ok_or(DecodeError::BadOrigin(o))?;
        let ticks = c.u32()?;
        if ticks > MAX_TICKS {
            return Err(DecodeError::TooManyTicks(ticks));
        }
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
use super::scoring::{GameData, LeaderboardScore};
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
        // `checksum` is a u64 and JSON numbers are IEEE doubles in every
        // JavaScript runtime that reads this, so a value above 2^53 would be
        // silently rounded on JSON.parse. Emit it as a decimal string; the
        // caller compares it verbatim or parses it as a BigInt. `score` and
        // `ticks` stay numeric — both are well inside the safe range.
        format!(
            "{{\"score\":{},\"checksum\":\"{}\",\"ticks\":{},\"ended\":\"{ended}\",\"matches\":{}}}",
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

/// The headless sim wired to replay `replay`: the feeder in
/// `TickInputSet::Feed` and the run's seed staged in `PendingSeed`. Split
/// out of [`verify`] so a test can add its own observer systems to the very
/// app the verifier runs; drive it with [`run_verify_app`].
pub fn build_verify_app(replay: &Replay) -> App {
    let mut app = build_headless_app();
    app.insert_resource(ReplayFeeder::new(replay))
        .add_systems(FixedPreUpdate, feed_tick.in_set(TickInputSet::Feed));
    // Seed the run exactly as the recording game did.
    app.world_mut().resource_mut::<PendingSeed>().0 = Some(replay.seed);
    app
}

/// Re-simulates `replay` and reports what the sim produced.
pub fn verify(replay: &Replay) -> Verdict {
    let mut app = build_verify_app(replay);
    run_verify_app(&mut app, replay)
}

/// Steps `app` (from [`build_verify_app`]) through the whole replay.
pub fn run_verify_app(app: &mut App, replay: &Replay) -> Verdict {
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
        if let Some(ended) = check_ended(app) {
            break ended;
        }
        app.update();
    };
    let score = u64::from(app.world().resource::<GameData>().leaderboard_score());
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

    /// A tick's edges as a replay is able to carry them: `Buttons::PAUSE` is
    /// masked, because `ReplayRecorder::push` strips it (a replayed pause
    /// would freeze the sim it is meant to reproduce), so it is the only bit
    /// a live run and its replay are allowed to disagree about — which is
    /// why sim systems must never read `pause_just_pressed`.
    #[derive(Resource, Default, Debug, Clone, PartialEq, Eq)]
    struct EdgeLog(Vec<(u32, u32, u32, u32)>);

    fn log_edges(
        tick: Res<SimTick>,
        input: Res<gamebient_input::TickInput>,
        mut log: ResMut<EdgeLog>,
    ) {
        let m = |b: gamebient_input::Buttons| b.difference(gamebient_input::Buttons::PAUSE).0;
        log.0.push((
            tick.0,
            m(input.edges.just_pressed),
            m(input.edges.just_released),
            m(input.edges.held),
        ));
    }

    fn log_edges_in_sim_set(app: &mut App) {
        app.init_resource::<EdgeLog>().add_systems(
            FixedUpdate,
            log_edges
                .in_set(crate::game::sim::SimSet)
                .after(recorder::record_tick),
        );
    }

    /// One scripted frame of a live run: the held set, and whether PAUSE is
    /// tapped (latched, never held — exactly what a keyboard Escape tap or a
    /// touch-overlay pause button produces).
    #[derive(Resource, Default)]
    struct Script {
        frames: Vec<(gamebient_input::Buttons, bool)>,
        at: usize,
    }

    fn run_script(mut script: ResMut<Script>, mut virt: ResMut<gamebient_input::VirtualInput>) {
        let i = script.at;
        script.at += 1;
        let Some(&(held, pause)) = script.frames.get(i) else {
            return;
        };
        virt.set_held(held);
        if pause {
            virt.latched |= gamebient_input::Buttons::PAUSE;
        }
    }

    /// `(sim tick, paused)` after every fixed tick of a live run, so a test
    /// can see a pause that silently let go.
    #[derive(Resource, Default, Debug, Clone, PartialEq, Eq)]
    struct PauseTrace(Vec<(u32, bool)>);

    fn log_paused(
        tick: Res<SimTick>,
        paused: Res<crate::game::states::Paused>,
        mut trace: ResMut<PauseTrace>,
    ) {
        trace.0.push((tick.0, paused.0));
    }

    /// Plays `frames` through the live headless app and leaves it in
    /// `Playing`, with an `EdgeLog` and a `PauseTrace` filled in.
    fn build_live_app(frames: Vec<(gamebient_input::Buttons, bool)>) -> App {
        let n = frames.len();
        let mut live = build_headless_app();
        live.insert_resource(Script { frames, at: 0 });
        log_edges_in_sim_set(&mut live);
        live.init_resource::<PauseTrace>().add_systems(
            FixedUpdate,
            log_paused
                .after(crate::game::sim::SimSet)
                .run_if(in_state(GameState::Playing)),
        );
        live.world_mut().resource_mut::<PendingSeed>().0 = Some([0x11u8; 32]);
        // Warm-up update: zero delta, no fixed tick. The script is added
        // afterwards so script frame N drives fixed tick N exactly.
        live.update();
        live.add_systems(
            PreUpdate,
            run_script.before(gamebient_input::input::accumulate_input),
        );
        live.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        for _ in 0..n {
            live.update();
        }
        live
    }

    /// Plays `frames` through the live headless app, seals the replay, then
    /// re-simulates it. Returns (live log, replay log, verdict, replay).
    fn live_then_replay(
        frames: Vec<(gamebient_input::Buttons, bool)>,
    ) -> (EdgeLog, EdgeLog, Verdict, Replay) {
        let mut live = build_live_app(frames);
        live.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::GameOver);
        live.update(); // OnExit(Playing) seals the run
        let replay = live
            .world()
            .resource::<recorder::ReplayRecorder>()
            .last_run()
            .cloned()
            .expect("run sealed on OnExit(Playing)");
        let live_log = live.world().resource::<EdgeLog>().clone();

        let mut replayed = build_verify_app(&replay);
        log_edges_in_sim_set(&mut replayed);
        let verdict = run_verify_app(&mut replayed, &replay);
        let replay_log = replayed.world().resource::<EdgeLog>().clone();
        (live_log, replay_log, verdict, replay)
    }

    #[test]
    fn a_pause_does_not_move_press_edges_between_a_live_run_and_its_replay() {
        use gamebient_input::Buttons;
        // Hold RIGHT for 9 ticks, tap PAUSE on tick 10 while still holding
        // RIGHT, swap to a held A for the 9 paused ticks, then resume on
        // frame 20 with A still down, and keep holding it. Only the 20
        // unpaused frames reach `SimSet`, so the replay never sees the
        // stretch where A was first pressed: the resume tick has to produce
        // `just_pressed(A)` / `just_released(RIGHT)` on both sides.
        let mut frames = vec![(Buttons::RIGHT, false); 9];
        frames.push((Buttons::RIGHT, true)); // 10: pause
        frames.extend(std::iter::repeat_n((Buttons::A, false), 9)); // 11..=19
        frames.push((Buttons::A, true)); // 20: resume, A still held
        frames.extend(std::iter::repeat_n((Buttons::A, false), 10)); // 21..=30

        let (live_log, replay_log, verdict, replay) = live_then_replay(frames);

        assert_eq!(replay.ticks, 20, "{live_log:?}");
        assert_eq!(live_log.0.len(), 20);
        // The scenario is the one the finding describes: A is pressed while
        // paused and still held on resume, so the resume tick (sim tick 10)
        // is the only tick that sees a press edge for A.
        let a_presses: Vec<u32> = live_log
            .0
            .iter()
            .filter(|(_, jp, _, _)| jp & Buttons::A.0 != 0)
            .map(|(t, _, _, _)| *t)
            .collect();
        assert_eq!(a_presses, vec![10], "live log: {live_log:?}");
        assert_eq!(
            live_log, replay_log,
            "a paused stretch moved the press edges the replay reproduces"
        );
        assert!(verdict.matches, "{verdict:?}");
    }

    #[test]
    fn resuming_on_the_tick_after_the_pause_keeps_the_edges_replayable() {
        use gamebient_input::Buttons;
        // The pause tick itself is skipped by `SimSet` too, so its held set
        // must be rewound as well — otherwise a resume on the very next tick
        // derives its edges against a tick the replay never recorded. Here
        // RIGHT is released on the same frame PAUSE is tapped, so tick 10's
        // held set differs from tick 9's.
        let mut frames = vec![(Buttons::RIGHT, false); 9];
        frames.push((Buttons::NONE, true)); // 10: release RIGHT and pause
        frames.push((Buttons::A, true)); // 11: press A and resume at once
        frames.extend(std::iter::repeat_n((Buttons::A, false), 9)); // 12..=20

        let (live_log, replay_log, verdict, replay) = live_then_replay(frames);

        assert_eq!(replay.ticks, 19, "{live_log:?}");
        assert_eq!(
            live_log, replay_log,
            "the pause tick's held set leaked into the resume tick's edges"
        );
        // The resume tick is sim tick 10: A pressed, RIGHT released relative
        // to sim tick 9 — not relative to the skipped pause tick.
        assert_eq!(
            live_log.0[9],
            (10, Buttons::A.0, Buttons::RIGHT.0, Buttons::A.0),
            "{live_log:?}"
        );
        assert!(verdict.matches, "{verdict:?}");
    }

    #[test]
    fn a_held_pause_stays_paused() {
        use gamebient_input::Buttons;
        // Every real pause path holds the button rather than only latching
        // it: keyboard `pressed(Escape)`, gamepad button 9, and the touch
        // overlay all put PAUSE in the held set for as long as it is down.
        // `toggle_pause` derives `pause_just_pressed` from
        // `(held | latched) \ prev`, so rewinding PAUSE out of `prev` while
        // paused would re-fire the press on the very next tick and let the
        // game run on under the pause overlay.
        let mut frames = vec![(Buttons::RIGHT, false); 5];
        frames.extend(std::iter::repeat_n(
            (Buttons::RIGHT.union(Buttons::PAUSE), false),
            6,
        ));

        let live = build_live_app(frames);
        let trace = live.world().resource::<PauseTrace>().clone();
        let tick = live.world().resource::<SimTick>().0;
        let paused = live.world().resource::<crate::game::states::Paused>().0;

        assert!(
            paused,
            "pause let go while the button was still held: {trace:?}"
        );
        assert_eq!(tick, 5, "the sim advanced while paused: {trace:?}");
        // Frames 1..=5 simulate ticks 1..=5; frame 6 pauses and frames
        // 6..=11 are all skipped, so the tick stays at 5 and paused stays
        // true for the whole held span.
        let mut expect: Vec<(u32, bool)> = (1..=5).map(|t| (t, false)).collect();
        expect.extend(std::iter::repeat_n((5, true), 6));
        assert_eq!(trace.0, expect);
    }

    #[test]
    fn a_held_pause_cycle_stays_replayable() {
        use gamebient_input::Buttons;
        // A whole held-pause cycle, the way a keyboard actually produces it:
        //   frames  1..=5   hold RIGHT              -> sim ticks 1..=5
        //   frames  6..=8   hold RIGHT|PAUSE        -> frame 6 pauses; skipped
        //   frames  9..=11  release everything      -> still paused; skipped
        //   frames 12..=14  hold A|PAUSE            -> frame 12 resumes
        //                                              -> sim ticks 6..=8
        //   frames 15..=19  hold A                  -> sim ticks 9..=13
        // Six frames are skipped, so 13 of the 19 frames reach `SimSet`.
        const SIM_TICKS: u32 = 13;
        let pause = Buttons::PAUSE;
        let mut frames = vec![(Buttons::RIGHT, false); 5];
        frames.extend(std::iter::repeat_n((Buttons::RIGHT.union(pause), false), 3));
        frames.extend(std::iter::repeat_n((Buttons::NONE, false), 3));
        frames.extend(std::iter::repeat_n((Buttons::A.union(pause), false), 3));
        frames.extend(std::iter::repeat_n((Buttons::A, false), 5));

        let (live_log, replay_log, verdict, replay) = live_then_replay(frames);

        assert_eq!(replay.ticks, SIM_TICKS, "{live_log:?}");
        assert_eq!(live_log.0.len(), SIM_TICKS as usize);
        assert_eq!(
            live_log, replay_log,
            "a held pause moved the press edges the replay reproduces"
        );
        // Sim tick 6 is the resume tick: A pressed and RIGHT released
        // relative to sim tick 5, which is the last tick the replay carries.
        assert_eq!(
            live_log.0[5],
            (6, Buttons::A.0, Buttons::RIGHT.0, Buttons::A.0),
            "{live_log:?}"
        );
        assert!(verdict.matches, "{verdict:?}");
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
            r#"{"score":1,"checksum":"2","ticks":3,"ended":"cap","matches":false}"#
        );
    }

    #[test]
    fn checksum_is_a_string_so_javascript_cannot_round_it() {
        // 15390901594743611022 > 2^53: as a JSON number it would come back
        // from JSON.parse as 15390901594743612000.
        let v = Verdict {
            score: 0,
            checksum: 15_390_901_594_743_611_022,
            ticks: 0,
            ended: Ended::GameOver,
            matches: true,
        };
        assert!(
            v.to_json().contains(r#""checksum":"15390901594743611022""#),
            "{}",
            v.to_json()
        );
    }

    #[test]
    fn rejects_an_absurd_tick_count_before_reading_runs() {
        // Six header-sized bytes could otherwise ask a verifier to
        // re-simulate for days: the cap is checked before any run is read.
        let mut r = sample();
        r.runs.clear();
        r.ticks = MAX_TICKS + 1;
        assert_eq!(
            Replay::decode(&r.encode()),
            Err(DecodeError::TooManyTicks(MAX_TICKS + 1))
        );
        // The boundary itself decodes far enough to reach the run check.
        r.ticks = MAX_TICKS;
        assert_eq!(
            Replay::decode(&r.encode()),
            Err(DecodeError::RunCountMismatch {
                header: MAX_TICKS,
                sum: 0
            })
        );
    }

    #[test]
    fn rejects_a_foreign_tick_rate() {
        let mut r = sample();
        r.tick_hz = 30;
        assert_eq!(
            Replay::decode(&r.encode()),
            Err(DecodeError::BadTickRate(30))
        );
        assert_eq!(crate::game::sim::TICK_HZ as u16, sample().tick_hz);
    }
}
