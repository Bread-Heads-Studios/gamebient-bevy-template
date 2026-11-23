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
//! fresh UUID, the real weight, a measured `advance_em`, and the shaped
//! `widest_em`, `advances` table and `kerns` list that
//! `cargo test print_glyph_tables -- --ignored --nocapture` prints
//! (`ui::font_metrics` then holds them to the font). Then list every
//! string the UI fits in `tests::fitted()`. Leave `STUDIO` and
//! `STUDIO_ITALIC` exactly as they are: the studio logo is shared.

use bevy::asset::uuid_handle;
use bevy::prelude::*;

pub use crate::look::refit::{Advances, Kern};
pub use crate::ui::fit::Face;

/// `DISPLAY`'s advance per printable ASCII glyph, ems, `' '..='~'`
/// (shaped, kerning included; `ui::font_metrics` holds it).
// Measured advances, not maths constants (0.3010 is not log10 2).
#[allow(clippy::approx_constant)]
const DISPLAY_ADVANCES: Advances = [
    0.2540, 0.2980, 0.5240, 0.6360, 0.6060, 0.7580, 0.5910, 0.3040, 0.4420, 0.3900, 0.5400, 0.6200,
    0.3140, 0.4320, 0.2980, 0.3880, 0.6480, 0.4520, 0.5940, 0.6080, 0.6360, 0.6000, 0.6180, 0.5540,
    0.6000, 0.6180, 0.2980, 0.2980, 0.6200, 0.6200, 0.6200, 0.5780, 1.0140, 0.6340, 0.6640, 0.6440,
    0.6660, 0.5540, 0.5340, 0.6620, 0.6560, 0.2640, 0.6240, 0.6260, 0.5420, 0.8820, 0.6700, 0.6760,
    0.6040, 0.6760, 0.6320, 0.6060, 0.6080, 0.6720, 0.6380, 0.8980, 0.6440, 0.6240, 0.5760, 0.3980,
    0.4620, 0.3580, 0.6200, 0.6200, 0.2960, 0.5780, 0.6380, 0.5860, 0.6380, 0.5770, 0.4560, 0.6380,
    0.6160, 0.2960, 0.2980, 0.5640, 0.2660, 0.8540, 0.6160, 0.6120, 0.6380, 0.6380, 0.3960, 0.5240,
    0.4660, 0.6160, 0.5480, 0.7840, 0.5920, 0.6160, 0.5180, 0.5060, 0.2580, 0.4660, 0.6200,
];

/// Display face: Space Grotesk Bold.
pub const DISPLAY: Face = Face {
    name: "Space Grotesk Bold",
    bytes: include_bytes!("fonts/SpaceGrotesk-Bold.ttf"),
    handle: uuid_handle!("a29aa787-3f39-45a5-8656-8b7913d0f2ea"),
    weight: FontWeight::BOLD,
    // Measured: A-Z 0-9 average 0.621 em.
    advance_em: 0.63,
    // Measured: widest printable ASCII glyph '@' 1.014 em (kerning included).
    widest_em: 1.014,
    advances: &DISPLAY_ADVANCES,
    // No residual widening pairs (ui::font_metrics checks).
    kerns: &[],
};

/// `BODY`'s advance per printable ASCII glyph, ems, `' '..='~'`
/// (shaped, kerning included; `ui::font_metrics` holds it).
// Measured advances, not maths constants (0.3010 is not log10 2).
#[allow(clippy::approx_constant)]
const BODY_ADVANCES: Advances = [
    0.2520, 0.3213, 0.5230, 0.6436, 0.6504, 1.0044, 0.6626, 0.3257, 0.3912, 0.3731, 0.6641, 0.6729,
    0.3189, 0.4654, 0.3189, 0.3790, 0.6763, 0.4375, 0.6231, 0.6363, 0.6661, 0.6124, 0.6397, 0.5958,
    0.6402, 0.6563, 0.3189, 0.3292, 0.6729, 0.6729, 0.6729, 0.5435, 0.9991, 0.7422, 0.6592, 0.7408,
    0.7222, 0.6055, 0.5879, 0.7491, 0.7603, 0.2916, 0.5796, 0.7129, 0.5655, 0.9371, 0.7740, 0.7686,
    0.6451, 0.7730, 0.6524, 0.6504, 0.6729, 0.7359, 0.7422, 1.0425, 0.7198, 0.7261, 0.6524, 0.3912,
    0.3936, 0.3731, 0.4815, 0.5655, 0.3511, 0.5743, 0.6338, 0.5923, 0.6387, 0.6011, 0.4068, 0.6314,
    0.6124, 0.2618, 0.2618, 0.5743, 0.2764, 0.9004, 0.6119, 0.6187, 0.6338, 0.6299, 0.4151, 0.5494,
    0.3746, 0.6124, 0.6036, 0.8394, 0.6729, 0.6050, 0.5660, 0.4727, 0.3711, 0.4546, 0.6729,
];

