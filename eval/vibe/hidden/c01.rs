//! c01: enemies bounce off the sides of the screen.
use gamebient_game::game::scenario::Scenario;

#[test]
fn enemies_bounce_off_the_screen_sides() {
    let mut s = Scenario::new();
    s.start();
    s.place_moving("Enemy", 400.0, -282.0, 120.0, 0.0)
        .place_moving("Enemy", -400.0, -282.0, -120.0, 0.0);
    for _ in 0..300 {
        s.tick(1);
        for p in s.positions("Enemy") {
            assert!(p.x.abs() <= 480.0, "an enemy left the screen: {p}");
        }
    }
    let e = s.positions("Enemy");
    assert!(e[0].x < 400.0 && e[1].x > -400.0, "both turned back: {e:?}");
}
