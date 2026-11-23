//! The game's typefaces, compiled in and installed at fixed handles.
//!
//! The GX house rule is **two faces, one optional accent**: `DISPLAY` for
//! titles, headlines, card titles and big numbers; `BODY` for prompts, HUD,
//! labels and card bodies; a game may add one `ACCENT` for a single job (a
//! script subtitle, an epitaph). `STUDIO` and `STUDIO_ITALIC` are the house
//! faces of the Bread Heads studio logo and are the same in every game.
//!
//! Each face is `include_bytes!`-ed and inserted into `Assets<Font>` at a
//! fixed `uuid_handle!` id by [`install_fonts`], so text never waits on the
//! asset server and nothing pops in on web. Licences and sources are in
//! `src/ui/fonts/README.md`.
//!
//! **The default font slot (`AssetId::default()`) is never touched.** The
//! cabinet frame's marquee and caption (`src/frame/marquee.rs`) render in
//! it. UI text opts into a face explicitly with [`display`], [`body`] or
//! `Face::font`.
//!
//! # Text spawned outside `UiPlugin`
//!
//! The replay verifier and the `tests/` harnesses build `GamePlugin`
//! without `UiPlugin`, so the fonts are not installed there. Code under
//! `src/game/` that spawns text (a pause overlay, a popup, a banner) takes
//! `Option<Res<UiFonts>>`, never `Res<UiFonts>`, and builds its `TextFont`
//! with [`body_or_default`] / [`display_or_default`]:
//!
//! ```ignore
//! fn spawn_popup(mut commands: Commands, fonts: Option<Res<UiFonts>>) {
//!     commands.spawn((Text::new("+100"), fonts::body_or_default(fonts.as_deref(), 22.0)));
//! }
//! ```
//!
//! TEMPLATE NOTE: replace `DISPLAY` and `BODY` with the game's own faces
//! (and add `ACCENT` if it has one): new TTF + licence in `src/ui/fonts/`, a
//! fresh UUID, the real weight, a measured `advance_em`, `widest_em` and
//! `advances` table (`cargo test print_advance_tables -- --ignored
//! --nocapture` prints the table from the TTF). Then list every string the
//! UI fits in `tests::FITTED`. Leave `STUDIO` and `STUDIO_ITALIC`
//! exactly as they are: the studio logo is shared.

use bevy::asset::uuid_handle;
use bevy::prelude::*;

pub use crate::look::refit::Advances;
pub use crate::ui::fit::Face;

/// `DISPLAY`'s advance per printable ASCII glyph, ems, `' '..='~'`
/// (measured from the TTF; `fonts::tests` holds it).
// Measured advances, not maths constants (0.3010 is not log10 2).
#[allow(clippy::approx_constant)]
const DISPLAY_ADVANCES: Advances = [
    0.2540, 0.2980, 0.5140, 0.6360, 0.6060, 0.7580, 0.5910, 0.2940, 0.3980, 0.3900, 0.5400, 0.6200,
    0.2940, 0.4320, 0.2980, 0.3880, 0.6480, 0.4520, 0.5940, 0.6080, 0.6360, 0.6000, 0.6180, 0.5540,
    0.6000, 0.6180, 0.2980, 0.2980, 0.6200, 0.6200, 0.6200, 0.5780, 1.0140, 0.6340, 0.6640, 0.6440,
    0.6660, 0.5540, 0.5340, 0.6620, 0.6560, 0.2640, 0.6100, 0.6260, 0.5420, 0.8820, 0.6700, 0.6760,
    0.6040, 0.6760, 0.6320, 0.6060, 0.5880, 0.6720, 0.6180, 0.8980, 0.6440, 0.6240, 0.5760, 0.3580,
    0.3880, 0.3580, 0.6200, 0.6200, 0.2960, 0.5780, 0.6380, 0.5860, 0.6380, 0.5770, 0.4360, 0.6380,
    0.6160, 0.2660, 0.2680, 0.5640, 0.2660, 0.8540, 0.6160, 0.6120, 0.6380, 0.6380, 0.3960, 0.5240,
    0.4560, 0.6160, 0.5480, 0.7840, 0.5920, 0.6160, 0.5180, 0.4660, 0.2580, 0.4660, 0.6200,
];

