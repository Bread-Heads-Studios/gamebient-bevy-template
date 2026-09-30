//! Feeds the pure frame logic from the running game: window size into the
//! layout, `GameState`, `Paused` and `Highlight` into the brightness
//! machine, the final score into the best-score file. Read-only toward the
//! game: nothing here writes a resource the game or the sim owns.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::display::{FrameInsets, GAME_HEIGHT, GAME_WIDTH, GameViewport, letterbox};
use crate::frame::Highlight;
use crate::frame::best_score::{best_score_path_from_env, load_best, save_best};
use crate::frame::brightness::{FrameBrightness, FrameEvent, FrameLevels, FramePhase};
use crate::frame::caption::{group_digits, marquee_caption};
use crate::frame::layout::{FrameLayout, PxRect, frame_layout};
use crate::game::scoring::{GameData, HIGHER_SCORE_IS_BETTER, LeaderboardScore};
use crate::game::states::{GameState, Paused};

/// True when `score` is a record over `best` for the given score order. The
/// order is `game::scoring::HIGHER_SCORE_IS_BETTER`, the game's own setting;
/// this file defines none, so a template sync cannot reset it.
fn is_record(higher_is_better: bool, best: u64, score: u64) -> bool {
    higher_is_better && score > best
}

/// The frame's view of each game state. Only `Playing` and `GameOver` are
/// named, which every game has; any other state, including ones a game adds
/// or renames (Voidrunner's `ItemKey`), is `Attract`. This file is copied
/// into games unchanged, so it must not name states some games lack.
pub fn phase_for(state: GameState) -> FramePhase {
    match state {
        GameState::Playing => FramePhase::Playing,
        GameState::GameOver => FramePhase::GameOver,
        _ => FramePhase::Attract,
    }
}

/// Everything the frame remembers while the game runs.
#[derive(Resource, Debug)]
pub struct FrameRuntime {
    brightness: FrameBrightness,
    /// What to draw this frame.
    pub levels: FrameLevels,
    pub phase: FramePhase,
    /// The score to beat. 0 means none is known.
    pub best: u64,
    /// The score of the run that just ended, until the next run starts.
    pub final_score: Option<u64>,
    /// Colour of the pulse in progress. `None` is the default colour.
    pub pulse_color: Option<[u8; 3]>,
    best_path: Option<PathBuf>,
    /// A run ended on entering `GameOver` and its score is not read yet.
    finish_pending: bool,
}

impl FrameRuntime {
    pub fn new(best: u64, best_path: Option<PathBuf>) -> Self {
        let mut brightness = FrameBrightness::new();
        let levels = brightness.step(0.0, FramePhase::Attract, false);
        Self {
            brightness,
            levels,
            phase: FramePhase::Attract,
            best,
            final_score: None,
            pulse_color: None,
            best_path,
            finish_pending: false,
        }
    }

    /// Reads the best score from its file, if there is one.
    pub fn from_env() -> Self {
        let path = best_score_path_from_env();
        if !HIGHER_SCORE_IS_BETTER {
            return Self::new(0, None);
        }
        let best = path.as_deref().map_or(0, load_best);
        Self::new(best, path)
    }

    /// Records the end of a run. A score above the best is a new high score
    /// and is written to the file; anything else is a plain game over.
    pub fn finish_run(&mut self, final_score: u64) -> FrameEvent {
        self.final_score = Some(final_score);
        let event = if is_record(HIGHER_SCORE_IS_BETTER, self.best, final_score) {
            self.best = final_score;
            if let Some(path) = &self.best_path
                && let Err(error) = save_best(path, final_score)
            {
                warn!("frame: could not write {}: {error}", path.display());
            }
            FrameEvent::NewHighScore
        } else {
            FrameEvent::GameOver
        };
        self.brightness.event(event);
        event
    }

    /// Asks for a pulse. Returns false when the rate limit refused it.
    pub fn highlight(&mut self, color: Option<[u8; 3]>) -> bool {
        let accepted = self.brightness.event(FrameEvent::Highlight);
        if accepted {
            self.pulse_color = color;
        }
        accepted
    }

    pub fn advance(&mut self, dt: f32, phase: FramePhase, paused: bool) {
        if phase == FramePhase::Playing {
            self.final_score = None;
        }
        self.phase = phase;
        self.levels = self.brightness.step(dt, phase, paused);
    }

    pub fn caption(&self) -> Option<String> {
        let best = (HIGHER_SCORE_IS_BETTER && self.best > 0).then(|| group_digits(self.best));
        let final_score = self.final_score.map(group_digits);
        marquee_caption(
            self.phase,
            self.levels.celebration.is_some(),
            best.as_deref(),
            final_score.as_deref(),
        )
    }
}

