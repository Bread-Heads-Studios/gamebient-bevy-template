//! g06 (starts from the hurt rule): enemies no longer cost a life.
use gamebient_game::game::scenario::{Buttons, Scenario};

#[test]
fn enemies_are_harmless() {
    let mut s = Scenario::new();
    s.place("Enemy", -200.0, -282.0).start();
    let lives = s.lives();
    s.hold(Buttons::RIGHT, 40).hold(Buttons::LEFT, 30);
    assert_eq!(s.lives(), lives);
}
