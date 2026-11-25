//! The game's look in one place: palette, card styles and the themed words
//! the shared screens use. `card.rs`, `fit.rs` and `studio_logo.rs` are
//! copied verbatim into games; this file is where a game becomes itself.
//!
//! TEMPLATE NOTE: replace the palette with the game's (match the box art
//! and marquee), restyle `CARD` as the game's surface (paper, chalkboard,
//! plaque, glass, label) and give `PAUSE_WORD` a themed word ("TIME OUT",
//! "PIT STOP", "OUT OF OFFICE"). Keep the pause hints' meaning.

use bevy::prelude::*;

use crate::look::LookTint;
use crate::look::{Contract, ParamKind, ParamSpec, RecipeSpec, ShapeSpec, SlotSpec, StringSpec};
use crate::ui::card::{CardRoles, CardStyle};
use crate::ui::fonts;

// --- Palette -------------------------------------------------------------
// The template's neutral "night arcade": ink and indigo, a signal-cyan
// accent and a warm coral counter-accent.

/// Deepest background.
pub const INK: Color = Color::srgb(0.035, 0.043, 0.094);
/// Upper sky / card body.
pub const NIGHT: Color = Color::srgb(0.071, 0.086, 0.188);
/// Horizon band.
pub const DUSK: Color = Color::srgb(0.20, 0.13, 0.36);
/// Primary accent: borders, grid lines, the best chip.
pub const SIGNAL: Color = Color::srgb(0.30, 0.86, 0.93);
/// Warm counter-accent: the horizon glow, headlines on dark cards.
pub const CORAL: Color = Color::srgb(1.0, 0.48, 0.36);
/// Main text.
pub const PAPER: Color = Color::srgb(0.96, 0.95, 0.92);
/// Secondary text.
pub const MUTED: Color = Color::srgb(0.62, 0.66, 0.78);

/// Depth under display text (`TextShadow`).
pub const TITLE_SHADOW: Color = Color::srgba(0.0, 0.0, 0.0, 0.55);

// --- Cards ---------------------------------------------------------------

/// The card surface's colours (the palette roles `card`, `cardtop`,
/// `cardbottom` and `cardshadow` default to these).
pub const CARD_FILL: Color = Color::srgba(0.06, 0.07, 0.15, 0.9);
pub const CARD_TOP: Color = Color::srgba(0.13, 0.13, 0.30, 0.94);
pub const CARD_BOTTOM: Color = Color::srgba(0.05, 0.06, 0.13, 0.94);
pub const CARD_SHADOW: Color = Color::srgba(0.0, 0.0, 0.0, 0.6);

/// The game's card surface: a dark glass panel with a cyan rim.
pub const CARD: CardStyle = CardStyle {
    fill: CARD_FILL,
    gradient: Some([CARD_TOP, CARD_BOTTOM]),
    border: Color::srgba(0.30, 0.86, 0.93, 0.85),
    border_px: 2.0,
    radius: 16.0,
    shadow: Some(CARD_SHADOW),
};

/// The prompt rail's plate: the same surface, flatter and with a hairline.
pub const RAIL: CardStyle = CardStyle {
    fill: Color::srgba(0.035, 0.043, 0.094, 0.78),
    gradient: None,
    border: Color::srgba(0.30, 0.86, 0.93, 0.35),
    border_px: 2.0,
    radius: 14.0,
    shadow: Some(Color::srgba(0.0, 0.0, 0.0, 0.45)),
};

/// The roles each card style takes under a pack (`ui::card::CardRoles`),
/// beside the style: the tint's alpha is the style's own alpha, so under
/// the default look every part is exactly the style's colour. Spawn with
/// `.card_look(&CARD, &CARD_ROLES)` after `card_with`.
pub const CARD_ROLES: CardRoles =
    CardRoles::new(LookTint::new("card"), LookTint::new("signal").alpha(0.85))
        .gradient(LookTint::new("cardtop"), LookTint::new("cardbottom"))
        .shadow(LookTint::new("cardshadow"));
pub const RAIL_ROLES: CardRoles = CardRoles::new(
    LookTint::new("ink").alpha(0.78),
    LookTint::new("signal").alpha(0.35),
)
.shadow(LookTint::new("cardshadow").alpha(0.45));
pub const CHIP_ROLES: CardRoles = CardRoles::new(
    LookTint::new("signal").alpha(0.12),
    LookTint::new("signal").alpha(0.8),
);
pub const BADGE_ROLES: CardRoles =
    CardRoles::new(LookTint::new("coral").alpha(0.18), LookTint::new("coral"));

/// Small pill for the best score and badges.
pub const CHIP: CardStyle = CardStyle {
    fill: Color::srgba(0.30, 0.86, 0.93, 0.12),
    gradient: None,
    border: Color::srgba(0.30, 0.86, 0.93, 0.8),
    border_px: 2.0,
    radius: 999.0,
    shadow: None,
};

