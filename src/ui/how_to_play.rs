use bevy::prelude::*;

use crate::game::audio::SfxEvent;
use crate::game::input::GameInput;
use crate::game::states::GameState;
use crate::ui::card::{S1, S2, S4, card_with};
use crate::ui::fonts::{BODY, DISPLAY};
use crate::ui::menu::PROMPT_PULSE;
use crate::ui::theme::{MUTED, NIGHT, PAPER, RAIL, SIGNAL, TITLE_SHADOW};
use crate::ui::transition::ScreenFade;

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

/// Screen header. TEMPLATE NOTE: a themed header ("PACKING SLIP", "SITE
/// BRIEFING").
pub const HEADLINE: &str = "HOW TO PLAY";
pub const LAUNCH_PROMPT: &str = "PRESS ENTER TO START";

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
/// camera at (0, 0, 20) looking at the origin. The view there is 16.6 world
/// units tall at every ratio; its width follows the game's ratio, and a
/// 170 px label is 3.9 units wide, so keep item centres within
/// |x| <= 9.0 at 4:3, 6.0 at 1:1 and 4.0 at 3:4.
pub fn spawn_how_to_play(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // A vignette that frames the showcase without covering it (UI draws
    // over the 3D view, so the centre stays clear).
    commands.spawn((
        HowToPlayScreen,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        BackgroundGradient::from(RadialGradient {
            position: UiPosition::CENTER,
            shape: RadialGradientShape::FarthestCorner,
            stops: vec![
                ColorStop::new(NIGHT.with_alpha(0.0), Val::Percent(35.0)),
                ColorStop::new(NIGHT.with_alpha(0.85), Val::Percent(100.0)),
            ],
            ..default()
        }),
    ));

    // Headline and launch prompt.
    commands
        .spawn((
            HowToPlayScreen,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::axes(Val::Px(0.0), Val::Px(S4 + S1)),
                ..default()
            },
        ))
        .with_children(|screen| {
            screen.spawn((
                Text::new(HEADLINE),
                DISPLAY.fitted(HEADLINE, 52.0),
                TextColor(PAPER),
                TextShadow {
                    offset: Vec2::new(0.0, 5.0),
                    color: TITLE_SHADOW,
                },
            ));
            screen
                .spawn(card_with(
                    &RAIL,
                    Node {
                        padding: UiRect::axes(Val::Px(S4), Val::Px(S2)),
                        ..default()
                    },
                ))
                .with_child((
                    Text::new(LAUNCH_PROMPT),
                    BODY.font(26.0),
                    TextColor(PAPER),
                    PROMPT_PULSE,
                ));
        });

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
            (Text::new("YOUR ITEM"), BODY.font(18.0), TextColor(SIGNAL),),
            (
                Text::new("Describe it here"),
                BODY.font(15.0),
                TextColor(MUTED),
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
    // The cabinet frame adds a second camera; `Without<FrameCamera>` keeps
    // this query on the game camera. See "Cabinet frame" in docs/conventions.md.
    camera_q: Query<
        (&Camera, &GlobalTransform),
        (With<Camera3d>, Without<crate::frame::FrameCamera>),
    >,
    ui_scale: Res<UiScale>,
    items: Query<&GlobalTransform, (With<Spin>, Without<Camera3d>)>,
    mut labels: Query<(&ItemLabel, &mut Node)>,
) {
    let Ok((camera, cam_tf)) = camera_q.single() else {
        return;
    };
    let view_min = camera
        .logical_viewport_rect()
        .map_or(Vec2::ZERO, |rect| rect.min);
    for (label, mut node) in &mut labels {
        let Ok(item_tf) = items.get(label.target) else {
            continue;
        };
        // Anchor just below the item so the label clears the mesh.
        let anchor = item_tf.translation() - Vec3::Y * 1.6;
        let Ok(item_px) = camera.world_to_viewport(cam_tf, anchor) else {
            continue;
        };
        // Window pixels to UI pixels inside the letterboxed viewport.
        let origin = crate::display::label_origin(item_px, view_min, ui_scale.0, LABEL_WIDTH);
        node.left = Val::Px(origin.x);
        node.top = Val::Px(origin.y);
    }
}

/// Fires a ScreenSweep SFX when the how-to-play screen is entered.
pub fn sweep_on_enter(mut sfx: MessageWriter<SfxEvent>) {
    sfx.write(SfxEvent::ScreenSweep);
}

/// Launch input: the canon Confirm action (Enter / Space / Z, any face
/// button, pad Start or A), gated on the fade being idle.
pub fn how_to_play_input(
    input: Res<GameInput>,
    mut fade: ResMut<ScreenFade>,
    mut sfx: MessageWriter<SfxEvent>,
) {
    if !fade.is_idle() {
        return;
    }
    if input.confirm_just_pressed && fade.request(GameState::Playing) {
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
