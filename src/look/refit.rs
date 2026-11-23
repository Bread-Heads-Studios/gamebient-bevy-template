//! [`LookFit`]: a themed line's font size follows the active pack's string.
//!
//! `LookText` swaps a line's words but never its size, so a line fitted at
//! spawn keeps the old size when a look arrives later (a hosted `look.set`,
//! a reload, a clear) and a longer pack word overruns its box. Put
//! `LookFit` next to `LookText(key)` and a `TextFont` in the right face;
//! `refit_text` (in `LookSet`, before text layout) sets `font_size` on the
//! frame the tag spawns and on every `LookChanged`, a clear included.
//!
//! The rule:
//!
//! - **The game's own string keeps exactly its authored size**: the size
//!   `ui::fit` gives it from the face's average advance, `advance_em`
//!   (`min(line_px / (chars * advance_em), max_size)`, floored), the same
//!   arithmetic as `Face::fitted`, so the default look is pixel-identical.
//! - **Any other string** is sized as if every glyph were the face's
//!   widest (`widest_em`), and never larger than the default's size. So any
//!   string a pack may set, up to the key's `max_chars`, fits `line_px`.
//!
//! `line_px` is the width the line may take, in UI pixels: the line share
//! of the view (`Face::look_fit` in `ui/fit.rs`) or a container's interior
//! (`Face::look_fit_in`), never the view when the line sits in a narrower
//! box. This file names no face; `ui/fit.rs` builds the tag from one.

use bevy::prelude::*;

use super::apply::LookChanged;
use super::runtime::ActiveLook;

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct LookFit {
    /// The string key (the same as the line's `LookText`).
    pub key: &'static str,
    /// The authored size cap (what spawn code handed `Face::fitted`).
    pub max_size: f32,
    /// The face's average advance, ems (`Face::advance_em`).
    pub advance_em: f32,
    /// The face's widest glyph, ems (`Face::widest_em`).
    pub widest_em: f32,
    /// Width the line may take, UI pixels.
    pub line_px: f32,
}

/// The largest whole size, up to `max_size`, at which `text` at `em` per
/// glyph fits `line_px`. At least 1.
pub fn size_for_em(em: f32, text: &str, max_size: f32, line_px: f32) -> f32 {
    let glyphs = text.chars().count().max(1) as f32;
    (line_px / (glyphs * em)).min(max_size).floor().max(1.0)
}

impl LookFit {
    pub const fn new(
        key: &'static str,
        max_size: f32,
        advance_em: f32,
        widest_em: f32,
        line_px: f32,
    ) -> Self {
        Self {
            key,
            max_size,
            advance_em,
            widest_em,
            line_px,
        }
    }

    /// The size of the game's own string `default`.
    pub fn default_size(&self, default: &str) -> f32 {
        size_for_em(self.advance_em, default, self.max_size, self.line_px)
    }

    /// The size of `text` on a line whose own string is `default`.
    pub fn size_for(&self, text: &str, default: &str) -> f32 {
        let own = self.default_size(default);
        if text == default {
            return own;
        }
        size_for_em(self.widest_em.max(self.advance_em), text, own, self.line_px)
    }

    /// The size under `look`. A key the contract lacks counts as its own
    /// default (`ActiveLook::string` returns the key itself).
    pub fn resolve(&self, look: &ActiveLook) -> f32 {
        let default = look.contract.default_string(self.key).unwrap_or(self.key);
        self.size_for(look.string(self.key), default)
    }
}

pub fn refit_text(
    look: Res<ActiveLook>,
    mut changed: MessageReader<LookChanged>,
    mut q: Query<(Ref<LookFit>, &mut TextFont)>,
) {
    let all = changed.read().count() > 0;
    for (fit, mut font) in &mut q {
        if all || fit.is_added() {
            let size = fit.resolve(&look);
            if font.font_size != size {
                font.font_size = size;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIT: LookFit = LookFit::new("title", 88.0, 0.63, 1.02, 864.0);

    #[test]
    fn the_default_keeps_the_average_advance_size() {
        // 864 / (14 * 0.63) = 97.9, capped at 88.
        assert_eq!(FIT.size_for("GAMEBIENT GAME", "GAMEBIENT GAME"), 88.0);
        let long = "A LONG DEFAULT TITLE LINE";
        assert_eq!(FIT.size_for(long, long), (864.0f32 / (25.0 * 0.63)).floor());
    }

    #[test]
    fn other_words_fit_at_the_widest_glyph_and_never_grow() {
        for n in 1..=40 {
            let w = "W".repeat(n);
            let size = FIT.size_for(&w, "GAMEBIENT GAME");
            assert!(size <= 88.0);
            if size > 1.0 {
                assert!(n as f32 * FIT.widest_em * size <= FIT.line_px, "{n}");
            }
        }
        // A short word does not outgrow the default's size.
        let shrunk = LookFit::new("k", 88.0, 0.63, 1.02, 300.0);
        let own = shrunk.default_size("A FAIRLY LONG ONE");
        assert!(own < 88.0);
        assert_eq!(shrunk.size_for("HI", "A FAIRLY LONG ONE"), own);
    }

    #[test]
    fn empty_text_does_not_divide_by_zero() {
        assert_eq!(size_for_em(0.6, "", 72.0, 864.0), 72.0);
    }
}
