---
paths:
  - "src/frame/**"
  - "src/display.rs"
---

# Display and cabinet frame

Loaded when you edit the display module or the frame. Full rules: "Screen
shape" and "Cabinet frame" in `docs/conventions.md`.

- `src/display.rs` is the only place the game's size is written.
- `Camera::world_to_viewport` includes the letterbox offset: use
  `display::label_origin(..)`, never a hand copy of the maths.
- The frame shares numbers and test vectors with the website
  (`src/lib/cabinetFrame/`); change both together.
- `FramePlugin` is native and windowed only; it does nothing on wasm and is
  never added to `GamePlugin`.
- Capture builds stay windowed via the `capture` feature only; do not edit
  `game_window` or the frame for a new capture feature.
- After an aspect change, `tools/frame-accept.sh` checks verbatim copies and
  screenshots.
