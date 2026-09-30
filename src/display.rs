//! The game's shape on screen: one pinned size, a letterboxed viewport on
//! native displays, and a UI scale that follows the viewport's short side.
//!
//! Every game carries its own copy of this file and sets `GAME_WIDTH` and
//! `GAME_HEIGHT` to one sanctioned size: 960x720 (4:3), 720x720 (1:1) or
//! 720x960 (3:4). This is presentation only. The replay verifier has no
//! window, so nothing under `src/game/` that runs in `SimSet` may read it.

use bevy::camera::{CameraUpdateSystems, RenderTarget, Viewport};
use bevy::prelude::*;
use bevy::window::{MonitorSelection, PresentMode, PrimaryWindow, WindowMode};
use gamebient_input::CanvasPolicy;

/// Pinned backbuffer width in physical pixels. Per game.
pub const GAME_WIDTH: u32 = 960;
/// Pinned backbuffer height in physical pixels. Per game.
pub const GAME_HEIGHT: u32 = 720;
/// Short side that all UI pixel values are authored against.
pub const REFERENCE_SHORT_SIDE: f32 = 720.0;

/// Native builds letterbox inside the window. On web the canvas is already
/// the game's size and the gamebient-input glue letterboxes the canvas.
const LETTERBOXED: bool = cfg!(not(target_arch = "wasm32"));

/// The primary window: pinned on web, borderless fullscreen on Linux native.
///
/// macOS and Windows dev machines get a GAME_WIDTH x GAME_HEIGHT window.
/// `GX_WINDOW_SIZE=WxH` (native only) opens a window of that size instead,
/// to check the letterbox against a TV's shape without a TV.
pub fn game_window(title: &str) -> Window {
    let policy = CanvasPolicy::Pinned {
        width: GAME_WIDTH,
        height: GAME_HEIGHT,
    };
    #[allow(unused_mut)]
    let mut window = Window {
        present_mode: PresentMode::AutoVsync,
        mode: window_mode_for(cfg!(target_os = "linux"), cfg!(feature = "autopilot")),
        ..policy.window(title)
    };
    #[cfg(not(target_arch = "wasm32"))]
    if let Some((width, height)) = std::env::var("GX_WINDOW_SIZE")
        .ok()
        .as_deref()
        .and_then(parse_window_size)
    {
        window.resolution = bevy::window::WindowResolution::new(width, height);
        window.mode = WindowMode::Windowed;
    }
    window
}

/// Cabinets run Linux and get the whole panel. The autopilot and record
/// harnesses capture the window, so they keep it the game's own size.
fn window_mode_for(linux_native: bool, harness: bool) -> WindowMode {
    if linux_native && !harness {
        WindowMode::BorderlessFullscreen(MonitorSelection::Primary)
    } else {
        WindowMode::Windowed
    }
}

/// Parses `GX_WINDOW_SIZE`: `"1080x1920"` is `Some((1080, 1920))`. Each side
/// must be 1..=8192.
pub fn parse_window_size(s: &str) -> Option<(u32, u32)> {
    let (w, h) = s.trim().split_once(['x', 'X'])?;
    let (w, h) = (w.parse::<u32>().ok()?, h.parse::<u32>().ok()?);
    ((1..=8192).contains(&w) && (1..=8192).contains(&h)).then_some((w, h))
}

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

/// Top-left corner, in UI pixels, that centres a label `label_width` UI
/// pixels wide under `item_px`.
///
/// `item_px` is what `Camera::world_to_viewport` returns: logical window
/// pixels, which already include the letterboxed viewport's offset.
/// `view_min` is `camera.logical_viewport_rect().min`. UI nodes are placed
/// relative to the viewport's corner, and their Px values are multiplied by
/// `UiScale` at layout time, so take the offset off and divide. Every screen
/// that pins UI to a world position goes through this.
pub fn label_origin(item_px: Vec2, view_min: Vec2, ui_scale: f32, label_width: f32) -> Vec2 {
    let local = (item_px - view_min) / ui_scale;
    Vec2::new(local.x - label_width / 2.0, local.y)
}

/// Marker for cameras that draw the cabinet frame and must NOT be letterboxed.
#[derive(Component)]
pub struct FrameCamera;

/// Rows reserved at the top for the marquee and the gap around the game.
/// Default is zero for both.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FrameInsets {
    pub reserve_top: u32,
    pub gap: u32,
}

