//! Records each sim tick's `TickInput` into a `Replay`; seals it when the
//! run ends and posts it to the host as `HostEvent::Run`. Always compiled:
//! this is how the shipped game submits a run.

use bevy::prelude::*;
use gamebient_input::{Buttons, HostEvent, TickInput};

use super::{Replay, TuneRecord};
use crate::game::scoring::{GameData, LeaderboardScore};
use crate::game::sim::{Checksum, RunSeed, TICK_HZ};
use crate::game::tunables::{TuneLog, hash};

#[derive(Resource)]
pub struct ReplayRecorder {
    pub replay: Replay,
    pub sealed: bool,
}

impl Default for ReplayRecorder {
    fn default() -> Self {
        Self {
            replay: empty(RunSeed::default()),
            sealed: false,
        }
    }
}

fn empty(seed: RunSeed) -> Replay {
    Replay {
        build: env!("GX_BUILD_ID").to_string(),
        tick_hz: TICK_HZ as u16,
        seed: seed.bytes,
        origin: seed.origin,
        content: 0,
        ticks: 0,
        score: 0,
        checksum: 0,
        starts: Vec::new(),
        tunes: Vec::new(),
        runs: Vec::new(),
    }
}

impl ReplayRecorder {
    pub fn begin(&mut self, seed: RunSeed) {
        self.replay = empty(seed);
        self.sealed = false;
    }

    /// Raw tick record: the held set, the sub-tick taps (pressed but not
    /// held) as a latch, and the quantized stick — exactly what the feeder
    /// writes back into `InputAccumulator`. Pause never rides along: it is
    /// consumed by `toggle_pause` before `SimSet` runs (so the pause tick
    /// itself is never recorded), but the RESUME tick would otherwise still
    /// carry `Buttons::PAUSE` in `held`/`just_pressed` and re-press pause on
    /// replay, freezing the sim it's supposed to reproduce.
    pub fn push(&mut self, t: &TickInput) {
        let held = t.edges.held.difference(Buttons::PAUSE).sanitized().0 as u16;
        let latched = t
            .edges
            .just_pressed
            .difference(t.edges.held)
            .difference(Buttons::PAUSE)
            .sanitized()
            .0 as u16;
        let (ax, ay) = t.axis;
        self.replay.push_tick(held, latched, ax, ay);
    }

    /// Copies the run's tune log in: start values on `begin`, the changes
    /// (and the content hash over both) on `seal`.
    pub fn set_tunes(&mut self, log: &TuneLog) {
        self.replay.starts = log
            .starts
            .iter()
            .map(|(id, v)| (*id, v.to_bytes()))
            .collect();
        self.replay.tunes = log
            .changes
            .iter()
            .map(|c| TuneRecord {
                tick: c.tick,
                id: c.id,
                bytes: c.value.to_bytes(),
            })
            .collect();
        self.replay.content = hash::content(log);
    }

    pub fn seal(&mut self, score: u64, checksum: u64) -> Replay {
        self.replay.score = score;
        self.replay.checksum = checksum;
        self.sealed = true;
        self.replay.clone()
    }

    pub fn last_run(&self) -> Option<&Replay> {
        self.sealed.then_some(&self.replay)
    }
}

/// `OnEnter(Playing)`, after `sim::begin_run` and `tunables::begin_run_tunables`.
pub fn begin_recording(seed: Res<RunSeed>, log: Res<TuneLog>, mut rec: ResMut<ReplayRecorder>) {
    rec.begin(*seed);
    rec.set_tunes(&log);
}

/// Last in `SimSet`: only sim ticks are recorded, so paused ticks are absent.
pub fn record_tick(t: Res<TickInput>, mut rec: ResMut<ReplayRecorder>) {
    rec.push(&t);
}

