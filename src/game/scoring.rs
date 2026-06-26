use bevy::prelude::*;

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
}
