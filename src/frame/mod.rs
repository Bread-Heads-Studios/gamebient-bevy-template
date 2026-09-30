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

/// The frame camera's marker, re-exported on every target so game code can
/// write `Without<crate::frame::FrameCamera>` in a camera query. Naming the
/// marker is not a read of display state.
pub use crate::display::FrameCamera;

use bevy::prelude::*;

/// Render layer of every frame sprite and of the frame camera, so game
/// cameras never draw the frame and the frame camera never draws the game.
pub const FRAME_LAYER: usize = 31;

/// Whether the frame runs. `GX_FRAME` = `off`, `0`, `false` or `no` (any case)
/// always disables it; `on`, `1`, `true` or `yes` enables it; anything else
/// counts as unset. Under a dev
/// harness (the `capture` feature, which `autopilot` and `record` imply) it
/// is off unless `GX_FRAME=on`, so footage and cover shots keep their framing.
pub fn frame_enabled(gx_frame: Option<&str>, harness: bool) -> bool {
    match gx_frame.map(str::trim) {
        Some(value)
            if ["off", "0", "false", "no"]
                .iter()
                .any(|w| value.eq_ignore_ascii_case(w)) =>
        {
            false
        }
        Some(value)
            if ["on", "1", "true", "yes"]
                .iter()
                .any(|w| value.eq_ignore_ascii_case(w)) =>
        {
            true
        }
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

        // `HostBridgePlugin` registers it too; `add_message` is idempotent.
        // Registered here so the frame does not depend on plugin order.
        app.add_message::<Highlight>();
        app.init_resource::<FrameInsets>();
        let gx_frame = std::env::var("GX_FRAME").ok();
        if !frame_enabled(gx_frame.as_deref(), cfg!(feature = "capture")) {
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
            .add_systems(
                Update,
                (
                    camera::skip_redundant_writeback,
                    driver::apply_layout,
                    driver::watch_game_over,
                    driver::finish_pending_run,
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
    fn the_usual_off_and_on_words_are_understood() {
        for off in ["0", "false", "FALSE", "no", " No "] {
            assert!(!frame_enabled(Some(off), false), "{off}");
            assert!(!frame_enabled(Some(off), true), "{off}");
        }
        for on in ["1", "true", "True", "yes", "YES"] {
            assert!(frame_enabled(Some(on), false), "{on}");
            assert!(frame_enabled(Some(on), true), "{on}");
        }
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
    /// The scan is literal, not a parser; the rule is in
    /// [`camera_query_flagged`]. Limits: it cannot see a query type split over
    /// more than four lines, a camera type behind an alias, or a query built
    /// in a macro.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn camera_queries_exclude_the_frame_camera() {
        use std::path::Path;

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
                if camera_query_flagged(&lines, i) {
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

    /// The camera-query scan's rule for line `i` of `lines`. A line is
    /// flagged when it is a camera query that could match the frame's
    /// `Camera2d` with no other required marker, and neither the line nor
    /// the two after it name `FrameCamera`:
    /// - it has `With<Camera2d>` or `With<Camera>`, or
    /// - it has `&Camera` or `&mut Camera` (followed by `,`, `)` or `>`) and
    ///   the `Query<...>`/`Single<...>`/`Populated<...>` that holds it has no
    ///   `With<` naming some other type (`With<Camera3d>`, a game's own
    ///   marker). Only a `With<` inside that same query counts: one in the
    ///   next parameter's query does not narrow this one.
    ///
    /// A query keyword (`Query<`, `Single<`, `Populated<`) must appear on the
    /// line or the three before. Comment lines and lines carrying
    /// `// frame-camera-ok: <reason>` are never flagged.
    fn camera_query_flagged(lines: &[&str], i: usize) -> bool {
        const KEYWORDS: [&str; 3] = ["Query<", "Single<", "Populated<"];
        let line = lines[i];
        if line.trim_start().starts_with("//") || line.contains("frame-camera-ok:") {
            return false;
        }
        let end = (i + 2).min(lines.len() - 1);
        let near = &lines[i..=end];
        let direct = line.contains("With<Camera2d>") || line.contains("With<Camera>");

        // The lines from three before to four after, joined, and where line
        // `i` starts in them.
        let start = i.saturating_sub(3);
        let window = lines[start..=(i + 4).min(lines.len() - 1)].join("\n");
        let line_at: usize = lines[start..i].iter().map(|l| l.len() + 1).sum();
        // Whether the query holding the camera type at `pos` has a `With<`
        // naming another type: scan from its opening `<` to its closing `>`.
        let narrowed_at = |pos: usize| -> bool {
            let Some(open) = KEYWORDS
                .iter()
                .filter_map(|k| window[..pos].rfind(k).map(|p| p + k.len()))
                .max()
            else {
                return false;
            };
            let mut depth = 1usize;
            for (at, c) in window[open..].char_indices() {
                let at = open + at;
                match c {
                    '<' => {
                        depth += 1;
                        let rest = &window[at + 1..];
                        if window[..at].ends_with("With")
                            && !(rest.starts_with("Camera>") || rest.starts_with("Camera2d>"))
                        {
                            return true;
                        }
                    }
                    '>' => {
                        depth -= 1;
                        if depth == 0 {
                            return false;
                        }
                    }
                    _ => {}
                }
            }
            false
        };
        let by_ref = ["&Camera", "&mut Camera"].iter().any(|needle| {
            line.match_indices(needle).any(|(at, _)| {
                matches!(
                    line[at + needle.len()..].chars().next(),
                    Some(',' | ')' | '>')
                ) && !narrowed_at(line_at + at)
            })
        });
        if !(direct || by_ref) {
            return false;
        }
        let in_query = lines[start..=i]
            .iter()
            .any(|l| KEYWORDS.iter().any(|k| l.contains(k)));
        let excluded = near.iter().any(|l| l.contains("FrameCamera"));
        in_query && !excluded
    }

    /// Whether line 0 of `src` is flagged by the camera-query scan.
    fn flagged(src: &str) -> bool {
        let lines: Vec<&str> = src.lines().collect();
        camera_query_flagged(&lines, 0)
    }

    /// Whether line `n` of `src` is flagged by the camera-query scan.
    fn flagged_at(src: &str, n: usize) -> bool {
        let lines: Vec<&str> = src.lines().collect();
        camera_query_flagged(&lines, n)
    }

    /// Pizza Pinball's `position_labels`: a bare camera query, then an
    /// unrelated query with a `With<>` of its own two lines later.
    #[test]
    fn a_with_in_the_next_query_does_not_narrow_a_bare_camera_query() {
        let src = "pub fn position_labels(\n    camera_q: Query<(&Camera, &GlobalTransform)>,\n    ui_scale: Res<UiScale>,\n    items: Query<&GlobalTransform, With<Spin>>,\n) {}";
        assert!(flagged_at(src, 1));
        // The same filter inside the camera's own multi-line query narrows it.
        let own = "fn f(\n    c: Query<\n        (&Camera, &GlobalTransform),\n        With<MainCamera>,\n    >,\n    items: Query<&GlobalTransform, With<Spin>>,\n) {}";
        assert!(!flagged_at(own, 2));
        // And a tuple filter on the same line.
        assert!(!flagged_at(
            "fn f(c: Query<&Camera, (With<A>, Without<B>)>) {}",
            0
        ));
    }

    #[test]
    fn the_scan_flags_only_queries_that_can_match_the_frame_camera() {
        assert!(flagged(
            "fn f(c: Single<&mut Transform, With<Camera2d>>) {}"
        ));
        assert!(flagged("fn f(c: Query<(&Camera, &GlobalTransform)>) {}"));
        assert!(flagged("fn f(c: Single<&Camera>) {}"));
        assert!(flagged("fn f(c: Single<&mut Camera>) {}"));
        assert!(flagged("fn f(c: Query<Entity, With<Camera>>) {}"));
        assert!(!flagged(
            "fn f(c: Query<&mut Transform, With<Camera3d>>) {}"
        ));
        assert!(!flagged(
            "fn f(c: Query<(&Camera, &GlobalTransform), With<MainCamera>>) {}"
        ));
        assert!(!flagged(
            "fn f(c: Query<(&Camera, &GlobalTransform),\n    With<MainCamera>>) {}"
        ));
        assert!(!flagged("fn f(c: Query<&CameraRig, With<Foo>>) {}"));
        assert!(!flagged(
            "fn f(c: Single<&mut Transform, With<Camera2d>>) {} // frame-camera-ok: shakes all"
        ));
        assert!(!flagged("// Single<&mut Transform, With<Camera2d>>"));
        assert!(!flagged(
            "fn f(c: Single<&mut Transform, With<Camera2d>>\n    Without<FrameCamera>) {}"
        ));
        assert!(!flagged(
            "fn f(c: Query<(&Camera, &GlobalTransform)>,\n    Without<FrameCamera>) {}"
        ));
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
