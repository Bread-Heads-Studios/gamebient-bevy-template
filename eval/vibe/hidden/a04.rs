//! a04 / a14: B fires a bullet the way the player faces; A doesn't.
use gamebient_game::game::scenario::{Buttons, Scenario};

fn newest_bullet(s: &mut Scenario) -> f32 {
    s.positions("Bullet").last().expect("a bullet").x
}

#[test]
fn b_fires_a_bullet_the_way_the_player_faces() {
    let mut s = Scenario::new();
    s.start().tick(2);
    let n = s.count("Bullet");
    s.press(Buttons::B).tick(2);
    assert_eq!(s.count("Bullet"), n + 1, "B should fire exactly one bullet");
    let b0 = newest_bullet(&mut s);
    s.tick(10);
    assert!(newest_bullet(&mut s) > b0 + 30.0, "facing right, the bullet flies right");

    s.hold(Buttons::LEFT, 5);
    s.press(Buttons::B).tick(2);
    let b0 = newest_bullet(&mut s);
    s.tick(10);
    assert!(newest_bullet(&mut s) < b0 - 30.0, "facing left, the bullet flies left");
}

#[test]
fn a_does_not_fire() {
    let mut s = Scenario::new();
    s.start().press(Buttons::A).tick(3);
    assert_eq!(s.count("Bullet"), 0);
}
