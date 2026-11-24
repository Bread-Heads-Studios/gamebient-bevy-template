//! Time travel in the editor (feature `editor`; phase 3, sub-project 5):
//! pause, step, seek, branch and speed over the run in progress. Spec: the
//! editor repo's `docs/superpowers/specs/2026-11-24-time-travel-design.md`.
//!
//! There are no save-states. A seek restarts the run in place
//! ([`seek::restart_run`]: the run's own start content, start values and
//! seed, every `GameEntity` despawned, then the `BeginRun` schedule) and
//! re-simulates the recording by calling `FixedMain` directly
//! ([`seek::run_ticks`]), [`PROGRESS_EVERY`] ticks a frame, with sounds,
//! music changes and highlights dropped ([`seek::drop_presentation`]). The
//! screen keeps its last frame until the seek ends: Bevy renders once a
//! frame. The recording is the run's own replay (`ReplayRecorder`, the
//! `TuneLog` and the `RulesLog`), played back through
//! [`seek::PlaybackFeeder`], never the verifier's `ReplayFeeder` resource.
//!
//! Any command marks the run edited (`RunEdited`): it is sealed with the
//! GXR2 edited bit, never posted, and refused by the verifier.
//!
//! This folder is copied verbatim into every game (`tools/frame-accept.sh
//! --check-copies`); a game declares its marks in `rule_table::MARKS`.
//!
//! | Direction   | Name            | Data |
//! |-------------|-----------------|------|
//! | host → game | `time.pause`    | — |
//! | host → game | `time.resume`   | — |
//! | host → game | `time.step`     | `{ n }`: 1–600 ticks (default 1), then paused |
//! | host → game | `time.seek`     | `{ tick, before_branch? }`: clamped to the recording; `before_branch` seeks in the run as it was before the last branch (undo) |
//! | host → game | `time.branch`   | — : drop the recording after this tick, re-simulate to it, play on live |
//! | host → game | `time.speed`    | `{ x }`: 0.5, 1 or 2 |
//! | game → host | `time`          | `{ tick, of, paused, speed, seeking, edited, run, over }`, plus `marks` after a branch or an undo and `refused` (a sentence) for a command not taken; `{ seeking: true, tick }` during a seek. `over`: the run is not in `Playing` (sent once as it leaves); a seek still takes a run that ended back in |
//! | game → host | `time.mark`     | `{ tick, kind, label }` |
//! | game → host | `time.diverged` | `{ tick }` |

pub mod branch;
pub mod commands;
pub mod marks;
pub mod run;
pub mod seek;

use bevy::prelude::*;
use gamebient_input::{HostEvent, TickInputSet};
use serde_json::{Value as Json, json};

use crate::game::content::level::{Level, LevelHash};
use crate::game::replay::Replay;
use crate::game::replay::recorder::{ReplayRecorder, RunEdited};
use crate::game::rules::{Rules, RulesHash, RulesLog, RulesText};
use crate::game::sim::{self, RunSeed, SimTick, TICK_HZ};
use crate::game::states::{self, GameState, Paused, QuietPause};
use crate::game::tunables::{TuneLog, Value};
use seek::{PlaybackFeeder, Seek};

/// The most ticks one `time.step` runs.
pub const STEP_MAX: u32 = 600;
/// A `time` update goes out every this many ticks while the run plays.
pub const UPDATE_EVERY: u32 = 30;
/// A seek re-simulates this many ticks a frame and reports after each batch.
pub const PROGRESS_EVERY: u32 = 2000;
/// The speeds `time.speed` takes.
pub const SPEEDS: [f32; 3] = [0.5, 1.0, 2.0];

/// The content files a run starts from, as the resources that hold them.
#[derive(Clone, Debug, PartialEq)]
pub struct Content {
    pub level: Option<Level>,
    pub level_hash: LevelHash,
    pub rules: Rules,
    pub rules_hash: RulesHash,
    pub rules_text: RulesText,
}

impl Content {
    pub fn capture(world: &World) -> Self {
        Self {
            level: world.get_resource::<Level>().cloned(),
            level_hash: world
                .get_resource::<LevelHash>()
                .copied()
                .unwrap_or_default(),
            rules: world.get_resource::<Rules>().cloned().unwrap_or_default(),
            rules_hash: world
                .get_resource::<RulesHash>()
                .copied()
                .unwrap_or_default(),
            rules_text: world
                .get_resource::<RulesText>()
                .cloned()
                .unwrap_or_default(),
        }
    }

