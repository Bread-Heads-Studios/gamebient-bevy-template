//! Pure brightness state machine for the cabinet frame. Inputs: the phase,
//! the pause flag, elapsed time and three events. Outputs: the multipliers
//! for bezel and marquee and the progress of the marquee's animations.
//! Values are Contract E of the aspect-ratio roadmap. The website has a
//! line-for-line port in `src/lib/cabinetFrame/brightness.ts`; keep the two
//! in step.

pub const BEZEL_IDLE: f32 = 0.35;
pub const BEZEL_PLAY: f32 = 0.20;
pub const MARQUEE_IDLE: f32 = 1.0;
pub const MARQUEE_PLAY: f32 = 0.45;
/// Seconds a full idle-to-play fade takes.
pub const FADE_SECS: f32 = 1.0;
/// Seconds the marquee holds 1.0 on game over.
pub const FLASH_SECS: f32 = 0.4;
/// Seconds the marquee holds 1.0 on a new high score.
pub const CELEBRATION_SECS: f32 = 4.0;
/// Sweep passes across the marquee during one celebration.
pub const CELEBRATION_SWEEPS: f32 = 4.0;
pub const PULSE_PEAK: f32 = 0.9;
pub const PULSE_SECS: f32 = 0.6;
/// A highlight inside this many seconds of the last accepted one is dropped.
pub const PULSE_MIN_INTERVAL_SECS: f32 = 10.0;
pub const SWEEP_PERIOD_SECS: f32 = 6.0;
/// Seconds one ambient sweep takes to cross the marquee.
pub const SWEEP_SECS: f32 = 1.5;

/// The frame's view of the game state. `Menu`, the studio logo and the
/// how-to-play screen are all `Attract`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FramePhase {
    Attract,
    Playing,
    GameOver,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameEvent {
    /// A run ended without beating the best score.
    GameOver,
    /// A run ended and its score beat the best score.
    NewHighScore,
    /// The game asked for a pulse.
    Highlight,
}

/// What to draw this frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameLevels {
    /// Bezel brightness multiplier, always within 0.20..=0.35.
    pub bezel: f32,
    /// Marquee brightness multiplier, always within 0.45..=1.0.
    pub marquee: f32,
    /// Ambient sweep progress across the marquee, 0..1, while one is running.
    pub sweep: Option<f32>,
    /// Pulse envelope, 0..1, while a highlight pulse is running.
    pub pulse: Option<f32>,
    /// Celebration progress, 0..1, while one is running.
    pub celebration: Option<f32>,
}

