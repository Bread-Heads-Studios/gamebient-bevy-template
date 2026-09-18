//! The other half of `tests/archetype_order.rs`: that file asks whether the
//! windowed build's extra *entities and components* change the sim, this one
//! asks whether its extra **resources and `Update` systems** do.
//!
//! It exists because on one game they did. Sundae Shooter's
//! `launcher::fire_scoop` and `launcher::swap_queue` refused to act while
//! `ui::transition::ScreenFade` was mid-transition — a reasonable-looking
//! "don't shoot during a wipe" gate, and a determinism bug three ways over:
//! the fade lives in `UiPlugin`, which the verifier does not build (so it
//! never blocks anything there); it is ticked from `Update` with the *frame*
//! delta (so how many sim ticks it covers depends on the frame rate); and it
//! is busy for the first ~24 ticks of every run, because entering `Playing`
//! goes through it. A run recorded in the browser came back claiming 195
//! points against 200 re-simulated: the verifier took a shot the player's
//! game had refused.
//!
//! No fixture could have caught that — `--selftest` and the two `.gxr` files
//! all run in bare apps with no fade at all, so the selftest's verdict was
//! byte-for-byte identical before and after the fix — and neither could
//! `archetype_order`, which adds components rather than resources. So:
//! record the selftest script in an app carrying the windowed build's
//! presentation state and its real `Update` systems, then `verify()` it in a
//! bare one, and require the same score, tick count and checksum. That is
//! the production question exactly: the browser records with all of this
//! present and the site re-simulates with none of it.
//!
//! # Adapt this during the port
//!
//! As shipped it covers `ScreenFade`, which every template-derived game has
//! and which is the trap that has actually bitten. Add your game's own
//! `!headless` shape to `record_windowed` below:
//!
//! * every `Update` system `GamePlugin::build` registers only when
//!   `!self.headless` and that could plausibly touch run state (juice,
//!   tweens, recoil, shake — the ones whose names sound harmless);
//! * every resource `UiPlugin`/`AssetsPlugin` own that a sim system might
//!   read. `Option<Res<T>>` in a sim system is a compile fix, not a
//!   determinism fix: the run still forks on whether `T` was there.
//!
//! The rule the test encodes: **a sim system may not read anything
//! `UiPlugin` owns.** `sim::end_run`'s `Option<ResMut<ScreenFade>>` is one
//! sanctioned touch, because it only *writes* a fade request on the tick
//! the run ends and `sim::RunOver` has already frozen `SimSet` by then.
//! `game::toggle_pause`'s `Option<Res<ScreenFade>>` is the other, and the
//! port checklist's rule 1 explains why it is safe (rule 8: the recorder
//! and the feeder both mask `Buttons::PAUSE`, so a replay can never re-play
//! the press the fade would have gated).
//!
//! # And then check the bot actually reaches the mechanics
//!
//! **A careful fixture bot under-tests this probe.** Measured on Attic
//! Excavator while adopting it: over the selftest script's full 1800 ticks,
//! `heavy::wobble_shake` fired zero times and `cat::cat_touch` never
//! connected — by construction, because that game's fixture router costs
//! +60 for a cell under heavy junk and +25 near an awake cat, so it avoids
//! precisely the two windowed-vs-headless divergences worth probing. Two
//! planted bugs (a *sim* system filtering its query `With<Visibility>`; the
//! checksum folding the `translation.x` only `wobble_shake` writes) passed
//! under the fixture bot and failed only under a second, reckless one. A
//! probe that never runs the system it is probing is green for the wrong
//! reason.
//!
//! So the probe is bot-table-driven: see [`Bot`]. The template ships the
//! fixture script plus [`reckless_script`], a placeholder — it is *a*
//! different bot, not necessarily one that reaches *your* game's
//! windowed-only mechanics. During the port, measure (a counter resource
//! like [`FadeBusyFrames`] is the whole technique), and if the fixture bot
//! is a careful router, replace `reckless_script` with one that goes
//! looking for trouble and assert the counter is non-zero. See the port
//! checklist, rule 1.

use bevy::prelude::*;