    pub fn put(&self, world: &mut World) {
        if let Some(level) = &self.level {
            world.insert_resource(level.clone());
        }
        world.insert_resource(self.level_hash);
        world.insert_resource(self.rules.clone());
        world.insert_resource(self.rules_hash);
        world.insert_resource(self.rules_text.clone());
    }
}

/// What a run started from: its seed, every start value, its content.
/// Captured on `OnEnter(Playing)` after the run-start chain; a seek starts
/// the run again from exactly this.
#[derive(Clone, Debug, PartialEq)]
pub struct RunStart {
    pub seed: RunSeed,
    pub starts: Vec<(u16, Value)>,
    pub content: Content,
}

impl RunStart {
    pub fn capture(world: &World) -> Self {
        Self {
            seed: *world.resource::<RunSeed>(),
            starts: world.resource::<TuneLog>().starts.clone(),
            content: Content::capture(world),
        }
    }
}

/// A moment the timeline marks.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Mark {
    pub tick: u32,
    pub kind: String,
    pub label: String,
}

impl Mark {
    pub fn new(tick: u32, kind: &str, label: &str) -> Self {
        Self {
            tick,
            kind: kind.to_string(),
            label: label.to_string(),
        }
    }

    pub fn to_json(&self) -> Json {
        json!({ "tick": self.tick, "kind": self.kind, "label": self.label })
    }
}

/// The run as it was before the last branch, kept for undo until the next
/// branch.
#[derive(Clone, Debug)]
pub struct Before {
    pub start: RunStart,
    pub recording: Replay,
    pub marks: Vec<Mark>,
    pub ended: bool,
}

/// The run's timeline.
#[derive(Resource, Debug)]
pub struct TimeTravel {
    /// Fresh runs since the app started: the editor clears its timeline
    /// when this changes. A seek or a branch never changes it.
    pub run: u32,
    pub start: Option<RunStart>,
    /// The whole recording (inputs, tunable changes, rules swaps). Brought
    /// up to date from the recorder whenever the run is live.
    pub recording: Option<Replay>,
    /// The recorded run ended (won or lost): a seek stops one tick short
    /// of its end, so the world stays in `Playing`.
    pub ended: bool,
    pub speed: f32,
    pub marks: Vec<Mark>,
    pub before: Option<Before>,
    /// Every tunable's value when the run left `Playing`: what an edit made
    /// on the game-over screen or in the level editor is told apart from.
    pub final_values: Vec<(u16, Value)>,
    /// The run left `Playing` straight into the level editor while the
    /// timeline held it paused.
    pub held_at_exit: bool,
    /// Set on entering the level editor from a paused run: the tick and
    /// checksum to branch at when the level editor hands back.
    pub edit_return: Option<(u32, u64)>,
}

impl Default for TimeTravel {
    fn default() -> Self {
        Self {
            run: 0,
            start: None,
            recording: None,
            ended: false,
            speed: 1.0,
            marks: Vec::new(),
            before: None,
            final_values: Vec::new(),
            held_at_exit: false,
            edit_return: None,
        }
    }
}

impl TimeTravel {
    /// Ticks in the recording.
    pub fn recorded(&self) -> u32 {
        self.recording.as_ref().map_or(0, |r| r.ticks)
    }
}

pub fn in_playing(world: &World) -> bool {
    world
        .get_resource::<State<GameState>>()
        .is_some_and(|s| *s.get() == GameState::Playing)
}

/// The `of` the editor shows: the recording, or the live run past its end.
pub fn length(world: &World) -> u32 {
    let recorder = world.resource::<ReplayRecorder>().replay.ticks;
    world.resource::<TimeTravel>().recorded().max(recorder)
}

/// The furthest tick a seek goes to: the recording's end, or one short of
/// it when the recorded run ended there.
pub fn max_seek(world: &World) -> u32 {
    let tt = world.resource::<TimeTravel>();
    let of = tt.recorded();
    if tt.ended { of.saturating_sub(1) } else { of }
}

/// The `time` answer.
pub fn time_json(world: &World) -> Json {
    let tt = world.resource::<TimeTravel>();
    json!({
        "tick": world.resource::<SimTick>().0,
        "of": length(world),
        "paused": world.resource::<Paused>().0,
        "speed": tt.speed,
        "seeking": world.contains_resource::<Seek>(),
        "edited": world.resource::<RunEdited>().0,
        "run": tt.run,
        "over": !in_playing(world),
    })
}

pub fn custom(name: &str, data: Json) -> HostEvent {
    HostEvent::Custom {
        name: name.to_string(),
        data: data.to_string(),
    }
}

