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
//!   `Face::fitted` gave it, from the face's average advance against the
//!   view's line (`default_px`): `min(default_px / (chars * advance_em),
//!   max_size)`, floored, the same arithmetic in the same order. This holds
//!   for a boxed line too (`in_box`): the box never re-fits the default,
//!   so tagging a line can never change the default look.
//! - **Any other string** is sized by its real width: the sum of its
//!   glyphs' advances from the face's per-glyph table (`advances`, one
//!   value in ems for each printable ASCII character `' '..='~'`, measured
//!   from the TTF), the largest whole size at which that width fits
//!   `line_px`, and never larger than the default's size. A pack tagline
//!   of ordinary words so comes out close to the default's size, and any
//!   string a pack may set, up to the key's `max_chars`, still fits
//!   `line_px` (the worst case, `max_chars` of the widest glyph, included).
//!   A character outside the table (the validator rejects them) counts as
//!   the widest glyph.
//!
//! `line_px` is the width a pack's word may take, in UI pixels: the line
//! share of the view (`Face::look_fit` in `ui/fit.rs`) or a container's
//! interior (`Face::look_fit_in`), never the view when the line sits in a
//! narrower box. This file names no face; `ui/fit.rs` builds the tag from
//! a `Face`, whose table is a `&'static [f32; 95]` the tag points at.
//!
//! **Only for a line whose whole text is `look.string(key)`.** The size is
//! worked out from the key's string, not from the `Text`, so a composed
//! line (`"OWN " + title`, a formatted score) gets the bare string's size
//! and overflows; size such a line from a game system instead (pattern 1),
//! with [`text_em`] for its width. Nor on a line whose size a game system
//! animates (a score pop): every `LookChanged` would reset it.

use bevy::prelude::*;

use super::apply::LookChanged;
use super::runtime::ActiveLook;

/// How many printable ASCII characters there are, `' '..='~'`: the length
/// of a face's advance table. Index `c as usize - 0x20`.
pub const PRINTABLE: usize = 95;

/// A face's per-glyph advances, in ems, for `' '..='~'` in order.
pub type Advances = [f32; PRINTABLE];

/// The advance of `c` in `advances`, in ems. A character outside printable
/// ASCII counts as the table's widest glyph.
pub fn glyph_em(advances: &Advances, c: char) -> f32 {
    match c {
        ' '..='~' => advances[c as usize - 0x20],
        _ => widest_em(advances),
    }
}

/// The widest advance in `advances`, in ems: `max_chars` of this is the
/// worst string a pack may set.
pub fn widest_em(advances: &Advances) -> f32 {
    advances.iter().copied().fold(0.0, f32::max)
}

/// The real width of `text` in a face with these `advances`, in ems (no
/// kerning: kerning only tightens a line).
pub fn text_em(advances: &Advances, text: &str) -> f32 {
    text.chars().map(|c| glyph_em(advances, c)).sum()
}

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct LookFit {
    /// The string key (the same as the line's `LookText`).
    pub key: &'static str,
    /// The authored size cap (what spawn code handed `Face::fitted`).
    pub max_size: f32,
    /// The face's average advance, ems (`Face::advance_em`). Sizes the
    /// game's own string only.
    pub advance_em: f32,
    /// The face's per-glyph advances, ems (`Face::advances`). Sizes any
    /// other string.
    pub advances: &'static Advances,
    /// Width a pack's word may take, UI pixels.
    pub line_px: f32,
    /// Width the game's own string is fitted against, UI pixels: the line
    /// `Face::fitted` uses. Equal to `line_px` unless boxed (`in_box`).
    pub default_px: f32,
}

/// The largest whole size, up to `max_size`, at which `text` at `em` per
/// glyph fits `line_px`. At least 1. (The game's own string's rule.)
pub fn size_for_em(em: f32, text: &str, max_size: f32, line_px: f32) -> f32 {
    let glyphs = text.chars().count().max(1) as f32;
    (line_px / (glyphs * em)).min(max_size).floor().max(1.0)
}

/// The largest whole size, up to `max_size`, at which a line `width_em`
/// ems wide fits `line_px`. At least 1; an empty line gets `max_size`.
pub fn size_for_width(width_em: f32, max_size: f32, line_px: f32) -> f32 {
    if width_em <= 0.0 {
        return max_size.floor().max(1.0);
    }
    (line_px / width_em).min(max_size).floor().max(1.0)
}

impl LookFit {
    pub const fn new(
        key: &'static str,
        max_size: f32,
        advance_em: f32,
        advances: &'static Advances,
        line_px: f32,
    ) -> Self {
        Self {
            key,
            max_size,
            advance_em,
            advances,
            line_px,
            default_px: line_px,
        }
    }

