//! a01 / a16: walking into a coin collects it (coin gone, coins +1).
use gamebient_game::game::scenario::{Buttons, Scenario};

#[test]
fn walking_into_a_coin_collects_only_that_coin() {
    let mut s = Scenario::new();
    s.place("Coin", -200.0, -290.0).place("Coin", 200.0, -290.0).start();
    let before = s.coins();
    let got = s.hold_until(Buttons::RIGHT, 120, |s| s.count("Coin") < 2);
    assert!(got, "walking into the coin didn't make it disappear");
    s.tick(30);
    assert_eq!(s.count("Coin"), 1, "only the touched coin goes");
    assert_eq!(s.coins(), before + 1, "the coins counter goes up by exactly 1");
}