/// `OnExit(Playing)`: seal, post to the host, and on native optionally
/// write `$GX_REPLAY_DIR/<unix-ms>.gxr`.
pub fn seal_run(
    data: Res<GameData>,
    sum: Res<Checksum>,
    log: Res<TuneLog>,
    mut rec: ResMut<ReplayRecorder>,
    mut host: MessageWriter<HostEvent>,
) {
    if rec.sealed {
        return;
    }
    rec.set_tunes(&log);
    let replay = rec.seal(u64::from(data.leaderboard_score()), sum.0);
    let bytes = replay.encode();
    host.write(HostEvent::Run(bytes.clone()));
    #[cfg(not(target_arch = "wasm32"))]
    if let Ok(dir) = std::env::var("GX_REPLAY_DIR") {
        let ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let path = std::path::Path::new(&dir).join(format!("{ms}.gxr"));
        if let Err(e) = std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&path, &bytes)) {
            warn!("replay: could not write {}: {e}", path.display());
        } else {
            info!(
                "replay: wrote {} ({} ticks, origin {:?})",
                path.display(),
                replay.ticks,
                replay.origin
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::sim::{RunSeed, SeedOrigin};
    use gamebient_input::{Buttons, GameInput, TickInput};

    fn tick(held: Buttons) -> TickInput {
        let mut g = GameInput::default();
        g.edges.held = held;
        TickInput {
            input: g,
            axis: (0, 0),
        }
    }

    #[test]
    fn records_raw_tick_input_and_seals() {
        let mut rec = ReplayRecorder::default();
        rec.begin(RunSeed {
            bytes: [1u8; 32],
            origin: SeedOrigin::Host,
        });
        rec.push(&tick(Buttons::RIGHT));
        rec.push(&tick(Buttons::RIGHT));
        rec.push(&tick(Buttons::NONE));
        let r = rec.seal(1500, 42);
        assert_eq!(r.ticks, 3);
        assert_eq!(r.runs.len(), 2);
        assert_eq!(r.runs[0].held, Buttons::RIGHT.0 as u16);
        assert_eq!(r.score, 1500);
        assert_eq!(r.checksum, 42);
        assert_eq!(r.origin, SeedOrigin::Host);
        assert_eq!(r.build, env!("GX_BUILD_ID"));
        assert!(rec.sealed);
    }

    #[test]
    fn push_masks_pause_out_of_held_and_latched() {
        // The resume tick after a pause is held/latched with Buttons::PAUSE
        // set (toggle_pause consumed the pause *press* tick before SimSet
        // ran, so it's never recorded, but a later tick can still carry the
        // bit if pause happens to still read as held/just-pressed). A
        // replay must never carry pause: replaying it would re-press pause
        // and freeze the sim it's supposed to reproduce.
        let mut rec = ReplayRecorder::default();
        rec.begin(RunSeed::default());
        rec.push(&tick(Buttons::A | Buttons::PAUSE));
        let r = rec.seal(0, 0);
        assert_eq!(r.runs[0].held, Buttons::A.0 as u16);
        assert!(r.runs[0].held & (Buttons::PAUSE.0 as u16) == 0);
    }

    #[test]
    fn tune_log_rides_in_the_replay() {
        use crate::game::tunables::{TuneChange, Value};
        let mut rec = ReplayRecorder::default();
        rec.begin(RunSeed::default());
        let log = TuneLog {
            starts: vec![(0, Value::Int(5))],
            changes: vec![TuneChange {
                tick: 9,
                id: 0,
                value: Value::Int(6),
            }],
        };
        rec.set_tunes(&log);
        let r = rec.seal(0, 0);
        assert_eq!(r.starts, vec![(0, Value::Int(5).to_bytes())]);
        assert_eq!(r.tunes[0].tick, 9);
        assert_eq!(r.content, hash::content(&log));
        assert_ne!(r.content, 0);
    }

    #[test]
    fn begin_resets_a_previous_run() {
        let mut rec = ReplayRecorder::default();
        rec.begin(RunSeed::default());
        rec.push(&tick(Buttons::A));
        rec.seal(1, 1);
        rec.begin(RunSeed::default());
        assert_eq!(rec.replay.ticks, 0);
        assert!(!rec.sealed);
    }
}
