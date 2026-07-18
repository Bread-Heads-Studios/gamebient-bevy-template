use bevy::prelude::*;

use crate::game::states::GameState;
use crate::ui::how_to_play::{SeenHowToPlay, start_target};
use crate::ui::transition::{Pulse, ScreenFade};

#[derive(Component)]
pub struct MenuRoot;

/// Spawns the title screen: styled text title with a pulsing start prompt
/// over a bottom scrim.
///
/// TEMPLATE NOTE — artwork variant: if your game has full-bleed title art
/// (logotype baked in, like Voidrunner/Hunted), replace the title text with
/// a cover-fit image and preload the handle at Startup so it never pops in:
///
/// ```ignore
/// #[derive(Resource)]
/// pub struct TitleArtwork(pub Handle<Image>);
/// pub fn preload_title_artwork(mut commands: Commands, assets: Res<AssetServer>) {
///     commands.insert_resource(TitleArtwork(assets.load("title.png")));
/// }
/// // In spawn_menu (root gets Overflow::clip()):
/// //   (ImageNode::new(artwork.0.clone()),
/// //    Node { width: Val::Percent(100.0), flex_shrink: 0.0, ..default() }),
/// // and register preload_title_artwork in the plugin's Startup set.
/// ```
pub fn spawn_menu(mut commands: Commands) {
    commands.spawn((
        MenuRoot,
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            row_gap: Val::Px(20.0),
            ..default()
        },
        BackgroundColor(Color::srgb(0.01, 0.02, 0.05)),
        children![
            (
                Text::new("GAMEBIENT GAME"),
                TextFont {
                    font_size: 72.0,
                    ..default()
                },
                TextColor(Color::srgb(0.2, 0.8, 1.0)),
            ),
            // Bottom strip: scrim holding the pulsing prompt + controls line.
            (
                Node {
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(10.0),
                    padding: UiRect::axes(Val::Px(0.0), Val::Px(24.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
                children![
                    (
                        Text::new("PRESS ENTER TO START"),
                        TextFont {
                            font_size: 28.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.9, 0.95, 1.0)),
                        Pulse {
                            speed: 3.0,
                            min: 0.25,
                            max: 1.0,
                        },
                    ),
                    (
                        Text::new("Arrows / WASD: Move  |  Z: A  X: B  |  Esc: Pause"),
                        TextFont {
                            font_size: 18.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.5, 0.6, 0.7)),
                    ),
                ],
            ),
        ],
    ));
}

/// Spawns the game-over screen.
pub fn spawn_game_over(mut commands: Commands) {
    commands.spawn((
        MenuRoot,
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            row_gap: Val::Px(20.0),
            ..default()
        },
        BackgroundColor(Color::srgb(0.05, 0.01, 0.02)),
        children![
            (
                Text::new("GAME OVER"),
                TextFont {
                    font_size: 64.0,
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.3, 0.2)),
            ),
            (
                Text::new("PRESS ENTER TO CONTINUE"),
                TextFont {
                    font_size: 24.0,
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
}

pub fn despawn_menu(mut commands: Commands, query: Query<Entity, With<MenuRoot>>) {
    for e in &query {
        commands.entity(e).despawn();
    }
}

/// ENTER routes `Menu -> HowToPlay/Playing` (once-per-session gate) and
/// `GameOver -> Menu`, always through the fade.
pub fn menu_input(
    input: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    state: Res<State<GameState>>,
    seen: Res<SeenHowToPlay>,
    mut fade: ResMut<ScreenFade>,
) {
    if !fade.is_idle() {
        return;
    }
    let mut confirm = input.any_just_pressed([KeyCode::Enter, KeyCode::Space, KeyCode::KeyZ]);
    if !confirm {
        for gamepad in &gamepads {
            if gamepad.just_pressed(GamepadButton::South)
                || gamepad.just_pressed(GamepadButton::North)
                || gamepad.just_pressed(GamepadButton::West)
                || gamepad.just_pressed(GamepadButton::East)
            {
                confirm = true;
                break;
            }
        }
    }
    if !confirm {
        return;
    }
    match state.get() {
        GameState::Menu => {
            let _ = fade.request(start_target(seen.0));
        }
        GameState::GameOver => {
            let _ = fade.request(GameState::Menu);
        }
        _ => {}
    }
}
