//! g01 (starts from the coin rule: coins +1): coins are now worth 5. One
//! coin rule, not two.
use gamebient_game::game::scenario::{Buttons, Scenario};

#[test]
fn a_coin_now_counts_five() {
    let mut s = Scenario::new();
    s.place("Coin", -200.0, -290.0).start();
    let before = s.coins();
    s.hold_until(Buttons::RIGHT, 120, |s| s.count("Coin") == 0);
    s.tick(10);
    assert_eq!(s.count("Coin"), 0);
    assert_eq!(s.coins(), before + 5);
}
