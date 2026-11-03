//! a02: touching an enemy costs one life (not one per tick of contact).
use gamebient_game::game::scenario::{Buttons, Scenario};

#[test]
fn touching_an_enemy_costs_one_life() {
    let mut s = Scenario::new();
    s.place("Enemy", -200.0, -282.0).start();
    let before = s.lives();
    let hit = s.hold_until(Buttons::RIGHT, 120, |s| s.lives() != before);
    assert!(hit, "walking into the enemy didn't cost a life");
    // Step back out. Losing more lives while walking out means the rule
    // fires on every tick of contact, not once per touch.
    s.hold(Buttons::LEFT, 30);
    assert_eq!(s.lives(), before - 1);
}
