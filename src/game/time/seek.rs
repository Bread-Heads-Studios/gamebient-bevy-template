//! The seek: start the run again in place and re-simulate the recording,
//! a batch of [`PROGRESS_EVERY`] ticks a frame, with nothing played.

use bevy::app::{FixedMain, FixedMainScheduleOrder};
use bevy::ecs::intern::Interned;
use bevy::ecs::message::{Message, Messages};
use bevy::ecs::schedule::{ExecutorKind, ScheduleLabel};
use bevy::prelude::*;
use serde_json::json;

use super::{
    Content, PROGRESS_EVERY, RunStart, TimeTravel, answer, answer_with_marks, custom, hold,
    in_playing, max_seek, refresh_recording, unhold,
};
use crate::frame::Highlight;
use crate::game::audio::{CueEvent, LayerEvent, SfxEvent};
use crate::game::content::level::LevelSource;
use crate::game::replay::feeder::{ReplayFeeder, queue_rules, queue_tick};
use crate::game::replay::recorder::RunEdited;
use crate::game::rules::run::RunRules;
use crate::game::rules::{PendingRules, Rules, RulesHash, RulesSwap, RulesText};
use crate::game::sim::{PendingSeed, RunOver, RunSeed, SimTick, tick_duration};
use crate::game::states::{GameState, Paused};
use crate::game::tunables::{PendingTunes, TunableIndex, Value, write};
use crate::game::{BeginRun, GameEntity};
use crate::ui::transition::ScreenFade;

/// The recording played back: the verifier's feeder under another name,
/// so the verifier app (which under `--all-features` carries this plugin
/// too) is never fed twice.
#[derive(Resource, Debug)]
pub struct PlaybackFeeder(pub ReplayFeeder);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeekKind {
    /// Move within the recording; ends paused.
    Plain,
    /// `time.branch`: the recording ends at the target; ends playing live.
    Branch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Start the run again in place, then feed.
    Restart,
    /// Waiting for `Playing` to be entered again (from GameOver or the
    /// level editor); `OnEnter(Playing)` starts the run, then feed.
    Reenter,
    /// Re-simulate toward the target.
    Feed,
}

/// The seek in progress. While it exists nothing is posted but progress,
/// and no command but another `time.seek` (kept as `next`) is taken.
#[derive(Resource, Debug)]
pub struct Seek {
    pub target: u32,
    pub kind: SeekKind,
    pub phase: Phase,
    pub held: HeldEdits,
    /// The checksum at the target before the edit (a branch).
    pub compare: Option<u64>,
    /// The editor's content, kept while a re-entry starts the run on its own.
    pub live: Option<Content>,
    /// The branch starts the run on an edited level.
    pub level_edited: bool,
    /// Answer with the whole mark list.
    pub send_marks: bool,
    /// A seek asked for while this one ran; started when this one ends.
    pub next: Option<u32>,
}

/// Edits waiting when a seek begins, put back when it ends so they land on
/// the next tick played, never inside the re-simulation.
#[derive(Debug, Default)]
pub struct HeldEdits {
    pub tunes: Vec<(u16, Value)>,
    pub rules: Option<RulesSwap>,
    /// Values changed outside the run (on the game-over screen, in the
    /// level editor), queued as changes after the re-simulation.
    pub edits: Vec<(u16, Value)>,
    /// A branch: afterwards the run follows the rules the editor shows.
    pub follow_rules: bool,
}

impl HeldEdits {
    pub fn take(world: &mut World, edits: Vec<(u16, Value)>, follow_rules: bool) -> Self {
        let tunes = core::mem::take(&mut world.resource_mut::<PendingTunes>().0);
        let rules = world
            .get_resource_mut::<PendingRules>()
            .and_then(|mut p| p.0.take());
        Self {
            tunes,
            rules,
            edits,
            follow_rules,
        }
    }

    pub fn restore(self, world: &mut World) {
        let mut rules = self.rules;
        if self.follow_rules && rules.is_none() {
            rules = shown_rules_swap(world);
        }
        let mut pending = self.edits;
        pending.extend(self.tunes);
        world.resource_mut::<PendingTunes>().0.extend(pending);
        if let Some(swap) = rules
            && let Some(mut p) = world.get_resource_mut::<PendingRules>()
        {
            p.0 = Some(swap);
        }
    }
}

