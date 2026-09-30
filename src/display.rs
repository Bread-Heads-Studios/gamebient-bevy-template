//! The game's shape on screen: one pinned size, a letterboxed viewport on
//! native displays, and a UI scale that follows the viewport's short side.
//!
//! Every game carries its own copy of this file and sets `GAME_WIDTH` and
//! `GAME_HEIGHT` to one sanctioned size: 960x720 (4:3), 720x720 (1:1) or
//! 720x960 (3:4). This is presentation only. The replay verifier has no
//! window, so nothing under `src/game/` that runs in `SimSet` may read it.

use bevy::prelude::*;

/// Pinned backbuffer width in physical pixels. Per game.
pub const GAME_WIDTH: u32 = 960;
/// Pinned backbuffer height in physical pixels. Per game.
pub const GAME_HEIGHT: u32 = 720;
/// Short side that all UI pixel values are authored against.
pub const REFERENCE_SHORT_SIDE: f32 = 720.0;

/// UI scale for a view of this logical size: min(w, h) / 720.
pub fn ui_scale_for(view_w: f32, view_h: f32) -> f32 {
    view_w.min(view_h) / REFERENCE_SHORT_SIDE
}

/// Where the game is drawn inside the window, in physical pixels.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameViewport {
    pub position: UVec2,
    pub size: UVec2,
}

/// Largest GAME_WIDTH:GAME_HEIGHT rect inside `window` after removing
/// `reserve_top` rows and `gap` pixels on every side, centred in what is left.
///
/// Integer arithmetic throughout, so the same window always gives the same
/// rect. If the insets leave nothing (or `game` has a zero side) the whole
/// window is returned: a game drawn over the frame beats a game not drawn.
pub fn letterbox(window: UVec2, game: UVec2, reserve_top: u32, gap: u32) -> GameViewport {
    let sides = gap.saturating_mul(2);
    let avail_w = window.x.saturating_sub(sides);
    let avail_h = window.y.saturating_sub(reserve_top).saturating_sub(sides);
    if avail_w == 0 || avail_h == 0 || game.x == 0 || game.y == 0 {
        return GameViewport {
            position: UVec2::ZERO,
            size: window,
        };
    }
    let (aw, ah) = (u64::from(avail_w), u64::from(avail_h));
    let (gw, gh) = (u64::from(game.x), u64::from(game.y));
    // Width-limited when the available area is narrower than the game.
    let (w, h) = if aw * gh <= ah * gw {
        (aw, (aw * gh / gw).max(1))
    } else {
        ((ah * gw / gh).max(1), ah)
    };
    let (w, h) = (w as u32, h as u32);
    GameViewport {
        position: UVec2::new(
            gap + (avail_w - w) / 2,
            reserve_top + gap + (avail_h - h) / 2,
        ),
        size: UVec2::new(w, h),
    }
}

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

    #[test]
    fn ui_scale_follows_the_short_side() {
        assert_eq!(ui_scale_for(960.0, 720.0), 1.0);
        assert_eq!(ui_scale_for(720.0, 960.0), 1.0);
        assert_eq!(ui_scale_for(720.0, 720.0), 1.0);
        assert_eq!(ui_scale_for(1440.0, 1080.0), 1.5);
        assert_eq!(ui_scale_for(1080.0, 1440.0), 1.5);
        assert_eq!(ui_scale_for(480.0, 360.0), 0.5);
    }

    fn boxed(window: (u32, u32), game: (u32, u32), top: u32, gap: u32) -> ((u32, u32), (u32, u32)) {
        let v = letterbox(
            UVec2::new(window.0, window.1),
            UVec2::new(game.0, game.1),
            top,
            gap,
        );
        ((v.position.x, v.position.y), (v.size.x, v.size.y))
    }

    #[test]
    fn letterbox_fills_a_window_of_the_same_ratio() {
        assert_eq!(boxed((960, 720), (960, 720), 0, 0), ((0, 0), (960, 720)));
        assert_eq!(
            boxed((1440, 1080), (960, 720), 0, 0),
            ((0, 0), (1440, 1080))
        );
        assert_eq!(
            boxed((1080, 1440), (720, 960), 0, 0),
            ((0, 0), (1080, 1440))
        );
    }

    #[test]
    fn letterbox_on_a_landscape_1080p_tv() {
        assert_eq!(
            boxed((1920, 1080), (960, 720), 0, 0),
            ((240, 0), (1440, 1080))
        );
        assert_eq!(
            boxed((1920, 1080), (720, 720), 0, 0),
            ((420, 0), (1080, 1080))
        );
        assert_eq!(
            boxed((1920, 1080), (720, 960), 0, 0),
            ((555, 0), (810, 1080))
        );
    }

    #[test]
    fn letterbox_on_a_portrait_1080p_tv_under_a_marquee() {
        // 360 rows reserved for the marquee leave 1080x1560.
        assert_eq!(
            boxed((1080, 1920), (720, 960), 360, 0),
            ((0, 420), (1080, 1440))
        );
        assert_eq!(
            boxed((1080, 1920), (720, 720), 360, 0),
            ((0, 600), (1080, 1080))
        );
        assert_eq!(
            boxed((1080, 1920), (960, 720), 360, 0),
            ((0, 735), (1080, 810))
        );
    }

    #[test]
    fn letterbox_keeps_the_gap_clear_on_every_side() {
        // 1920x1080 less 8 px all round leaves 1904x1064; 4:3 is height-limited.
        assert_eq!(
            boxed((1920, 1080), (960, 720), 0, 8),
            ((251, 8), (1418, 1064))
        );
    }

    #[test]
    fn letterbox_never_leaves_the_available_area() {
        let windows = [
            (1920, 1080),
            (1080, 1920),
            (1280, 720),
            (720, 1280),
            (960, 720),
            (800, 600),
            (1366, 768),
        ];
        let games = [(960, 720), (720, 720), (720, 960)];
        let insets = [(0, 0), (360, 0), (0, 8), (360, 8)];
        for (w, h) in windows {
            for (gw, gh) in games {
                for (top, gap) in insets {
                    let v = letterbox(UVec2::new(w, h), UVec2::new(gw, gh), top, gap);
                    assert!(v.position.x >= gap, "{w}x{h} {gw}x{gh} {top} {gap}: {v:?}");
                    assert!(
                        v.position.y >= top + gap,
                        "{w}x{h} {gw}x{gh} {top} {gap}: {v:?}"
                    );
                    assert!(v.position.x + v.size.x <= w - gap, "{v:?}");
                    assert!(v.position.y + v.size.y <= h - gap, "{v:?}");
                    // Within one pixel of the game's ratio.
                    let cross =
                        i64::from(v.size.x) * i64::from(gh) - i64::from(v.size.y) * i64::from(gw);
                    assert!(cross.abs() < i64::from(gw.max(gh)), "{v:?}");
                }
            }
        }
    }

    #[test]
    fn letterbox_falls_back_to_the_whole_window_when_nothing_is_left() {
        // Insets that swallow the window, or a zero-sized game.
        assert_eq!(boxed((200, 100), (960, 720), 100, 0), ((0, 0), (200, 100)));
        assert_eq!(boxed((200, 100), (960, 720), 0, 100), ((0, 0), (200, 100)));
        assert_eq!(boxed((200, 100), (0, 720), 0, 0), ((0, 0), (200, 100)));
    }
}
