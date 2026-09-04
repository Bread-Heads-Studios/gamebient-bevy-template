# Backbuffer Pin Rollout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Pin every game's web render backbuffer at 1280×720 (CSS-transform letterbox scaling), porting voidrunner's proven fix to the template and four game repos.

**Architecture:** Verbatim port from voidrunner `main` (commits `5cdafcf` + `45dc406`): wasm-only window config (`scale_factor_override(1.0)`, `fit_canvas_to_parent: false`), pinned canvas layout box + `transform: scale(var(--game-scale))` + `fit()` JS in index.html, WebGL2 probe on a throwaway canvas. Each repo is one task with the same recipe; per-repo notes cover the differences.

**Tech Stack:** Rust 2024 / Bevy 0.18, plain JS/CSS.

**Spec:** `docs/superpowers/specs/2026-07-19-backbuffer-pin-design.md`.
**Reference:** extract voidrunner's implementation with
`git -C ../voidrunner show main:src/main.rs` and `git -C ../voidrunner show main:index.html` — the `#game-container`/`#game` CSS block (with both load-bearing comments), the `fit()` IIFE, the WebGL2 probe pattern, and the `#[cfg]` window-field shape are the source of truth. Copy comments verbatim.

## The recipe (identical per repo)

1. Branch `feat/render-pin` off `main`. (Template: branch exists with the spec commit — use it.)
2. **src/main.rs** — mirror voidrunner's window config:
   - Delete the `RENDER_SCALE` const and its doc comment if present.
   - Window fields (exact voidrunner shape, comments included):
     ```rust
                 // On web, pin the backbuffer to a fixed 1280×720 with a 1.0
                 // scale factor and let CSS scale the fixed-size canvas up to
                 // fill the viewport (letterboxed). We must NOT fit the canvas
                 // to its parent: winit would then resize the backbuffer to the
                 // element's pixel size, shading full-resolution pixels and
                 // defeating the low-res render target the Pi 5 depends on.
                 #[cfg(target_arch = "wasm32")]
                 resolution: WindowResolution::new(1280, 720).with_scale_factor_override(1.0),
                 #[cfg(not(target_arch = "wasm32"))]
                 resolution: WindowResolution::new(1280, 720),
                 #[cfg(target_arch = "wasm32")]
                 fit_canvas_to_parent: false,
     ```
     Repos whose current config uses a different resolution expression
     (`(1280, 720).into()`, `(1280u32, 720u32).into()`) normalize to
     `WindowResolution::new(1280, 720)`. Native keeps no `fit_canvas_to_parent`
     line (struct default false). Import `WindowResolution` if not already.
3. **index.html** — port three blocks from voidrunner's:
   - `#game-container` CSS: add flex centering (+ its comment).
   - `#game` CSS: replace the `width/height: 100%` sizing with the pinned
     `1280px/720px !important` + `flex: none` + `transform` block — copy the
     full comment block including the flex-shrink note.
   - Add the `fit()` IIFE script (before the keyEvent bridge listener).
   - WebGL2 probe: replace `gameCanvas.getContext('webgl2')` with the
     voidrunner pattern probing `document.createElement('canvas')`.
   - Touch NOTHING else (each repo's overlays/styling stay).
4. Gates: `cargo test`, `cargo clippy --all-targets -- -D warnings`,
   `cargo fmt --all -- --check`, and `cargo build` (native cfg correctness).
5. Commit: `fix: pin web backbuffer at 1280x720, letterbox via CSS transform`
   (implementer). Do NOT push/PR/build_web — the controller verifies with
   `vr-verify.mjs` (backbuffer assertion at DPR 2 + narrow viewport) and the
   flow driver, then ships.

### Task 1: gamebient-bevy-template (canonical; verify first before fan-out)
Facts: has `RENDER_SCALE 0.5` + `with_scale_factor_override(RENDER_SCALE)` + `fit_canvas_to_parent: true` (src/main.rs:15,37-40); index.html is closest to voidrunner's (same lineage).

### Task 2: Hunted
Facts: `resolution: (1280, 720).into()` (src/main.rs:53), no RENDER_SCALE, `fit_canvas_to_parent: true` (:56); index.html has Hunted-specific overlay content — port only the three blocks.

### Task 3: pizza-pinball
Facts: `RENDER_SCALE 0.5` (src/main.rs:17,44-47); touch overlay already removed; probe at index.html:226.

### Task 4: BeerPong
Facts: plain `WindowResolution::new(1280, 720)` (src/main.rs:28), `fit_canvas_to_parent: true` (:30); probe at index.html:176.

### Task 5: Gravestone_Gauntlet
Facts: `(1280u32, 720u32).into()` (src/main.rs:25), `fit_canvas_to_parent: true` (:28); 2D game — same treatment; GG's index.html has custom gravestone/green styling — leave it, port only the three blocks; probe at index.html:246.

## Self-review notes
- `update_ui_scale` exists in template/pizza-pinball/BeerPong (spec: accepted no-op on web; native unchanged) — do not remove it.
- GG/Hunted may lack `use bevy::window::WindowResolution` — add as needed.
