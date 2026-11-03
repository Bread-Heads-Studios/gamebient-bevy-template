//! a03 / a15: A jumps; no jumping again in mid-air; B doesn't jump.
use gamebient_game::game::scenario::{Buttons, Scenario};

fn rise_after(s: &mut Scenario, presses: &[u32]) -> f32 {
    s.start().tick(5);
    let y0 = s.pos("Player").unwrap().y;
    let mut apex = y0;
    for t in 0..120 {
        if presses.contains(&t) {
            s.press(Buttons::A);
        } else {
            s.tick(1);
        }
        apex = apex.max(s.pos("Player").unwrap().y);
    }
    let y1 = s.pos("Player").unwrap().y;
    assert!((y1 - y0).abs() < 1.0, "the player should land again ({y0} -> {y1})");
    apex - y0
}

#[test]
fn a_jumps() {
    let rise = rise_after(&mut Scenario::new(), &[0]);
    assert!(rise > 40.0, "pressing A should jump (rose {rise} px)");
}

#[test]
fn no_second_jump_in_the_air() {
    let single = rise_after(&mut Scenario::new(), &[0]);
    let double = rise_after(&mut Scenario::new(), &[0, 12]);
    assert!(double <= single + 2.0, "a second press in mid-air jumped again ({single} vs {double})");
}

#[test]
fn b_does_not_jump() {
    let mut s = Scenario::new();
    s.start().tick(5);
    let y0 = s.pos("Player").unwrap().y;
    s.press(Buttons::B).tick(10);
    assert!((s.pos("Player").unwrap().y - y0).abs() < 1.0);
}
