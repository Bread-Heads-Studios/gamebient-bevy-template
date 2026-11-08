//! d04: the run ends (a win) when the last coin is gone, not before.
use gamebient_game::game::scenario::Scenario;

#[test]
fn no_coins_left_ends_the_run() {
    let mut s = Scenario::new();
    s.place("Coin", 300.0, -290.0).place("Coin", 400.0, -290.0).start().tick(10);
    assert!(!s.run_over(), "ended while coins were left");
    s.remove_all("Coin").tick(3);
    assert!(s.run_over(), "still going with no coins left");
}
