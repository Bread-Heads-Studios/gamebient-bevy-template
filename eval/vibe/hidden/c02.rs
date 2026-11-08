//! c02: the player can jump again once in mid-air, but not twice.
//! (The fixture has no jump: the change has to add one, on A.)
use gamebient_game::game::scenario::{Buttons, Scenario};

fn rise(presses: &[u32]) -> f32 {
    let mut s = Scenario::new();
    s.start().tick(5);
    let y0 = s.pos("Player").unwrap().y;
    let mut apex = y0;
    for t in 0..150 {
        if presses.contains(&t) {
            s.press(Buttons::A);
        } else {
            s.tick(1);
        }
        apex = apex.max(s.pos("Player").unwrap().y);
    }
    apex - y0
}

#[test]
fn double_jump_but_not_triple() {
    let single = rise(&[0]);
    let double = rise(&[0, 12]);
    let triple = rise(&[0, 12, 24]);
    assert!(single > 40.0, "A should jump (rose {single})");
    assert!(double > single + 20.0, "a second press in the air should go higher ({single} vs {double})");
    assert!(triple <= double + 2.0, "a third press should do nothing ({double} vs {triple})");
}