/// Display face: Space Grotesk Bold.
pub const DISPLAY: Face = Face {
    name: "Space Grotesk Bold",
    bytes: include_bytes!("fonts/SpaceGrotesk-Bold.ttf"),
    handle: uuid_handle!("a29aa787-3f39-45a5-8656-8b7913d0f2ea"),
    weight: FontWeight::BOLD,
    // Measured: A-Z 0-9 average 0.621 em.
    advance_em: 0.63,
    // Measured: widest printable ASCII glyph '@' 1.014 em.
    widest_em: 1.015,
    advances: &DISPLAY_ADVANCES,
};

/// `BODY`'s advance per printable ASCII glyph, ems, `' '..='~'`
/// (measured from the TTF; `fonts::tests` holds it).
// Measured advances, not maths constants (0.3010 is not log10 2).
#[allow(clippy::approx_constant)]
const BODY_ADVANCES: Advances = [
    0.2520, 0.3213, 0.5230, 0.6436, 0.6504, 1.0044, 0.6626, 0.3257, 0.3731, 0.3731, 0.5396, 0.6729,
    0.3189, 0.4654, 0.3189, 0.3790, 0.6597, 0.4229, 0.6231, 0.6363, 0.6661, 0.6124, 0.6397, 0.5762,
    0.6402, 0.6397, 0.3189, 0.3292, 0.6729, 0.6729, 0.6729, 0.5435, 0.9991, 0.7276, 0.6592, 0.7369,
    0.7222, 0.6055, 0.5879, 0.7491, 0.7457, 0.2769, 0.5796, 0.7032, 0.5655, 0.9224, 0.7593, 0.7686,
    0.6451, 0.7730, 0.6524, 0.6504, 0.6602, 0.7359, 0.7276, 1.0201, 0.7198, 0.7134, 0.6524, 0.3731,
    0.3790, 0.3731, 0.4815, 0.4693, 0.3511, 0.5743, 0.6241, 0.5826, 0.6241, 0.5914, 0.3887, 0.6255,
    0.6124, 0.2618, 0.2618, 0.5694, 0.2618, 0.9004, 0.6119, 0.6089, 0.6241, 0.6241, 0.3970, 0.5494,
    0.3531, 0.6124, 0.5870, 0.8394, 0.5689, 0.5884, 0.5660, 0.4546, 0.3589, 0.4546, 0.6729,
];

/// Body face: Inter SemiBold (text optical size).
pub const BODY: Face = Face {
    name: "Inter SemiBold",
    bytes: include_bytes!("fonts/Inter-SemiBold.ttf"),
    handle: uuid_handle!("46cc46ce-59b9-45f6-ad60-e2d2f02426fa"),
    weight: FontWeight::SEMIBOLD,
    // Measured: A-Z 0-9 average 0.672 em.
    advance_em: 0.68,
    // Measured: widest printable ASCII glyph 'W' 1.020 em.
    widest_em: 1.021,
    advances: &BODY_ADVANCES,
};

/// `STUDIO`'s advance per printable ASCII glyph, ems, `' '..='~'`
/// (measured from the TTF; `fonts::tests` holds it).
// Measured advances, not maths constants (0.3010 is not log10 2).
#[allow(clippy::approx_constant)]
const STUDIO_ADVANCES: Advances = [
    0.1845, 0.3270, 0.4385, 0.6635, 0.6125, 0.8285, 0.9795, 0.2155, 0.3700, 0.3675, 0.5850, 0.5365,
    0.3105, 0.3525, 0.3025, 0.4495, 0.6975, 0.4735, 0.6395, 0.5925, 0.6620, 0.6045, 0.6460, 0.5575,
    0.6505, 0.6500, 0.3010, 0.3150, 0.4960, 0.5365, 0.4960, 0.5955, 0.9220, 0.7540, 0.7450, 0.6950,
    0.8030, 0.6695, 0.6275, 0.7560, 0.8590, 0.4330, 0.5530, 0.8220, 0.6405, 0.9515, 0.7530, 0.7795,
    0.7310, 0.7815, 0.7900, 0.6250, 0.7165, 0.7440, 0.7325, 1.0915, 0.7335, 0.6920, 0.6370, 0.3745,
    0.4495, 0.3800, 0.4470, 0.4875, 0.2715, 0.5555, 0.6015, 0.5115, 0.6075, 0.5215, 0.4095, 0.5740,
    0.6315, 0.3195, 0.3170, 0.6180, 0.3215, 0.9340, 0.6320, 0.5770, 0.6130, 0.5995, 0.4940, 0.4875,
    0.4040, 0.6205, 0.5410, 0.8360, 0.5650, 0.5625, 0.5040, 0.4140, 0.2935, 0.4140, 0.5955,
];