use gamebient_game::game::replay::recorder::ReplayRecorder;
use gamebient_game::game::replay::selftest::{SELFTEST_SEED, SELFTEST_TICKS, script};
use gamebient_game::game::replay::{Replay, build_headless_app, verify};
use gamebient_game::game::sim::{PendingSeed, RunOver, SimTick};
use gamebient_game::game::states::GameState;
use gamebient_game::ui::transition::ScreenFade;
use gamebient_input::{Buttons, VirtualInput};

/// `ui::transition::update_fade` minus the overlay it paints: the fade ticks
/// on the frame delta and drives the state change, exactly as the windowed
/// build does.
fn tick_fade(
    time: Res<Time>,
    mut fade: ResMut<ScreenFade>,
    mut next: ResMut<NextState<GameState>>,
) {
    if let Some(target) = fade.tick(time.delta_secs()) {
        next.set(target);
    }
}

/// Which bot drives the recording — the probe's bot table.
///
/// A plain `&[(&str, fn)]` table is what this wants to be and cannot: each
/// script is a Bevy system with its own parameter list (the fixture's takes
/// `Res<SimTick>`, a game's reckless one usually takes queries over its own
/// entities), so they share no function-pointer type. An enum plus the
/// `match` in [`record_windowed`] is the same table with the registration
/// written out once per row.
///
/// Add rows freely. Every row costs one `#[test]` and one recorded run.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Bot {
    /// `replay::selftest::script`, the fixture's own bot. This is the run
    /// whose tick count is a known constant, and the one the committed
    /// `.gxr` fixtures were recorded from.
    Selftest,
    /// The second slot: a bot that plays differently from the fixture's, so
    /// the probe reaches mechanics the fixture's script routes around. The
    /// template ships [`reckless_script`] here as a placeholder — see this
    /// file's module comment.
    Reckless,
}

/// A deliberately careless bot: the second row of the table, shipped so the
/// slot is wired rather than described.
///
/// It is a **placeholder**. Being different from the fixture's script is all
/// it is: it holds one of the four directions, mashes A every tick and B
/// often, on a cheap deterministic scramble of the tick index. That is
/// enough to shake loose a presentation system that only runs while the
/// player is moving or firing, and it is not enough for a game whose
/// interesting divergences live behind an objective the bot has to *seek*
/// (Attic Excavator's teetering heavies: its replacement picks the
/// shallowest idle heavy and digs down beside it).
///
/// Two constraints on whatever replaces it, both of which this one honours:
///
/// * **Pure input.** It may read the world to decide what to press, but it
///   may only ever write `VirtualInput`. Writing run state would make the
///   recording unreplayable, which is the one thing the probe cannot have.
/// * **No RNG.** Not `rand::rng()` (the run would differ every time) and not
///   `sim::GameRng` (drawing from the sim's stream from `PreUpdate` desyncs
///   every later draw). A hash of the tick is deterministic and free.
fn reckless_script(tick: Res<SimTick>, mut virt: ResMut<VirtualInput>) {
    let h = tick.0.wrapping_mul(2_654_435_761) >> 11;
    let held = match h % 4 {
        0 => Buttons::LEFT,
        1 => Buttons::RIGHT,
        2 => Buttons::UP,
        _ => Buttons::DOWN,
    };
    virt.set_held(held);
    virt.latched |= Buttons::A;
    if h.is_multiple_of(3) {
        virt.latched |= Buttons::B;
    }
}

/// Frames on which `ScreenFade` had something to do.
///
/// This is the shipped instance of the measurement the module comment asks
/// for: the fade is the one presentation mechanic the template's own probe
/// carries, so if this counter were zero the probe would be passing because
/// nothing presentational ever happened during the run — green for the
/// wrong reason. Both tests assert it is not.
///
/// **During the port, add the counters your game needs beside it**: one per
/// windowed-only system whose absence in the verifier would matter, counting
/// the frames on which that system had a live entity to write. Then assert
/// non-zero under whichever bot is supposed to reach it. That assertion is
/// what stops the second row of the bot table quietly decaying into a
/// duplicate of the first.
#[derive(Resource, Default)]
struct FadeBusyFrames(u32);

fn count_fade_busy(mut frames: ResMut<FadeBusyFrames>, fade: Res<ScreenFade>) {
    if !fade.is_idle() {
        frames.0 += 1;
    }
}

