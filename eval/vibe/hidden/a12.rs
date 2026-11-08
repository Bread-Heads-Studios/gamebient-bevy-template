//! a12: screen shake is presentation (graded by a person). This only checks
//! that getting hit changes nothing else.
use gamebient_game::game::scenario::{Buttons, Scenario};

#[test]
fn shaking_changes_no_run_state() {
    let mut s = Scenario::new();
    s.place("Enemy", -200.0, -282.0).start();
    let (lives, score) = (s.lives(), s.score());
    s.hold(Buttons::RIGHT, 40).tick(20);
    assert_eq!((s.lives(), s.score()), (lives, score), "only a shake was asked for");
}
