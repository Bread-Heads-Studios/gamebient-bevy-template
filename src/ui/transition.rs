use bevy::prelude::*;

use crate::game::states::GameState;

/// Default fade half-duration (fade-out or fade-in) in seconds.
pub const DEFAULT_FADE_SECS: f32 = 0.4;

/// Boot fade-in duration (black → studio logo) in seconds.
pub const BOOT_FADE_SECS: f32 = 0.6;

/// Full-screen fade driver. Screens request state changes through this so
/// every transition passes through a fade-to-black instead of a hard cut.
#[derive(Resource)]
pub struct ScreenFade {
    phase: FadePhase,
    /// Overlay opacity: 0.0 = invisible, 1.0 = fully black.
    alpha: f32,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum FadePhase {
    Idle,
    FadingOut { target: GameState, duration: f32 },
    FadingIn { duration: f32 },
}

impl Default for ScreenFade {
    fn default() -> Self {
        Self {
            phase: FadePhase::Idle,
            alpha: 0.0,
        }
    }
}

impl ScreenFade {
    /// Boot configuration: screen starts fully black and fades in over the
    /// studio logo.
    pub fn boot() -> Self {
        Self {
            phase: FadePhase::FadingIn {
                duration: BOOT_FADE_SECS,
            },
            alpha: 1.0,
        }
    }

    /// Requests a fade-out into `target`. Returns false (and does nothing)
    /// if a fade is already in progress.
    pub fn request(&mut self, target: GameState) -> bool {
        self.request_with(target, DEFAULT_FADE_SECS)
    }

    /// Like [`Self::request`] with an explicit fade-out/fade-in duration.
    pub fn request_with(&mut self, target: GameState, duration: f32) -> bool {
        let duration = duration.max(f32::EPSILON);
        if self.phase != FadePhase::Idle {
            return false;
        }
        self.phase = FadePhase::FadingOut { target, duration };
        true
    }

    pub fn is_idle(&self) -> bool {
        self.phase == FadePhase::Idle
    }

    pub fn alpha(&self) -> f32 {
        self.alpha
    }

    /// Advances the fade by `dt` seconds. Returns the target state exactly
    /// once, at the moment the fade-out completes (screen fully black).
    pub fn tick(&mut self, dt: f32) -> Option<GameState> {
        match self.phase {
            FadePhase::Idle => None,
            FadePhase::FadingOut { target, duration } => {
                self.alpha = (self.alpha + dt / duration).min(1.0);
                if self.alpha >= 1.0 {
                    self.phase = FadePhase::FadingIn { duration };
                    Some(target)
                } else {
                    None
                }
            }
            FadePhase::FadingIn { duration } => {
                self.alpha = (self.alpha - dt / duration).max(0.0);
                if self.alpha <= 0.0 {
                    self.phase = FadePhase::Idle;
                }
                None
            }
        }
    }
}

/// Marker for the persistent full-screen fade overlay node.
#[derive(Component)]
pub struct FadeOverlay;

/// Spawns the fade overlay above all other UI. Alpha starts at the boot
/// fade's value so the first frame is already black.
pub fn spawn_fade_overlay(mut commands: Commands, fade: Res<ScreenFade>) {
    commands.spawn((
        FadeOverlay,
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            position_type: PositionType::Absolute,
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, fade.alpha())),
        GlobalZIndex(1000),
    ));
}

/// Ticks the fade, applies the deferred state switch, and syncs the overlay.
pub fn update_fade(
    time: Res<Time>,
    mut fade: ResMut<ScreenFade>,
    mut next_state: ResMut<NextState<GameState>>,
    mut overlay: Query<&mut BackgroundColor, With<FadeOverlay>>,
) {
    if let Some(target) = fade.tick(time.delta_secs()) {
        next_state.set(target);
    }
    if let Ok(mut bg) = overlay.single_mut()
        && bg.0.alpha() != fade.alpha()
    {
        *bg = BackgroundColor(Color::srgba(0.0, 0.0, 0.0, fade.alpha()));
    }
}

/// Sinusoidal alpha pulse for prompt text ("PRESS ENTER TO START" etc.).
#[derive(Component)]
pub struct Pulse {
    pub speed: f32,
    pub min: f32,
    pub max: f32,
}

/// Animates the alpha of any `Text` tagged with `Pulse`.
pub fn pulse_text(time: Res<Time>, mut query: Query<(&Pulse, &mut TextColor)>) {
    let t = time.elapsed_secs();
    for (pulse, mut color) in &mut query {
        let a = pulse.min + (pulse.max - pulse.min) * (0.5 + 0.5 * (t * pulse.speed).sin());
        color.0.set_alpha(a);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boot_fade_in_reaches_idle() {
        let mut fade = ScreenFade::boot();
        assert!(!fade.is_idle());
        assert_eq!(fade.tick(0.3), None);
        assert!(fade.alpha() > 0.0 && fade.alpha() < 1.0);
        assert_eq!(fade.tick(0.4), None);
        assert!(fade.is_idle());
        assert_eq!(fade.alpha(), 0.0);
    }

    #[test]
    fn request_fades_out_emits_target_once_then_fades_in() {
        let mut fade = ScreenFade::default();
        assert!(fade.request(GameState::Playing));
        assert_eq!(fade.tick(0.2), None);
        // Fade-out completes: target emitted exactly once, screen fully black.
        assert_eq!(fade.tick(0.3), Some(GameState::Playing));
        assert_eq!(fade.alpha(), 1.0);
        assert_eq!(fade.tick(0.2), None);
        assert_eq!(fade.tick(0.3), None);
        assert!(fade.is_idle());
    }

    #[test]
    fn request_while_busy_is_rejected() {
        let mut fade = ScreenFade::default();
        assert!(fade.request(GameState::Playing));
        assert!(!fade.request(GameState::Menu));
        fade.tick(0.5); // completes the fade-out
        assert!(!fade.request(GameState::Menu)); // still fading in
    }

    #[test]
    fn non_positive_duration_is_clamped_not_soft_locked() {
        let mut fade = ScreenFade::default();
        assert!(fade.request_with(GameState::Menu, 0.0));
        // One tick at any dt must complete the fade-out (no NaN, no lock).
        assert_eq!(fade.tick(0.016), Some(GameState::Menu));
        fade.tick(1.0);
        assert!(fade.is_idle());
    }
}
