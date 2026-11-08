//! c04: bullets that leave one side come back on the other.
use gamebient_game::game::scenario::Scenario;

#[test]
fn bullets_wrap() {
    let mut s = Scenario::new();
    s.start();
    s.place_moving("Bullet", 470.0, 0.0, 600.0, 0.0).tick(10);
    assert_eq!(s.count("Bullet"), 1, "the bullet should still exist");
    let b = s.pos("Bullet").unwrap();
    assert!(b.x < 0.0 && b.x >= -480.0, "it should be back on the left: {b}");
}