    /// The same line drawn inside a box `box_px` wide: a pack's words fit
    /// the box; the game's own string keeps its size against `default_px`
    /// (even if that overflows the box: the default look never changes).
    pub const fn in_box(mut self, box_px: f32) -> Self {
        self.line_px = box_px;
        self
    }

    /// The face's widest glyph, ems (for `max_chars` worst-case checks).
    pub fn widest_em(&self) -> f32 {
        widest_em(self.advances)
    }

    /// The size of the game's own string `default`.
    pub fn default_size(&self, default: &str) -> f32 {
        size_for_em(self.advance_em, default, self.max_size, self.default_px)
    }

    /// The size of `text` on a line whose own string is `default`.
    pub fn size_for(&self, text: &str, default: &str) -> f32 {
        let own = self.default_size(default);
        if text == default {
            return own;
        }
        size_for_width(text_em(self.advances, text), own, self.line_px)
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

    /// A stand-in face: every glyph 0.6 em but 'W' (1.02) and 'I' (0.3).
    const fn stand_in() -> Advances {
        let mut a = [0.6; PRINTABLE];
        a['W' as usize - 0x20] = 1.02;
        a['I' as usize - 0x20] = 0.3;
        a
    }
    static ADVANCES: Advances = stand_in();
    const FIT: LookFit = LookFit::new("title", 88.0, 0.63, &ADVANCES, 864.0);

    #[test]
    fn the_table_reads_by_character() {
        assert_eq!(glyph_em(&ADVANCES, 'W'), 1.02);
        assert_eq!(glyph_em(&ADVANCES, 'I'), 0.3);
        assert_eq!(glyph_em(&ADVANCES, ' '), 0.6);
        assert_eq!(glyph_em(&ADVANCES, '~'), 0.6);
        // Outside the table: the widest glyph.
        assert_eq!(glyph_em(&ADVANCES, '\u{e9}'), 1.02);
        assert_eq!(widest_em(&ADVANCES), 1.02);
        assert_eq!(FIT.widest_em(), 1.02);
        assert!((text_em(&ADVANCES, "WIW") - 2.34).abs() < 1e-6);
    }

    #[test]
    fn the_default_keeps_the_average_advance_size() {
        // 864 / (14 * 0.63) = 97.9, capped at 88.
        assert_eq!(FIT.size_for("GAMEBIENT GAME", "GAMEBIENT GAME"), 88.0);
        let long = "A LONG DEFAULT TITLE LINE";
        assert_eq!(FIT.size_for(long, long), (864.0f32 / (25.0 * 0.63)).floor());
    }

    #[test]
    fn other_words_fit_by_their_real_width_and_never_grow() {
        for n in 1..=40 {
            let w = "W".repeat(n);
            let size = FIT.size_for(&w, "GAMEBIENT GAME");
            assert!(size <= 88.0);
            if size > 1.0 {
                assert!(n as f32 * 1.02 * size <= FIT.line_px, "{n}");
            }
        }
        // Narrow glyphs are not sized as wide ones: 40 I's (12 em) get
        // about 864 / 12 = 72, 40 W's about 21.
        let narrow = FIT.size_for(&"I".repeat(40), "GAMEBIENT GAME");
        assert!((71.0..=72.0).contains(&narrow), "{narrow}");
        assert!(FIT.size_for(&"W".repeat(40), "GAMEBIENT GAME") <= 21.0);
        // In a box: the default keeps its view size, other words fit the box.
        let boxed = FIT.in_box(200.0);
        assert_eq!(boxed.size_for("GAMEBIENT GAME", "GAMEBIENT GAME"), 88.0);
        let w = "W".repeat(14);
        assert!(14.0 * 1.02 * boxed.size_for(&w, "GAMEBIENT GAME") <= 200.0);
        // A short word does not outgrow the default's size.
        let shrunk = LookFit::new("k", 88.0, 0.63, &ADVANCES, 300.0);
        let own = shrunk.default_size("A FAIRLY LONG ONE");
        assert!(own < 88.0);
        assert_eq!(shrunk.size_for("HI", "A FAIRLY LONG ONE"), own);
    }

    #[test]
    fn empty_text_does_not_divide_by_zero() {
        assert_eq!(size_for_em(0.6, "", 72.0, 864.0), 72.0);
        assert_eq!(size_for_width(0.0, 72.0, 864.0), 72.0);
        assert_eq!(FIT.size_for("", "GAMEBIENT GAME"), 88.0);
    }
}
