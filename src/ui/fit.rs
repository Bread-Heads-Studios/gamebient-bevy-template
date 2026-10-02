//! Text sizing against the game's UI width, so one kit fits 4:3, 1:1 and
//! 3:4: a title that is comfortable at 960 wide must still fit at 720.
//!
//! Fitting is per face. Each embedded face (`src/ui/fonts.rs`) declares a
//! measured `advance_em`, the average glyph advance in ems it promises never
//! to exceed on the strings the UI fits; `fonts::tests` parses the real TTFs
//! and holds every face to that promise. This file is copied verbatim into
//! games, so it names no font of its own.

use bevy::prelude::*;

use crate::display::{GAME_HEIGHT, GAME_WIDTH, REFERENCE_SHORT_SIDE};

/// One embedded typeface: its bytes, the fixed handle it is installed at,
/// its real weight, and its fitting metric.
pub struct Face {
    /// Human-readable family and weight, for test messages.
    pub name: &'static str,
    /// The TTF, compiled in with `include_bytes!`.
    pub bytes: &'static [u8],
    /// Fixed `uuid_handle!` id the font is inserted at in `Assets<Font>`.
    pub handle: Handle<Font>,
    /// The face's real weight. Text asks for exactly this weight so the
    /// shaper never synthesises a different one.
    pub weight: FontWeight,
    /// Worst-case average advance per character, in ems, for uppercase plus
    /// digits text (the case every fitted line is written in). Measured from
    /// the TTF and checked by `fonts::tests`; never guess it.
    pub advance_em: f32,
}

impl Face {
    /// A `TextFont` in this face at `size`.
    pub fn font(&self, size: f32) -> TextFont {
        TextFont {
            font: self.handle.clone(),
            font_size: size,
            weight: self.weight,
            ..default()
        }
    }

    /// A `TextFont` in this face, sized with [`fit_font_size`] to fit one
    /// line of the game's UI width.
    pub fn fitted(&self, text: &str, max_size: f32) -> TextFont {
        self.font(fit_font_size(self, text, max_size, ui_width()))
    }
}

/// Share of the view's width a single line of text may take.
pub const MAX_LINE_SHARE: f32 = 0.9;

/// Width of the game view in UI pixels, the unit `Val::Px` and `font_size`
/// are authored in: 960 at 4:3, 720 at 1:1 and at 3:4.
pub fn ui_width() -> f32 {
    GAME_WIDTH as f32 * REFERENCE_SHORT_SIDE / GAME_WIDTH.min(GAME_HEIGHT) as f32
}

/// The largest whole font size, up to `max_size`, at which `text` set in
/// `face` fits on one line of a view `view_w` UI pixels wide.
pub fn fit_font_size(face: &Face, text: &str, max_size: f32, view_w: f32) -> f32 {
    fit_size_for_advance(face.advance_em, text, max_size, view_w)
}

/// [`fit_font_size`] on a bare advance, for a face that is not embedded
/// (Bevy's default Fira Mono is 0.6 em).
pub fn fit_size_for_advance(advance_em: f32, text: &str, max_size: f32, view_w: f32) -> f32 {
    let glyphs = text.chars().count().max(1) as f32;
    let fit = view_w * MAX_LINE_SHARE / (glyphs * advance_em);
    fit.min(max_size).floor().max(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stand-in face with Fira Mono's 0.6 em advance, so these tests pin
    /// the arithmetic and not any one game's fonts.
    const MONO: Face = Face {
        name: "test mono",
        bytes: &[],
        handle: bevy::asset::uuid_handle!("0f170000-0000-4000-8000-000000000001"),
        weight: FontWeight::NORMAL,
        advance_em: 0.6,
    };

    #[test]
    fn ui_width_is_the_game_width_at_a_720_short_side() {
        let short = GAME_WIDTH.min(GAME_HEIGHT) as f32;
        assert_eq!(ui_width() * short, GAME_WIDTH as f32 * REFERENCE_SHORT_SIDE);
        assert!(ui_width() >= REFERENCE_SHORT_SIDE);
    }

    #[test]
    fn short_text_keeps_its_authored_size() {
        assert_eq!(fit_font_size(&MONO, "GAMEBIENT GAME", 72.0, 960.0), 72.0);
        assert_eq!(fit_font_size(&MONO, "GAMEBIENT GAME", 72.0, 720.0), 72.0);
        assert_eq!(fit_font_size(&MONO, "GAME OVER", 64.0, 720.0), 64.0);
    }

    #[test]
    fn long_text_shrinks_to_fit_one_line() {
        // 22 characters: 950 px at size 72, wider than a 720 view.
        assert_eq!(
            fit_font_size(&MONO, "GRAND THEFT AUTO-REPLY", 72.0, 960.0),
            65.0
        );
        assert_eq!(
            fit_font_size(&MONO, "GRAND THEFT AUTO-REPLY", 72.0, 720.0),
            49.0
        );
    }

    #[test]
    fn a_wider_face_fits_smaller() {
        let wide = Face {
            advance_em: 0.72,
            ..MONO
        };
        let text = "GRAND THEFT AUTO-REPLY";
        assert!(fit_font_size(&wide, text, 72.0, 720.0) < fit_font_size(&MONO, text, 72.0, 720.0));
        assert_eq!(
            fit_font_size(&wide, text, 72.0, 720.0),
            fit_size_for_advance(0.72, text, 72.0, 720.0)
        );
    }

    #[test]
    fn fitted_text_is_never_wider_than_its_share_of_the_view() {
        let long = "W".repeat(60);
        for advance_em in [0.5, 0.6, 0.72] {
            let face = Face { advance_em, ..MONO };
            for text in [
                "A",
                "GAMEBIENT GAME",
                "GRAND THEFT AUTO-REPLY",
                long.as_str(),
            ] {
                for view_w in [720.0, 960.0] {
                    let size = fit_font_size(&face, text, 72.0, view_w);
                    let width = text.chars().count() as f32 * face.advance_em * size;
                    assert!(
                        width <= view_w * MAX_LINE_SHARE,
                        "{text} at {view_w}: {width}"
                    );
                    assert!(size >= 1.0);
                }
            }
        }
    }

    #[test]
    fn empty_text_does_not_divide_by_zero() {
        assert_eq!(fit_font_size(&MONO, "", 72.0, 960.0), 72.0);
    }

    #[test]
    fn font_carries_the_face_handle_and_weight() {
        let font = MONO.font(31.0);
        assert_eq!(font.font, MONO.handle);
        assert_eq!(font.font_size, 31.0);
        assert_eq!(font.weight, FontWeight::NORMAL);
    }
}
