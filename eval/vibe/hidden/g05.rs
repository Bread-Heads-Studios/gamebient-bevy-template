//! g05 (starts from the coin rule: +10 score): coins give an extra life and
//! no points, and still disappear.
use gamebient_game::game::scenario::{Buttons, Scenario};

#[test]
fn coins_give_a_life_not_points() {
    let mut s = Scenario::new();
    s.place("Coin", -200.0, -290.0).start();
    let (score, lives) = (s.score(), s.lives());
    s.hold_until(Buttons::RIGHT, 120, |s| s.count("Coin") == 0);
    s.tick(10);
    assert_eq!(s.count("Coin"), 0);
    assert_eq!(s.score(), score);
    assert_eq!(s.lives(), lives + 1);
}
