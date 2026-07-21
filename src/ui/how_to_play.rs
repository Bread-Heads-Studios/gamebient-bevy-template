use bevy::prelude::*;

use crate::game::audio::SfxEvent;
use crate::game::states::GameState;
use crate::ui::transition::{Pulse, ScreenFade};

/// Set once the player has seen the how-to-play screen this session; never
/// reset, so the screen shows exactly once per boot.
#[derive(Resource, Default)]
pub struct SeenHowToPlay(pub bool);

/// Where the title screen's start action goes: the how-to-play screen on the
/// first run of a session, straight into gameplay afterwards.
pub fn start_target(seen_how_to_play: bool) -> GameState {
    if seen_how_to_play {
        GameState::Playing
    } else {
        GameState::HowToPlay
    }
}

/// Marker for every entity (3D and UI) spawned by the how-to-play screen.
#[derive(Component)]
pub struct HowToPlayScreen;

/// Slow Y-axis rotation for showcase items.
#[derive(Component)]
pub struct Spin;

/// UI label pinned each frame to a showcase item's screen position.
#[derive(Component)]
pub struct ItemLabel {
    pub target: Entity,
}

const LABEL_WIDTH: f32 = 170.0;

/// Marks the how-to-play screen as seen for the rest of the session.
pub fn mark_seen(mut seen: ResMut<SeenHowToPlay>) {
    seen.0 = true;
}

/// Spawns the showcase and screen chrome.
///
/// TEMPLATE NOTE: replace the placeholder item below with your game's real
/// entities — spawn each one from the same meshes/materials gameplay uses so
/// the key always matches what players see, give it `HowToPlayScreen + Spin`,
/// and pair it with a label. Grid positions sit in front of the Startup
/// camera at (0, 0, 20) looking at the origin.
pub fn spawn_how_to_play(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Headline and launch prompt.
    commands.spawn((
        HowToPlayScreen,
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            padding: UiRect::axes(Val::Px(0.0), Val::Px(40.0)),
            ..default()
        },
        children![
            (
                Text::new("HOW TO PLAY"),
                TextFont {
                    font_size: 48.0,
                    ..default()
                },
                TextColor(Color::srgb(0.2, 0.8, 1.0)),
            ),
            (
                Text::new("PRESS ENTER TO START"),
                TextFont {
                    font_size: 26.0,
                    ..default()
                },
                TextColor(Color::srgb(0.9, 0.95, 1.0)),
                Pulse {
                    speed: 3.0,
                    min: 0.25,
                    max: 1.0,
                },
            ),
        ],
    ));

    // Placeholder showcase item: swap for your game's real entities.
    let item = commands
        .spawn((
            HowToPlayScreen,
            Spin,
            Mesh3d(meshes.add(Cuboid::new(1.5, 1.5, 1.5))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(0.2, 0.8, 1.0),
                emissive: LinearRgba::new(0.1, 0.6, 1.0, 1.0),
                ..default()
            })),
            Transform::from_xyz(0.0, 1.0, 0.0),
        ))
        .id();
    commands.spawn((
        HowToPlayScreen,
        ItemLabel { target: item },
        Node {
            position_type: PositionType::Absolute,
            width: Val::Px(LABEL_WIDTH),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(2.0),
            ..default()
        },
        children![
            (
                Text::new("YOUR ITEM"),
                TextFont {
                    font_size: 17.0,
                    ..default()
                },
                TextColor(Color::srgb(0.2, 0.8, 1.0)),
            ),
            (
                Text::new("Describe it here"),
                TextFont {
                    font_size: 13.0,
                    ..default()
                },
                TextColor(Color::srgb(0.55, 0.6, 0.7)),
            ),
        ],
    ));
}

/// Slowly rotates showcase items around Y.
pub fn spin_items(time: Res<Time>, mut query: Query<&mut Transform, With<Spin>>) {
    for mut tf in &mut query {
        tf.rotate_y(0.6 * time.delta_secs());
    }
}

/// Pins each label under its 3D item by projecting the item's position to
/// viewport coordinates. Runs every frame so labels stay correct on resize.
///
/// Labels are placed from default (identity) GlobalTransforms on the first
/// frame after state entry; this is invisible only because HowToPlay is
/// always entered through a near-black ScreenFade. Keep it that way.
pub fn position_labels(
    camera_q: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    ui_scale: Res<UiScale>,
    items: Query<&GlobalTransform, (With<Spin>, Without<Camera3d>)>,
    mut labels: Query<(&ItemLabel, &mut Node)>,
) {
    let Ok((camera, cam_tf)) = camera_q.single() else {
        return;
    };
    for (label, mut node) in &mut labels {
        let Ok(item_tf) = items.get(label.target) else {
            continue;
        };
        // Anchor just below the item so the label clears the mesh.
        let anchor = item_tf.translation() - Vec3::Y * 1.6;
        let Ok(viewport) = camera.world_to_viewport(cam_tf, anchor) else {
            continue;
        };
        // Node Px values get multiplied by UiScale at layout time; divide so
        // the node lands on the viewport-pixel position we computed.
        node.left = Val::Px(viewport.x / ui_scale.0 - LABEL_WIDTH / 2.0);
        node.top = Val::Px(viewport.y / ui_scale.0);
    }
}

/// Fires a ScreenSweep SFX when the how-to-play screen is entered.
pub fn sweep_on_enter(mut sfx: MessageWriter<SfxEvent>) {
    sfx.write(SfxEvent::ScreenSweep);
}

/// Launch input: Enter/Space or gamepad South, gated on the fade being idle.
pub fn how_to_play_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    mut fade: ResMut<ScreenFade>,
    mut sfx: MessageWriter<SfxEvent>,
) {
    if !fade.is_idle() {
        return;
    }
    let mut start = keyboard.any_just_pressed([KeyCode::Enter, KeyCode::Space, KeyCode::KeyZ]);
    if !start {
        for gamepad in &gamepads {
            if gamepad.just_pressed(GamepadButton::South)
                || gamepad.just_pressed(GamepadButton::North)
                || gamepad.just_pressed(GamepadButton::West)
                || gamepad.just_pressed(GamepadButton::East)
            {
                start = true;
                break;
            }
        }
    }
    if start && fade.request(GameState::Playing) {
        sfx.write(SfxEvent::Confirm);
    }
}

/// Despawns everything the how-to-play screen spawned.
pub fn despawn_how_to_play(mut commands: Commands, query: Query<Entity, With<HowToPlayScreen>>) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_start_shows_how_to_play_then_skips_it() {
        assert_eq!(start_target(false), GameState::HowToPlay);
        assert_eq!(start_target(true), GameState::Playing);
    }
}
