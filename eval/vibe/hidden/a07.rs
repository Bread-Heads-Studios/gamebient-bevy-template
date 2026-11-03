//! a07: the run ends when lives reach zero, not before.
use gamebient_game::game::scenario::Scenario;

#[test]
fn zero_lives_ends_the_run() {
    let mut s = Scenario::new();
    s.start().tick(5).set_lives(1).tick(10);
    assert!(!s.run_over(), "one life left is not game over");
    s.set_lives(0).tick(3);
    assert!(s.run_over(), "no lives left should end the run");
}
