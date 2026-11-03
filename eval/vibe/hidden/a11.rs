//! a11: two enemies that bump into each other both turn around.
use gamebient_game::game::scenario::Scenario;

#[test]
fn enemies_turn_around_when_they_bump() {
    let mut s = Scenario::new();
    s.start();
    s.place_moving("Enemy", -100.0, -282.0, 120.0, 0.0)
        .place_moving("Enemy", 100.0, -282.0, -120.0, 0.0)
        .tick(150);
    let e = s.positions("Enemy");
    assert_eq!(e.len(), 2);
    assert!(e[0].x < -60.0 && e[1].x > 60.0, "both should have turned back: {e:?}");
}
