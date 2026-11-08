//! b04: the run ends (a win) after one minute, not before.
use gamebient_game::game::scenario::Scenario;

#[test]
fn the_run_ends_after_one_minute() {
    let mut s = Scenario::new();
    s.start();
    while s.now() < 3590 {
        s.tick(1);
    }
    assert!(!s.run_over(), "ended before a minute");
    s.tick(20);
    assert!(s.run_over(), "still going after a minute");
}
