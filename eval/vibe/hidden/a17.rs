//! a17: the player already walks left and right; nothing should change.
use gamebient_game::game::scenario::{Buttons, Scenario};

#[test]
fn the_player_walks_left_and_right() {
    let mut s = Scenario::new();
    s.start().tick(5);
    let x0 = s.pos("Player").unwrap().x;
    s.hold(Buttons::RIGHT, 30);
    let x1 = s.pos("Player").unwrap().x;
    assert!((x1 - x0 - 120.0).abs() < 5.0, "{x0} -> {x1}");
    s.hold(Buttons::LEFT, 30);
    assert!((s.pos("Player").unwrap().x - x0).abs() < 5.0);
}
