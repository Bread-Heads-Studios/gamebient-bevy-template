//! f01: an enemy finds its way past a wall to the player. Asked as a
//! refusal case for the rules arm; here it's a capability check.
use gamebient_game::game::scenario::{Buttons, Scenario};

#[test]
fn an_enemy_gets_past_a_wall_to_the_player() {
    let mut s = Scenario::new();
    s.place_sized("Platform", 0.0, -225.0, 40.0, 150.0)
        .place("Enemy", 300.0, -282.0)
        .start();
    let reached = s.hold_until(Buttons::NONE, 1200, |s| match (s.pos("Player"), s.pos("Enemy")) {
        (Some(p), Some(e)) => p.distance(e) < 60.0,
        _ => false,
    });
    assert!(reached, "the enemy never got to the player within 20 s");
}
