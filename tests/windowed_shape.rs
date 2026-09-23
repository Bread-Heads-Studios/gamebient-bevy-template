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
//! * **`AssetsPlugin` itself**, on hand-made asset stores — see
//!   [`record_windowed`], which carries the four lines a port needs.
//!   Resources are not the only way presentation reaches the sim: a
//!   decorator that writes a *field of a component* on an entity the sim
//!   owns is the other, and it is the one Dive Rise was caught by.
//!   `creature_visuals::face_movement` mirrors the player's own
//!   `Transform.scale.x` to face the swim direction, and
//!   `swim_burst::dash` read that sign back as the direction of a dash
//!   thrown with no stick held. The verifier builds no `AssetsPlugin`, so
//!   the scale was `+1` there for the whole run and a dash from a
//!   standstill while facing left went one way in the browser and the
//!   other in the verifier. Nothing could see it: the fixtures run bare (so
//!   both sides read `+1` and agree by accident), `archetype_order` adds
//!   components but never writes an existing one's *value*, and this file
//!   used to leave the decorators out.
//!
//! The rule the test encodes, in the width Dive Rise taught it: **a sim
//! system may not read any state the windowed build writes and the
//! verifier does not** — a resource, or a field of a component on an entity
//! the sim owns (`Transform.scale`/`rotation`, `Visibility`). Latch such a
//! thing from `TickInput` into sim-owned state and let presentation *paint*
//! it, one-directionally; a facing latched in the movement system from
//! `TickInput.move_x` is the shape.
//! `sim::end_run`'s `Option<ResMut<ScreenFade>>` is one
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
//!
//! **Two rows are not a target, they are the shipped minimum.** With the
//! real `AssetsPlugin` carried and both rows in place, Dive Rise's probe
//! still passed with the facing bug present: its forager was facing right
//! on every one of the six blind dashes it threw, and its diver mashed A
//! every tick but held `DOWN` every tick too, so its stick was never zero
//! and the fallback was never reached. Each bot reached one half of the
//! bug and neither reached both. A third row — hold LEFT for 90 ticks,
//! release for 30, tap A during the release — reaches both, and two
//! counters keep it honest: one asserted non-zero for *every* bot, so the
//! asset layer can never go inert unnoticed, and one asserted non-zero for
//! the new row alone, so it cannot decay into a copy of another. Add rows
//! until each windowed-only mechanic has one that reaches it, then plant
//! the bug back and confirm the right row goes red.

use bevy::prelude::*;
use bevy::shader::Shader;

use gamebient_game::game::replay::recorder::ReplayRecorder;
use gamebient_game::game::replay::selftest::{SELFTEST_SEED, SELFTEST_TICKS, script};
use gamebient_game::game::replay::{Replay, build_headless_app, verify};
use gamebient_game::game::sim::{PendingSeed, RunOver, SimTick, tick_duration};
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
    /// The third slot, and the one row here that is not a placeholder:
    /// [`coaster_script`] **releases the stick**. Both rows above hold a
    /// direction on every single tick, so between them they never produce
    /// an idle-stick tick — and a "do it the way you're facing" fallback
    /// fires on exactly those. Dive Rise's sprite-mirror desync survived
    /// two rows for that reason and died against this one.
    Coaster,
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