/// The scripted run, recorded in an app shaped like the windowed game:
/// `ScreenFade` present and mid-fade at the start (`boot()` is the state the
/// player's first run really begins in), plus the `Update` systems
/// `GamePlugin` registers only when `!headless`.
///
/// This is where a port adds its own presentation systems — see the module
/// comment.
fn record_windowed(bot: Bot) -> Replay {
    let mut app = build_headless_app();
    app.insert_resource(ScreenFade::boot());
    // The bot table. One arm per `Bot`; each arm registers that row's script
    // where the fixture's own harness registers it, so the recorded input is
    // the input the sim saw.
    match bot {
        Bot::Selftest => app.add_systems(
            PreUpdate,
            script.before(gamebient_input::input::accumulate_input),
        ),
        Bot::Reckless => app.add_systems(
            PreUpdate,
            reckless_script.before(gamebient_input::input::accumulate_input),
        ),
    };
    app.init_resource::<FadeBusyFrames>();
    app.add_systems(Update, (tick_fade, count_fade_busy));
    app.world_mut().resource_mut::<PendingSeed>().0 = Some(SELFTEST_SEED);
    app.update();
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Playing);
    app.update();
    // Bounded exactly like `selftest::record_scripted_run`: a script that
    // ends its own run freezes `SimSet` and leaves `Playing`, and `SimTick`
    // then never reaches `SELFTEST_TICKS`.
    while app.world().resource::<SimTick>().0 < SELFTEST_TICKS {
        if app.world().resource::<RunOver>().0
            || *app.world().resource::<State<GameState>>().get() != GameState::Playing
        {
            break;
        }
        app.update();
    }
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::GameOver);
    app.update();
    assert!(
        app.world().resource::<FadeBusyFrames>().0 > 0,
        "{bot:?}: ScreenFade was idle for the whole run, so this app was not \
         windowed-shaped in the one way the shipped probe measures. Check \
         that ScreenFade::boot() is still inserted and tick_fade still \
         registered — otherwise the test passes without probing anything."
    );
    app.world()
        .resource::<ReplayRecorder>()
        .last_run()
        .cloned()
        .expect("run sealed on OnExit(Playing)")
}

#[test]
fn a_run_recorded_with_the_windowed_presentation_still_verifies_without_it() {
    let replay = record_windowed(Bot::Selftest);
    assert_eq!(
        replay.ticks, SELFTEST_TICKS,
        "the windowed-shaped run should reach the same tick count as the bare one"
    );
    let v = verify(&replay);
    assert!(
        v.matches,
        "a run recorded with ScreenFade and the windowed Update systems present \
         did not reproduce in a bare verifier app. Some sim system is reading \
         presentation state — the fade is the classic one, and it is frame-rate \
         dependent as well as absent headless. See this file's module comment \
         and the port checklist, rule 1.\n{v:?} vs claimed score {} checksum {}",
        replay.score, replay.checksum
    );
    assert_eq!(v.ticks, SELFTEST_TICKS);
}

/// The same question under the table's second bot. No tick-count constant
/// here: a reckless run ends when it ends (a real game over, on a game that
/// has one), and the point is that however it ends, the windowed recording
/// re-simulates bare.
///
/// On the template's own game this is a weaker test than the first — the
/// fixture script already exercises everything there is. It is a real one as
/// soon as a port replaces [`reckless_script`] with a bot that reaches what
/// the fixture's routes around, which is the case the module comment and the
/// checklist's rule 1 are about.
#[test]
fn a_reckless_run_recorded_with_the_windowed_presentation_still_verifies_without_it() {
    let replay = record_windowed(Bot::Reckless);
    assert!(
        replay.ticks > 0,
        "the reckless run recorded no ticks at all; it is testing nothing"
    );
    let v = verify(&replay);
    assert!(
        v.matches,
        "a reckless run recorded with the windowed presentation present did not \
         reproduce in a bare verifier app. This bot reaches windowed-only \
         systems the fixture's careful script routes around — that is what the \
         second row of the bot table is for.\n{v:?} vs claimed score {} \
         checksum {}",
        replay.score, replay.checksum
    );
    assert_eq!(v.ticks, replay.ticks);
}
