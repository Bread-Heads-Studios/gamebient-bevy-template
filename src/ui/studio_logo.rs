//! The Bread Heads Studios boot card: the one screen that is the same in
//! every game. It is synced from the template; copy it verbatim and never
//! theme it. It needs `fonts::{STUDIO, STUDIO_ITALIC}` (shared ids) and
//! `assets/breadheads_logo.png`.

use bevy::prelude::*;

use crate::game::input::GameInput;
use crate::game::states::GameState;
use crate::ui::card::{S1, S2, S3};
use crate::ui::fit::fit_font_size;
use crate::ui::fonts::{STUDIO, STUDIO_ITALIC};
use crate::ui::transition::ScreenFade;

/// Marker for the studio logo screen root node.
#[derive(Component)]
pub struct StudioLogoRoot;

/// The lockup that settles (scales from `SETTLE_FROM` to 1) over the hold.
#[derive(Component)]
pub struct StudioLogoSettle;

/// Auto-advance timer: covers the 0.6 s boot fade-in plus a ~1.6 s hold.
#[derive(Resource)]
pub struct StudioLogoTimer(pub Timer);

pub const STUDIO_NAME: &str = "BREAD HEADS STUDIOS";
pub const PRESENTS: &str = "PRESENTS";

/// Lockup scale at the start of the hold; it eases to 1.0 by the end.
pub const SETTLE_FROM: f32 = 1.02;

/// The bread palette: crust at the heart of the gradient, burnt to black.
const CRUST: Color = Color::srgb(0.60, 0.33, 0.13);
const BAKE: Color = Color::srgb(0.33, 0.16, 0.06);
const EMBER: Color = Color::srgb(0.09, 0.04, 0.015);
const CREAM: Color = Color::srgb(0.97, 0.90, 0.77);
const WHEAT: Color = Color::srgb(0.86, 0.70, 0.48);

/// Spawns the "BREAD HEADS STUDIOS PRESENTS" boot screen.
pub fn spawn_studio_logo(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(StudioLogoTimer(Timer::from_seconds(2.2, TimerMode::Once)));
    let name_size = fit_font_size(&STUDIO, STUDIO_NAME, 46.0, REFERENCE_VIEW);
    let rule = || {
        (
            Node {
                width: Val::Px(S3 + S2),
                height: Val::Px(2.0),
                ..default()
            },
            BackgroundColor(WHEAT.with_alpha(0.6)),
        )
    };
    commands.spawn((
        StudioLogoRoot,
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::BLACK),
        BackgroundGradient::from(RadialGradient {
            position: UiPosition::new(Vec2::new(0.0, -0.08), Val::ZERO, Val::ZERO),
            shape: RadialGradientShape::FarthestCorner,
            stops: vec![
                ColorStop::new(CRUST, Val::Percent(0.0)),
                ColorStop::new(BAKE, Val::Percent(30.0)),
                ColorStop::new(EMBER, Val::Percent(62.0)),
                ColorStop::new(Color::BLACK, Val::Percent(100.0)),
            ],
            ..default()
        }),
        children![(
            StudioLogoSettle,
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(S2),
                ..default()
            },
            UiTransform::from_scale(Vec2::splat(SETTLE_FROM)),
            children![
                (
                    ImageNode::new(asset_server.load("breadheads_logo.png")),
                    Node {
                        width: Val::Px(220.0),
                        height: Val::Px(220.0),
                        margin: UiRect::bottom(Val::Px(S1)),
                        border_radius: BorderRadius::MAX,
                        ..default()
                    },
                    // A warm oven glow behind the disc.
                    BoxShadow::new(
                        CRUST.with_alpha(0.55),
                        Val::Px(0.0),
                        Val::Px(0.0),
                        Val::Px(6.0),
                        Val::Px(48.0),
                    ),
                ),
                (
                    Text::new(STUDIO_NAME),
                    STUDIO.font(name_size),
                    TextColor(CREAM),
                    TextShadow {
                        offset: Vec2::new(0.0, 3.0),
                        color: Color::srgba(0.05, 0.02, 0.0, 0.7),
                    },
                ),
                (
                    Node {
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(S2),
                        ..default()
                    },
                    children![
                        rule(),
                        (
                            Text::new(PRESENTS),
                            STUDIO_ITALIC.font(24.0),
                            TextColor(WHEAT),
                        ),
                        rule(),
                    ],
                ),
            ],
        )],
    ));
}

/// Narrowest UI width any game has (1:1 and 3:4), so the studio card is the
/// same size in every game.
const REFERENCE_VIEW: f32 = 720.0;

/// Auto-advances to the title screen when the timer elapses; any key or
/// gamepad button skips immediately. Also settles the lockup's scale over
/// the hold (a `UiTransform`, never `font_size`).
pub fn advance_studio_logo(
    time: Res<Time>,
    mut timer: ResMut<StudioLogoTimer>,
    input: Res<GameInput>,
    mut fade: ResMut<ScreenFade>,
    mut settle: Query<&mut UiTransform, With<StudioLogoSettle>>,
) {
    timer.0.tick(time.delta());
    for mut transform in &mut settle {
        transform.scale = Vec2::splat(settle_scale(timer.0.fraction()));
    }
    // Any canon button (keyboard, gamepad, pad or host) skips.
    let skip = input.any_just_pressed;
    if latch_and_should_advance(&mut timer.0, skip) {
        let _ = fade.request_with(GameState::Menu, 0.6);
    }
}

/// Lockup scale at `fraction` (0 to 1) of the hold: `SETTLE_FROM` easing
/// out to 1.0.
pub fn settle_scale(fraction: f32) -> f32 {
    let t = fraction.clamp(0.0, 1.0);
    let eased = 1.0 - (1.0 - t).powi(2);
    SETTLE_FROM + (1.0 - SETTLE_FROM) * eased
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

    #[test]
    fn lockup_settles_by_one_to_two_percent() {
        assert_eq!(settle_scale(0.0), SETTLE_FROM);
        assert_eq!(settle_scale(1.0), 1.0);
        assert!((1.01..=1.02).contains(&SETTLE_FROM));
        let mut last = settle_scale(0.0);
        for step in 1..=10 {
            let s = settle_scale(step as f32 / 10.0);
            assert!(s <= last);
            last = s;
        }
    }
}
