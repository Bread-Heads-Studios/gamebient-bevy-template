//! The frame's own camera. It draws first (`order: -1`) over the whole
//! window; game cameras draw after it, inside the viewport `DisplayPlugin`
//! gives them.

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;

use crate::display::FrameCamera;
use crate::frame::FRAME_LAYER;

/// `Startup`: the camera that draws the frame. `Msaa::Off` keeps it cheap. A
/// non-HDR game camera with `Msaa::Off` shares its main textures with this
/// one; that is harmless for a camera that clears, which overwrites them.
pub fn spawn_frame_camera(mut commands: Commands) {
    commands.spawn((
        FrameCamera,
        Camera2d,
        Camera {
            order: -1,
            clear_color: ClearColorConfig::Custom(Color::BLACK),
            ..default()
        },
        Msaa::Off,
        RenderLayers::layer(FRAME_LAYER),
    ));
}

/// With the frame camera in front of it, a game camera is no longer the
/// first on the window, so Bevy's default (`MsaaWriteback::Auto`) adds a
/// full-window blit to it every frame. A camera that clears throws that
/// blit away at once; turning it off saves the fill on the Pi. A camera
/// that does not clear (an overlay) depends on the blit and is left alone.
/// Runs on cameras that are added or changed, so a camera whose
/// `clear_color` changes later is noticed; it writes only when the value
/// would change, so it does not re-trigger itself.
pub fn skip_redundant_writeback(
    mut cameras: Query<&mut Camera, (Changed<Camera>, Without<FrameCamera>)>,
) {
    for mut camera in &mut cameras {
        let clears = !matches!(camera.clear_color, ClearColorConfig::None);
        if clears && camera.msaa_writeback == MsaaWriteback::Auto {
            camera.msaa_writeback = MsaaWriteback::Off;
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;

    use super::*;

    #[test]
    fn the_frame_camera_draws_first_on_its_own_layer() {
        let mut world = World::new();
        world.run_system_once(spawn_frame_camera).unwrap();
        let mut query =
            world.query_filtered::<(&Camera, &RenderLayers, &Msaa), With<FrameCamera>>();
        let (camera, layers, msaa) = query.single(&world).unwrap();
        assert_eq!(camera.order, -1);
        assert!(
            camera.viewport.is_none(),
            "the frame covers the whole window"
        );
        assert_eq!(*layers, RenderLayers::layer(FRAME_LAYER));
        assert_eq!(*msaa, Msaa::Off);
    }

    #[test]
    fn a_later_clear_colour_change_is_noticed() {
        let mut world = World::new();
        let mut schedule = Schedule::default();
        schedule.add_systems(skip_redundant_writeback);
        let cam = world
            .spawn(Camera {
                clear_color: ClearColorConfig::None,
                ..default()
            })
            .id();
        schedule.run(&mut world);
        assert_eq!(
            world.get::<Camera>(cam).unwrap().msaa_writeback,
            MsaaWriteback::Auto
        );
        world.get_mut::<Camera>(cam).unwrap().clear_color = ClearColorConfig::Default;
        schedule.run(&mut world);
        assert_eq!(
            world.get::<Camera>(cam).unwrap().msaa_writeback,
            MsaaWriteback::Off
        );
    }

    #[test]
    fn writeback_is_dropped_only_where_the_clear_would_discard_it() {
        let mut world = World::new();
        let clears = world.spawn(Camera::default()).id();
        let overlay = world
            .spawn(Camera {
                clear_color: ClearColorConfig::None,
                ..default()
            })
            .id();
        let frame = world.spawn((Camera::default(), FrameCamera)).id();
        world.run_system_once(skip_redundant_writeback).unwrap();
        let writeback = |e: Entity| world.get::<Camera>(e).unwrap().msaa_writeback;
        assert_eq!(writeback(clears), MsaaWriteback::Off);
        assert_eq!(writeback(overlay), MsaaWriteback::Auto);
        assert_eq!(writeback(frame), MsaaWriteback::Auto);
    }
}
