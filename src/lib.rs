//! Library half of the game so a second entry point (`src/bin/verify.rs`)
//! can build the headless sim from the same modules as the windowed game.
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

pub mod assets;
pub mod game;
pub mod ui;