/// Inserts GameViewport, applies it to every Camera without FrameCamera,
/// and keeps UiScale in step. On wasm the viewport is the whole canvas.
///
/// Add it after `DefaultPlugins`. A camera that renders to an image or has
/// no target is left alone; a window camera that sets its own viewport
/// (split screen, a minimap) must carry `FrameCamera` and place itself
/// inside `GameViewport`.
pub struct DisplayPlugin;

impl Plugin for DisplayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FrameInsets>()
            .init_resource::<UiScale>()
            .insert_resource(GameViewport {
                position: UVec2::ZERO,
                size: UVec2::new(GAME_WIDTH, GAME_HEIGHT),
            })
            // PostUpdate, before Bevy computes camera targets and lays out
            // UI: a camera spawned this frame is letterboxed before it is
            // first drawn, and UI is laid out once, at the final scale.
            .add_systems(
                PostUpdate,
                (update_game_viewport, apply_game_viewport, sync_ui_scale)
                    .chain()
                    .before(CameraUpdateSystems),
            );
    }
}

/// The game's rect in a window of this physical size.
fn viewport_for(window: UVec2, insets: FrameInsets, letterboxed: bool) -> GameViewport {
    if letterboxed {
        letterbox(
            window,
            UVec2::new(GAME_WIDTH, GAME_HEIGHT),
            insets.reserve_top,
            insets.gap,
        )
    } else {
        GameViewport {
            position: UVec2::ZERO,
            size: window,
        }
    }
}

/// Recomputed every frame and written only when it differs, which covers a
/// window resize, a scale-factor change and a `FrameInsets` change alike.
fn update_game_viewport(
    windows: Query<&Window, With<PrimaryWindow>>,
    insets: Res<FrameInsets>,
    mut viewport: ResMut<GameViewport>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let size = window.resolution.physical_size();
    // A minimised window reports 0x0; keep the last good viewport.
    if size.x == 0 || size.y == 0 {
        return;
    }
    viewport.set_if_neq(viewport_for(size, *insets, LETTERBOXED));
}

/// Native only: every window camera that is not part of the cabinet frame
/// draws inside the game viewport.
fn apply_game_viewport(
    viewport: Res<GameViewport>,
    mut cameras: Query<(&mut Camera, &RenderTarget), Without<FrameCamera>>,
) {
    if !LETTERBOXED {
        return;
    }
    for (mut camera, target) in &mut cameras {
        if !matches!(target, RenderTarget::Window(_)) {
            continue;
        }
        let current = camera
            .viewport
            .as_ref()
            .map(|v| (v.physical_position, v.physical_size));
        if current != Some((viewport.position, viewport.size)) {
            camera.viewport = Some(Viewport {
                physical_position: viewport.position,
                physical_size: viewport.size,
                ..default()
            });
        }
    }
}

/// Scales all UI by the viewport's short side so hardcoded `Val::Px` and
/// `font_size` values, authored against 720, hold on any display.
fn sync_ui_scale(
    windows: Query<&Window, With<PrimaryWindow>>,
    viewport: Res<GameViewport>,
    mut ui_scale: ResMut<UiScale>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let logical = viewport.size.as_vec2() / window.resolution.scale_factor();
    let scale = ui_scale_for(logical.x, logical.y);
    if scale > 0.0 && (ui_scale.0 - scale).abs() > 0.001 {
        ui_scale.0 = scale;
    }
}

#[cfg(test)]
mod tests {
    use bevy::window::WindowResolution;

    use super::*;

    const GAME: UVec2 = UVec2::new(GAME_WIDTH, GAME_HEIGHT);

