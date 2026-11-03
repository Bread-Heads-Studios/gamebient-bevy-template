//! a13: landing on an enemy squashes it; walking into one doesn't.
use gamebient_game::game::scenario::{Buttons, Scenario};

#[test]
fn landing_on_an_enemy_squashes_it() {
    let mut s = Scenario::new();
    s.place("Enemy", -100.0, -282.0).start();
    s.teleport("Player", -100.0, -100.0).tick(60);
    assert_eq!(s.count("Enemy"), 0);
}

#[test]
fn walking_into_an_enemy_does_not_squash_it() {
    let mut s = Scenario::new();
    s.place("Enemy", -200.0, -282.0).start();
    s.hold(Buttons::RIGHT, 40);
    assert_eq!(s.count("Enemy"), 1);
}
