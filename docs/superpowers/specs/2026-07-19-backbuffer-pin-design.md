# Backbuffer Pin Rollout — Fixed 1280×720 Render Target on Web

**Date:** 2026-07-19
**Status:** Approved
**Reference implementation:** voidrunner commits `5cdafcf` ("Retry rendering") + `45dc406` ("fix: pin canvas against flex-shrink squeeze"), merged via voidrunner PR #10.
**Scope:** gamebient-bevy-template, Hunted, pizza-pinball, BeerPong, Gravestone_Gauntlet. Website untouched (it iframes the games).

## Problem

On the web, winit runs a ResizeObserver on the `#game` canvas and reports the
element's layout-box size to Bevy, which calls `set_physical_resolution` on
every resize (`bevy_winit react_to_resize`). This overrides
`WindowResolution` — and `scale_factor_override` does NOT gate it;
`fit_canvas_to_parent` in Bevy 0.18 only sets CSS `width/height: 100%`.
Result with the current full-viewport canvas CSS: the backbuffer tracks the
element at viewport × devicePixelRatio. On a 4K kiosk that is ~8.3M shaded
pixels/frame instead of the intended ~0.92M (720p) — ~9× the fill-rate on
hardware (Pi 5 / WebGL2) that is fill-rate bound.

Current state: template and pizza-pinball ship the `RENDER_SCALE 0.5`
override, silently defeated on web; Hunted, BeerPong, and Gravestone_Gauntlet
have no scale mechanism at all. All five probe WebGL2 support on `#game`
itself, which locks the canvas's context attributes before wgpu configures
its own.

## Fix (verbatim port of voidrunner's)

Per repo, three interlocking pieces — copy voidrunner's implementation
including its load-bearing comments:

1. **`src/main.rs`**: remove `RENDER_SCALE` where present. Window config:
   - wasm32: `WindowResolution::new(1280, 720).with_scale_factor_override(1.0)`
     and `fit_canvas_to_parent: false` (via `#[cfg(target_arch = "wasm32")]`
     attributes on the struct fields, voidrunner's shape)
   - native: plain `WindowResolution::new(1280, 720)`, `fit_canvas_to_parent`
     absent/false as today
2. **`index.html`**:
   - `#game-container`: flex, centered (letterboxes the scaled canvas)
   - `#game`: `width: 1280px !important; height: 720px !important;`
     `flex: none;` (defeats flex-shrink squeeze on narrow viewports — the
     `45dc406` follow-up), `transform: scale(var(--game-scale, 1));`
     `transform-origin: center center;`
   - `fit()` script: sets `--game-scale = min(innerWidth/1280, innerHeight/720)`
     on load + resize; touches only the CSS variable
   - WebGL2 support probe moved to `document.createElement('canvas')` —
     never call `getContext` on `#game`
3. Keep every game-specific token/styling difference in its own index.html;
   only the container/canvas CSS block, fit() script, and probe change.

Known side effect (accepted): `update_ui_scale` computes 1.0 on web since the
logical height is pinned at 720 — UI is then scaled visually by the same CSS
transform as the 3D content. Native behavior unchanged.

## Verification (per repo)

- Backbuffer assertion in headless Chrome at `deviceScaleFactor: 2`: after
  boot and after a viewport resize, the canvas `width`/`height` attributes
  (the backbuffer) remain exactly 1280×720 while the CSS layout scales.
  Narrow-viewport case (< 1280 px wide) asserted too (flex-squeeze
  regression). Reuse the `vr-verify.mjs` pattern staged in
  `~/.cache/gamebient-verify/`.
- Standard flow drive (screens render, game plays, letterboxing visible).
- Gates: `cargo test`, `clippy -D warnings`, `fmt --check`; native
  `cargo build` (cfg-attr correctness).

## Delivery

Branch `feat/render-pin` off `main` per repo; template first (canonical),
then the four games in parallel; one PR each.
