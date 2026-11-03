//! a08: enemies head for the player, including ones that appear later.
use gamebient_game::game::scenario::Scenario;

#[test]
fn enemies_walk_toward_the_player() {
    let mut s = Scenario::new();
    s.place("Enemy", 200.0, -282.0).place("Enemy", -460.0, -282.0).start().tick(60);
    let e = s.positions("Enemy");
    assert!(e[0].x < 170.0, "the enemy on the right walks left: {e:?}");
    assert!(e[1].x > -430.0, "the enemy on the left walks right: {e:?}");

    s.place("Enemy", 400.0, -282.0).tick(60);
    let late = *s.positions("Enemy").last().unwrap();
    assert!(late.x < 370.0, "an enemy that appears later walks too: {late}");
}