/// Badge for a new best: the warm accent.
pub const BADGE: CardStyle = CardStyle {
    fill: Color::srgba(1.0, 0.48, 0.36, 0.18),
    gradient: None,
    border: Color::srgb(1.0, 0.48, 0.36),
    border_px: 2.0,
    radius: 999.0,
    shadow: None,
};

// --- Copy ----------------------------------------------------------------

/// The pause card's word. ASCII, short, themed.
pub const PAUSE_WORD: &str = "TAKE FIVE";
/// Pause hints: ESC resumes, ENTER quits to the title. Keep the meaning.
pub const PAUSE_RESUME: &str = "ESC: RESUME";
pub const PAUSE_QUIT: &str = "ENTER: QUIT TO TITLE";

// --- Look contract -------------------------------------------------------
// What a Look Pack may change in this game (docs/look-packs.md). Roles and
// keys are the game's own; keep defaults equal to the constants above.

/// The template player's colour (`game::player`).
pub const PLAYER: Color = Color::srgb(0.2, 0.8, 1.0);

pub const PALETTE: &[(&str, Color)] = &[
    ("ink", INK),
    ("night", NIGHT),
    ("dusk", DUSK),
    ("signal", SIGNAL),
    ("coral", CORAL),
    ("paper", PAPER),
    ("muted", MUTED),
    ("player", PLAYER),
    // Card surface (`CARD_ROLES`); rims, rails, chips and badges reuse
    // `signal`, `ink` and `coral`.
    ("card", CARD_FILL),
    ("cardtop", CARD_TOP),
    ("cardbottom", CARD_BOTTOM),
    ("cardshadow", CARD_SHADOW),
    // Sponsor end card (`look::end_card`), on the game-over card.
    ("endcard", Color::srgba(0.035, 0.043, 0.094, 0.0)),
    ("endcardhead", PAPER),
    ("endcardbody", MUTED),
    ("endcardurl", SIGNAL),
];

pub const STRINGS: &[StringSpec] = &[
    StringSpec {
        key: "title",
        default: crate::ui::menu::TITLE,
        max_chars: 20,
    },
    StringSpec {
        key: "pause",
        default: PAUSE_WORD,
        max_chars: 12,
    },
    StringSpec {
        key: "pause.resume",
        default: PAUSE_RESUME,
        max_chars: 24,
    },
    StringSpec {
        key: "pause.quit",
        default: PAUSE_QUIT,
        max_chars: 24,
    },
];

pub const SLOTS: &[SlotSpec] = &[SlotSpec {
    name: "player",
    image: false,
    recipes: &[RecipeSpec {
        name: "cuboid",
        params: &[ParamSpec {
            name: "size",
            kind: ParamKind::Number {
                min: 0.25,
                max: 3.0,
            },
        }],
    }],
    shape: Some(ShapeSpec::cube(12, 3000, 1.5)),
}];

/// `sfx.<SfxEvent::name>` and `music.<GameState, lower case>`.
pub const AUDIO_CUES: &[&str] = &[
    "sfx.confirm",
    "sfx.pause",
    "sfx.screensweep",
    "music.studiologo",
    "music.menu",
    // Only applies when HowToPlay is entered from a state with a different
    // cue: Menu and HowToPlay share MENU_CUE, so Menu -> HowToPlay is no change.
    "music.howtoplay",
    "music.playing",
    "music.gameover",
    "music.demoend",
];

pub const FONTS: &[&str] = &[
    fonts::DISPLAY.name,
    fonts::BODY.name,
    fonts::STUDIO.name,
    fonts::STUDIO_ITALIC.name,
];

pub const LOOK: Contract = Contract {
    palette: PALETTE,
    strings: STRINGS,
    slots: SLOTS,
    audio: AUDIO_CUES,
    fonts: FONTS,
    // No part of the template's own art glows; a game with emissive roles
    // lists the ones a pack may dim.
    glow: &[],
};

/// Outer width of the game-over card (a `max_width`: the card is as wide as
/// its content up to this).
pub fn game_over_card_width() -> f32 {
    crate::ui::fit::ui_width() * 0.9
}

/// The width the end card's `EndCardSlot` is given: its parent's content
/// width, which is the parent's width less its padding and border. Here the
/// game-over card less `card_node`'s `S4` padding and `CARD`'s border on each
/// side. **A game writes this one function** for its own game-over card
/// (dive-rise: its `GAME_OVER_CARD_W` less the border `card_with` adds on
/// each side); the
/// copyable `look_contract_tests` call it to hold `end_card_style()` to it.
pub fn end_card_slot_width() -> f32 {
    game_over_card_width() - 2.0 * (crate::ui::card::S4 + CARD.border_px)
}

