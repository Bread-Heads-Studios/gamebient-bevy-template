use bevy::prelude::*;

use crate::game::states::GameState;
use crate::ui::transition::ScreenFade;

/// Marker for the studio logo screen root node.
#[derive(Component)]
pub struct StudioLogoRoot;

/// Auto-advance timer: covers the 0.6 s boot fade-in plus a ~1.6 s hold.
#[derive(Resource)]
pub struct StudioLogoTimer(pub Timer);

/// Spawns the "BREAD HEADS STUDIOS PRESENTS" boot screen.
pub fn spawn_studio_logo(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(StudioLogoTimer(Timer::from_seconds(2.2, TimerMode::Once)));
    commands.spawn((
        StudioLogoRoot,
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            row_gap: Val::Px(28.0),
            ..default()
        },
        BackgroundColor(Color::BLACK),
        children![
            (
                ImageNode::new(asset_server.load("breadheads_logo.png")),
                Node {
                    width: Val::Px(220.0),
                    height: Val::Px(220.0),
                    ..default()
                },
            ),
            (
                Text::new("BREAD HEADS STUDIOS"),
                TextFont {
                    font_size: 30.0,
                    ..default()
                },
                TextColor(Color::srgb(0.92, 0.9, 0.88)),
            ),
            (
                Text::new("PRESENTS"),
                TextFont {
                    font_size: 16.0,
                    ..default()
                },
                TextColor(Color::srgb(0.5, 0.48, 0.45)),
            ),
        ],
    ));
}

/// Auto-advances to the title screen when the timer elapses; any key or
/// gamepad button skips immediately.
pub fn advance_studio_logo(
    time: Res<Time>,
    mut timer: ResMut<StudioLogoTimer>,
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    mut fade: ResMut<ScreenFade>,
) {
    timer.0.tick(time.delta());
    let skip = keyboard.get_just_pressed().next().is_some()
        || gamepads
            .iter()
            .any(|g| g.get_just_pressed().next().is_some());
    if latch_and_should_advance(&mut timer.0, skip) {
        let _ = fade.request_with(GameState::Menu, 0.6);
    }
}

/// Pure decision core of [`advance_studio_logo`]: a skip latches the timer
/// to finished (set_elapsed + zero-tick so `is_finished()` flips this frame),
/// so the advance re-fires every frame until the fade accepts the request.
fn latch_and_should_advance(timer: &mut Timer, skip: bool) -> bool {
    if skip {
        let duration = timer.duration();
        timer.set_elapsed(duration);
        timer.tick(std::time::Duration::ZERO);
    }
    timer.is_finished()
}

pub fn despawn_studio_logo(mut commands: Commands, query: Query<Entity, With<StudioLogoRoot>>) {
    commands.remove_resource::<StudioLogoTimer>();
    for entity in &query {
        commands.entity(entity).despawn();
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn young_timer_without_skip_does_not_advance() {
        let mut timer = Timer::from_seconds(2.2, TimerMode::Once);
        timer.tick(Duration::from_millis(500));
        assert!(!latch_and_should_advance(&mut timer, false));
    }

    #[test]
    fn elapsed_timer_advances_without_skip() {
        let mut timer = Timer::from_seconds(2.2, TimerMode::Once);
        timer.tick(Duration::from_millis(2300));
        assert!(latch_and_should_advance(&mut timer, false));
    }

    #[test]
    fn skip_latches_young_timer_and_stays_latched() {
        let mut timer = Timer::from_seconds(2.2, TimerMode::Once);
        timer.tick(Duration::from_millis(100));
        // Skip pressed while the timer is young: advance fires this frame...
        assert!(latch_and_should_advance(&mut timer, true));
        // ...and keeps firing on later frames without the skip being held,
        // so a request rejected during the boot fade-in is retried until
        // the fade accepts it.
        timer.tick(Duration::from_millis(16));
        assert!(latch_and_should_advance(&mut timer, false));
    }
}
