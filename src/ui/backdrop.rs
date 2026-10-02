//! The template's title backdrop: a dusk gradient sky, a warm horizon glow
//! and a perspective grid floor drifting toward the viewer. Built from UI
//! nodes only (no shaders, no sim entities), so it is cheap on a Pi 4 and
//! invisible to the replay verifier.
//!
//! Motion budget: the grid is the one moving layer, one line spacing every
//! `GRID_PERIOD_SECS` (0.25 Hz), and the composition is complete on its
//! first frame, so a shot at t = 1.0 s is settled.
//!
//! TEMPLATE NOTE: replace this with the game's own backdrop: the live world
//! in attract mode, a cosmetic copy of it (menu-marker entities, never sim
//! markers, despawned on exit), or a motif of the game's world. Never a flat
//! fill.

use bevy::prelude::*;

use crate::display::{GAME_HEIGHT, GAME_WIDTH, REFERENCE_SHORT_SIDE};
use crate::ui::theme::{CORAL, DUSK, INK, NIGHT, SIGNAL};

/// Horizon height as a share of the view, from the top.
pub const HORIZON: f32 = 0.6;
/// Seconds for the grid to advance by one line (0.25 Hz).
pub const GRID_PERIOD_SECS: f32 = 4.0;
/// Horizontal grid lines on the floor at any moment.
const ROWS: usize = 11;
/// Converging grid lines either side of the centre line.
const COLUMNS_EACH_SIDE: i32 = 9;
/// Spacing of the converging lines where they meet the bottom edge.
const COLUMN_SPACING_PX: f32 = 120.0;

/// Floor colour just below the horizon: a dim reflection of the glow.
const FLOOR_NEAR_HORIZON: Color = Color::srgb(0.11, 0.06, 0.14);

/// One horizontal floor line; `index` orders them from near to far.
#[derive(Component)]
pub struct GridRow {
    pub index: usize,
}

/// Height of the game view in UI pixels (720 at 4:3 and 1:1, 960 at 3:4).
pub fn ui_height() -> f32 {
    GAME_HEIGHT as f32 * REFERENCE_SHORT_SIDE / GAME_WIDTH.min(GAME_HEIGHT) as f32
}

/// Distance of row `index` from the horizon, as a share of the floor's
/// height, at grid phase `phase` (0 to 1). Rows come toward the viewer as
/// the phase grows and wrap from the bottom edge back to the horizon; the
/// 1/depth spacing gives the perspective.
pub fn row_depth_share(index: usize, phase: f32) -> f32 {
    // Depth runs from 1 (the bottom edge) to ROWS + 1 (far), so the share
    // runs from 1 down to 1 / (ROWS + 1).
    let slot = (index as f32 + 1.0 - phase.rem_euclid(1.0)).rem_euclid(ROWS as f32);
    1.0 / (1.0 + slot)
}

/// Grid phase at `secs` seconds.
pub fn grid_phase(secs: f32) -> f32 {
    (secs / GRID_PERIOD_SECS).rem_euclid(1.0)
}

