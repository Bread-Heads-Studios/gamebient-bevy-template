//! a09: a coin is worth 10 points, once.
use gamebient_game::game::scenario::{Buttons, Scenario};

#[test]
fn a_coin_is_worth_ten_points_once() {
    let mut s = Scenario::new();
    s.place("Coin", -200.0, -290.0).start();
    let before = s.score();
    let got = s.hold_until(Buttons::RIGHT, 120, |s| s.score() != before);
    assert!(got, "touching the coin gave no points");
    s.hold(Buttons::RIGHT, 40).tick(20);
    assert_eq!(s.score(), before + 10, "exactly 10 points for one coin");
}
