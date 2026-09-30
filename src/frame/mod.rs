//! Cabinet frame: bezel art behind the letterboxed game and, on portrait
//! displays, a marquee band above it. See docs/conventions.md, "Cabinet
//! frame".
//!
//! Native only. `FramePlugin` exists on every target so `main.rs` needs no
//! `cfg`, but on wasm it does nothing: there the host page draws the frame.
//! The one part compiled everywhere is [`Highlight`], the message games
//! write.

pub mod highlight;

#[cfg(not(target_arch = "wasm32"))]
pub mod art;
#[cfg(not(target_arch = "wasm32"))]
pub mod best_score;
#[cfg(not(target_arch = "wasm32"))]
pub mod bezel;
#[cfg(not(target_arch = "wasm32"))]
pub mod brightness;
#[cfg(not(target_arch = "wasm32"))]
pub mod camera;
#[cfg(not(target_arch = "wasm32"))]
pub mod caption;
#[cfg(not(target_arch = "wasm32"))]
pub mod driver;
#[cfg(not(target_arch = "wasm32"))]
pub mod layout;
#[cfg(not(target_arch = "wasm32"))]
pub mod marquee;

pub use highlight::Highlight;

use bevy::prelude::*;

/// Render layer of every frame sprite and of the frame camera, so game
/// cameras never draw the frame and the frame camera never draws the game.
pub const FRAME_LAYER: usize = 31;

/// Whether the frame runs. `GX_FRAME=off` always disables it. Under a dev
/// harness (the `autopilot` and `record` features) it is off unless
/// `GX_FRAME=on`, so footage and cover shots keep their framing.
pub fn frame_enabled(gx_frame: Option<&str>, harness: bool) -> bool {
    match gx_frame.map(str::trim) {
        Some(value) if value.eq_ignore_ascii_case("off") => false,
        Some(value) if value.eq_ignore_ascii_case("on") => true,
        _ => !harness,
    }
}

/// Draws the cabinet frame. Add it in `main.rs` only, after `DisplayPlugin`.
/// It must never be added by `GamePlugin`: the headless verifier builds
/// `GamePlugin` alone and has to stay free of it.
pub struct FramePlugin;

impl Plugin for FramePlugin {
    #[cfg(not(target_arch = "wasm32"))]
    fn build(&self, app: &mut App) {
        use crate::display::FrameInsets;
        use crate::game::states::GameState;

        // `HostBridgePlugin` registers it too; `add_message` is idempotent.
        // Registered here so the frame does not depend on plugin order.
        app.add_message::<Highlight>();
        app.init_resource::<FrameInsets>();
        let gx_frame = std::env::var("GX_FRAME").ok();
        if !frame_enabled(gx_frame.as_deref(), cfg!(feature = "autopilot")) {
            // Off: the game takes the whole window, as if no frame existed.
            app.insert_resource(FrameInsets::default());
            return;
        }

        app.insert_resource(driver::FrameRuntime::from_env())
            .insert_resource(driver::FrameGeometry::empty())
            .add_systems(
                Startup,
                (
                    camera::spawn_frame_camera,
                    art::load_frame_art,
                    bezel::spawn_bezel,
                    marquee::spawn_marquee,
                )
                    .chain(),
            )
            .add_systems(OnEnter(GameState::GameOver), driver::on_game_over)
            .add_systems(
                Update,
                (
                    camera::skip_redundant_writeback,
                    driver::apply_layout,
                    driver::step_frame,
                    bezel::layout_bezel,
                    bezel::paint_bezel,
                    marquee::layout_marquee,
                    marquee::paint_marquee,
                )
                    .chain(),
            );
    }

    #[cfg(target_arch = "wasm32")]
    fn build(&self, _app: &mut App) {}
}

#[cfg(test)]
mod tests {
    use bevy::input::InputPlugin;
    use bevy::state::app::StatesPlugin;

    use super::*;

    #[test]
    fn off_disables_the_frame() {
        assert!(!frame_enabled(Some("off"), false));
        assert!(!frame_enabled(Some(" OFF "), false));
        assert!(!frame_enabled(Some("off"), true));
    }

    #[test]
    fn the_frame_is_on_by_default() {
        assert!(frame_enabled(None, false));
        assert!(frame_enabled(Some(""), false));
        assert!(frame_enabled(Some("anything"), false));
    }

    #[test]
    fn dev_harnesses_need_to_ask_for_the_frame() {
        assert!(!frame_enabled(None, true));
        assert!(frame_enabled(Some("on"), true));
        assert!(frame_enabled(Some("ON"), true));
    }

    /// Every camera query outside `src/frame/` and `src/display.rs` must
    /// exclude the frame camera, or a second `Camera2d` breaks `.single()`
    /// and moves the bezel along with the game camera.
    ///
    /// The scan is literal, not a parser. A line is a camera query when it
    /// contains one of the camera patterns below and a `Query<`, `Single<` or
    /// `Populated<` appears on it or on one of the three lines before it.
    /// It passes when `FrameCamera` appears on that line or the two after,
    /// or when the line has `// frame-camera-ok: <reason>`. Limits: it
    /// cannot see a query type split over more than four lines, a camera
    /// type behind an alias, or a query built in a macro; comment lines are
    /// skipped.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn camera_queries_exclude_the_frame_camera() {
        use std::path::Path;

        const PATTERNS: [&str; 7] = [
            "With<Camera2d>",
            "With<Camera3d>",
            "With<Camera>",
            "&Camera,",
            "&Camera)",
            "&mut Camera,",
            "&mut Camera)",
        ];
        const KEYWORDS: [&str; 3] = ["Query<", "Single<", "Populated<"];

        fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    walk(&path, out);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    out.push(path);
                }
            }
        }

        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        walk(&src, &mut files);
        let mut offenders = Vec::new();
        for path in files {
            let rel = path.strip_prefix(&src).unwrap();
            if rel.starts_with("frame") || rel == Path::new("display.rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            let lines: Vec<&str> = text.lines().collect();
            for (i, line) in lines.iter().enumerate() {
                if line.trim_start().starts_with("//") || line.contains("frame-camera-ok:") {
                    continue;
                }
                if !PATTERNS.iter().any(|p| line.contains(p)) {
                    continue;
                }
                let start = i.saturating_sub(3);
                let in_query = lines[start..=i]
                    .iter()
                    .any(|l| KEYWORDS.iter().any(|k| l.contains(k)));
                let end = (i + 2).min(lines.len() - 1);
                let excluded = lines[i..=end].iter().any(|l| l.contains("FrameCamera"));
                if in_query && !excluded {
                    offenders.push(format!("{}:{}: {}", rel.display(), i + 1, line.trim()));
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "camera queries that do not exclude FrameCamera:\n{}",
            offenders.join("\n")
        );
    }

    /// The verifier's app. Nothing of the frame may be in it.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn the_headless_build_has_no_frame() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(StatesPlugin)
            .add_plugins(InputPlugin)
            .add_plugins(crate::game::GamePlugin { headless: true });
        app.update();
        let world = app.world_mut();
        assert!(!world.contains_resource::<driver::FrameRuntime>());
        assert!(!world.contains_resource::<driver::FrameGeometry>());
        assert!(!world.contains_resource::<art::FrameArt>());
        let mut cameras = world.query_filtered::<Entity, With<crate::display::FrameCamera>>();
        assert_eq!(cameras.iter(world).count(), 0);
    }
}