/// The rules the editor shows, as a swap, when the run is on others.
fn shown_rules_swap(world: &World) -> Option<RulesSwap> {
    if world.get_resource::<LevelSource>() == Some(&LevelSource::Test) {
        return None;
    }
    let shown = world.get_resource::<Rules>()?;
    let running = world.get_resource::<RunRules>()?;
    (shown.0 != running.file).then(|| RulesSwap {
        file: shown.0.clone(),
        hash: world.get_resource::<RulesHash>().map_or(0, |h| h.0),
        bytes: world
            .get_resource::<RulesText>()
            .map(|t| t.0.clone())
            .unwrap_or_default(),
    })
}

/// Why a `time.seek` was not taken (the `time` answer's `refused`).
pub const NO_RUN: &str = "There is no run to seek in.";
pub const NO_UNDO: &str = "There is no branch to undo.";
pub const NOTHING_RECORDED: &str = "Nothing is recorded yet.";

/// `time.seek` (and an undo: `before_branch`), in `Playing` or after the
/// run ended (`GameOver`), which enters `Playing` again first. A seek
/// asked for while one runs waits for it. `Err` says why it was not taken.
pub fn start_seek(world: &mut World, tick: u32, before_branch: bool) -> Result<(), &'static str> {
    if let Some(mut s) = world.get_resource_mut::<Seek>() {
        s.next = Some(tick);
        return Ok(());
    }
    let playing = in_playing(world);
    let over = world
        .get_resource::<State<GameState>>()
        .is_some_and(|s| *s.get() == GameState::GameOver);
    if !playing && !over {
        return Err(NO_RUN);
    }
    if before_branch {
        if !restore_before(world) {
            return Err(NO_UNDO);
        }
    } else if playing {
        refresh_recording(world);
    }
    let ready = {
        let tt = world.resource::<TimeTravel>();
        tt.start.is_some() && tt.recording.is_some()
    };
    if !ready {
        return Err(NOTHING_RECORDED);
    }
    let target = tick.min(max_seek(world));
    let phase = if !playing {
        Phase::Reenter
    } else if !before_branch
        && world.contains_resource::<PlaybackFeeder>()
        && target >= world.resource::<SimTick>().0
    {
        Phase::Feed
    } else {
        Phase::Restart
    };
    let edits = if playing {
        Vec::new()
    } else {
        super::run::value_edits(world)
    };
    let held = HeldEdits::take(world, edits, false);
    if playing {
        hold(world);
    }
    world.insert_resource(Seek {
        target,
        kind: SeekKind::Plain,
        phase,
        held,
        compare: None,
        live: None,
        level_edited: false,
        send_marks: before_branch,
        next: None,
    });
    if phase == Phase::Reenter {
        request_playing(world);
    }
    Ok(())
}

/// The run as it was before the last branch becomes the run again.
fn restore_before(world: &mut World) -> bool {
    let mut tt = world.resource_mut::<TimeTravel>();
    let Some(b) = tt.before.take() else {
        return false;
    };
    tt.start = Some(b.start);
    tt.recording = Some(b.recording);
    tt.marks = b.marks;
    tt.ended = b.ended;
    true
}

