//! Text sizing against the game's UI width, so one kit fits 4:3, 1:1 and
//! 3:4: a title that is comfortable at 960 wide must still fit at 720.

use crate::display::{GAME_HEIGHT, GAME_WIDTH, REFERENCE_SHORT_SIDE};

/// Advance of one glyph in ems. Bevy's default font is Fira Mono, 0.6 em;
/// change this if the game ships its own typeface.
pub const GLYPH_ADVANCE_EM: f32 = 0.6;

/// Share of the view's width a single line of text may take.
pub const MAX_LINE_SHARE: f32 = 0.9;

/// Width of the game view in UI pixels, the unit `Val::Px` and `font_size`
/// are authored in: 960 at 4:3, 720 at 1:1 and at 3:4.
pub fn ui_width() -> f32 {
    GAME_WIDTH as f32 * REFERENCE_SHORT_SIDE / GAME_WIDTH.min(GAME_HEIGHT) as f32
}

/// The largest whole font size, up to `max_size`, at which `text` fits on
/// one line of a view `view_w` UI pixels wide.
pub fn fit_font_size(text: &str, max_size: f32, view_w: f32) -> f32 {
    let glyphs = text.chars().count().max(1) as f32;
    let fit = view_w * MAX_LINE_SHARE / (glyphs * GLYPH_ADVANCE_EM);
    fit.min(max_size).floor().max(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_width_is_the_game_width_at_a_720_short_side() {
        let short = GAME_WIDTH.min(GAME_HEIGHT) as f32;
        assert_eq!(ui_width() * short, GAME_WIDTH as f32 * REFERENCE_SHORT_SIDE);
        assert!(ui_width() >= REFERENCE_SHORT_SIDE);
    }

    #[test]
    fn short_text_keeps_its_authored_size() {
        assert_eq!(fit_font_size("GAMEBIENT GAME", 72.0, 960.0), 72.0);
        assert_eq!(fit_font_size("GAMEBIENT GAME", 72.0, 720.0), 72.0);
        assert_eq!(fit_font_size("GAME OVER", 64.0, 720.0), 64.0);
    }

    #[test]
    fn long_text_shrinks_to_fit_one_line() {
        // 22 characters: 950 px at size 72, wider than a 720 view.
        assert_eq!(fit_font_size("GRAND THEFT AUTO-REPLY", 72.0, 960.0), 65.0);
        assert_eq!(fit_font_size("GRAND THEFT AUTO-REPLY", 72.0, 720.0), 49.0);
    }

    #[test]
    fn fitted_text_is_never_wider_than_its_share_of_the_view() {
        let long = "W".repeat(60);
        for text in [
            "A",
            "GAMEBIENT GAME",
            "GRAND THEFT AUTO-REPLY",
            long.as_str(),
        ] {
            for view_w in [720.0, 960.0] {
                let size = fit_font_size(text, 72.0, view_w);
                let width = text.chars().count() as f32 * GLYPH_ADVANCE_EM * size;
                assert!(
                    width <= view_w * MAX_LINE_SHARE,
                    "{text} at {view_w}: {width}"
                );
                assert!(size >= 1.0);
            }
        }
    }

    #[test]
    fn empty_text_does_not_divide_by_zero() {
        assert_eq!(fit_font_size("", 72.0, 960.0), 72.0);
    }
}
