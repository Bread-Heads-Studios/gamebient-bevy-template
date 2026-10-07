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
//! fresh UUID, the real weight, and a measured `advance_em`. Then list every
//! string the UI fits in `tests::FITTED`. Leave `STUDIO` and `STUDIO_ITALIC`
//! exactly as they are: the studio logo is shared.

use bevy::asset::uuid_handle;
use bevy::prelude::*;

pub use crate::ui::fit::Face;

/// Display face: Space Grotesk Bold.
pub const DISPLAY: Face = Face {
    name: "Space Grotesk Bold",
    bytes: include_bytes!("fonts/SpaceGrotesk-Bold.ttf"),
    handle: uuid_handle!("a29aa787-3f39-45a5-8656-8b7913d0f2ea"),
    weight: FontWeight::BOLD,
    // Measured: A-Z 0-9 average 0.621 em.
    advance_em: 0.63,
};

/// Body face: Inter SemiBold (text optical size).
pub const BODY: Face = Face {
    name: "Inter SemiBold",
    bytes: include_bytes!("fonts/Inter-SemiBold.ttf"),
    handle: uuid_handle!("46cc46ce-59b9-45f6-ad60-e2d2f02426fa"),
    weight: FontWeight::SEMIBOLD,
    // Measured: A-Z 0-9 average 0.672 em.
    advance_em: 0.68,
};

/// House studio face: Fraunces Black (72pt optical size, Soft 50). Shared
/// by every game; do not change.
pub const STUDIO: Face = Face {
    name: "Fraunces Black",
    bytes: include_bytes!("fonts/Fraunces-Black.ttf"),
    handle: uuid_handle!("adace312-4d1f-4a3e-9d03-269ad8d6db35"),
    weight: FontWeight::BLACK,
    // Measured: A-Z 0-9 average 0.702 em.
    advance_em: 0.71,
};

/// House studio italic: Fraunces Italic (9pt optical size, Soft 50).
/// Shared by every game; do not change.
pub const STUDIO_ITALIC: Face = Face {
    name: "Fraunces Italic",
    bytes: include_bytes!("fonts/Fraunces-Italic.ttf"),
    handle: uuid_handle!("46ce4214-b1f6-4136-8d3c-4f7459b4d742"),
    weight: FontWeight::NORMAL,
    // Measured: A-Z 0-9 average 0.636 em.
    advance_em: 0.64,
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
    fn fitted() -> Vec<(&'static Face, &'static str)> {
        vec![
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
        ]
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
            let real = real_advance_em(face, text);
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
                let size = crate::ui::fit::fit_font_size(face, text, 200.0, view_w);
                let width = real_advance_em(face, text) * size;
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
