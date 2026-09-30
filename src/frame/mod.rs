//! Cabinet frame: bezel art behind the letterboxed game and, on portrait
//! displays, a marquee band above it. See docs/conventions.md, "Cabinet
//! frame".

#[cfg(not(target_arch = "wasm32"))]
pub mod brightness;
pub mod highlight;
#[cfg(not(target_arch = "wasm32"))]
pub mod layout;

pub use highlight::Highlight;
