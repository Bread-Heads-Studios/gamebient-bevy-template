//! Pure cabinet-frame geometry. Everything is in physical pixels with the
//! origin at the window's top-left, the same space `display::letterbox`
//! works in. The website has a line-for-line port in
//! `src/lib/cabinetFrame/layout.ts`; keep the two in step.

use bevy::math::{UVec2, Vec2};

use crate::display::{FrameInsets, GameViewport};

/// Gap between game and bezel at a 1080 short side, in pixels.
pub const GAP_AT_1080: u32 = 8;
const GAP_REFERENCE_SHORT_SIDE: u32 = 1080;

/// A rectangle in physical pixels. `x` and `y` may be negative: the bezel is
/// cover-fitted, so it overhangs the window on one axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PxRect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

/// Everything the frame needs to know about one window size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameLayout {
    /// The marquee band. `None` on landscape and square displays.
    pub marquee: Option<PxRect>,
    /// What `DisplayPlugin` must reserve for the frame.
    pub insets: FrameInsets,
    /// The square bezel, centred on the window.
    pub bezel: PxRect,
}

/// 8 px at a 1080 short side, scaled linearly and rounded to nearest.
pub fn gap_for(window: UVec2) -> u32 {
    let short = window.x.min(window.y);
    (GAP_AT_1080 * short + GAP_REFERENCE_SHORT_SIDE / 2) / GAP_REFERENCE_SHORT_SIDE
}

pub fn frame_layout(window: UVec2) -> FrameLayout {
    let marquee = (window.y > window.x).then_some(PxRect {
        x: 0,
        y: 0,
        w: window.x,
        h: window.x / 3,
    });
    let side = window.x.max(window.y);
    FrameLayout {
        marquee,
        insets: FrameInsets {
            reserve_top: marquee.map_or(0, |m| m.h),
            gap: gap_for(window),
        },
        bezel: PxRect {
            x: (window.x as i32 - side as i32) / 2,
            y: (window.y as i32 - side as i32) / 2,
            w: side,
            h: side,
        },
    }
}

/// The black rectangle drawn under the game: the game grown by the gap on
/// every side. The ring the game does not cover is the gap.
pub fn backing_rect(game: GameViewport, gap: u32) -> PxRect {
    PxRect {
        x: game.position.x as i32 - gap as i32,
        y: game.position.y as i32 - gap as i32,
        w: game.size.x + 2 * gap,
        h: game.size.y + 2 * gap,
    }
}

/// Where the art's lettering stops, as a share of the marquee's height:
/// row 288 of 360 (Contract E, art geometry). Below it is the score line's.
pub const SCORE_STRIP_TOP: f32 = 0.8;
/// Centre of the score line as a share of the marquee's height: row 324 of
/// 360, the middle of the strip.
pub const CAPTION_Y: f32 = 0.9;
/// Font height of the score line as a share of the marquee's width: 40 px
/// at 1080.
pub const CAPTION_SIZE_PER_WIDTH: f32 = 40.0 / 1080.0;
/// Line box of the score line, in font heights. 40 x 1.2 = 48 rows of 72.
pub const CAPTION_LINE_HEIGHT: f32 = 1.2;

/// The score line's centre, measured down from the marquee's top, and its
/// font height. Same units as the arguments.
pub fn caption_box(marquee_w: f32, marquee_h: f32) -> (f32, f32) {
    (marquee_h * CAPTION_Y, marquee_w * CAPTION_SIZE_PER_WIDTH)
}

/// Left edge of the fallback marquee's title box, in 1080-wide art pixels.
const TITLE_BOX_LEFT: u32 = 475;
/// Right edge of the fallback marquee's title box, in 1080-wide art pixels.
const TITLE_BOX_RIGHT: u32 = 1052;
const TITLE_BOX_REFERENCE_W: u32 = 1080;
/// Vertical centre of the title box, in 360-tall art pixels.
const TITLE_CENTRE_ROW: u32 = 154;
/// Tallest title, in 360-tall art pixels.
const TITLE_MAX_SIZE_ROWS: u32 = 84;
const TITLE_REFERENCE_H: u32 = 360;
/// Share of the box width a title may fill.
const TITLE_FILL: f32 = 0.92;
/// Average advance of the template's monospaced font, in em.
pub const TITLE_ADVANCE_EM: f32 = 0.6;

/// Where the game's title goes on the shared fallback marquee, and how big.
/// The website implements the same function with the same vectors.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TitleBox {
    pub box_left: u32,
    pub box_right: u32,
    pub centre_x: f32,
    pub centre_y: f32,
    pub font_size: f32,
}