/// Body face: Inter SemiBold (text optical size).
pub const BODY: Face = Face {
    name: "Inter SemiBold",
    bytes: include_bytes!("fonts/Inter-SemiBold.ttf"),
    handle: uuid_handle!("46cc46ce-59b9-45f6-ad60-e2d2f02426fa"),
    weight: FontWeight::SEMIBOLD,
    // Measured: A-Z 0-9 average 0.672 em.
    advance_em: 0.68,
    // Measured: widest printable ASCII glyph 'W' 1.042 em (kerning included).
    widest_em: 1.043,
    advances: &BODY_ADVANCES,
    // No residual widening pairs (ui::font_metrics checks).
    kerns: &[],
};

/// `STUDIO`'s advance per printable ASCII glyph, ems, `' '..='~'`
/// (shaped, kerning included; `ui::font_metrics` holds it).
// Measured advances, not maths constants (0.3010 is not log10 2).
#[allow(clippy::approx_constant)]
const STUDIO_ADVANCES: Advances = [
    0.1845, 0.3270, 0.4385, 0.6635, 0.6125, 0.8285, 0.9795, 0.2155, 0.3700, 0.3675, 0.5850, 0.5365,
    0.3105, 0.3525, 0.3025, 0.4495, 0.6975, 0.4735, 0.6395, 0.5925, 0.6620, 0.6045, 0.6460, 0.5575,
    0.6505, 0.6500, 0.3010, 0.3150, 0.4960, 0.5365, 0.4960, 0.5955, 0.9220, 0.7540, 0.7450, 0.6950,
    0.8030, 0.6695, 0.6275, 0.7560, 0.8590, 0.4330, 0.5530, 0.8220, 0.6405, 0.9515, 0.7530, 0.7795,
    0.7310, 0.7815, 0.7970, 0.6250, 0.7165, 0.7440, 0.7325, 1.0915, 0.7335, 0.6975, 0.6370, 0.3745,
    0.4495, 0.3800, 0.4470, 0.4875, 0.2715, 0.5555, 0.6015, 0.5115, 0.6075, 0.5215, 0.4155, 0.5920,
    0.6315, 0.3195, 0.3170, 0.6235, 0.3215, 0.9340, 0.6320, 0.5770, 0.6130, 0.5995, 0.4940, 0.4875,
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
    // Measured: widest printable ASCII glyph 'W' 1.091 em (kerning included).
    widest_em: 1.092,
    advances: &STUDIO_ADVANCES,
    // No residual widening pairs (ui::font_metrics checks).
    kerns: &[],
};

/// `STUDIO_ITALIC`'s advance per printable ASCII glyph, ems, `' '..='~'`
/// (shaped, kerning included; `ui::font_metrics` holds it).
// Measured advances, not maths constants (0.3010 is not log10 2).
#[allow(clippy::approx_constant)]
const STUDIO_ITALIC_ADVANCES: Advances = [
    0.2040, 0.3170, 0.3485, 0.6535, 0.5860, 0.7565, 0.6980, 0.1730, 0.3850, 0.3635, 0.6240, 0.5650,
    0.2600, 0.4085, 0.2495, 0.4520, 0.6510, 0.4435, 0.6015, 0.5530, 0.5980, 0.5610, 0.5990, 0.5415,
    0.5745, 0.6010, 0.3020, 0.3075, 0.5210, 0.5680, 0.5210, 0.4910, 0.8960, 0.6595, 0.6690, 0.6590,
    0.7565, 0.6340, 0.6120, 0.7120, 0.8030, 0.3895, 0.3700, 0.7220, 0.5735, 0.8530, 0.7430, 0.7745,
    0.6580, 0.7745, 0.6945, 0.5775, 0.6275, 0.7345, 0.6540, 0.9935, 0.6625, 0.6205, 0.6075, 0.3820,
    0.4515, 0.3720, 0.4835, 0.4785, 0.2250, 0.5540, 0.5545, 0.4610, 0.5670, 0.4885, 0.3655, 0.5320,
    0.5800, 0.3330, 0.3095, 0.5625, 0.3180, 0.8535, 0.5945, 0.5310, 0.5505, 0.5735, 0.4815, 0.4380,
    0.3775, 0.6065, 0.5075, 0.7620, 0.6035, 0.5220, 0.5070, 0.4140, 0.2935, 0.4035, 0.6405,
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
    // Measured: widest printable ASCII glyph 'W' 0.9935 em (kerning included).
    widest_em: 0.994,
    advances: &STUDIO_ITALIC_ADVANCES,
    // No residual widening pairs (ui::font_metrics checks).
    kerns: &[],
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

    /// The shaped width of `text` in `face`, in ems (`ui::font_metrics`).
    fn real_advance_em(face: &Face, text: &str) -> f32 {
        crate::ui::font_metrics::real_advance_em(face, text)
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