/// Spawns the backdrop as a full-view child of `parent`, behind its other
/// children. `dim` darkens it for screens that put a card over it.
pub fn spawn_backdrop(parent: &mut ChildSpawnerCommands, dim: bool) {
    let floor_h = ui_height() * (1.0 - HORIZON);
    let view_w = crate::ui::fit::ui_width();
    parent
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundGradient::from(LinearGradient::to_bottom(vec![
                ColorStop::new(INK, Val::Percent(0.0)),
                ColorStop::new(NIGHT, Val::Percent(32.0)),
                ColorStop::new(DUSK, Val::Percent(HORIZON * 100.0)),
                ColorStop::new(FLOOR_NEAR_HORIZON, Val::Percent(HORIZON * 100.0)),
                ColorStop::new(Color::srgb(0.02, 0.025, 0.06), Val::Percent(100.0)),
            ])),
        ))
        .with_children(|sky| {
            // Horizon glow: a wide warm ellipse rising from the horizon,
            // clipped to the sky.
            sky.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(HORIZON * 100.0),
                    ..default()
                },
                BackgroundGradient::from(RadialGradient {
                    position: UiPosition::BOTTOM,
                    shape: RadialGradientShape::Ellipse(Val::Percent(60.0), Val::Percent(55.0)),
                    stops: vec![
                        ColorStop::new(CORAL.with_alpha(0.42), Val::Percent(0.0)),
                        ColorStop::new(CORAL.with_alpha(0.12), Val::Percent(45.0)),
                        ColorStop::new(CORAL.with_alpha(0.0), Val::Percent(100.0)),
                    ],
                    ..default()
                }),
            ));
            // A bright hairline on the horizon itself.
            sky.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Percent(HORIZON * 100.0),
                    width: Val::Percent(100.0),
                    height: Val::Px(2.0),
                    ..default()
                },
                BackgroundGradient::from(LinearGradient::to_right(vec![
                    ColorStop::new(CORAL.with_alpha(0.0), Val::Percent(0.0)),
                    ColorStop::new(CORAL.with_alpha(0.85), Val::Percent(50.0)),
                    ColorStop::new(CORAL.with_alpha(0.0), Val::Percent(100.0)),
                ])),
            ));
            // The floor: converging lines plus drifting rows, clipped.
            sky.spawn(Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(HORIZON * 100.0),
                width: Val::Percent(100.0),
                height: Val::Px(floor_h),
                overflow: Overflow::clip(),
                ..default()
            })
            .with_children(|floor| {
                let cx = view_w / 2.0;
                for j in -COLUMNS_EACH_SIDE..=COLUMNS_EACH_SIDE {
                    let dx = j as f32 * COLUMN_SPACING_PX;
                    let len = dx.hypot(floor_h);
                    let angle = column_angle(dx, floor_h);
                    floor.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(cx + dx / 2.0 - 1.0),
                            top: Val::Px(floor_h / 2.0 - len / 2.0),
                            width: Val::Px(2.0),
                            height: Val::Px(len),
                            ..default()
                        },
                        BackgroundGradient::from(LinearGradient::to_bottom(vec![
                            ColorStop::new(SIGNAL.with_alpha(0.0), Val::Percent(0.0)),
                            ColorStop::new(SIGNAL.with_alpha(0.24), Val::Percent(100.0)),
                        ])),
                        UiTransform::from_rotation(Rot2::radians(angle)),
                    ));
                }
                for index in 0..ROWS {
                    floor.spawn((
                        GridRow { index },
                        Node {
                            position_type: PositionType::Absolute,
                            top: Val::Px(0.0),
                            width: Val::Percent(100.0),
                            height: Val::Px(2.0),
                            ..default()
                        },
                        BackgroundColor(SIGNAL.with_alpha(0.0)),
                        row_pose(index, 0.0, floor_h),
                    ));
                }
            });
            if dim {
                sky.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    BackgroundColor(INK.with_alpha(0.55)),
                ));
            }
        });
}

/// Position of row `index` at grid phase `phase` on a floor `floor_h` tall.
fn row_pose(index: usize, phase: f32, floor_h: f32) -> UiTransform {
    let y = row_depth_share(index, phase) * floor_h;
    UiTransform::from_translation(Val2::px(0.0, y - 1.0))
}

/// Brightness of a row by its depth share: near rows bright, far rows fade
/// into the horizon glow.
fn row_alpha(share: f32) -> f32 {
    0.34 * share.sqrt()
}

/// Drifts the floor rows toward the viewer. Presentation only, on the app
/// clock (`Time<Real>`), never `font_size`.
pub fn drift_grid(
    time: Res<Time<Real>>,
    mut rows: Query<(&GridRow, &mut UiTransform, &mut BackgroundColor)>,
) {
    if rows.is_empty() {
        return;
    }
    let floor_h = ui_height() * (1.0 - HORIZON);
    let phase = grid_phase(time.elapsed_secs());
    for (row, mut transform, mut color) in &mut rows {
        *transform = row_pose(row.index, phase, floor_h);
        color.0 = SIGNAL.with_alpha(row_alpha(row_depth_share(row.index, phase)));
    }
}

/// Rotation for a converging line whose bottom end sits `dx` from the
/// centre. `UiTransform` rotates clockwise on screen (y down), which takes
/// the node's bottom end `(0, h)` to `(-h sin a, h cos a)`; solving for
/// `dx / 2` gives `a = -atan2(dx, floor_h)`.
fn column_angle(dx: f32, floor_h: f32) -> f32 {
    -dx.atan2(floor_h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_stay_on_the_floor_and_wrap() {
        for index in 0..ROWS {
            for step in 0..=20 {
                let share = row_depth_share(index, step as f32 / 20.0);
                assert!(share > 0.0 && share <= 1.0, "{index} {share}");
            }
        }
    }

    #[test]
    fn rows_come_toward_the_viewer() {
        // Away from the wrap, a row's share grows with the phase.
        let a = row_depth_share(3, 0.1);
        let b = row_depth_share(3, 0.4);
        assert!(b > a);
    }

    #[test]
    fn grid_motion_is_slow() {
        const { assert!(1.0 / GRID_PERIOD_SECS <= 0.5) };
        assert_eq!(grid_phase(GRID_PERIOD_SECS), 0.0);
    }

    #[test]
    fn converging_lines_fan_out_to_their_bottom_points() {
        let floor_h = 288.0_f32;
        for dx in [-480.0_f32, -120.0, 0.0, 120.0, 480.0] {
            let half = dx.hypot(floor_h) / 2.0;
            let (sin, cos) = column_angle(dx, floor_h).sin_cos();
            // The bottom end, relative to the node's centre after rotation.
            let end = Vec2::new(-half * sin, half * cos);
            assert!(
                (end - Vec2::new(dx, floor_h) / 2.0).length() < 0.01,
                "{dx}: {end}"
            );
        }
    }
}
