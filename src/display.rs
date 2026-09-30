//! The game's shape on screen: one pinned size, a letterboxed viewport on
//! native displays, and a UI scale that follows the viewport's short side.
//!
//! Every game carries its own copy of this file and sets `GAME_WIDTH` and
//! `GAME_HEIGHT` to one sanctioned size: 960x720 (4:3), 720x720 (1:1) or
//! 720x960 (3:4). This is presentation only. The replay verifier has no
//! window, so nothing under `src/game/` that runs in `SimSet` may read it.

/// Pinned backbuffer width in physical pixels. Per game.
pub const GAME_WIDTH: u32 = 960;
/// Pinned backbuffer height in physical pixels. Per game.
pub const GAME_HEIGHT: u32 = 720;
/// Short side that all UI pixel values are authored against.
pub const REFERENCE_SHORT_SIDE: f32 = 720.0;

#[cfg(test)]
mod tests {
    use gamebient_input::CanvasPolicy;

    use super::*;

    #[test]
    fn game_size_is_a_sanctioned_ratio_with_a_720_short_side() {
        let label = CanvasPolicy::Pinned {
            width: GAME_WIDTH,
            height: GAME_HEIGHT,
        }
        .aspect_label();
        assert!(
            matches!(label.as_deref(), Some("4:3" | "1:1" | "3:4")),
            "GAME_WIDTH x GAME_HEIGHT must be 960x720, 720x720 or 720x960, got {label:?}"
        );
        assert_eq!(GAME_WIDTH.min(GAME_HEIGHT) as f32, REFERENCE_SHORT_SIDE);
    }
}
