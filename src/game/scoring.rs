use bevy::prelude::*;

use super::content::kit::Size;
use super::player::Player;

/// A collectible: the `Coin` kind's `on_spawn` inserts it.
#[derive(Component)]
pub struct Coin;

/// Points one coin is worth.
pub const COIN_POINTS: u32 = 1;

/// Whether two axis-aligned boxes (centre, size) overlap. Touching edges do
/// not count.
pub fn overlaps(a: Vec2, a_size: Vec2, b: Vec2, b_size: Vec2) -> bool {
    let d = (a - b).abs();
    let r = (a_size + b_size) * 0.5;
    d.x < r.x && d.y < r.y
}

/// In `SimSet` after `move_player`: a player overlapping a coin (boxes from
/// `Size`) scores [`COIN_POINTS`] and the coin is despawned. The score sum
/// does not depend on the order coins are visited in.
pub fn collect_coins(
    mut commands: Commands,
    player: Query<(&Transform, &Size), With<Player>>,
    coins: Query<(Entity, &Transform, &Size), With<Coin>>,
    mut score: MessageWriter<ScoreEvent>,
) {
    let Ok((ptf, psize)) = player.single() else {
        return;
    };
    let p = ptf.translation.truncate();
    for (e, tf, size) in &coins {
        if overlaps(p, psize.0, tf.translation.truncate(), size.0) {
            score.write(ScoreEvent {
                points: COIN_POINTS,
            });
            commands.entity(e).despawn();
        }
    }
}

/// Buffered message awarding score.
#[derive(Message)]
pub struct ScoreEvent {
    pub points: u32,
}

/// Core run data: score and lives.
#[derive(Resource)]
pub struct GameData {
    pub score: u32,
    pub high_score: u32,
    pub lives: u32,
}

impl Default for GameData {
    fn default() -> Self {
        Self {
            score: 0,
            high_score: 0,
            lives: 3,
        }
    }
}

impl GameData {
    /// Apply a score award: accumulate and track the high score. This is pure
    /// logic, unit-tested below — the pattern to follow for every testable game
    /// rule. Keep rules in methods/functions like this (not buried in systems)
    /// so they can be tested without spinning up a Bevy `App`.
    pub fn add_score(&mut self, points: u32) {
        self.score += points;
        if self.score > self.high_score {
            self.high_score = self.score;
        }
    }
}

/// Score order of this game, read by the cabinet frame (`src/frame/`). A game
/// whose score is better when lower (a time, a stroke count) sets this to
/// `false`, and must also set `score_order` in `assets/info.json` to match.
/// The frame then shows no score to beat, never announces a new high score
/// and never writes the best-score file. Lives here, not in `src/frame/`,
/// because frame files are copied into games unchanged and this file is the
/// game's own.
pub const HIGHER_SCORE_IS_BETTER: bool = true;

/// The single integer the leaderboard ranks, in the order
/// `HIGHER_SCORE_IS_BETTER` names (higher wins by default, lower for a time
/// or a stroke count). The template's `GameData` has `score`; games without
/// one (golf, racing) compute it here.
pub trait LeaderboardScore {
    fn leaderboard_score(&self) -> u32;
}

impl LeaderboardScore for GameData {
    fn leaderboard_score(&self) -> u32 {
        self.score
    }
}

/// Reads `ScoreEvent`s and updates `GameData`.
pub fn handle_score_events(mut reader: MessageReader<ScoreEvent>, mut game_data: ResMut<GameData>) {
    for event in reader.read() {
        game_data.add_score(event.points);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlap_is_strict_aabb() {
        let one = Vec2::ONE;
        assert!(overlaps(
            Vec2::ZERO,
            one,
            Vec2::new(0.7, 0.0),
            Vec2::splat(0.6)
        ));
        assert!(!overlaps(
            Vec2::ZERO,
            one,
            Vec2::new(0.9, 0.0),
            Vec2::splat(0.6)
        ));
        assert!(!overlaps(
            Vec2::ZERO,
            one,
            Vec2::new(0.0, 3.0),
            Vec2::splat(0.6)
        ));
    }

    #[test]
    fn add_score_accumulates() {
        let mut d = GameData::default();
        d.add_score(50);
        d.add_score(100);
        assert_eq!(d.score, 150);
    }

    #[test]
    fn high_score_tracks_peak() {
        let mut d = GameData::default();
        d.add_score(200);
        assert_eq!(d.high_score, 200);
        d.high_score = 500;
        d.add_score(10);
        assert_eq!(d.score, 210);
        assert_eq!(d.high_score, 500);
    }

    #[test]
    fn leaderboard_score_is_score_for_the_template() {
        let d = GameData {
            score: 42,
            ..GameData::default()
        };
        assert_eq!(d.leaderboard_score(), 42);
    }

    #[test]
    fn reset_preserves_high_score_clears_run_data() {
        // Simulate a completed run.
        let mut d = GameData::default();
        d.add_score(300);
        d.lives = 1;
        assert_eq!(d.high_score, 300);

        // Simulate reset_game_data logic.
        let high_score = d.high_score;
        d = GameData::default();
        d.high_score = high_score;

        // Score and lives reset; high score survives.
        assert_eq!(d.score, 0);
        assert_eq!(d.lives, 3);
        assert_eq!(d.high_score, 300);
    }
}