    /// `DisplayPlugin` on `MinimalPlugins`, with a primary window of the
    /// given physical size. No renderer: the systems only read a `Window`
    /// and write `Camera.viewport` and `UiScale`.
    fn display_app(resolution: WindowResolution) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins).add_plugins(DisplayPlugin);
        app.world_mut().spawn((
            Window {
                resolution,
                ..default()
            },
            PrimaryWindow,
        ));
        app
    }

    fn viewport_of(app: &App, camera: Entity) -> Option<(UVec2, UVec2)> {
        app.world()
            .get::<Camera>(camera)
            .and_then(|c| c.viewport.as_ref())
            .map(|v| (v.physical_position, v.physical_size))
    }

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

    #[test]
    fn fullscreen_only_on_a_linux_cabinet_build() {
        assert_eq!(
            window_mode_for(true, false),
            WindowMode::BorderlessFullscreen(MonitorSelection::Primary)
        );
        // Autopilot and record builds capture the window: keep it game-sized.
        assert_eq!(window_mode_for(true, true), WindowMode::Windowed);
        assert_eq!(window_mode_for(false, false), WindowMode::Windowed);
        assert_eq!(window_mode_for(false, true), WindowMode::Windowed);
    }

    #[test]
    fn window_size_override_parses_width_x_height() {
        assert_eq!(parse_window_size("1080x1920"), Some((1080, 1920)));
        assert_eq!(parse_window_size(" 450X800 "), Some((450, 800)));
        assert_eq!(parse_window_size("0x800"), None);
        assert_eq!(parse_window_size("9000x800"), None);
        assert_eq!(parse_window_size("450"), None);
        assert_eq!(parse_window_size("450x"), None);
        assert_eq!(parse_window_size("wide"), None);
    }

    #[test]
    fn game_window_is_pinned_and_targets_the_game_canvas() {
        let window = game_window("Test");
        assert_eq!(window.title, "Test");
        assert_eq!(window.canvas.as_deref(), Some("#game"));
        assert!(!window.fit_canvas_to_parent);
        assert_eq!(window.present_mode, PresentMode::AutoVsync);
        // A developer running the tests with the override set gets that size.
        if std::env::var_os("GX_WINDOW_SIZE").is_none() {
            assert_eq!(
                CanvasPolicy::from_window(&window),
                CanvasPolicy::Pinned {
                    width: GAME_WIDTH,
                    height: GAME_HEIGHT,
                }
            );
        }
    }

    #[test]
    fn plugin_letterboxes_game_cameras_and_scales_ui() {
        let mut app = display_app(WindowResolution::new(1080, 1920));
        let game = app.world_mut().spawn(Camera::default()).id();
        app.update();

        let expected = letterbox(UVec2::new(1080, 1920), GAME, 0, 0);
        assert_eq!(*app.world().resource::<GameViewport>(), expected);
        assert_eq!(
            viewport_of(&app, game),
            Some((expected.position, expected.size))
        );
        let scale = ui_scale_for(expected.size.x as f32, expected.size.y as f32);
        assert!((app.world().resource::<UiScale>().0 - scale).abs() < 1e-4);
    }

    #[test]
    fn plugin_leaves_frame_cameras_and_offscreen_cameras_alone() {
        let mut app = display_app(WindowResolution::new(1080, 1920));
        let frame = app.world_mut().spawn((Camera::default(), FrameCamera)).id();
        let offscreen = app
            .world_mut()
            .spawn((
                Camera::default(),
                RenderTarget::None {
                    size: UVec2::new(64, 64),
                },
            ))
            .id();
        app.update();
        assert_eq!(viewport_of(&app, frame), None);
        assert_eq!(viewport_of(&app, offscreen), None);
    }

    #[test]
    fn plugin_follows_frame_insets() {
        let mut app = display_app(WindowResolution::new(1080, 1920));
        let game = app.world_mut().spawn(Camera::default()).id();
        app.update();
        app.insert_resource(FrameInsets {
            reserve_top: 360,
            gap: 8,
        });
        app.update();

        let expected = letterbox(UVec2::new(1080, 1920), GAME, 360, 8);
        assert_eq!(*app.world().resource::<GameViewport>(), expected);
        assert_eq!(
            viewport_of(&app, game),
            Some((expected.position, expected.size))
        );
    }

    #[test]
    fn plugin_follows_a_window_resize() {
        let mut app = display_app(WindowResolution::new(1080, 1920));
        let game = app.world_mut().spawn(Camera::default()).id();
        app.update();
        let mut windows = app.world_mut().query::<&mut Window>();
        windows
            .single_mut(app.world_mut())
            .unwrap()
            .resolution
            .set_physical_resolution(1920, 1080);
        app.update();

        let expected = letterbox(UVec2::new(1920, 1080), GAME, 0, 0);
        assert_eq!(
            viewport_of(&app, game),
            Some((expected.position, expected.size))
        );
    }

    #[test]
    fn plugin_letterboxes_a_camera_spawned_later() {
        let mut app = display_app(WindowResolution::new(1920, 1080));
        app.update();
        let late = app.world_mut().spawn(Camera::default()).id();
        app.update();
        let expected = letterbox(UVec2::new(1920, 1080), GAME, 0, 0);
        assert_eq!(
            viewport_of(&app, late),
            Some((expected.position, expected.size))
        );
    }

    #[test]
    fn ui_scale_uses_the_logical_viewport_on_a_hidpi_display() {
        // A 2x display: 1920x1440 physical is 960x720 logical.
        let mut app =
            display_app(WindowResolution::new(1920, 1440).with_scale_factor_override(2.0));
        app.world_mut().spawn(Camera::default());
        app.update();
        let v = *app.world().resource::<GameViewport>();
        let scale = ui_scale_for(v.size.x as f32 / 2.0, v.size.y as f32 / 2.0);
        assert!((app.world().resource::<UiScale>().0 - scale).abs() < 1e-4);
    }

    #[test]
    fn the_web_viewport_is_the_whole_canvas() {
        let insets = FrameInsets {
            reserve_top: 360,
            gap: 8,
        };
        assert_eq!(
            viewport_for(UVec2::new(960, 720), insets, false),
            GameViewport {
                position: UVec2::ZERO,
                size: UVec2::new(960, 720),
            }
        );
        assert_eq!(
            viewport_for(UVec2::new(1080, 1920), insets, true),
            letterbox(UVec2::new(1080, 1920), GAME, 360, 8)
        );
    }

    #[test]
    fn labels_are_centred_under_the_item_inside_the_viewport() {
        // No letterbox, UI scale 1: the label's left edge is half its width
        // left of the item.
        assert_eq!(
            label_origin(Vec2::new(480.0, 400.0), Vec2::ZERO, 1.0, 170.0),
            Vec2::new(480.0 - 85.0, 400.0)
        );
        // A 4:3 game on a portrait 1080x1920 panel: viewport 1080x810 at
        // y = 555, UI scale 810 / 720. An item at the viewport's centre
        // (window 540, 960) is at UI (480, 360).
        let origin = label_origin(Vec2::new(540.0, 960.0), Vec2::new(0.0, 555.0), 1.125, 170.0);
        assert!((origin.x - (480.0 - 85.0)).abs() < 1e-3);
        assert!((origin.y - 360.0).abs() < 1e-3);
        // The width is the caller's: a 60 px score popup over the same item.
        let popup = label_origin(Vec2::new(540.0, 960.0), Vec2::new(0.0, 555.0), 1.125, 60.0);
        assert!((popup.x - (480.0 - 30.0)).abs() < 1e-3);
    }

    fn rust_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let entries = std::fs::read_dir(dir).unwrap_or_else(|e| {
            panic!(
                "cannot read {}: {e}; set SIM_SOURCE_DIR to the directory that holds this game's sim files",
                dir.display()
            )
        });
        for entry in entries {
            let path = entry.unwrap().path();
            if path.is_dir() {
                rust_files(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }

    /// Directory, relative to the crate root, whose `.rs` files must not
    /// read display state. Games with a flat `src/` layout (no `src/game/`)
    /// point this at the directory that holds their sim files, never at
    /// `src/` itself, which contains this file.
    const SIM_SOURCE_DIR: &str = "src/game";

    /// The verifier has no window, so a sim system that reads the viewport
    /// forks on the display the run was played on. Comments are skipped; a
    /// dev harness or presentation system under `src/game/` that needs the
    /// size carries `allow-display: <reason>` on the line.
    #[test]
    fn game_code_does_not_read_display_state() {
        let needles = ["GameViewport", "FrameInsets", "FrameCamera", "display::"];
        let mut files = Vec::new();
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(SIM_SOURCE_DIR);
        rust_files(&dir, &mut files);
        let mut hits = Vec::new();
        for file in files {
            let text = std::fs::read_to_string(&file).unwrap();
            for (n, line) in text.lines().enumerate() {
                let code = line.trim_start();
                if code.starts_with("//") || line.contains("allow-display") {
                    continue;
                }
                for needle in needles {
                    if code.contains(needle) {
                        hits.push(format!("{}:{}: {}", file.display(), n + 1, code.trim()));
                    }
                }
            }
        }
        assert!(
            hits.is_empty(),
            "display state named in src/game/ (presentation only; see src/display.rs):\n{}",
            hits.join("\n")
        );
    }
}