pub fn title_box(marquee_w: u32, marquee_h: u32, chars: usize, advance_em: f32) -> TitleBox {
    let box_left = marquee_w * TITLE_BOX_LEFT / TITLE_BOX_REFERENCE_W;
    let box_right = marquee_w * TITLE_BOX_RIGHT / TITLE_BOX_REFERENCE_W;
    let box_w = box_right - box_left;
    let centre_x = box_left as f32 + box_w as f32 / 2.0;
    let centre_y = marquee_h as f32 * TITLE_CENTRE_ROW as f32 / TITLE_REFERENCE_H as f32;
    let max_size = marquee_h as f32 * TITLE_MAX_SIZE_ROWS as f32 / TITLE_REFERENCE_H as f32;
    let fit = box_w as f32 * TITLE_FILL / (chars.max(1) as f32 * advance_em);
    TitleBox {
        box_left,
        box_right,
        centre_x,
        centre_y,
        font_size: max_size.min(fit).floor().max(1.0),
    }
}

/// Centre and size of `rect` in `Camera2d` world units: logical pixels,
/// origin at the window centre, y up.
pub fn to_world(rect: PxRect, window: UVec2, scale_factor: f32) -> (Vec2, Vec2) {
    let size = Vec2::new(rect.w as f32, rect.h as f32);
    let centre = Vec2::new(rect.x as f32, rect.y as f32) + size / 2.0;
    let half = window.as_vec2() / 2.0;
    (
        Vec2::new(centre.x - half.x, half.y - centre.y) / scale_factor,
        size / scale_factor,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::letterbox;

    fn rect(x: i32, y: i32, w: u32, h: u32) -> PxRect {
        PxRect { x, y, w, h }
    }

    #[test]
    fn title_box_vectors_t1_to_t6() {
        // (w, h, chars, advance, box_left, box_right, font_size)
        let rows = [
            (1080, 360, 15, 0.6, 475, 1052, 58.0),
            (1080, 360, 15, 0.72, 475, 1052, 49.0),
            (1080, 360, 8, 0.6, 475, 1052, 84.0),
            (1080, 360, 22, 0.72, 475, 1052, 33.0),
            (540, 180, 15, 0.6, 237, 526, 29.0),
            (1080, 360, 0, 0.6, 475, 1052, 84.0),
        ];
        for (i, (w, h, chars, advance, left, right, size)) in rows.into_iter().enumerate() {
            let t = title_box(w, h, chars, advance);
            assert_eq!(
                (t.box_left, t.box_right, t.font_size),
                (left, right, size),
                "T{}",
                i + 1
            );
        }
    }

    #[test]
    fn title_box_centre_at_1080_by_360() {
        let t = title_box(1080, 360, 15, TITLE_ADVANCE_EM);
        assert_eq!(t.centre_x, 763.5);
        assert_eq!(t.centre_y, 154.0);
    }

    #[test]
    fn l1_landscape_1080p() {
        let l = frame_layout(UVec2::new(1920, 1080));
        assert_eq!(l.marquee, None);
        assert_eq!(
            l.insets,
            FrameInsets {
                reserve_top: 0,
                gap: 8
            }
        );
        assert_eq!(l.bezel, rect(0, -420, 1920, 1920));
    }

    #[test]
    fn l2_portrait_1080p() {
        let l = frame_layout(UVec2::new(1080, 1920));
        assert_eq!(l.marquee, Some(rect(0, 0, 1080, 360)));
        assert_eq!(
            l.insets,
            FrameInsets {
                reserve_top: 360,
                gap: 8
            }
        );
        assert_eq!(l.bezel, rect(-420, 0, 1920, 1920));
    }

    #[test]
    fn l3_landscape_720p() {
        let l = frame_layout(UVec2::new(1280, 720));
        assert_eq!(l.marquee, None);
        assert_eq!(
            l.insets,
            FrameInsets {
                reserve_top: 0,
                gap: 5
            }
        );
        assert_eq!(l.bezel, rect(0, -280, 1280, 1280));
    }

    #[test]
    fn l4_portrait_720p() {
        let l = frame_layout(UVec2::new(720, 1280));
        assert_eq!(l.marquee, Some(rect(0, 0, 720, 240)));
        assert_eq!(
            l.insets,
            FrameInsets {
                reserve_top: 240,
                gap: 5
            }
        );
        assert_eq!(l.bezel, rect(-280, 0, 1280, 1280));
    }

    #[test]
    fn square_display_has_no_marquee() {
        assert_eq!(frame_layout(UVec2::new(1080, 1080)).marquee, None);
    }

    #[test]
    fn gap_scales_linearly_with_the_short_side() {
        assert_eq!(gap_for(UVec2::new(1920, 1080)), 8);
        assert_eq!(gap_for(UVec2::new(1080, 1920)), 8);
        assert_eq!(gap_for(UVec2::new(1280, 720)), 5);
        assert_eq!(gap_for(UVec2::new(3840, 2160)), 16);
        assert_eq!(gap_for(UVec2::new(0, 0)), 0);
    }

    /// L5 to L10: the game rectangle the website must also produce. These
    /// go through plan 2's `letterbox`, which floors; L9 and L10 are the
    /// cases where that matters. A failure here means the website's port in
    /// `src/lib/cabinetFrame/layout.ts` no longer matches the template.
    #[test]
    fn l5_to_l10_game_rect_matches_the_shared_vectors() {
        let cases = [
            (
                UVec2::new(1080, 1920),
                UVec2::new(720, 720),
                (8, 608, 1064, 1064),
            ),
            (
                UVec2::new(1080, 1920),
                UVec2::new(960, 720),
                (8, 741, 1064, 798),
            ),
            (
                UVec2::new(1920, 1080),
                UVec2::new(720, 720),
                (428, 8, 1064, 1064),
            ),
            (
                UVec2::new(1920, 1080),
                UVec2::new(720, 960),
                (561, 8, 798, 1064),
            ),
            (
                UVec2::new(1080, 1920),
                UVec2::new(720, 960),
                (8, 431, 1064, 1418),
            ),
            (
                UVec2::new(1920, 1080),
                UVec2::new(960, 720),
                (251, 8, 1418, 1064),
            ),
        ];
        for (window, game, (x, y, w, h)) in cases {
            let insets = frame_layout(window).insets;
            let viewport = letterbox(window, game, insets.reserve_top, insets.gap);
            assert_eq!(viewport.position, UVec2::new(x, y), "{window} {game}");
            assert_eq!(viewport.size, UVec2::new(w, h), "{window} {game}");
        }
    }

    /// L11: with nothing left after the insets, plan 2 returns the whole
    /// window. The website's port must do the same.
    #[test]
    fn l11_nothing_left_is_the_whole_window() {
        let viewport = letterbox(UVec2::new(10, 10), UVec2::new(960, 720), 0, 8);
        assert_eq!(viewport.position, UVec2::ZERO);
        assert_eq!(viewport.size, UVec2::new(10, 10));
    }

    #[test]
    fn m1_m2_the_score_line_sits_in_the_middle_of_its_strip() {
        let (centre, font) = caption_box(1080.0, 360.0);
        assert!((centre - 324.0).abs() < 0.001 && (font - 40.0).abs() < 0.001);
        let (centre, font) = caption_box(720.0, 240.0);
        assert!((centre - 216.0).abs() < 0.001 && (font - 26.667).abs() < 0.001);
    }

    #[test]
    fn the_score_line_stays_inside_rows_288_to_360() {
        for (w, h) in [(1080.0, 360.0), (720.0, 240.0), (2160.0, 720.0)] {
            let (centre, font) = caption_box(w, h);
            let half = font * CAPTION_LINE_HEIGHT / 2.0;
            assert!(centre - half >= h * SCORE_STRIP_TOP, "{w}x{h}: top");
            assert!(centre + half <= h, "{w}x{h}: bottom");
        }
    }

    #[test]
    fn backing_is_the_game_grown_by_the_gap() {
        let game = GameViewport {
            position: UVec2::new(8, 608),
            size: UVec2::new(1064, 1064),
        };
        assert_eq!(backing_rect(game, 8), rect(0, 600, 1080, 1080));
    }

    #[test]
    fn to_world_centres_on_the_window_with_y_up() {
        let window = UVec2::new(1080, 1920);
        let (centre, size) = to_world(rect(0, 0, 1080, 360), window, 1.0);
        assert_eq!(centre, Vec2::new(0.0, 780.0));
        assert_eq!(size, Vec2::new(1080.0, 360.0));
        let (centre, size) = to_world(rect(0, 0, 1080, 360), window, 2.0);
        assert_eq!(centre, Vec2::new(0.0, 390.0));
        assert_eq!(size, Vec2::new(540.0, 180.0));
    }
}
