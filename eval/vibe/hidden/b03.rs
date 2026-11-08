//! b03: the player walks at half speed (120 px/s instead of 240).
use gamebient_game::game::scenario::{Buttons, Scenario};

#[test]
fn half_speed() {
    let mut s = Scenario::new();
    s.start().tick(5);
    let x0 = s.pos("Player").unwrap().x;
    s.hold(Buttons::RIGHT, 60);
    let dx = s.pos("Player").unwrap().x - x0;
    assert!((dx - 120.0).abs() < 6.0, "walked {dx} px in 1 s");
}