/// House studio face: Fraunces Black (72pt optical size, Soft 50). Shared
/// by every game; do not change.
pub const STUDIO: Face = Face {
    name: "Fraunces Black",
    bytes: include_bytes!("fonts/Fraunces-Black.ttf"),
    handle: uuid_handle!("adace312-4d1f-4a3e-9d03-269ad8d6db35"),
    weight: FontWeight::BLACK,
    // Measured: A-Z 0-9 average 0.702 em.
    advance_em: 0.71,
    // Measured: widest printable ASCII glyph 'W' 1.0915 em.
    widest_em: 1.092,
    advances: &STUDIO_ADVANCES,
};

/// `STUDIO_ITALIC`'s advance per printable ASCII glyph, ems, `' '..='~'`
/// (measured from the TTF; `fonts::tests` holds it).
// Measured advances, not maths constants (0.3010 is not log10 2).
#[allow(clippy::approx_constant)]
const STUDIO_ITALIC_ADVANCES: Advances = [
    0.2040, 0.3170, 0.3385, 0.6535, 0.5860, 0.7565, 0.6985, 0.1630, 0.3650, 0.3635, 0.6240, 0.5650,
    0.2560, 0.4085, 0.2455, 0.4520, 0.6470, 0.4355, 0.5915, 0.5345, 0.5680, 0.5550, 0.5930, 0.5295,
    0.5685, 0.5930, 0.2980, 0.3035, 0.5210, 0.5680, 0.5210, 0.4910, 0.8960, 0.6495, 0.6670, 0.6510,
    0.7545, 0.6290, 0.6020, 0.7080, 0.7920, 0.3785, 0.3670, 0.7160, 0.5615, 0.8530, 0.7190, 0.7725,
    0.6580, 0.7725, 0.6905, 0.5775, 0.5895, 0.7150, 0.6360, 0.9755, 0.6575, 0.5895, 0.6075, 0.3720,
    0.4305, 0.3720, 0.4835, 0.4785, 0.2250, 0.5520, 0.5525, 0.4570, 0.5665, 0.4885, 0.3250, 0.5240,
    0.5765, 0.3180, 0.3075, 0.5590, 0.3155, 0.8535, 0.5945, 0.5310, 0.5505, 0.5385, 0.4715, 0.4380,
    0.3775, 0.5915, 0.5720, 0.8370, 0.5935, 0.5160, 0.5010, 0.4040, 0.2935, 0.4035, 0.6405,
];

/// House studio italic: Fraunces Italic (9pt optical size, Soft 50).
/// Shared by every game; do not change.
pub const STUDIO_ITALIC: Face = Face {
    name: "Fraunces Italic",
    bytes: include_bytes!("fonts/Fraunces-Italic.ttf"),
    handle: uuid_handle!("46ce4214-b1f6-4136-8d3c-4f7459b4d742"),
    weight: FontWeight::NORMAL,
    // Measured: A-Z 0-9 average 0.636 em.
    advance_em: 0.64,
    // Measured: widest printable ASCII glyph 'W' 0.9755 em.
    widest_em: 0.976,
    advances: &STUDIO_ITALIC_ADVANCES,
};

/// Every embedded face, in install order.
pub const ALL: [&Face; 4] = [&DISPLAY, &BODY, &STUDIO, &STUDIO_ITALIC];

/// Present once [`install_fonts`] has run, which is only in builds with
/// `UiPlugin`. Code outside `src/ui/` reads it as `Option<Res<UiFonts>>`.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct UiFonts;

impl UiFonts {
    pub fn display(&self, size: f32) -> TextFont {
        display(size)
    }

    pub fn body(&self, size: f32) -> TextFont {
        body(size)
    }
}