/// `Update`: one batch of the seek in progress.
pub fn drive_seek(world: &mut World) {
    let Some(phase) = world.get_resource::<Seek>().map(|s| s.phase) else {
        return;
    };
    match phase {
        Phase::Reenter => {
            if !in_playing(world) {
                request_playing(world);
            }
            return;
        }
        Phase::Restart => {
            let (start, recording) = {
                let tt = world.resource::<TimeTravel>();
                (tt.start.clone(), tt.recording.clone())
            };
            let (Some(start), Some(recording)) = (start, recording) else {
                let seek = world
                    .remove_resource::<Seek>()
                    .expect("the seek in progress");
                finish(world, seek);
                return;
            };
            world.remove_resource::<PlaybackFeeder>();
            restart_run(world, &start);
            world.insert_resource(PlaybackFeeder(ReplayFeeder::new(&recording)));
            world.resource_mut::<Seek>().phase = Phase::Feed;
        }
        Phase::Feed => {
            if !world.contains_resource::<PlaybackFeeder>()
                && world.resource::<SimTick>().0 == 0
                && let Some(recording) = world.resource::<TimeTravel>().recording.clone()
            {
                world.insert_resource(PlaybackFeeder(ReplayFeeder::new(&recording)));
            }
        }
    }
    hold(world);
    let target = world.resource::<Seek>().target;
    let batch = target
        .saturating_sub(world.resource::<SimTick>().0)
        .min(PROGRESS_EVERY);
    let ran = run_ticks(world, batch);
    drop_presentation(world);
    let now = world.resource::<SimTick>().0;
    if now < target && ran == batch && batch > 0 {
        world.write_message(custom("time", json!({ "seeking": true, "tick": now })));
        return;
    }
    let seek = world
        .remove_resource::<Seek>()
        .expect("the seek in progress");
    finish(world, seek);
}

/// The seek is over: playback stays for what is left of the recording (a
/// branch plays live), a branch says whether it diverged, held edits go
/// back in the queue, and the editor hears where it landed.
pub fn finish(world: &mut World, seek: Seek) {
    let Seek {
        kind,
        held,
        compare,
        level_edited,
        send_marks,
        next,
        ..
    } = seek;
    let exhausted = world
        .get_resource::<PlaybackFeeder>()
        .is_some_and(|f| f.0.remaining == 0);
    if exhausted || kind == SeekKind::Branch {
        world.remove_resource::<PlaybackFeeder>();
    }
    if kind == SeekKind::Branch {
        super::branch::settle(world, compare, level_edited);
    }
    held.restore(world);
    match kind {
        SeekKind::Plain => hold(world),
        SeekKind::Branch => unhold(world),
    }
    if send_marks {
        answer_with_marks(world);
    } else {
        answer(world);
    }
    if let Some(next) = next
        && let Err(reason) = start_seek(world, next, false)
    {
        super::answer_refused(world, reason);
    }
}

/// Present while [`run_ticks`] runs a batch (a seek's or a `time.step`'s):
/// the 30-tick `time` update stays quiet, so only the batch's own answer
/// says where it stopped (`Paused` reads false inside the batch).
#[derive(Resource, Debug)]
pub struct RunningTicks;

/// Runs `n` sim ticks now, inside this frame, by calling `FixedMain` the way
/// `run_fixed_main_schedule` does (the generic `Time` is the fixed step for
/// each tick, so `delta_secs()` in `FixedUpdate` is the tick). Stops early
/// when the run ends. Returns the ticks run.
///
/// The fixed schedules run on the single-threaded executor for the batch
/// (then go back to what they had): dispatching a tick's handful of small
/// systems to the thread pool costs far more than running them, about
/// 50 µs a tick against 1 µs. The order is the same either way, so the
/// checksum is too.
pub fn run_ticks(world: &mut World, n: u32) -> u32 {
    if n == 0 {
        return 0;
    }
    let executors = set_fixed_executors(world, ExecutorKind::SingleThreaded);
    let was = world.resource::<Paused>().0;
    world.resource_mut::<Paused>().bypass_change_detection().0 = false;
    world.insert_resource(RunningTicks);
    let mut fixed = Time::<()>::default();
    let mut ran = 0;
    world.schedule_scope(FixedMain, |world, schedule| {
        for _ in 0..n {
            if run_has_ended(world) {
                break;
            }
            fixed.advance_by(tick_duration());
            *world.resource_mut::<Time>() = fixed;
            schedule.run(world);
            ran += 1;
        }
    });
    let virt = world.resource::<Time<Virtual>>().as_generic();
    *world.resource_mut::<Time>() = virt;
    world.resource_mut::<Paused>().bypass_change_detection().0 = was;
    world.remove_resource::<RunningTicks>();
    for (label, kind) in executors {
        if let Some(s) = world.resource_mut::<Schedules>().get_mut(label) {
            s.set_executor_kind(kind);
        }
    }
    ran
}