/// The bot that lets go of the stick, and taps while it is let go.
///
/// Holds LEFT for 90 ticks, releases for 30, and taps A in the middle of
/// the release. Three things a game's sim can disagree with its presentation
/// about meet on those ticks: the player has a *facing* (something moved it
/// left), the stick is *idle* (so a direction-less action has to get its
/// direction from somewhere), and an action *fires*.
///
/// That combination is what neither shipped row above can produce — they
/// both hold a direction on every tick — and it is the one Dive Rise's
/// `swim_burst::dash` fell through, reading the sprite's mirrored
/// `Transform.scale.x` (written by an `AssetsPlugin` `Update` system, and
/// `+1` for ever in the verifier) as the direction of a dash thrown from a
/// standstill.
///
/// Keep the row when you adapt this file, and point it at whatever this
/// game's direction-less action is (a dash, a swing, a drop, a fire button
/// with no aim). Same two constraints as [`reckless_script`]: pure input,
/// no RNG.
fn coaster_script(tick: Res<SimTick>, mut virt: ResMut<VirtualInput>) {
    let phase = tick.0 % 120;
    if phase < 90 {
        virt.set_held(Buttons::LEFT);
    } else {
        // Stick released — and an action thrown while it is.
        virt.set_held(Buttons::NONE);
        if phase == 105 {
            virt.latched |= Buttons::A;
        }
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
/// player's first run really begins in), the game's own `AssetsPlugin` on
/// hand-made asset stores, plus the `Update` systems `GamePlugin` registers
/// only when `!headless`.
///
/// This is where a port adds its own presentation systems — see the module
/// comment.
fn record_windowed(bot: Bot) -> Replay {
    record_windowed_after_menu(bot, 0)
}

/// [`record_windowed`], but with `menu_ticks` fixed ticks of **app life**
/// burned in `Menu` before the run starts — the shape a real player's run
/// has, and the one shape no other probe in this repo had.
///
/// It is here because that gap cost Dive Rise a production `UNVERIFIED`.
/// `Time<Fixed>::elapsed()` counts from **app start**, not from run start,
/// and nine of its sim systems drove a drift, a weave, an orbit or a lunge
/// phase off it. Every probe the fleet had — `record_windowed` itself,
/// `selftest::record_scripted_run`, both committed `.gxr` fixtures, the
/// `--playtest` harness — enters `Playing` in the app's first frames,
/// exactly like `replay::run_verify_app` does, so all of them agreed with
/// the verifier by accident. A player who watched the studio logo, read the
/// menu and then pressed Start did not: their run started with a different
/// clock and re-simulated into a different game.
///
/// `build_headless_app` steps exactly one fixed tick per `update()`
/// (`TimeUpdateStrategy::ManualDuration(tick_duration())`), so the dwell
/// loop below is `menu_ticks` ticks of app life with no run in progress.
///
/// See `sim::RunClock` and
/// [`a_run_does_not_depend_on_how_long_the_app_was_up_before_it`].
fn record_windowed_after_menu(bot: Bot, menu_ticks: u32) -> Replay {
    let mut app = build_headless_app();
    // The real `src/assets/` decorators, on hand-made asset stores.
    // `build_headless_app` is `MinimalPlugins`, so none of this exists by
    // default and `AssetsPlugin` cannot bake anything there; these four
    // lines are what a port needs to carry it:
    //
    // * `AssetPlugin` for the `AssetServer` — any plugin that *loads*
    //   something (a shader kit, a font, an atlas) will not build without
    //   one, and inserting bare `Assets<T>` stores is not enough for it;
    // * `init_asset::<T>()` for each store the plugin bakes its art
    //   resources out of. This template's own art is a `Mesh` and a
    //   `StandardMaterial`; `Shader` and `ColorMaterial` are here because a
    //   2D game's `AssetsPlugin` wants them and a port should not have to
    //   rediscover the list.
    //
    // It is here because leaving it out cost Dive Rise a desync that
    // nothing else could see: a sim system read `Transform.scale.x`, a
    // field an `AssetsPlugin` `Update` system writes to mirror the sprite,
    // which is `+1` for ever in the verifier. See the module comment.
    //
    // Also insert here whatever a game's `main.rs` inserts rather than its
    // plugin (a playtest speed multiplier, a settings resource) — otherwise
    // the plugin's systems are present but never run.
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.init_asset::<Shader>();
    app.init_asset::<Mesh>();
    app.init_asset::<ColorMaterial>();
    app.init_asset::<StandardMaterial>();
    app.add_plugins(gamebient_game::assets::AssetsPlugin);
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
        Bot::Coaster => app.add_systems(
            PreUpdate,
            coaster_script.before(gamebient_input::input::accumulate_input),
        ),
    };
    app.init_resource::<FadeBusyFrames>();
    app.add_systems(Update, (tick_fade, count_fade_busy));
    app.world_mut().resource_mut::<PendingSeed>().0 = Some(SELFTEST_SEED);
    app.update();
    // The menu dwell: the studio logo, the title, the how-to-play screen.
    for _ in 0..menu_ticks {
        app.update();
    }
    // Proved, not assumed. If this app ever stopped advancing `Time<Fixed>`
    // outside `Playing` — a different `TimeUpdateStrategy`, a run condition
    // on the fixed loop — the dwell would cost 1 319 updates and probe
    // nothing, and the row below would go green for the wrong reason.
    let elapsed_before_run = app.world().resource::<Time<Fixed>>().elapsed();
    assert!(
        menu_ticks == 0 || elapsed_before_run >= tick_duration() * menu_ticks,
        "the menu dwell did not advance Time<Fixed>, so this row probes \
         nothing: {elapsed_before_run:?} after {menu_ticks} ticks"
    );
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Playing);
    app.update();
    // The asset stores are live, proved rather than assumed: this
    // template's `player::spawn_player` inserts `Mesh3d`/`MeshMaterial3d`
    // only when `Assets<Mesh>` and `Assets<StandardMaterial>` are both
    // present, which is the same `Option<Res<...>>` fork every game's
    // decorators hang off. If this count is zero the asset lines above did
    // nothing and the probe is back to covering resources only.
    //
    // The template's own `AssetsPlugin` is a stub, so this is the strongest
    // statement it can make about itself. **In a game it is not enough**:
    // assert there too that a decorator has actually written the component
    // *field* a sim system might read (a mirrored `scale.x`, a rotation),
    // under every bot in the table — see the module comment.
    let meshed = app
        .world_mut()
        .query_filtered::<Entity, With<Mesh3d>>()
        .iter(app.world())
        .count();
    assert!(
        meshed > 0,
        "{bot:?}: no entity carries a Mesh3d, so AssetPlugin/init_asset above \
         did not take effect and this app is not windowed-shaped in the asset \
         dimension. The decorators that write component values on sim \
         entities are the half of this probe that resources cannot cover."
    );
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

/// **The production bug this file had no row for.**
///
/// A real run on colecovisiongx.com came back `UNVERIFIED / mismatch`:
/// 11 877 ticks claiming score 144, re-simulating to 56 in the site's
/// verifier, in a locally rebuilt wasm one and natively alike. Nothing was
/// wrong with the replay — the *recording* side was reading a clock the
/// verifier does not have. `Time<Fixed>::elapsed()` inside `FixedUpdate`
/// counts from **app start**, and nine of that game's sim systems drove
/// their drift, weave, orbit and lunge phases off it, so a run's whole world
/// depended on how long the player had been staring at the menu.
/// `replay::run_verify_app` enters `Playing` on its second `update()`; a
/// player does not.
///
/// The property this asserts is the general one: **a run's outcome does not
/// depend on how long the app was up before it started.** It is the mutation
/// check for `sim::RunClock` — reverting a single one of Dive Rise's nine
/// sites failed it (score 39 against 33), and on this template planting a
/// `time.elapsed_secs()` read in `player::move_player` fails it while every
/// other row in this file stays green.
///
/// Two things about the number. 1 319 ticks is ~22 s of menu and
/// deliberately not round: a multiple of anything in the sim could agree by
/// luck. And it is longer than `SELFTEST_TICKS`, so the offset it introduces
/// is larger than the run it perturbs.
///
/// **A short, quiet run will not reproduce this class of bug**, on this
/// probe or in a browser. Measured on Dive Rise: a 1 720-tick browser run
/// that ate nothing and took no damage verified `matches: true` on the
/// *broken* build, and re-simulating it at eight different clock offsets
/// gave the same checksum every time — nothing the clock drove had reached
/// anything the checksum folds. So when a port adapts this file, point the
/// dwell at a bot that actually plays: a long run that scores, spawns and
/// collides, not one that idles.
#[test]
fn a_run_does_not_depend_on_how_long_the_app_was_up_before_it() {
    const MENU_TICKS: u32 = 1319;
    let cold = record_windowed(Bot::Selftest);
    let warm = record_windowed_after_menu(Bot::Selftest, MENU_TICKS);
    assert_eq!(
        (warm.ticks, warm.score, warm.checksum),
        (cold.ticks, cold.score, cold.checksum),
        "the same seed and the same inputs produced a different run after \
         {MENU_TICKS} ticks in the menu. Some sim system is reading a clock \
         that starts with the app rather than with the run — almost always \
         `Time::elapsed_secs()` inside `FixedUpdate`, which is \
         `Time<Fixed>`'s app-lifetime elapsed. Read `sim::RunClock` instead; \
         see its doc comment and determinism rule 7."
    );
    assert_eq!(
        warm.runs, cold.runs,
        "sanity: the bot pressed different buttons in the two recordings, so \
         the comparison above was never about the clock. The scripts are \
         driven by SimTick, which begin_run rewinds, so this should be \
         impossible — check what the menu dwell left in VirtualInput."
    );
    let v = verify(&warm);
    assert!(
        v.matches,
        "a run recorded after {MENU_TICKS} ticks of menu did not reproduce in \
         a bare verifier app, which starts its run immediately.\n{v:?} vs \
         claimed score {} checksum {}",
        warm.score, warm.checksum
    );
    assert_eq!(v.ticks, SELFTEST_TICKS);
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
/// The third row, and the one that asks the question the other two cannot:
/// what does the sim do on a tick when **nothing is held**?
///
/// A game whose direction-less action falls back on presentation state — a
/// mirrored sprite scale, a rotation an animator writes — agrees with
/// itself in both apps until a run produces a facing *and* an idle stick
/// *and* a press on the same tick. Both rows above hold a direction every
/// tick, so neither ever produces one. This row does, sixty times over a
/// 3600-tick script.
///
/// On the template's own game it is, like the reckless row, weaker than
/// the fixture script — the template's `AssetsPlugin` is a stub, so no
/// decorator writes a component value for a sim system to read back.
/// Measured on this template while adopting the row: a planted
/// `tf.scale.x.signum()` fallback in `move_player` passes all three rows on
/// its own, and fails **this** row (and only this row) as soon as the
/// template's `AssetsPlugin` is given a `face_movement` system of the kind
/// every real game has. That is the pair of mutations to re-run in a game
/// after adapting this file.
#[test]
fn a_run_that_releases_the_stick_still_verifies_without_the_windowed_presentation() {
    let replay = record_windowed(Bot::Coaster);
    assert_eq!(
        replay.ticks, SELFTEST_TICKS,
        "the coaster run should reach the same tick count as the bare one"
    );
    let v = verify(&replay);
    assert!(
        v.matches,
        "a run that releases the stick, recorded with the windowed presentation \
         present, did not reproduce in a bare verifier app. The usual cause is a \
         direction-less action reading a facing the presentation layer owns — a \
         mirrored Transform.scale.x, an animator's rotation — which the verifier \
         never writes. Latch the facing from TickInput into sim-owned state and \
         let presentation paint it.\n{v:?} vs claimed score {} checksum {}",
        replay.score, replay.checksum
    );
    assert_eq!(v.ticks, SELFTEST_TICKS);
}

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
