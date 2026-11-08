//! f06: an enemy takes three hits to destroy. Lenient by design: a "hit" is
//! tested as a bullet. A change that counts stomps instead needs a person.
use gamebient_game::game::scenario::Scenario;

#[test]
fn an_enemy_takes_three_hits() {
    let mut s = Scenario::new();
    s.place("Enemy", 100.0, -282.0).start().tick(5);
    for hit in 1..=3 {
        s.place_moving("Bullet", 20.0, -282.0, 600.0, 0.0).tick(20);
        let left = s.count("Enemy");
        if hit < 3 {
            assert_eq!(left, 1, "destroyed after only {hit} hit(s)");
        } else {
            assert_eq!(left, 0, "still there after 3 hits");
        }
    }
}
