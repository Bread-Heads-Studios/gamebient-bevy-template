//! a06: reaching the goal ends the run (a win).
use gamebient_game::game::scenario::{Buttons, Scenario};

#[test]
fn reaching_the_goal_ends_the_run() {
    let mut s = Scenario::new();
    s.place("Goal", -150.0, -268.0).start().tick(10);
    assert!(!s.run_over(), "the run must not end before the goal is reached");
    let won = s.hold_until(Buttons::RIGHT, 120, |s| s.run_over());
    assert!(won, "walking into the goal should end the run");
}
