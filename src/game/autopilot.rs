//! Feature-gated self-playtest harness (`cargo run --features autopilot`).
//!
//! Drives the REAL input path by writing the gamebient-input crate's
//! `VirtualInput` (the same source its touch overlay and `gx:input` host relay
//! use) before `collect_input` folds it into `GameInput`, plays a scripted tour
//! (logo → title → how-to-play → a bot-played run → pause → game over), and
//! saves renderer screenshots at each beat. Used to verify playability and
//! visuals without a human at the keyboard, and by `make-cartridge.sh` to
//! capture the box-cover gameplay shot. Never compiled into shipping builds.
//!
//! Beats and file names are a platform-wide contract (the cover pipeline and
//! the `designing-cartridge-covers` skill rely on them):
//!
//! | file                    | when                                   |
//! |-------------------------|----------------------------------------|
//! | `01-studio-logo.png`    | boot screen                            |
//! | `02-title.png`          | title screen                           |
//! | `03-how-to-play.png`    | instruction screen                     |
//! | `04-early-play.png`     | ~3 s into the run                      |
//! | `05-mid-play.png`       | ~30 s in, the bot mid-action           |
//! | `06-<moment>.png`       | the game's signature moment (see below)|
//! | `07-late-play.png`      | ~48 s in                               |
//! | `08-pause.png`          | the pause overlay over live play       |
//! | `09-game-over.png`      | the real game-over screen              |
//!
//! Env: `AUTOPILOT_DIR` (default `/tmp/gamebient-game-shots`), `AUTOPILOT_SCALE`
//! (window scale-factor override; `1.5` captures 1920x1080).
//!
//! # Customizing for your game
//!
//! Two places need game-specific code; everything else is the shared tour:
//!
//! 1. **The bot** (`drive_bot`): read your game's state and set `held` /
//!    `tap` so the mid/late shots show real action (score on the HUD, things
//!    happening). Reuse any existing playtest bot rather than duplicating it.
//! 2. **The signature moment** (`06-...`): replace `SIGNATURE_BEAT` and the
//!    condition in `signature_moment_ready` with your game's payoff event
//!    (a delivery, a K.O., a combo banner, a level clear).
//!
//! Also make `force_game_over` use your real losing path (zero lives, expire
//! the clock) so `09-game-over` is the real screen, not a state jump.

use bevy::app::AppExit;
use bevy::prelude::*;
#[cfg(not(feature = "record"))]
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use gamebient_input::{Buttons, VirtualInput};

use super::player::Player;
use super::scoring::{GameData, ScoreEvent};
use super::states::{GameState, Paused};
use crate::ui::transition::ScreenFade;

/// File stem of the signature-moment shot. Rename to describe the moment.
const SIGNATURE_BEAT: &str = "06-first-score";

/// Where the screenshots land; override with `AUTOPILOT_DIR`.
fn shot_dir() -> String {
    std::env::var("AUTOPILOT_DIR").unwrap_or_else(|_| "/tmp/gamebient-game-shots".into())
}

/// `AUTOPILOT_SCALE=1.5` renders the 1280x720 window at 1920x1080 so the
/// cover shot is hi-res. Applied once at startup.
fn apply_scale_override(mut windows: Query<&mut Window>) {
    let Some(scale) = std::env::var("AUTOPILOT_SCALE")
        .ok()
        .and_then(|s| s.parse::<f32>().ok())
    else {
        return;
    };
    for mut window in &mut windows {
        window.resolution.set_scale_factor_override(Some(scale));
    }
}

pub struct AutopilotPlugin;

impl Plugin for AutopilotPlugin {
    fn build(&self, app: &mut App) {
        std::fs::create_dir_all(shot_dir()).expect("autopilot shot dir");
        app.init_resource::<Autopilot>()
            .add_systems(Startup, apply_scale_override)
            // Before the crate's collector so this frame's snapshot carries
            // the injected buttons, exactly like a touch-overlay press.
            .add_systems(
                PreUpdate,
                drive_autopilot.before(gamebient_input::input::collect_input),
            );
    }
}

#[derive(Resource, Default)]
struct Autopilot {
    last_state: Option<GameState>,
    /// Seconds since the current `GameState` was entered.
    t: f32,
    /// One-shot script beats already fired in the current state.
    fired: Vec<&'static str>,
    signature_done: bool,
}

impl Autopilot {
    fn once(&mut self, beat: &'static str) -> bool {
        if self.fired.contains(&beat) {
            false
        } else {
            self.fired.push(beat);
            true
        }
    }
}

/// One-frame press edge: `latched` bits read as just-pressed on the next
/// `collect_input` without entering the held set, so no release bookkeeping
/// is needed (the same path a sub-frame touch tap takes).
fn tap(virt: &mut VirtualInput, button: Buttons) {
    virt.latched |= button;
}

/// Captures a named beat. Under `record` every frame is already captured
/// (and a second `Screenshot` of the same window in one frame is dropped as
/// a duplicate), so the beat is logged instead and `tools/cut_clips.py`
/// extracts the still from `tour.mp4`.
fn shot(commands: &mut Commands, name: &str) {
    #[cfg(feature = "record")]
    {
        info!("autopilot: beat {name}");
        commands.write_message(super::record::RecordBeat(name.to_string()));
    }
    #[cfg(not(feature = "record"))]
    {
        let path = format!("{}/{name}.png", shot_dir());
        info!("autopilot: screenshot {path}");
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
    }
}

