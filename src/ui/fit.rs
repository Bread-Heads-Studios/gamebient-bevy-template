//! Text sizing against the game's UI width, so one kit fits 4:3, 1:1 and
//! 3:4: a title that is comfortable at 960 wide must still fit at 720.
//!
//! Fitting is per face. Each embedded face (`src/ui/fonts.rs`) declares a
//! measured `advance_em`, the average glyph advance in ems it promises never
//! to exceed on the strings the UI fits; `fonts::tests` parses the real TTFs
//! and holds every face to that promise. This file is copied verbatim into
//! games, so it names no font of its own.
//!
//! Average-advance fitting is right for the game's own fixed strings,
//! which `fonts::tests` measures. A string a pack supplies can be any
//! printable ASCII up to the key's `max_chars`, so pack-swappable lines
//! fit on each face's per-glyph `advances` table (and residual `kerns`)
//! instead: build a `look::LookFit` with [`Face::look_fit`] /
//! [`Face::look_fit_in`]. The game's own string keeps exactly its `fitted`
//! size there; any other string is sized from an estimate of its shaped
//! width from the tables, never larger than the default's size, then
//! clamped after layout so it fits (`look/refit.rs`). `widest_em` is the
//! table's widest glyph, for `max_chars` worst-case checks.

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
    /// The widest single glyph's advance, in ems, over printable ASCII
    /// (`' '..='~'`, every character the Look Pack validator accepts in a
    /// string): the largest of `advances`. Checked by `ui::font_metrics`; it
    /// is the worst case for a pack string of `max_chars`.
    pub widest_em: f32,
    /// Every printable ASCII character's advance, in ems, `' '..='~'` in
    /// order (index `c as usize - 0x20`): the widest its glyph takes when
    /// shaped as Bevy shapes it in any three-character context, kerning and
    /// contextual alternates included: a close estimate of the drawn line
    /// (`look::refit::clamp_fit` corrects the rare longer-range case after
    /// layout). Measured and checked by `ui::font_metrics`; `LookFit` sizes
    /// a pack's string by it.
    pub advances: &'static crate::look::refit::Advances,
    /// Residual widening pairs (a ligature wider than its parts), in ems,
    /// sorted by `(left, right)`; usually `&[]`. Measured and checked by
    /// `ui::font_metrics`.
    pub kerns: &'static [crate::look::refit::Kern],
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

    /// A `LookFit` for string `key` set in this face on one line of the
    /// game's UI width (the line `fitted` fits): the game's own string gets
    /// exactly `fitted(default, max_size)`'s size, any other the largest
    /// size, no larger than that, at which its estimated width (`advances`
    /// + `kerns`) fits the line, clamped after layout if Bevy finds it wider.
    pub fn look_fit(&self, key: &'static str, max_size: f32) -> crate::look::LookFit {
        crate::look::LookFit::new(
            key,
            max_size,
            self.advance_em,
            self.advances,
            self.kerns,
            ui_width() * MAX_LINE_SHARE,
        )
    }

    /// [`Face::look_fit`] for a line inside a box `box_px` UI pixels wide
    /// (a card's or a rail's interior: its width less padding and border).
    /// Only a pack's words are fitted to the box (the whole box is their
    /// line); the game's own string keeps exactly its `fitted(default,
    /// max_size)` size, so switching a line to this never changes the
    /// default look, even where that size overflows the box.
    pub fn look_fit_in(
        &self,
        key: &'static str,
        max_size: f32,
        box_px: f32,
    ) -> crate::look::LookFit {
        self.look_fit(key, max_size).in_box(box_px)
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
        widest_em: 0.6,
        advances: &[0.6; crate::look::refit::PRINTABLE],
        kerns: &[],
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
    fn look_fit_gives_the_default_its_fitted_size_bit_for_bit() {
        let wide = Face {
            advance_em: 0.63,
            widest_em: 1.02,
            advances: &[1.02; crate::look::refit::PRINTABLE],
            ..MONO
        };
        for text in ["A", "GAMEBIENT GAME", "GRAND THEFT AUTO-REPLY", "W"] {
            for max in [24.0, 72.0, 88.0] {
                assert_eq!(
                    wide.look_fit("k", max).default_size(text),
                    wide.fitted(text, max).font_size,
                    "{text} {max}"
                );
            }
        }
        // In a box the default still gets its `fitted` size, even when that
        // overflows the box; only other words fit it.
        let boxed = wide.look_fit_in("k", 46.0, 200.0);
        assert_eq!(boxed.line_px, 200.0);
        let own = "LAST SLICE";
        let size = boxed.size_for(own, own);
        assert_eq!(size, wide.fitted(own, 46.0).font_size);
        assert!(
            10.0 * 0.63 * size > 200.0,
            "the case where the box would bind"
        );
        let pack = "WWWWWWWWWW";
        assert!(10.0 * 1.02 * boxed.size_for(pack, own) <= 200.0);
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
