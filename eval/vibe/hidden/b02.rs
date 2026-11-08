//! b02: the player starts with 5 lives.
use gamebient_game::game::scenario::Scenario;

#[test]
fn five_lives() {
    let mut s = Scenario::new();
    s.start();
    assert_eq!(s.lives(), 5);
}