/// Puts `FixedMain` and every schedule it runs on `kind`; returns what
/// each had.
fn set_fixed_executors(
    world: &mut World,
    kind: ExecutorKind,
) -> Vec<(Interned<dyn ScheduleLabel>, ExecutorKind)> {
    let mut labels = vec![FixedMain.intern()];
    if let Some(order) = world.get_resource::<FixedMainScheduleOrder>() {
        labels.extend(order.labels.iter().copied());
    }
    let mut schedules = world.resource_mut::<Schedules>();
    labels
        .into_iter()
        .filter_map(|label| {
            let s = schedules.get_mut(label)?;
            let was = s.get_executor_kind();
            s.set_executor_kind(kind);
            Some((label, was))
        })
        .collect()
}

fn run_has_ended(world: &World) -> bool {
    world.resource::<RunOver>().0
        || !matches!(
            *world.resource::<NextState<GameState>>(),
            NextState::Unchanged
        )
        || !in_playing(world)
}

/// Starts the run again in place from `start`: its content, start values
/// and seed back for the run-start chain, every `GameEntity` gone, the
/// `BeginRun` schedule, then the editor's own content back for the next
/// fresh run.
pub fn restart_run(world: &mut World, start: &RunStart) {
    let live = Content::capture(world);
    start.content.put(world);
    put_starts(world, &start.starts);
    world.resource_mut::<PendingSeed>().0 = Some(start.seed.bytes);
    despawn_run(world);
    drop_presentation(world);
    // `BeginRun` clears `RunEdited` for a fresh run; this is the same run.
    let edited = *world.resource::<RunEdited>();
    world.run_schedule(BeginRun);
    world.resource_mut::<RunEdited>().set_if_neq(edited);
    // `begin_run` calls a pending seed a host seed; the run had its own.
    world.resource_mut::<RunSeed>().origin = start.seed.origin;
    live.put(world);
}

/// Writes each start value back into its tunable.
pub fn put_starts(world: &mut World, starts: &[(u16, Value)]) {
    let idx = world.resource::<TunableIndex>().clone();
    for (id, v) in starts {
        if let Some(entry) = idx.by_id(*id) {
            let _ = write(world, entry, v);
        }
    }
}

/// Despawns every entity the run spawned (art children go with them).
pub fn despawn_run(world: &mut World) {
    let run: Vec<Entity> = {
        let mut q = world.query_filtered::<Entity, With<GameEntity>>();
        q.iter(world).collect()
    };
    for e in run {
        let _ = world.try_despawn(e);
    }
}

/// Drops every sound, music change and highlight a re-simulation asked
/// for, before `Update`'s audio and the frame read them.
pub fn drop_presentation(world: &mut World) {
    fn clear<M: Message>(world: &mut World) {
        if let Some(mut m) = world.get_resource_mut::<Messages<M>>() {
            m.clear();
        }
    }
    clear::<SfxEvent>(world);
    clear::<CueEvent>(world);
    clear::<LayerEvent>(world);
    clear::<Highlight>(world);
}

/// `FixedPreUpdate`, `TickInputSet::Feed`: the recording's next tick. Once
/// it runs out the run is live from here.
pub fn feed_playback(world: &mut World) {
    let Some(mut feeder) = world.remove_resource::<PlaybackFeeder>() else {
        return;
    };
    if feeder.0.remaining == 0 {
        return;
    }
    queue_rules(world, &mut feeder.0);
    queue_tick(world, &mut feeder.0);
    world.insert_resource(feeder);
}

/// Asks for `Playing`: through the windowed build's fade once it is idle
/// (a bare `NextState` would be overruled by a fade in flight), at once in
/// the headless build. Called again each frame until it takes.
pub fn request_playing(world: &mut World) {
    if let Some(mut fade) = world.get_resource_mut::<ScreenFade>() {
        if fade.is_idle() {
            let _ = fade.request(GameState::Playing);
        }
        return;
    }
    world
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Playing);
}
