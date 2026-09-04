//! Input comes from the `gamebient-input` crate: one `GameInput` resource
//! populated from keyboard, gamepads, the touch overlay and the ColecoVision
//! GX host protocol, using the Gamebient canon bindings. Gameplay systems
//! read ONLY `GameInput`, never raw input, so bindings live in one place
//! across every game.
//!
//! Re-exported here so `crate::game::input::GameInput` keeps resolving.

pub use gamebient_input::GameInput;
