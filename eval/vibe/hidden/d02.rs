//! d02: touching a coin removes it, gives 10 points and plays a sound.
use gamebient_game::game::scenario::{Buttons, Scenario};

#[test]
fn coin_disappears_scores_and_sounds() {
    let mut s = Scenario::new();
    s.place("Coin", -200.0, -290.0).start();
    let (score, sounds) = (s.score(), s.sounds().len());
    let got = s.hold_until(Buttons::RIGHT, 120, |s| s.count("Coin") == 0);
    assert!(got, "the coin didn't disappear");
    s.tick(10);
    assert_eq!(s.score(), score + 10);
    assert!(s.sounds().len() > sounds, "no sound played");
}
