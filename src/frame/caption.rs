//! What the marquee says under the title art. Never a live score: the
//! caption is empty for the whole of a run. The website has a port in
//! `src/lib/cabinetFrame/caption.ts`; keep the strings identical.

use crate::frame::brightness::FramePhase;

/// `1234567` becomes `"1,234,567"`.
pub fn group_digits(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// The caption for this moment, or `None` for an empty marquee. `best` and
/// `final_score` arrive already formatted. A lower-is-better game shows no
/// numbers (its leaderboard value may be an encoding, not a readable time):
/// only `NEW BEST` after a record.
pub fn marquee_caption(
    phase: FramePhase,
    celebrating: bool,
    higher_is_better: bool,
    best: Option<&str>,
    final_score: Option<&str>,
) -> Option<String> {
    if !higher_is_better {
        return (phase == FramePhase::GameOver && celebrating).then(|| "NEW BEST".to_string());
    }
    match phase {
        FramePhase::Playing => None,
        FramePhase::Attract => best.map(|b| format!("SCORE TO BEAT {b}")),
        FramePhase::GameOver => final_score.map(|s| {
            if celebrating {
                format!("NEW HIGH SCORE {s}")
            } else {
                format!("FINAL SCORE {s}")
            }
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c1_attract_shows_the_score_to_beat() {
        assert_eq!(
            marquee_caption(FramePhase::Attract, false, true, Some("12,340"), None).as_deref(),
            Some("SCORE TO BEAT 12,340")
        );
    }

    #[test]
    fn c2_attract_with_no_best_shows_nothing() {
        assert_eq!(
            marquee_caption(FramePhase::Attract, false, true, None, None),
            None
        );
    }

    #[test]
    fn c3_play_never_shows_a_score() {
        assert_eq!(
            marquee_caption(
                FramePhase::Playing,
                false,
                true,
                Some("12,340"),
                Some("900")
            ),
            None
        );
        assert_eq!(
            marquee_caption(FramePhase::Playing, true, true, Some("12,340"), Some("900")),
            None
        );
    }

    #[test]
    fn c4_game_over_shows_the_final_score() {
        assert_eq!(
            marquee_caption(
                FramePhase::GameOver,
                false,
                true,
                Some("12,340"),
                Some("900")
            )
            .as_deref(),
            Some("FINAL SCORE 900")
        );
    }

    #[test]
    fn c5_celebration_names_the_new_high_score() {
        assert_eq!(
            marquee_caption(
                FramePhase::GameOver,
                true,
                true,
                Some("12,340"),
                Some("13,000")
            )
            .as_deref(),
            Some("NEW HIGH SCORE 13,000")
        );
    }

    #[test]
    fn game_over_without_a_score_shows_nothing() {
        assert_eq!(
            marquee_caption(FramePhase::GameOver, false, true, Some("12,340"), None),
            None
        );
    }

    #[test]
    fn c6_digits_are_grouped_in_threes() {
        assert_eq!(group_digits(0), "0");
        assert_eq!(group_digits(999), "999");
        assert_eq!(group_digits(1000), "1,000");
        assert_eq!(group_digits(1_234_567), "1,234,567");
    }

    #[test]
    fn lower_is_better_shows_no_numbers() {
        assert_eq!(
            marquee_caption(FramePhase::Attract, false, false, Some("12"), None),
            None
        );
        assert_eq!(
            marquee_caption(FramePhase::GameOver, false, false, Some("12"), Some("900")),
            None
        );
        assert_eq!(
            marquee_caption(FramePhase::Playing, true, false, None, Some("900")),
            None
        );
    }

    #[test]
    fn lower_is_better_celebration_says_new_best() {
        assert_eq!(
            marquee_caption(FramePhase::GameOver, true, false, Some("12"), Some("900")).as_deref(),
            Some("NEW BEST")
        );
    }
}
