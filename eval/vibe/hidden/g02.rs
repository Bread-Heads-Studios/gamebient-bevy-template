//! g02: a door blocks the way until the player picks up the key.
use gamebient_game::game::scenario::{Buttons, Scenario};

#[test]
fn a_door_blocks_the_way() {
    let mut s = Scenario::new();
    s.place("Door", -150.0, -268.0).start();
    s.hold(Buttons::RIGHT, 60);
    assert!(s.pos("Player").unwrap().x < -150.0, "walked through a closed door");
    assert_eq!(s.count("Door"), 1);
}

#[test]
fn the_key_opens_the_door() {
    let mut s = Scenario::new();
    s.place("Key", -230.0, -290.0).place("Door", -150.0, -268.0).start();
    s.hold(Buttons::RIGHT, 90);
    assert_eq!(s.count("Key"), 0, "the key is picked up");
    assert_eq!(s.count("Door"), 0, "the door opens");
    assert!(s.pos("Player").unwrap().x > -100.0, "and the player can walk on");
}