/// The display face at `size`.
pub fn display(size: f32) -> TextFont {
    DISPLAY.font(size)
}

/// The body face at `size`.
pub fn body(size: f32) -> TextFont {
    BODY.font(size)
}

/// The display face if the fonts are installed, else the default font.
pub fn display_or_default(fonts: Option<&UiFonts>, size: f32) -> TextFont {
    fonts.map_or_else(|| TextFont::from_font_size(size), |f| f.display(size))
}

/// The body face if the fonts are installed, else the default font.
pub fn body_or_default(fonts: Option<&UiFonts>, size: f32) -> TextFont {
    fonts.map_or_else(|| TextFont::from_font_size(size), |f| f.body(size))
}

/// Inserts every embedded face into `Assets<Font>` at its fixed handle and
/// inserts [`UiFonts`].
///
/// `UiPlugin` runs this from `Plugin::finish`, not from `Startup`: Bevy runs
/// the initial state's `OnEnter` (the studio logo's text) in
/// `StateTransition`, which comes *before* `PreStartup`, so a `Startup`
/// system would install the fonts after the first `Text` was spawned.
pub fn install_fonts(mut commands: Commands, mut fonts: ResMut<Assets<Font>>) {
    for face in ALL {
        let font = Font::try_from_bytes(face.bytes.to_vec())
            .unwrap_or_else(|e| panic!("embedded font {} is invalid: {e:?}", face.name));
        fonts
            .insert(face.handle.id(), font)
            .expect("uuid font ids always insert");
    }
    commands.insert_resource(UiFonts);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::fit::MAX_LINE_SHARE;
    use crate::ui::{demo_end, how_to_play, menu, studio_logo, theme};

    /// Every string the template's UI renders at a fitted size, with the
    /// face it is set in. A game lists its own here.
    fn fitted() -> Vec<(&'static Face, String)> {
        let rows: Vec<(&'static Face, &str)> = vec![
            (&DISPLAY, menu::TITLE),
            (&BODY, menu::TAGLINE),
            (&BODY, menu::START_PROMPT),
            (&BODY, menu::CONTROLS),
            (&DISPLAY, menu::GAME_OVER_HEADLINE),
            (&BODY, menu::CONTINUE_PROMPT),
            (&DISPLAY, demo_end::DEMO_HEADLINE),
            (&BODY, demo_end::RESTART_PROMPT),
            (&BODY, demo_end::FULL_GAME_LINE),
            (&DISPLAY, theme::PAUSE_WORD),
            (&DISPLAY, how_to_play::HEADLINE),
            (&STUDIO, studio_logo::STUDIO_NAME),
            (&STUDIO_ITALIC, studio_logo::PRESENTS),
        ];
        let mut rows: Vec<(&'static Face, String)> =
            rows.into_iter().map(|(f, t)| (f, t.to_string())).collect();
        // The demo card's "OWN <TITLE> TO KEEP PLAYING" line. This row is the
        // template title; a game with a longer title must re-check it.
        rows.push((&DISPLAY, demo_end::own_line(menu::TITLE)));
        rows
    }

    /// Real advance of `text` in `face`, in ems: the sum of the glyphs'
    /// horizontal advances over units-per-em, read from the TTF.
    fn real_advance_em(face: &Face, text: &str) -> f32 {
        let parsed = ttf_parser::Face::parse(face.bytes, 0)
            .unwrap_or_else(|e| panic!("{} does not parse: {e}", face.name));
        let upem = f32::from(parsed.units_per_em());
        text.chars()
            .map(|c| {
                let glyph = parsed
                    .glyph_index(c)
                    .unwrap_or_else(|| panic!("{} has no glyph for {c:?}", face.name));
                f32::from(parsed.glyph_hor_advance(glyph).unwrap_or(0)) / upem
            })
            .sum()
    }

    #[test]
    fn every_face_parses_and_is_static() {
        for face in ALL {
            let parsed = ttf_parser::Face::parse(face.bytes, 0).unwrap();
            assert!(
                !parsed.is_variable(),
                "{} must be a static instance",
                face.name
            );
            assert_eq!(
                parsed.weight().to_number(),
                face.weight.0,
                "{}: declared weight differs from the TTF",
                face.name
            );
            assert!(Font::try_from_bytes(face.bytes.to_vec()).is_ok());
        }
    }

    #[test]
    fn declared_advance_covers_every_fitted_string() {
        for (face, text) in fitted() {
            let chars = text.chars().count() as f32;
            let real = real_advance_em(face, &text);
            assert!(
                real <= chars * face.advance_em,
                "{:?} in {}: real {real:.3} em > declared {:.3} em ({} x {})",
                text,
                face.name,
                chars * face.advance_em,
                chars,
                face.advance_em
            );
        }
    }

    #[test]
    fn declared_advance_covers_uppercase_and_digits() {
        // The metric's definition: at least the average advance of A-Z 0-9.
        let set = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
        for face in ALL {
            let avg = real_advance_em(face, set) / set.len() as f32;
            assert!(
                avg <= face.advance_em,
                "{}: A-Z 0-9 average {avg:.3} > {}",
                face.name,
                face.advance_em
            );
            // ...and not so loose that titles shrink for nothing.
            assert!(
                face.advance_em - avg < 0.03,
                "{}: advance_em {} is loose against {avg:.3}",
                face.name,
                face.advance_em
            );
        }
    }

    /// Every glyph the Look Pack validator accepts in a string.
    const PRINTABLE: std::ops::RangeInclusive<char> = ' '..='~';

    #[test]
    fn declared_widest_covers_every_printable_ascii_glyph() {
        for face in ALL {
            let (widest, c) = PRINTABLE
                .map(|c| (real_advance_em(face, &c.to_string()), c))
                .fold((0.0, ' '), |a, b| if b.0 > a.0 { b } else { a });
            assert!(
                widest <= face.widest_em,
                "{}: {c:?} is {widest:.4} em > widest_em {}",
                face.name,
                face.widest_em
            );
            assert!(
                face.widest_em - widest < 0.01,
                "{}: widest_em {} is loose against {c:?} {widest:.4}",
                face.name,
                face.widest_em
            );
            assert!(face.widest_em >= face.advance_em, "{}", face.name);
        }
    }

    /// Every face's per-glyph table is the TTF's advances, rounded up to
    /// four decimals (never under, so a sum is never short), and its
    /// `widest_em` covers the table's widest glyph.
    #[test]
    fn declared_advances_match_every_printable_ascii_glyph() {
        for face in ALL {
            for (i, c) in PRINTABLE.enumerate() {
                let real = real_advance_em(face, &c.to_string());
                let declared = face.advances[i];
                assert!(
                    declared >= real && declared - real < 2e-4,
                    "{}: {c:?} declared {declared} em, the TTF says {real:.5}",
                    face.name
                );
            }
            let widest = crate::look::refit::widest_em(face.advances);
            assert!(
                face.widest_em >= widest && face.widest_em - widest < 0.01,
                "{}: widest_em {} against the table's {widest}",
                face.name,
                face.widest_em
            );
        }
    }

    /// Prints each face's table in the form `fonts.rs` declares it. A game
    /// adding a face runs `cargo test print_advance_tables -- --ignored
    /// --nocapture` and pastes the rows (into a const carrying
    /// `#[allow(clippy::approx_constant)]`: a measured 0.3010 trips it).
    #[test]
    #[ignore = "a measuring tool, not a check"]
    fn print_advance_tables() {
        for face in ALL {
            println!("// {}", face.name);
            let rows: Vec<String> = PRINTABLE
                .map(|c| {
                    // Four decimals, never under the TTF's value as an f32.
                    let real = real_advance_em(face, &c.to_string());
                    let mut v = (f64::from(real) * 1e4).round() / 1e4;
                    if (v as f32) < real {
                        v += 1e-4;
                    }
                    format!("{v:.4}")
                })
                .collect();
            for row in rows.chunks(8) {
                println!("    {},", row.join(", "));
            }
        }
    }

    /// A pack's ordinary words come out close to the default's size: they
    /// are fitted by their real width, not as if every glyph were the
    /// widest. Attic Excavator's tagline box was the case that shrank a
    /// pack tagline to about 14 px against the default 21.
    #[test]
    fn a_realistic_pack_string_gets_close_to_the_default_size() {
        let pack = "CHILL IT. CHUCK IT.";
        for face in ALL {
            for (default, max) in [(menu::TAGLINE, 21.0), (menu::TITLE, 88.0)] {
                // A box the widest-glyph rule would have bound in.
                let box_px = pack.len() as f32 * face.widest_em * max * 0.7;
                let fit = face.look_fit_in("k", max, box_px);
                let own = fit.default_size(default);
                let size = fit.size_for(pack, default);
                let old = crate::look::refit::size_for_em(face.widest_em, pack, own, box_px);
                assert!(
                    size >= own - 1.0,
                    "{}: {pack:?} at {size} against the default's {own} (old rule {old})",
                    face.name
                );
                assert!(old < own - 1.0, "{}: the box does not bind", face.name);
                // ...and it really fits, in the TTF's advances.
                assert!(
                    real_advance_em(face, pack) * size <= box_px,
                    "{}",
                    face.name
                );
            }
            // The default string itself is untouched.
            let fit = face.look_fit("k", 21.0);
            assert_eq!(
                fit.size_for(menu::TAGLINE, menu::TAGLINE),
                face.fitted(menu::TAGLINE, 21.0).font_size
            );
        }
    }

    /// The worst string a pack may set for any key (its `max_chars` of the
    /// face's widest glyph) fits the line a `LookFit` gives it, in the real
    /// TTF's advances, on the view and in a narrow box; the default string
    /// keeps exactly its `fitted` size.
    #[test]
    fn a_worst_case_pack_string_fits_its_look_fit_line() {
        for face in ALL {
            let widest = PRINTABLE
                .max_by(|a, b| {
                    real_advance_em(face, &a.to_string())
                        .total_cmp(&real_advance_em(face, &b.to_string()))
                })
                .unwrap();
            for spec in theme::STRINGS {
                let worst = widest.to_string().repeat(spec.max_chars);
                for fit in [
                    face.look_fit(spec.key, 88.0),
                    face.look_fit_in(spec.key, 88.0, 300.0),
                ] {
                    let size = fit.size_for(&worst, spec.default);
                    let width = real_advance_em(face, &worst) * size;
                    assert!(
                        size == 1.0 || width <= fit.line_px,
                        "{} {}: {worst:?} at {size} is {width} > {}",
                        face.name,
                        spec.key,
                        fit.line_px
                    );
                }
                assert_eq!(
                    face.look_fit(spec.key, 88.0)
                        .size_for(spec.default, spec.default),
                    face.fitted(spec.default, 88.0).font_size,
                    "{} {}",
                    face.name,
                    spec.key
                );
            }
        }
    }

    #[test]
    fn fitted_strings_fit_at_every_ratio() {
        for (face, text) in fitted() {
            for view_w in [720.0, 960.0] {
                let size = crate::ui::fit::fit_font_size(face, &text, 200.0, view_w);
                let width = real_advance_em(face, &text) * size;
                assert!(width <= view_w * MAX_LINE_SHARE, "{text:?} at {view_w}");
            }
        }
    }

    #[test]
    fn handles_are_distinct_and_never_the_default_slot() {
        for (i, a) in ALL.iter().enumerate() {
            assert_ne!(a.handle.id(), AssetId::<Font>::default(), "{}", a.name);
            for b in &ALL[i + 1..] {
                assert_ne!(a.handle.id(), b.handle.id(), "{} / {}", a.name, b.name);
            }
        }
    }

    #[test]
    fn studio_faces_keep_their_shared_ids() {
        // Every game carries the same studio logo; these ids are part of it.
        let studio: Handle<Font> = uuid_handle!("adace312-4d1f-4a3e-9d03-269ad8d6db35");
        let italic: Handle<Font> = uuid_handle!("46ce4214-b1f6-4136-8d3c-4f7459b4d742");
        assert_eq!(STUDIO.handle, studio);
        assert_eq!(STUDIO_ITALIC.handle, italic);
    }

    #[test]
    fn fallbacks_use_the_default_font_without_ui_fonts() {
        assert_eq!(
            body_or_default(None, 20.0).font.id(),
            AssetId::<Font>::default()
        );
        assert_eq!(body_or_default(Some(&UiFonts), 20.0).font, BODY.handle);
        assert_eq!(
            display_or_default(Some(&UiFonts), 20.0).font,
            DISPLAY.handle
        );
    }
}
