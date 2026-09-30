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