/// Steers the whole session. Runs before `collect_input` so the injected
/// buttons are indistinguishable from a human player's.
#[allow(clippy::too_many_arguments)]
fn drive_autopilot(
    mut commands: Commands,
    mut auto: ResMut<Autopilot>,
    mut virt: ResMut<VirtualInput>,
    time: Res<Time>,
    state: Res<State<GameState>>,
    paused: Res<Paused>,
    mut fade: ResMut<ScreenFade>,
    mut data: ResMut<GameData>,
    mut scores: MessageWriter<ScoreEvent>,
    player: Query<&Transform, With<Player>>,
    mut app_exit: MessageWriter<AppExit>,
) {
    let current = *state.get();
    if auto.last_state != Some(current) {
        auto.last_state = Some(current);
        auto.t = 0.0;
        auto.fired.clear();
    }
    auto.t += time.delta_secs();
    let t = auto.t;

    match current {
        GameState::StudioLogo => {
            if t > 1.2 && auto.once("shot") {
                shot(&mut commands, "01-studio-logo");
            }
            if t > 1.5 && auto.once("skip") {
                tap(&mut virt, Buttons::START);
            }
        }
        GameState::Menu => {
            if t > 1.0 && auto.once("shot") {
                shot(&mut commands, "02-title");
            }
            if t > 1.4 && auto.once("start") {
                tap(&mut virt, Buttons::START);
            }
        }
        GameState::HowToPlay => {
            if t > 1.2 && auto.once("shot") {
                shot(&mut commands, "03-how-to-play");
            }
            if t > 2.2 && auto.once("start") {
                tap(&mut virt, Buttons::START);
            }
        }
        GameState::Playing => {
            // Timed beats around the bot play.
            if t > 3.0 && auto.once("shot-early") {
                shot(&mut commands, "04-early-play");
            }
            if t > 30.0 && auto.once("shot-mid") {
                shot(&mut commands, "05-mid-play");
            }
            if t > 48.0 && auto.once("shot-late") {
                shot(&mut commands, "07-late-play");
            }
            if !auto.signature_done && signature_moment_ready(&data) {
                auto.signature_done = true;
                shot(&mut commands, SIGNATURE_BEAT);
            }

            // Pause beat: freeze, capture the overlay, resume.
            if t > 50.0 && auto.once("pause") {
                tap(&mut virt, Buttons::PAUSE);
            }
            if t > 50.7 && auto.once("shot-pause") {
                shot(&mut commands, "08-pause");
            }
            if t > 51.2 && auto.once("unpause") {
                tap(&mut virt, Buttons::PAUSE);
            }
            // End the tour through the real losing path.
            if t > 54.0 && auto.once("game-over") {
                force_game_over(&mut data, &mut fade);
            }

            if paused.0 || (50.0..51.5).contains(&t) {
                virt.set_held(Buttons::NONE);
                return;
            }
            drive_bot(t, &mut virt, &mut scores, &player);
        }
        GameState::GameOver => {
            virt.set_held(Buttons::NONE);
            if t > 1.0 && auto.once("shot") {
                shot(&mut commands, "09-game-over");
            }
            if t > 1.6 && auto.once("exit") {
                info!("autopilot: tour complete, exiting");
                app_exit.write(AppExit::Success);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Game-specific section. Replace these three functions for your game.
// ---------------------------------------------------------------------------

/// Bot policy for the template game: sweep the cube left and right across
/// the arena and bank a few points so the HUD shows a score. Replace with a
/// policy that reads your game's world (nearest target, hazards, timers) and
/// plays well enough that the mid/late shots show real action.
fn drive_bot(
    t: f32,
    virt: &mut VirtualInput,
    scores: &mut MessageWriter<ScoreEvent>,
    player: &Query<&Transform, With<Player>>,
) {
    let Ok(tf) = player.single() else {
        return;
    };
    // Bounce between x = -6 and x = 6.
    let heading_right = (t / 3.0).floor() as i32 % 2 == 0;
    let held = match (heading_right, tf.translation.x) {
        (true, x) if x < 6.0 => Buttons::RIGHT,
        (false, x) if x > -6.0 => Buttons::LEFT,
        _ => Buttons::NONE,
    };
    virt.set_held(held);
    // The template has nothing to score on; award points every ~4 s so the
    // HUD is not blank in the shots. Delete this in a real game.
    if (t * 10.0) as i32 % 40 == 0 {
        scores.write(ScoreEvent { points: 100 });
    }
}

/// True when the signature moment is on screen. The template fires on the
/// first score; a real game keys this off its payoff event.
fn signature_moment_ready(data: &GameData) -> bool {
    data.score > 0
}

/// Ends the run through the real losing path. The template has no losing
/// condition, so it zeroes lives and requests the game-over fade directly;
/// a real game should instead trip whatever its own systems watch (lives,
/// health, a clock) and let them run.
fn force_game_over(data: &mut GameData, fade: &mut ScreenFade) {
    data.lives = 0;
    let _ = fade.request(GameState::GameOver);
}
