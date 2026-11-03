//! b05: a destroyed enemy drops a coin a quarter of the time.
//!
//! Lenient by design: "destroyed" is tested as the enemy being removed from
//! the world. A change that only drops coins from its own new destroy path
//! (e.g. bullets) fails here and needs a person to judge.
use gamebient_game::game::scenario::Scenario;

#[test]
fn about_a_quarter_of_destroyed_enemies_drop_a_coin() {
    let mut s = Scenario::new();
    for i in 0..200 {
        let x = -460.0 + (i % 46) as f32 * 20.0;
        let y = 300.0 - (i / 46) as f32 * 40.0;
        s.place("Enemy", x, y);
    }
    s.start().tick(1);
    s.remove_all("Enemy").tick(3);
    let coins = s.count("Coin");
    assert!((25..=75).contains(&coins), "{coins} coins from 200 enemies (expected ~50)");
}
