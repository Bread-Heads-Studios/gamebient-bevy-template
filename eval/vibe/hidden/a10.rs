//! a10: bullets that leave the screen are removed; others stay.
use gamebient_game::game::scenario::Scenario;

#[test]
fn bullets_off_screen_disappear() {
    let mut s = Scenario::new();
    s.start();
    s.place_moving("Bullet", 440.0, 0.0, 600.0, 0.0)
        .place_moving("Bullet", -440.0, 50.0, -600.0, 0.0)
        .place_moving("Bullet", 0.0, 100.0, 0.0, 0.0)
        .tick(30);
    assert_eq!(s.count("Bullet"), 1, "only the bullet still on screen is left");
}
