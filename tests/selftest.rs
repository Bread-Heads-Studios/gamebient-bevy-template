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
    // Flip LEFT on in the first run: the cube stops moving, the folded
    // transform differs, and the claimed checksum no longer reproduces.
    let mut replay = record_scripted_run();
    replay.runs[0].held ^= 4;
    assert!(!verify(&replay).matches);
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
