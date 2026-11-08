//! d01: falling off the bottom costs one life and puts the player back at
//! the start (where the level placed them).
use gamebient_game::game::scenario::{Buttons, Scenario, START};

#[test]
fn falling_off_the_bottom_costs_a_life_and_respawns() {
    let mut s = Scenario::new();
    s.start().tick(5);
    let lives = s.lives();
    s.remove_all("Platform");
    let fell = s.hold_until(Buttons::NONE, 180, |s| s.lives() != lives);
    assert!(fell, "falling off the bottom didn't cost a life");
    // Put the floor back so the respawned player has something to stand on.
    s.place_sized("Platform", 0.0, -330.0, 960.0, 60.0).tick(60);
    assert_eq!(s.lives(), lives - 1);
    let p = s.pos("Player").expect("the player still exists");
    assert!(p.distance(START) < 40.0, "back at the start {START}, not {p}");
}