impl FrameLevels {
    /// Where the sweep band is, 0..1 across the marquee, or `None` when no
    /// band should be drawn. A celebration replaces the ambient sweep with
    /// faster passes. Reduced motion removes both.
    pub fn sweep_progress(&self, reduced_motion: bool) -> Option<f32> {
        if reduced_motion {
            return None;
        }
        match self.celebration {
            Some(progress) => Some((progress * CELEBRATION_SWEEPS).fract()),
            None => self.sweep,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FrameBrightness {
    /// Seconds since the machine was made. `f64` so a cabinet left on for
    /// months still resolves single frames.
    clock: f64,
    bezel: f32,
    marquee: f32,
    /// Seconds spent in `Attract` since it was last entered.
    attract: f64,
    flash_until: f64,
    celebration_until: f64,
    pulse_start: Option<f64>,
    last_pulse: Option<f64>,
}

impl Default for FrameBrightness {
    fn default() -> Self {
        Self::new()
    }
}

/// Moves `current` toward `target` by at most `max_step`.
fn approach(current: f32, target: f32, max_step: f32) -> f32 {
    let delta = target - current;
    if delta.abs() <= max_step {
        target
    } else {
        current + max_step.copysign(delta)
    }
}

impl FrameBrightness {
    pub fn new() -> Self {
        Self {
            clock: 0.0,
            bezel: BEZEL_IDLE,
            marquee: MARQUEE_IDLE,
            attract: 0.0,
            flash_until: 0.0,
            celebration_until: 0.0,
            pulse_start: None,
            last_pulse: None,
        }
    }

    /// Feeds one event. Returns whether it was accepted; only `Highlight`
    /// can be refused, by the rate limit.
    pub fn event(&mut self, event: FrameEvent) -> bool {
        match event {
            FrameEvent::GameOver => {
                self.flash_until = self.clock + f64::from(FLASH_SECS);
                true
            }
            FrameEvent::NewHighScore => {
                self.celebration_until = self.clock + f64::from(CELEBRATION_SECS);
                true
            }
            FrameEvent::Highlight => {
                let allowed = self
                    .last_pulse
                    .is_none_or(|last| self.clock - last >= f64::from(PULSE_MIN_INTERVAL_SECS));
                if allowed {
                    self.pulse_start = Some(self.clock);
                    self.last_pulse = Some(self.clock);
                }
                allowed
            }
        }
    }

    /// Advances the machine by `dt` seconds and returns what to draw. A
    /// negative or non-finite `dt` counts as zero.
    pub fn step(&mut self, dt: f32, phase: FramePhase, paused: bool) -> FrameLevels {
        let dt = if dt.is_finite() { dt.max(0.0) } else { 0.0 };
        self.clock += f64::from(dt);

        let playing = phase == FramePhase::Playing;
        let (bezel_target, marquee_target) = if playing {
            (BEZEL_PLAY, MARQUEE_PLAY)
        } else {
            (BEZEL_IDLE, MARQUEE_IDLE)
        };
        self.bezel = approach(
            self.bezel,
            bezel_target,
            (BEZEL_IDLE - BEZEL_PLAY) * dt / FADE_SECS,
        );
        self.marquee = approach(
            self.marquee,
            marquee_target,
            (MARQUEE_IDLE - MARQUEE_PLAY) * dt / FADE_SECS,
        );
        self.attract = if phase == FramePhase::Attract {
            self.attract + f64::from(dt)
        } else {
            0.0
        };

        let celebration = (self.clock < self.celebration_until).then(|| {
            let left = (self.celebration_until - self.clock) as f32;
            1.0 - left / CELEBRATION_SECS
        });
        let flashing = self.clock < self.flash_until;
        let held_at_full = celebration.is_some() || flashing;

        let pulse = self
            .pulse_start
            .map(|start| (self.clock - start) as f32)
            .filter(|t| *t < PULSE_SECS && playing && !paused && !held_at_full)
            .map(|t| 1.0 - (2.0 * t / PULSE_SECS - 1.0).abs());

        let marquee = if held_at_full {
            MARQUEE_IDLE
        } else if let Some(envelope) = pulse {
            self.marquee + (PULSE_PEAK - self.marquee).max(0.0) * envelope
        } else {
            self.marquee
        };

        let sweep = if phase == FramePhase::Attract && !held_at_full {
            let position = (self.attract % f64::from(SWEEP_PERIOD_SECS)) as f32;
            (position < SWEEP_SECS).then_some(position / SWEEP_SECS)
        } else {
            None
        };

        FrameLevels {
            bezel: self.bezel,
            marquee,
            sweep,
            pulse,
            celebration,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.001
    }

    fn close_opt(a: Option<f32>, b: Option<f32>) -> bool {
        match (a, b) {
            (Some(a), Some(b)) => close(a, b),
            (None, None) => true,
            _ => false,
        }
    }

    fn settled_play() -> FrameBrightness {
        let mut m = FrameBrightness::new();
        m.step(1.0, FramePhase::Playing, false);
        m.step(1.0, FramePhase::Playing, false);
        m
    }

    #[test]
    fn b1_starts_at_idle() {
        let mut m = FrameBrightness::new();
        let l = m.step(0.0, FramePhase::Attract, false);
        assert!(close(l.bezel, 0.35) && close(l.marquee, 1.0), "{l:?}");
    }

    #[test]
    fn b2_fades_to_play_over_one_second() {
        let mut m = FrameBrightness::new();
        let l = m.step(0.5, FramePhase::Playing, false);
        assert!(close(l.bezel, 0.275) && close(l.marquee, 0.725), "{l:?}");
        let l = m.step(0.5, FramePhase::Playing, false);
        assert!(close(l.bezel, 0.20) && close(l.marquee, 0.45), "{l:?}");
        let l = m.step(0.5, FramePhase::Playing, false);
        assert!(close(l.bezel, 0.20) && close(l.marquee, 0.45), "{l:?}");
    }

    #[test]
    fn b3_highlight_pulses_the_marquee_only() {
        let mut m = settled_play();
        assert!(m.event(FrameEvent::Highlight));
        let l = m.step(0.15, FramePhase::Playing, false);
        assert!(close(l.marquee, 0.675) && close(l.bezel, 0.20), "{l:?}");
        let l = m.step(0.15, FramePhase::Playing, false);
        assert!(close(l.marquee, 0.9) && close(l.bezel, 0.20), "{l:?}");
        let l = m.step(0.3, FramePhase::Playing, false);
        assert!(close(l.marquee, 0.45) && close(l.bezel, 0.20), "{l:?}");
    }

    #[test]
    fn b4_highlight_is_limited_to_one_per_ten_seconds() {
        let mut m = settled_play();
        assert!(m.event(FrameEvent::Highlight));
        m.step(5.0, FramePhase::Playing, false);
        assert!(!m.event(FrameEvent::Highlight));
        m.step(5.0, FramePhase::Playing, false);
        assert!(m.event(FrameEvent::Highlight));
    }

    #[test]
    fn b5_game_over_flashes_then_fades_up() {
        let mut m = settled_play();
        assert!(m.event(FrameEvent::GameOver));
        let l = m.step(0.25, FramePhase::GameOver, false);
        assert!(close(l.marquee, 1.0) && close(l.bezel, 0.2375), "{l:?}");
        let l = m.step(0.25, FramePhase::GameOver, false);
        assert!(close(l.marquee, 0.725) && close(l.bezel, 0.275), "{l:?}");
    }

    #[test]
    fn b6_new_high_score_celebrates_for_four_seconds() {
        let mut m = FrameBrightness::new();
        assert!(m.event(FrameEvent::NewHighScore));
        let l = m.step(1.0, FramePhase::GameOver, false);
        assert!(close_opt(l.celebration, Some(0.25)), "{l:?}");
        assert!(close(l.marquee, 1.0), "{l:?}");
        let l = m.step(3.0, FramePhase::GameOver, false);
        assert_eq!(l.celebration, None);
        assert!(close(l.marquee, 1.0), "{l:?}");
    }

    /// The owner's hard requirement: whatever happens, the bezel stays
    /// between play and idle brightness.
    #[test]
    fn b7_bezel_never_leaves_its_range() {
        let phases = [
            FramePhase::Attract,
            FramePhase::Playing,
            FramePhase::GameOver,
        ];
        let events = [
            None,
            Some(FrameEvent::GameOver),
            Some(FrameEvent::NewHighScore),
            Some(FrameEvent::Highlight),
        ];
        let steps = [0.0, 0.1, 0.5, 3.0];
        let mut m = FrameBrightness::new();
        for round in 0..3 {
            for phase in phases {
                for event in events {
                    for dt in steps {
                        for paused in [false, true] {
                            if let Some(event) = event {
                                m.event(event);
                            }
                            let l = m.step(dt, phase, paused);
                            assert!(
                                (BEZEL_PLAY - 1e-6..=BEZEL_IDLE + 1e-6).contains(&l.bezel),
                                "round {round} {phase:?} {event:?} dt {dt}: bezel {}",
                                l.bezel
                            );
                            assert!(
                                (MARQUEE_PLAY - 1e-6..=MARQUEE_IDLE + 1e-6).contains(&l.marquee),
                                "round {round} {phase:?} {event:?} dt {dt}: marquee {}",
                                l.marquee
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn b8_sweep_runs_every_six_seconds_in_attract_only() {
        let mut m = FrameBrightness::new();
        let l = m.step(1.0, FramePhase::Attract, false);
        assert!(close_opt(l.sweep, Some(0.6667)), "{l:?}");
        let l = m.step(1.0, FramePhase::Attract, false);
        assert_eq!(l.sweep, None);
        let l = m.step(4.5, FramePhase::Attract, false);
        assert!(close_opt(l.sweep, Some(0.3333)), "{l:?}");
        let l = m.step(0.1, FramePhase::Playing, false);
        assert_eq!(l.sweep, None);
    }

    #[test]
    fn b9_pause_suppresses_the_pulse() {
        let mut m = settled_play();
        assert!(m.event(FrameEvent::Highlight));
        let l = m.step(0.3, FramePhase::Playing, true);
        assert!(close(l.marquee, 0.45), "{l:?}");
        assert_eq!(l.pulse, None);
    }

    #[test]
    fn b10_sweep_progress_follows_the_celebration_and_reduced_motion() {
        let l = FrameLevels {
            bezel: 0.35,
            marquee: 1.0,
            sweep: None,
            pulse: None,
            celebration: Some(0.3),
        };
        assert!(close_opt(l.sweep_progress(false), Some(0.2)));
        assert_eq!(l.sweep_progress(true), None);
        let l = FrameLevels {
            sweep: Some(0.5),
            celebration: None,
            ..l
        };
        assert!(close_opt(l.sweep_progress(false), Some(0.5)));
        assert_eq!(l.sweep_progress(true), None);
    }

    #[test]
    fn a_negative_step_is_treated_as_zero() {
        let mut m = FrameBrightness::new();
        let l = m.step(-5.0, FramePhase::Playing, false);
        assert!(close(l.bezel, 0.35) && close(l.marquee, 1.0), "{l:?}");
    }

    /// A non-finite step must not poison the state: NaN would make every
    /// later multiplier NaN, and infinity would jump the clock.
    #[test]
    fn a_non_finite_step_is_treated_as_zero() {
        for phase in [
            FramePhase::Attract,
            FramePhase::Playing,
            FramePhase::GameOver,
        ] {
            for dt in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
                let mut m = FrameBrightness::new();
                m.event(FrameEvent::Highlight);
                let l = m.step(dt, phase, false);
                assert!(
                    l.bezel.is_finite() && l.marquee.is_finite(),
                    "{phase:?} {dt}: {l:?}"
                );
                assert!(
                    (BEZEL_PLAY..=BEZEL_IDLE).contains(&l.bezel),
                    "{phase:?} {dt}: {l:?}"
                );
                assert!(
                    (MARQUEE_PLAY..=MARQUEE_IDLE).contains(&l.marquee),
                    "{phase:?} {dt}: {l:?}"
                );
                // The machine still works afterwards.
                let l = m.step(1.0, FramePhase::Playing, false);
                assert!(l.bezel.is_finite() && l.marquee.is_finite(), "{l:?}");
                assert!(
                    (BEZEL_PLAY..=BEZEL_IDLE).contains(&l.bezel)
                        && (MARQUEE_PLAY..=MARQUEE_IDLE).contains(&l.marquee),
                    "{l:?}"
                );
            }
        }
    }
}