/// The layout in force and the window it was computed for.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct FrameGeometry {
    pub window: UVec2,
    pub scale_factor: f32,
    pub layout: FrameLayout,
    pub game: GameViewport,
}

impl FrameGeometry {
    /// The value before the first window size is seen. Nothing is drawn
    /// from it: every rectangle is empty.
    pub fn empty() -> Self {
        Self {
            window: UVec2::ZERO,
            scale_factor: 1.0,
            layout: FrameLayout {
                marquee: None,
                insets: FrameInsets::default(),
                bezel: PxRect {
                    x: 0,
                    y: 0,
                    w: 0,
                    h: 0,
                },
            },
            game: GameViewport {
                position: UVec2::ZERO,
                size: UVec2::ZERO,
            },
        }
    }
}

/// Recomputes the layout when the window changes, and hands the insets to
/// `DisplayPlugin`. Both resources are only written when the value differs,
/// so change detection fires once per resize.
pub fn apply_layout(
    windows: Query<&Window, With<PrimaryWindow>>,
    mut insets: ResMut<FrameInsets>,
    mut geometry: ResMut<FrameGeometry>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let size = window.physical_size();
    if size.x == 0 || size.y == 0 {
        return;
    }
    let layout = frame_layout(size);
    let next = FrameGeometry {
        window: size,
        scale_factor: window.scale_factor(),
        layout,
        game: letterbox(
            size,
            UVec2::new(GAME_WIDTH, GAME_HEIGHT),
            layout.insets.reserve_top,
            layout.insets.gap,
        ),
    };
    if *insets != layout.insets {
        *insets = layout.insets;
    }
    if *geometry != next {
        *geometry = next;
    }
}

/// Once per frame: take in highlights, then advance the brightness machine
/// on the wall clock. `Time<Real>` so a game that slows or stops virtual
/// time does not freeze a fade halfway.
pub fn step_frame(
    time: Res<Time<Real>>,
    state: Res<State<GameState>>,
    paused: Res<Paused>,
    mut highlights: MessageReader<Highlight>,
    mut runtime: ResMut<FrameRuntime>,
) {
    for highlight in highlights.read() {
        runtime.highlight(highlight.color);
    }
    runtime.advance(time.delta_secs(), phase_for(*state.get()), paused.0);
}

/// `OnEnter(GameState::GameOver)`: mark the run as ended. The score is not
/// read here: the order of this system against a game's own
/// `OnEnter(GameOver)` systems (a time bonus, say) is undefined, so
/// [`finish_pending_run`] takes it on the first `Update` tick in `GameOver`,
/// after every `OnEnter` system has run.
pub fn on_game_over(mut runtime: ResMut<FrameRuntime>) {
    runtime.finish_pending = true;
}

