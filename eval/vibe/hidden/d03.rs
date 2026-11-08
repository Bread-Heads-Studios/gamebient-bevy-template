//! d03: B fires only while there is ammo, and each shot uses one.
use gamebient_game::game::scenario::{Buttons, Scenario};

#[test]
fn shooting_runs_out_of_ammo() {
    let mut s = Scenario::new();
    s.start().tick(2);
    let mut fired = Vec::new();
    for _ in 0..30 {
        s.press(Buttons::B);
        let px = s.pos("Player").unwrap().x;
        // A bullet fired on this press is still next to the player; older
        // ones are at least 5 ticks (50 px) away.
        fired.push(s.positions("Bullet").iter().any(|b| (b.x - px).abs() < 40.0));
        s.tick(5);
    }
    let n = fired.iter().filter(|f| **f).count();
    assert!(n >= 1, "B never fired");
    assert!(n < 30, "B never ran out ({n} shots)");
    assert!(fired[n..].iter().all(|f| !f), "once out of ammo it stays out: {fired:?}");
}