/// Width of the end card's text column in a slot `slot_w` wide, where
/// `slot_w` is the width the slot is given (its parent's content width, see
/// `end_card_slot_width`). What is left of it beside the end card's own
/// widest image (`IMAGE_MAX_WIDTH`), its gap (16) and its own padding (2 x
/// `end_card::PAD` = 32), up to 480.
pub fn end_card_text_width(slot_w: f32) -> f32 {
    (slot_w - crate::look::end_card::IMAGE_MAX_WIDTH - 16.0 - 32.0).min(480.0)
}

/// The end card's fonts and sizes (`look::end_card`).
pub fn end_card_style() -> crate::look::EndCardStyle {
    crate::look::EndCardStyle {
        display: fonts::DISPLAY.font(28.0),
        body: fonts::BODY.font(18.0),
        display_advance_em: fonts::DISPLAY.advance_em,
        body_advance_em: fonts::BODY.advance_em,
        text_width: end_card_text_width(end_card_slot_width()),
        headline_size: 28.0,
        body_size: 18.0,
        url_size: 16.0,
    }
}

/// The look contract's tests every game needs. Games copy this module
/// unchanged: it uses only `crate::look`, this file (`PALETTE`, `STRINGS`,
/// `end_card_text_width`, `end_card_style`, `end_card_slot_width`). A game that
/// recopies it writes its own `end_card_slot_width()` (the width its
/// `EndCardSlot` is given) and builds `end_card_style()` from it.
#[cfg(test)]
mod look_contract_tests {
    use super::*;

    #[test]
    fn the_contract_declares_every_end_card_role() {
        for role in crate::look::end_card::ROLES {
            assert!(
                PALETTE.iter().any(|(r, _)| *r == role),
                "{role} missing from PALETTE"
            );
        }
    }

    #[test]
    fn the_end_card_column_fits_beside_its_image() {
        // `slot_w` is the width the slot is given.
        for slot_w in [520.0, 560.0, 648.0, 864.0] {
            let text = end_card_text_width(slot_w);
            let row = crate::look::end_card::IMAGE_MAX_WIDTH + 16.0 + text + 2.0 * 16.0;
            assert!(row <= slot_w + 0.001, "{slot_w}: {row}");
            assert!(text >= 200.0, "{slot_w}: {text}");
            assert!(text <= 480.0, "{slot_w}: {text}");
        }
        // A narrow slot gets a narrow column; a wide one is capped.
        assert!(end_card_text_width(560.0) < end_card_text_width(648.0));
        assert_eq!(end_card_text_width(2000.0), 480.0);
    }

    /// The game's style follows the game's own slot: a style built from the
    /// view (or any other width) overflows a narrower card under a sponsor
    /// pack. The game provides `end_card_slot_width()`.
    #[test]
    fn the_end_card_style_follows_the_slot_and_fits_inside_it() {
        let slot_w = end_card_slot_width();
        let s = end_card_style();
        assert_eq!(s.text_width, end_card_text_width(slot_w));
        let row = crate::look::end_card::IMAGE_MAX_WIDTH + 16.0 + s.text_width + 2.0 * 16.0;
        assert!(
            row <= slot_w + 0.001,
            "row {row} wider than the slot {slot_w}"
        );
    }

    #[test]
    fn string_defaults_fit_their_own_limits() {
        for s in STRINGS {
            assert!(
                s.default.len() <= s.max_chars,
                "{} default is longer than {}",
                s.key,
                s.max_chars
            );
        }
    }
}

/// Template-only: this reads this game's own audio module
/// (`game::audio::{MUSIC, SfxEvent, music_key}`). A game writes its own
/// version against its own sound list, or drops it while `AUDIO_CUES` is
/// empty.
#[cfg(test)]
mod look_contract_game_tests {
    use super::*;
    use crate::game::audio::{MUSIC, SfxEvent};

    /// The template's slot sits inside its game-over card.
    #[test]
    fn the_slot_is_narrower_than_the_card() {
        assert!(end_card_slot_width() < game_over_card_width());
    }

    #[test]
    fn every_sound_the_game_plays_is_a_cue_a_pack_can_override() {
        for e in SfxEvent::ALL {
            let cue = format!("sfx.{}", e.name());
            assert!(
                AUDIO_CUES.contains(&cue.as_str()),
                "{cue} missing from AUDIO_CUES"
            );
        }
        // A silent state (no cue, e.g. Editing) plays nothing, so a pack has
        // nothing there to override (`audio::switch_music` returns before
        // looking up the pack's track).
        for (state, _) in MUSIC.iter().filter(|(_, cue)| cue.is_some()) {
            let cue = crate::game::audio::music_key(state);
            assert!(
                AUDIO_CUES.contains(&cue.as_str()),
                "{cue} missing from AUDIO_CUES"
            );
        }
    }
}
