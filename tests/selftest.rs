use gamebient_game::game::replay::selftest::{SELFTEST_TICKS, record_scripted_run};
use gamebient_game::game::replay::{Ended, Replay, verify};

#[test]
fn recorded_run_replays_to_the_same_verdict() {
    let replay = record_scripted_run();
    assert_eq!(replay.ticks, SELFTEST_TICKS);
    let v = verify(&replay);
    assert!(
        v.matches,
        "{v:?} vs claimed score {} checksum {}",
        replay.score, replay.checksum
    );
    assert_eq!(v.ended, Ended::InputExhausted);
    assert_eq!(v.ticks, SELFTEST_TICKS);
}

#[test]
fn tampered_inputs_do_not_verify() {
    // Deliberately NOT "flip one bit in runs[0]". That shape passes
    // vacuously in any game that opens on an intro card or a title plaque:
    // tick-1 input moves nothing, so the tampered replay reproduces the
    // claimed checksum exactly and the test reports "tampering detected"
    // when what it actually detected was nothing at all. (Grand Theft
    // Auto-Reply's 2.4 s shift card is the worked example.)
    //
    // Tamper over the BACK HALF of the runs instead, and in the shape a
    // cheat would take: strip every press edge (`latched`) and flip the
    // D-pad bits in `held`, i.e. claim a score off inputs that were never
    // played. Assert on the checksum as well as on `matches`, so the test
    // fails for the right reason — a changed verdict, not a decode error.
    let replay = record_scripted_run();

    // Guard against vacuity from the other end: if the untampered replay did
    // not verify, "the tampered one does not" would prove nothing.
    let honest = verify(&replay);
    assert!(
        honest.matches,
        "the untampered replay must verify before tampering proves anything: {honest:?}"
    );

    const DPAD: u16 = 0b1111; // Buttons::UP | DOWN | LEFT | RIGHT

    let mut tampered = replay.clone();
    let half = tampered.runs.len() / 2;
    assert!(
        tampered.runs.len() >= 2,
        "the selftest script produced {} input run(s); it needs to vary its \
         input enough to have a back half to tamper with",
        tampered.runs.len()
    );
    for run in &mut tampered.runs[half..] {
        run.latched = 0;
        run.held ^= DPAD;
    }
    assert_ne!(
        tampered.runs, replay.runs,
        "the tamper changed nothing; the script's back half presses no buttons"
    );

    let v = verify(&tampered);
    assert!(
        !v.matches,
        "a replay with its back half's inputs rewritten still verified: {v:?}"
    );
    assert_ne!(
        v.checksum, tampered.checksum,
        "the re-simulated checksum equals the claimed one, so `matches: false` \
         came from the score alone (or from nothing) — the tamper did not \
         change what the sim did: {v:?}"
    );
}

#[test]
fn committed_fixture_still_verifies() {
    let bytes = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/selftest.gxr"))
        .expect("run `cargo run --features verify --bin verify -- --selftest --write tests/fixtures/selftest.gxr`");
    let replay = Replay::decode(&bytes).unwrap();
    let v = verify(&replay);
    assert!(
        v.matches,
        "sim changed: regenerate the fixture with --selftest --write (see docs/replay-verification.md)\n{v:?}"
    );
}

#[test]
fn wasm_fixture_decodes_and_claims_the_selftest_tick_count_native_cannot_verify_it() {
    // `tests/fixtures/selftest-wasm.gxr` is recorded *by the wasm verifier
    // module* (`node tools/verify_fixture.mjs --record …` after
    // `tools/build_verify.sh`), which is what makes CI's fixture gate
    // wasm-vs-wasm. Native code deliberately does not re-simulate it: for a
    // game whose sim calls sin/cos/powf the native checksum may differ from
    // the wasm one by an ulp that snowballs, and asserting `matches` here
    // would reintroduce exactly the native-vs-wasm comparison the wasm
    // fixture exists to replace. What native CAN check is that the file is
    // present, decodes as GXR1, and claims the tick count the current
    // script produces -- which catches a stale or truncated fixture and a
    // SELFTEST_TICKS change nobody regenerated for.
    let bytes = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/selftest-wasm.gxr"
    ))
    .expect(
        "run `bash tools/build_verify.sh && node tools/verify_fixture.mjs \
         --record tests/fixtures/selftest-wasm.gxr`",
    );
    let replay = Replay::decode(&bytes).expect("selftest-wasm.gxr decodes as GXR1");
    assert_eq!(
        replay.ticks, SELFTEST_TICKS,
        "the wasm fixture is stale; regenerate it (see docs/replay-verification.md)"
    );
}
