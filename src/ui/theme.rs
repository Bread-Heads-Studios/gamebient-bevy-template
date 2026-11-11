//! The game's look in one place: palette, card styles and the themed words
//! the shared screens use. `card.rs`, `fit.rs` and `studio_logo.rs` are
//! copied verbatim into games; this file is where a game becomes itself.
//!
//! TEMPLATE NOTE: replace the palette with the game's (match the box art
//! and marquee), restyle `CARD` as the game's surface (paper, chalkboard,
//! plaque, glass, label) and give `PAUSE_WORD` a themed word ("TIME OUT",
//! "PIT STOP", "OUT OF OFFICE"). Keep the pause hints' meaning.

use bevy::prelude::*;

use crate::look::{Contract, ParamKind, ParamSpec, RecipeSpec, SlotSpec, StringSpec};
use crate::ui::card::CardStyle;
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

/// The game's card surface: a dark glass panel with a cyan rim.
pub const CARD: CardStyle = CardStyle {
    fill: Color::srgba(0.06, 0.07, 0.15, 0.9),
    gradient: Some([
        Color::srgba(0.13, 0.13, 0.30, 0.94),
        Color::srgba(0.05, 0.06, 0.13, 0.94),
    ]),
    border: Color::srgba(0.30, 0.86, 0.93, 0.85),
    border_px: 2.0,
    radius: 16.0,
    shadow: Some(Color::srgba(0.0, 0.0, 0.0, 0.6)),
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
}];

/// `sfx.<SfxEvent::name>` and `music.<GameState, lower case>`.
pub const AUDIO_CUES: &[&str] = &[
    "sfx.confirm",
    "sfx.pause",
    "sfx.screensweep",
    "music.studiologo",
    "music.menu",
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
};

#[cfg(test)]
mod look_contract_tests {
    use super::*;
    use crate::game::audio::{MUSIC, SfxEvent};

    #[test]
    fn every_sound_the_game_plays_is_a_cue_a_pack_can_override() {
        for e in SfxEvent::ALL {
            let cue = format!("sfx.{}", e.name());
            assert!(
                AUDIO_CUES.contains(&cue.as_str()),
                "{cue} missing from AUDIO_CUES"
            );
        }
        for (state, _) in MUSIC {
            let cue = format!("music.{}", format!("{state:?}").to_lowercase());
            assert!(
                AUDIO_CUES.contains(&cue.as_str()),
                "{cue} missing from AUDIO_CUES"
            );
        }
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
