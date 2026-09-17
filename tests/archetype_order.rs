//! The archetype-order probe: does an `Update` system that *only* decorates
//! sim entities with presentation components change what the sim does?
//!
//! **Why this test exists, and why the usual one is not enough.** The port
//! checklist's "remove the system and re-run `--selftest`" check proves a
//! presentation system is *value*-safe — it writes nothing the checksum
//! folds. It cannot detect this bug at all, because the decoration changes
//! nothing about a headless checksum: there is no decoration in a headless
//! run. What it changes is the **archetype** of the entities it touches, and
//! archetype order is the order a `Query` iterates in. The windowed game
//! decorates and iterates one order; the verifier adds no render plugins,
//! never decorates, and iterates another. Every fixture stays green (nothing
//! renders during a selftest) and only replays of real, rendered runs
//! diverge. Grand Theft Auto-Reply lost three of six seeds that way.
//!
//! So the probe is differential: record the selftest script **twice** — once
//! in a plain headless app, once in a headless app that also runs `Update`
//! systems inserting inert markers onto every entity the sim queries — and
//! assert that
//!
//! 1. both recordings agree on score, checksum and tick count, and
//! 2. the decorated recording still `verify()`s, i.e. re-simulates correctly
//!    in a *plain* app. That second one is the production question exactly:
//!    a browser records in a decorated app and Node verifies in a bare one.
//!
//! Repeated across several seeds and at 1, 2 and 3 sim ticks per frame,
//! because the bug is cadence-sensitive: at one tick per frame the `Update`
//! decoration and the `FixedUpdate` sim interleave one-to-one and a reorder
//! can stay hidden.
//!
//! # Extending this for your game (do this during the port)
//!
//! On this template the sim queries exactly one entity (`Player`), so the
//! probe is a smoke test: with one entity there is no order to get wrong, and
//! what it really proves here is that the inserts and the child spawn are
//! value-safe and cadence-independent. **It only grows teeth once the markers
//! reproduce your game's own decorators**, so:
//!
//! * Add one marker per presentation component your `Update` systems attach
//!   to sim entities, and one `Update` system per real decorator.
//! * **Reproduce the decorators' branching.** A decorator that inserts a
//!   different component set on different entities (golden crumbs get a
//!   sparkle, rivals get a mood, a boss gets a health bar) is what splits one
//!   archetype into several — which is what reorders the query. A marker set
//!   that is the same for every entity cannot reproduce the bug.
//! * Keep spawning a child of a sim entity (below): `ChildOf` puts `Children`
//!   on the **parent**, so a decorator that only spawns children still moves
//!   its parent's archetype.
//! * Extend `trace_order` to fold the order of every sim query whose system
//!   accumulates order-sensitively (see the checklist, "The two order
//!   traps"). The trace is diagnostic — a `same=false, order_same=false` line
//!   in the output names the failure precisely — while the assertion is on
//!   the checksum and the verdict.
//!
//! A failure here is real and is not ulp drift: both sides are the same
//! build, the same arithmetic and the same seed.

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use gamebient_input::{Buttons, VirtualInput};

use gamebient_game::game::player::Player;
use gamebient_game::game::replay::recorder::ReplayRecorder;
use gamebient_game::game::replay::selftest::SELFTEST_TICKS;
use gamebient_game::game::replay::{Replay, build_headless_app, verify};
use gamebient_game::game::sim::{self, PendingSeed, RunOver, SimTick};
use gamebient_game::game::states::GameState;

/// Stand-ins for the presentation components a real game's `Update` systems
/// attach to sim entities (a mesh, a material, a sparkle, a mood). Inert on
/// purpose: nothing in the sim reads them, which is the whole point — if the
/// sim's behaviour changes, it changed through the archetype, not the value.
#[derive(Component)]
struct FakeMesh;
#[derive(Component)]
struct FakeSparkle;
#[derive(Component)]
struct FakeChildVisual;

/// Folds the ORDER (not the identity) in which the sim's own queries hand
/// entities back, so a pure reordering is visible even on a run whose outcome
/// happens to come out the same. Diagnostic only; the assertions below are on
/// the checksum and the verdict.
#[derive(Resource, Default)]
struct OrderTrace(u64);

