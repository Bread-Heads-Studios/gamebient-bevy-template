---
paths:
  - "src/ui/**"
---

# UI code

Loaded when you edit UI. Full rules: "Typography and cards" and "Screen
shape" in `docs/conventions.md`.

- Set a face on every `Text`: `DISPLAY.font(size)`, `BODY.fitted(text, max)`
  or `fonts::body(size)`. Nothing renders in the default Fira Mono, and
  never touch the default font slot (the cabinet frame uses it).
- Every string you fit goes into `fonts::tests::fitted()`.
- Never animate `font_size` (each size leaks a glyph atlas). Animate
  `UiTransform` or alpha.
- Author against a 720 short side. Never set `UiScale` and never write a
  resolution; sizes come from `src/display.rs`.
- `fit.rs`, `card.rs` and `studio_logo.rs` are copied into games verbatim.
  Change them here only if every game should get the change.
- The title settles by t = 1.0 s (autopilot shots fire then); nothing
  flashes faster than 3 Hz.
