//! c03: coins float where they're placed.
use gamebient_game::game::scenario::Scenario;

#[test]
fn coins_float() {
    let mut s = Scenario::new();
    s.place("Coin", 0.0, 0.0).start().tick(60);
    let c = s.pos("Coin").unwrap();
    assert!(c.y.abs() < 1.0, "the coin moved to {c}");
}
