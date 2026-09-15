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
