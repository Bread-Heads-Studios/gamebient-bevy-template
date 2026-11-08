//! g03 (starts from the hurt rule: touching an enemy costs a life):
//! landing on an enemy squashes it without costing a life; walking into one
//! still costs one.
use gamebient_game::game::scenario::{Buttons, Scenario};

#[test]
fn landing_on_an_enemy_squashes_it_safely() {
    let mut s = Scenario::new();
    s.place("Enemy", -100.0, -282.0).start();
    let lives = s.lives();
    s.teleport("Player", -100.0, -100.0).tick(60);
    assert_eq!(s.count("Enemy"), 0, "the enemy is squashed");
    assert_eq!(s.lives(), lives, "squashing must not cost a life");
}

#[test]
fn walking_into_an_enemy_still_hurts() {
    let mut s = Scenario::new();
    s.place("Enemy", -200.0, -282.0).start();
    let lives = s.lives();
    s.hold(Buttons::RIGHT, 40).hold(Buttons::LEFT, 30);
    assert_eq!(s.lives(), lives - 1);
    assert_eq!(s.count("Enemy"), 1);
}
