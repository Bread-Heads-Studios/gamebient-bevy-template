//! c05: jumps go higher than the default jump strength (600 px/s,
//! about 120 px). The fixture has no jump: the change has to add one on A,
//! or a person grades it.
use gamebient_game::game::scenario::{Buttons, Scenario};

#[test]
fn jumps_are_higher() {
    let mut s = Scenario::new();
    s.start().tick(5);
    let y0 = s.pos("Player").unwrap().y;
    s.press(Buttons::A);
    let mut apex = y0;
    for _ in 0..90 {
        s.tick(1);
        apex = apex.max(s.pos("Player").unwrap().y);
    }
    assert!(apex - y0 > 135.0, "rose {} px", apex - y0);
}
