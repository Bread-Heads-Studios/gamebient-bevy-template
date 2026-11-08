//! a05: a bullet that hits an enemy removes both, and awards nothing.
use gamebient_game::game::scenario::Scenario;

#[test]
fn bullets_destroy_enemies() {
    let mut s = Scenario::new();
    s.place("Enemy", 100.0, -282.0).start().tick(5);
    let score = s.score();
    s.place_moving("Bullet", 20.0, -282.0, 600.0, 0.0).tick(20);
    assert_eq!(s.count("Enemy"), 0, "the enemy is destroyed");
    assert_eq!(s.count("Bullet"), 0, "the bullet is used up");
    assert_eq!(s.score(), score, "no points were asked for");
}
