use bevy::prelude::*;

pub mod input;
pub mod player;
pub mod scoring;
pub mod states;

use states::GameState;

/// Marker for entities spawned during a run; despawned on cleanup.
#[derive(Component)]
pub struct GameEntity;

/// Owns the game state machine, core resources, and gameplay systems.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>()
            .init_resource::<input::GameInput>()
            .add_systems(PreUpdate, input::collect_input)
            .init_resource::<scoring::GameData>()
            .add_message::<scoring::ScoreEvent>()
            .add_systems(Startup, setup_scene)
            .add_systems(OnEnter(GameState::Playing), player::spawn_player)
            .add_systems(
                Update,
                (player::move_player, scoring::handle_score_events)
                    .run_if(in_state(GameState::Playing).and(states::not_paused)),
            )
            .add_systems(OnExit(GameState::Playing), cleanup_game_entities)
            .init_resource::<states::Paused>()
            .add_systems(OnEnter(GameState::Playing), (reset_paused, reset_game_data))
            .add_systems(
                Update,
                (toggle_pause, pause_quit)
                    .chain()
                    .run_if(in_state(GameState::Playing)),
            )
            .add_systems(OnExit(GameState::Playing), cleanup_pause_overlay);
    }
}

/// One-time scene setup: a camera and a directional light.
fn setup_scene(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 0.0, 20.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        DirectionalLight::default(),
        Transform::from_xyz(4.0, 8.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

/// Despawns all run entities when leaving `Playing`.
fn cleanup_game_entities(mut commands: Commands, query: Query<Entity, With<GameEntity>>) {
    for e in &query {
        commands.entity(e).despawn();
    }
}

/// Marker for the "PAUSED" overlay so it can be despawned on unpause.
#[derive(Component)]
struct PauseOverlay;

/// Resets pause state when a run starts.
fn reset_paused(mut paused: ResMut<states::Paused>) {
    paused.0 = false;
}

/// Resets per-run data when a run starts (preserving the cross-run high score).
fn reset_game_data(mut data: ResMut<scoring::GameData>) {
    let high_score = data.high_score;
    *data = scoring::GameData::default();
    data.high_score = high_score;
}

/// Despawns the pause overlay when leaving Playing (e.g. quit while paused).
fn cleanup_pause_overlay(mut commands: Commands, query: Query<Entity, With<PauseOverlay>>) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
}

/// Toggles pause on Escape or gamepad Start and shows/hides the overlay.
fn toggle_pause(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    mut paused: ResMut<states::Paused>,
    overlay_query: Query<Entity, With<PauseOverlay>>,
    fade: Res<crate::ui::transition::ScreenFade>,
) {
    if !fade.is_idle() {
        return;
    }
    let mut pressed = keyboard.just_pressed(KeyCode::Escape);
    for gamepad in &gamepads {
        if gamepad.just_pressed(GamepadButton::Start) {
            pressed = true;
            break;
        }
    }
    if !pressed {
        return;
    }

    paused.0 = !paused.0;

    if paused.0 {
        commands
            .spawn((
                PauseOverlay,
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(16.0),
                    position_type: PositionType::Absolute,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
                GlobalZIndex(100),
            ))
            .with_children(|parent| {
                parent.spawn((
                    Text::new("PAUSED"),
                    TextFont {
                        font_size: 64.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.2, 0.8, 1.0)),
                ));
                parent.spawn((
                    Text::new("ESC: RESUME"),
                    TextFont {
                        font_size: 22.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.9, 0.95, 1.0)),
                ));
                parent.spawn((
                    Text::new("ENTER: QUIT TO TITLE"),
                    TextFont {
                        font_size: 22.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.5, 0.6, 0.7)),
                ));
            });
    } else {
        for entity in &overlay_query {
            commands.entity(entity).despawn();
        }
    }
}

/// While paused, Enter (or gamepad East) quits back to the title screen
/// through the fade. Run state resets on the next Playing entry
/// (`reset_paused`); OnExit(Playing) systems handle cleanup.
fn pause_quit(
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    paused: Res<states::Paused>,
    mut fade: ResMut<crate::ui::transition::ScreenFade>,
) {
    if !paused.0 || !fade.is_idle() {
        return;
    }
    let mut quit = keyboard.just_pressed(KeyCode::Enter);
    if !quit {
        for gamepad in &gamepads {
            if gamepad.just_pressed(GamepadButton::East) {
                quit = true;
                break;
            }
        }
    }
    if quit {
        let _ = fade.request(GameState::Menu);
    }
}
