//! The highlight hook: one message a game writes for a short celebratory
//! beat (boss down, level clear, big combo). `game::host` relays it to web
//! hosts as `HostEvent::Highlight`; the native frame pulses the marquee.
//! Compiled on every target, headless included, so a sim system can write
//! it without caring where it runs.

use bevy::prelude::*;

/// Write one of these for a short celebratory beat. The frame limits the
/// marquee to one pulse per ten seconds, so calling it often is harmless.
///
/// ```ignore
/// fn boss_defeated(mut highlights: MessageWriter<Highlight>) {
///     highlights.write(Highlight::rgb(0xff, 0xcc, 0x00));
/// }
/// ```
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Highlight {
    /// The game's accent colour for this beat, or `None` for the default.
    pub color: Option<[u8; 3]>,
}

impl Highlight {
    /// A pulse in the frame's default colour.
    pub const fn plain() -> Self {
        Self { color: None }
    }

    /// A pulse in the given sRGB colour.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self {
            color: Some([r, g, b]),
        }
    }

    /// `"#rrggbb"`, the form the host protocol carries.
    pub fn hex(self) -> Option<String> {
        self.color
            .map(|[r, g, b]| format!("#{r:02x}{g:02x}{b:02x}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_is_lowercase_rrggbb() {
        assert_eq!(
            Highlight::rgb(0xff, 0xcc, 0x00).hex().as_deref(),
            Some("#ffcc00")
        );
        assert_eq!(
            Highlight::rgb(0x0a, 0x00, 0xb1).hex().as_deref(),
            Some("#0a00b1")
        );
    }

    #[test]
    fn plain_has_no_colour() {
        assert_eq!(Highlight::plain().hex(), None);
        assert_eq!(Highlight::plain(), Highlight::default());
    }
}
