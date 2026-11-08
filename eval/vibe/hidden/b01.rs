//! b01: every 3 seconds an enemy appears somewhere along the top.
use gamebient_game::game::scenario::Scenario;

#[test]
fn an_enemy_appears_every_three_seconds_along_the_top() {
    let mut s = Scenario::new();
    s.start();
    let n0 = s.count("Enemy");
    s.tick(170);
    assert!(s.count("Enemy") <= n0 + 1, "too many enemies before 3 s");
    s.tick(20);
    assert!(s.count("Enemy") > n0, "no enemy by 3 s");
    let top = s.positions("Enemy").iter().map(|p| p.y).fold(f32::MIN, f32::max);
    assert!(top > 150.0, "the new enemy should start near the top (highest y = {top})");
    s.tick(180);
    let n = s.count("Enemy");
    assert!((n0 + 2..=n0 + 3).contains(&n), "{n} enemies after ~6 s");
    let xs: Vec<f32> = s.positions("Enemy").iter().map(|p| p.x).collect();
    assert!(xs.iter().all(|x| x.abs() <= 480.0), "spawned on screen: {xs:?}");
    assert!(
        xs.windows(2).any(|w| (w[0] - w[1]).abs() > 1.0),
        "\"somewhere along the top\" should vary: {xs:?}"
    );
}