/// `Update`, before `step_frame`: if a run has just ended, compare its final
/// score with the best. Runs once per game over.
pub fn finish_pending_run(data: Res<GameData>, mut runtime: ResMut<FrameRuntime>) {
    if runtime.finish_pending {
        runtime.finish_pending = false;
        runtime.finish_run(u64::from(data.leaderboard_score()));
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;
    use bevy::window::WindowResolution;

    use super::*;

    #[test]
    fn every_state_maps_to_a_phase() {
        assert_eq!(phase_for(GameState::StudioLogo), FramePhase::Attract);
        assert_eq!(phase_for(GameState::Menu), FramePhase::Attract);
        assert_eq!(phase_for(GameState::HowToPlay), FramePhase::Attract);
        assert_eq!(phase_for(GameState::Playing), FramePhase::Playing);
        assert_eq!(phase_for(GameState::GameOver), FramePhase::GameOver);
    }

    #[test]
    fn a_score_above_the_best_is_a_new_high_score() {
        let mut rt = FrameRuntime::new(100, None);
        assert_eq!(rt.finish_run(150), FrameEvent::NewHighScore);
        assert_eq!(rt.best, 150);
        assert_eq!(rt.final_score, Some(150));
        assert_eq!(rt.finish_run(120), FrameEvent::GameOver);
        assert_eq!(rt.best, 150);
        assert_eq!(rt.final_score, Some(120));
        assert_eq!(
            rt.finish_run(150),
            FrameEvent::GameOver,
            "a tie is not a record"
        );
    }

    #[test]
    fn a_lower_is_better_game_never_records() {
        assert!(is_record(true, 10, 11));
        assert!(!is_record(true, 10, 10));
        assert!(!is_record(false, 10, 5));
        assert!(!is_record(false, 0, 5));
    }

    /// A game's own `OnEnter(GameOver)` system may add a bonus. Its order
    /// against the frame's is undefined, so the frame reads the score on the
    /// first `Update` tick in `GameOver`.
    #[test]
    fn the_final_score_includes_a_bonus_added_on_enter() {
        use bevy::state::app::StatesPlugin;

        fn bonus(mut data: ResMut<GameData>) {
            data.score += 500;
        }
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin))
            .init_state::<GameState>()
            .insert_resource(GameData {
                score: 100,
                ..default()
            })
            .insert_resource(FrameRuntime::new(0, None))
            // The frame's system runs before the game's bonus, the order
            // that would make an `OnEnter` read too early.
            .add_systems(OnEnter(GameState::GameOver), (on_game_over, bonus).chain())
            .add_systems(Update, finish_pending_run);
        app.update();
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::GameOver);
        app.update();
        app.update();
        let runtime = app.world().resource::<FrameRuntime>();
        assert_eq!(runtime.final_score, Some(600));
        assert_eq!(runtime.best, 600);
    }

    #[test]
    fn a_run_is_finished_once() {
        let mut world = World::new();
        world.insert_resource(GameData::default());
        world.insert_resource(FrameRuntime::new(0, None));
        world.run_system_once(on_game_over).unwrap();
        world.run_system_once(finish_pending_run).unwrap();
        world.resource_mut::<GameData>().score = 999;
        world.run_system_once(finish_pending_run).unwrap();
        assert_eq!(world.resource::<FrameRuntime>().final_score, Some(0));
    }

    #[test]
    fn a_zero_score_never_celebrates() {
        let mut rt = FrameRuntime::new(0, None);
        assert_eq!(rt.finish_run(0), FrameEvent::GameOver);
    }

    #[test]
    fn a_new_high_score_is_written_to_the_file() {
        let dir = std::env::temp_dir().join(format!("gx-frame-driver-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("best-score");
        let mut rt = FrameRuntime::new(10, Some(path.clone()));
        rt.finish_run(5);
        assert!(!path.exists(), "a losing run writes nothing");
        rt.finish_run(25);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "25\n");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_refused_highlight_keeps_the_first_colour() {
        let mut rt = FrameRuntime::new(0, None);
        assert!(rt.highlight(Some([255, 204, 0])));
        assert!(!rt.highlight(Some([1, 2, 3])));
        assert_eq!(rt.pulse_color, Some([255, 204, 0]));
    }

    #[test]
    fn caption_follows_the_phase() {
        let mut rt = FrameRuntime::new(12340, None);
        rt.advance(0.0, FramePhase::Attract, false);
        assert_eq!(rt.caption().as_deref(), Some("SCORE TO BEAT 12,340"));
        rt.advance(0.1, FramePhase::Playing, false);
        assert_eq!(rt.caption(), None);
        rt.finish_run(900);
        rt.advance(0.1, FramePhase::GameOver, false);
        assert_eq!(rt.caption().as_deref(), Some("FINAL SCORE 900"));
        rt.advance(0.1, FramePhase::Playing, false);
        assert_eq!(
            rt.final_score, None,
            "a new run forgets the last final score"
        );
        rt.finish_run(13000);
        rt.advance(0.1, FramePhase::GameOver, false);
        assert_eq!(rt.caption().as_deref(), Some("NEW HIGH SCORE 13,000"));
    }

    #[test]
    fn no_best_score_means_no_attract_caption() {
        let mut rt = FrameRuntime::new(0, None);
        rt.advance(0.0, FramePhase::Attract, false);
        assert_eq!(rt.caption(), None);
    }

    #[test]
    fn apply_layout_sets_insets_and_geometry_from_the_window() {
        let mut world = World::new();
        world.insert_resource(FrameInsets::default());
        world.insert_resource(FrameGeometry::empty());
        world.spawn((
            Window {
                resolution: WindowResolution::new(1080, 1920),
                ..default()
            },
            PrimaryWindow,
        ));
        world.run_system_once(apply_layout).unwrap();
        assert_eq!(
            *world.resource::<FrameInsets>(),
            FrameInsets {
                reserve_top: 360,
                gap: 8
            }
        );
        let geometry = *world.resource::<FrameGeometry>();
        assert_eq!(geometry.window, UVec2::new(1080, 1920));
        assert_eq!(
            geometry.layout.marquee,
            Some(PxRect {
                x: 0,
                y: 0,
                w: 1080,
                h: 360
            })
        );
        assert_eq!(
            geometry.game,
            letterbox(
                UVec2::new(1080, 1920),
                UVec2::new(GAME_WIDTH, GAME_HEIGHT),
                360,
                8
            )
        );
    }
}
