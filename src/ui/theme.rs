//! The game's look in one place: palette, card styles and the themed words
//! the shared screens use. `card.rs`, `fit.rs` and `studio_logo.rs` are
//! copied verbatim into games; this file is where a game becomes itself.
//!
//! TEMPLATE NOTE: replace the palette with the game's (match the box art
//! and marquee), restyle `CARD` as the game's surface (paper, chalkboard,
//! plaque, glass, label) and give `PAUSE_WORD` a themed word ("TIME OUT",
//! "PIT STOP", "OUT OF OFFICE"). Keep the pause hints' meaning.

use bevy::prelude::*;

use crate::ui::card::CardStyle;

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