pub fn answer(world: &mut World) {
    let j = time_json(world);
    world.write_message(custom("time", j));
}

/// The `time` answer to a command the game did not take, with the reason
/// (`refused`), so the editor never takes it for a late update.
pub fn answer_refused(world: &mut World, reason: &str) {
    let mut j = time_json(world);
    j["refused"] = reason.into();
    world.write_message(custom("time", j));
}

/// The `time` answer with the run's whole mark list (after a branch or an
/// undo, when the editor replaces its marks).
pub fn answer_with_marks(world: &mut World) {
    let mut j = time_json(world);
    j["marks"] = Json::Array(
        world
            .resource::<TimeTravel>()
            .marks
            .iter()
            .map(Mark::to_json)
            .collect(),
    );
    world.write_message(custom("time", j));
}

/// Keeps a mark the run has not had yet and, unless a seek is
/// re-simulating, tells the editor.
pub fn push_mark(world: &mut World, mark: Mark) {
    let quiet = world.contains_resource::<Seek>();
    let mut tt = world.resource_mut::<TimeTravel>();
    if tt.marks.contains(&mark) {
        return;
    }
    tt.marks.push(mark.clone());
    if !quiet {
        world.write_message(custom("time.mark", mark.to_json()));
    }
}

/// The timeline's pause: the sim stops (`Paused`) with no overlay.
pub fn hold(world: &mut World) {
    world.resource_mut::<Paused>().set_if_neq(Paused(true));
    world
        .resource_mut::<QuietPause>()
        .set_if_neq(QuietPause(true));
}

pub fn unhold(world: &mut World) {
    world.resource_mut::<Paused>().set_if_neq(Paused(false));
    world
        .resource_mut::<QuietPause>()
        .set_if_neq(QuietPause(false));
}

pub fn mark_edited(world: &mut World) {
    world
        .resource_mut::<RunEdited>()
        .set_if_neq(RunEdited(true));
}

/// While the run is live (no playback), the recording is the recorder's
/// run so far.
pub fn refresh_recording(world: &mut World) {
    if world.contains_resource::<PlaybackFeeder>() {
        return;
    }
    let r = world
        .resource::<ReplayRecorder>()
        .snapshot(world.resource::<TuneLog>(), world.resource::<RulesLog>());
    world.resource_mut::<TimeTravel>().recording = Some(r);
}

/// The catalog's `time`: what the editor's timeline may ask for.
pub fn catalog() -> Json {
    json!({
        "tick_hz": TICK_HZ,
        "step_max": STEP_MAX,
        "speeds": SPEEDS,
        "update_every": UPDATE_EVERY,
        "progress_every": PROGRESS_EVERY,
    })
}

/// The player's own pause key can let go of the timeline's pause; the
/// overlay rule then goes back to normal.
fn clear_quiet_on_resume(paused: Res<Paused>, mut quiet: ResMut<QuietPause>) {
    if !paused.0 && quiet.0 {
        quiet.0 = false;
    }
}

/// Added by `GamePlugin` in editor builds, in both modes.
pub struct TimePlugin;

impl Plugin for TimePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TimeTravel>()
            .init_resource::<commands::TimeCommands>()
            .init_resource::<marks::MarkWatch>()
            .add_systems(
                Update,
                (
                    commands::read_time_commands,
                    commands::apply_time_commands,
                    seek::drive_seek,
                    clear_quiet_on_resume,
                )
                    .chain(),
            )
            .add_systems(
                FixedPreUpdate,
                seek::feed_playback.in_set(TickInputSet::Feed).run_if(
                    in_state(GameState::Playing)
                        .and(states::not_paused)
                        .and(sim::run_not_over)
                        .and(resource_exists::<PlaybackFeeder>),
                ),
            )
            .add_systems(
                FixedUpdate,
                (marks::detect_marks, commands::report_every)
                    .chain()
                    .in_set(sim::SimSet)
                    .after(crate::game::replay::recorder::record_tick),
            )
            .add_systems(
                crate::game::BeginRun,
                marks::reset_mark_watch.after(crate::game::rules::begin_run_rules),
            )
            .add_systems(
                OnEnter(GameState::Playing),
                (
                    run::prepare_reentry.before(crate::game::run_begin_run),
                    run::on_run_start.after(crate::game::run_begin_run),
                ),
            )
            .add_systems(
                OnExit(GameState::Playing),
                run::on_run_exit.before(crate::game::replay::recorder::seal_run),
            )
            .add_systems(OnEnter(GameState::Editing), run::note_edit_return);
    }
}