fn trace_order(players: Query<Entity, With<Player>>, mut trace: ResMut<OrderTrace>) {
    // Position-sensitive but identity-insensitive: fold each entity's RANK
    // among the ids returned alongside it, so re-ordering the same set shows
    // up while a different allocation of the same order does not.
    let ids: Vec<Entity> = players.iter().collect();
    let mut sorted: Vec<u64> = ids.iter().map(|e| e.to_bits()).collect();
    sorted.sort_unstable();
    for (i, e) in ids.iter().enumerate() {
        let rank = sorted.binary_search(&e.to_bits()).unwrap() as u64;
        trace.0 = trace.0.rotate_left(7).wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ (i as u64)
            ^ rank.wrapping_mul(31);
    }
}

/// Mimics a game's `attach_*_visuals`: decorates every sim entity that has
/// not been decorated yet, from `Update`, with a branch (so more than one
/// archetype results) and a child (so `Children` lands on the parent).
fn decorate_players(mut commands: Commands, new: Query<Entity, (With<Player>, Without<FakeMesh>)>) {
    for (i, e) in new.iter().enumerate() {
        commands.entity(e).insert(FakeMesh);
        // The branch. On a game with many sim entities this is what splits
        // one archetype into two; keep it even where it is degenerate.
        if i.is_multiple_of(2) {
            commands.entity(e).insert(FakeSparkle);
        }
        commands.spawn((FakeChildVisual, Transform::default(), ChildOf(e)));
    }
}

/// The selftest's own script, duplicated here rather than imported: the
/// template's is private, and a game's own is the one to copy in anyway.
fn script(tick: Res<SimTick>, mut virt: ResMut<VirtualInput>) {
    let t = tick.0;
    let held = if (t / 120).is_multiple_of(2) {
        Buttons::RIGHT
    } else {
        Buttons::LEFT
    };
    virt.set_held(held);
    if t.is_multiple_of(60) {
        virt.latched |= Buttons::A;
    }
}

/// Records the scripted run, optionally with the decorators running in
/// `Update`, at `ticks_per_frame` sim ticks per `app.update()`.
fn record(decorate: bool, seed: [u8; 32], ticks_per_frame: u32) -> (Replay, u64) {
    let mut app = build_headless_app();
    if ticks_per_frame > 1 {
        app.insert_resource(TimeUpdateStrategy::ManualDuration(
            sim::tick_duration() * ticks_per_frame,
        ));
    }
    app.add_systems(
        PreUpdate,
        script.before(gamebient_input::input::accumulate_input),
    );
    app.init_resource::<OrderTrace>();
    app.add_systems(
        FixedUpdate,
        trace_order.in_set(sim::SimSet).after(sim::advance_tick),
    );
    if decorate {
        app.add_systems(Update, decorate_players);
    }

    app.world_mut().resource_mut::<PendingSeed>().0 = Some(seed);
    app.update();
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Playing);
    app.update();
    // Bounded exactly like `record_scripted_run`: a run that ends itself
    // freezes SimSet, so SimTick would never reach SELFTEST_TICKS.
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

    let trace = app.world().resource::<OrderTrace>().0;
    let replay = app
        .world()
        .resource::<ReplayRecorder>()
        .last_run()
        .cloned()
        .expect("run sealed on OnExit(Playing)");
    (replay, trace)
}

#[test]
fn presentation_inserts_do_not_change_the_sim() {
    let mut bad = Vec::new();
    for tpf in [1u32, 2, 3] {
        for s in [0x5eu8, 0x11, 0x22, 0x33] {
            let seed = [s; 32];
            let (plain, trace_plain) = record(false, seed, tpf);
            let (decorated, trace_decorated) = record(true, seed, tpf);
            let verdict = verify(&decorated);
            let same = (plain.score, plain.checksum, plain.ticks)
                == (decorated.score, decorated.checksum, decorated.ticks);
            println!(
                "tpf={tpf} seed=0x{s:02x}  plain(score={},ck={},t={})  \
                 decorated(score={},ck={},t={})  same={same}  order_same={}  \
                 verify(decorated).matches={}",
                plain.score,
                plain.checksum,
                plain.ticks,
                decorated.score,
                decorated.checksum,
                decorated.ticks,
                trace_plain == trace_decorated,
                verdict.matches
            );
            if !same || !verdict.matches {
                bad.push(format!(
                    "tpf={tpf} seed=0x{s:02x}: same={same} matches={} \
                     order_same={} plain_ck={} decorated_ck={} verdict_ck={}",
                    verdict.matches,
                    trace_plain == trace_decorated,
                    plain.checksum,
                    decorated.checksum,
                    verdict.checksum
                ));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "decorating sim entities from Update changed what the sim did. This is \
         the archetype-order trap, not ulp drift — same build, same seed, same \
         arithmetic. See docs/replay-verification.md rule 1 and the port \
         checklist's \"The two order traps\".\n{bad:#?}"
    );
}
