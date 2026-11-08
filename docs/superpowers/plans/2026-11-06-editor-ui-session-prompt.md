# Prompt: design and build the web editor UI

Paste everything below the line into a new Claude Code session on the
`gamebient-bevy-template` environment (it installs Bevy's Linux build
libraries). Start the session on branch `ccr-cbe4cf1d-upzv0t`, or on `main`
once that branch is merged.

---

You're designing and building the first version of a **web-based visual editor
for Gamebient games** (Bevy 0.18, Rust, built for wasm/WebGL2 and native). The
audience is people who are new to programming and game development, so
everything a person sees must be plain words, never code.

## Read first

- `AGENTS.md`, especially "Deterministic sim", the replay sections and the
  gotchas. They're hard rules: an editor that breaks replay verification is
  worse than no editor.
- `docs/superpowers/specs/2026-11-01-natural-language-rules-design.md`: the
  earlier design. Its fixed rule vocabulary has been **replaced** by the
  approach below. Its determinism section, readbacks, and "three layers, one
  game" still apply.
- `eval/vibe/README.md` and `eval/vibe/sample-run/`: what happened when a
  coding agent edited a game from a beginner's sentence. Changes were correct
  and well explained, but took 3–6 minutes each, even when the person only
  wanted a different number. That wait is the problem this design solves.
- `eval/vibe/fixture/`: **Coin Run**, a small platformer laid over the
  template, with a headless test harness (`src/game/scenario.rs`). It's the
  editor's demo game.

## The design: an agent writes rules, cards tune them

A person types requests in a chat. A coding agent changes the game's Rust
code. With each change, it shows a **card** in the chat: a plain-English
readback plus a few controls for the numbers worth tuning. Moving a control
changes the running game **instantly, with no model call and no rebuild**. In
short, the model decides what exists, and the controls decide how much.

Example card, after "Press B to shoot, but only if you have ammo":

```
┌ Shooting uses ammo ───────────────────── ✓ 3 checks pass ┐
│ Pressing B fires a bullet the way you face, if you have   │
│ ammo. Each shot uses one.                                 │
│  Starting ammo     [====●=========]  5                     │
│  Bullet speed      [=========●====]  600 px/s              │
│  Fire button       ( A ) (●B )                             │
│  [ ▶ Try it ]   [ Undo ]   [ Show what changed ]           │
└────────────────────────────────────────────────────────────┘
```

The agent emits a card as data, never as UI code:

```json
{ "title": "Shooting uses ammo",
  "readback": "Pressing B fires a bullet the way you face, if you have ammo. Each shot uses one.",
  "controls": [
    { "kind": "slider", "bind": "ammo.start", "label": "Starting ammo", "min": 0, "max": 50, "step": 1 },
    { "kind": "slider", "bind": "things.bullet.speed", "label": "Bullet speed", "min": 100, "max": 1200, "unit": "px/s" },
    { "kind": "button", "bind": "ammo.fire_button", "label": "Fire button", "options": ["A", "B"] } ],
  "checks": ["shooting_runs_out_of_ammo"] }
```

Rules the design depends on:

1. **A fixed catalog of controls.** Slider, number, toggle, colour, choice,
   button picker (the A/B canon), sound picker, a place-it-on-the-stage
   picker, and a curve. Build each once, fully accessible: keyboard only,
   screen-reader labels, large-text and reduced-motion modes. The agent can
   only choose from the catalog and fill in fields.
2. **Every card is checked against the game.** A `bind` must name a tunable
   the game really exposes, and ranges must fit the field's type. A card that
   doesn't match is rejected with a plain message, never rendered half-working.
3. **Tunables are reflected Rust values.** Gameplay numbers live in
   `Reflect` resources (`reflect_auto_register` is already on) that carry
   labels, ranges and units, for example through `bevy_reflect` custom
   attributes. The game reports its tunables (paths, types, ranges, current
   values) to the editor, and accepts value changes from it. Changes apply
   at a sim-tick boundary, never in the middle of a tick.
4. **Tunable values are game content.** They feed a content hash in the replay
   header, and a change made mid-run is recorded. A replay recorded with one
   set of values must not verify against another. Keep `cargo test
   --all-targets --all-features` (including `tests/windowed_shape.rs`,
   `tests/archetype_order.rs` and the selftest fixtures) green.
5. **Cards also live in an inspector.** The chat is history. Each card is
   pinned to its rule or thing in a properties panel that shows the game as
   it is now.
6. **Checks re-run when a control is released.** A card lists the scenario
   tests for its rule, and the editor shows a non-blocking warning when a
   value breaks one (for example "⚠ B never fires" at 0 ammo).

## Build in milestones, and show me each one working

1. **Design pass, before any code.** Propose the architecture: the editor app,
   how it hosts the game (the website already frames the wasm game and talks
   to it over the `gamebient-input` host protocol, so extend that rather than
   inventing a second channel), the tunables protocol, the card schema
   (JSON Schema, versioned), and the control catalog. Make a short clickable
   mockup of the main screen (game preview, chat with cards, inspector).
   Recommend a frontend stack and say why; plain TypeScript with a small,
   accessible component layer is fine, and so is React. **Before writing
   code, ask me where the editor lives**: a new `gamebient-editor`
   repository (my default, so `init-game.sh` doesn't copy an editor into
   every game) or a folder in this repo for the prototype.
2. **Tunables in the game.** Coin Run (built from `eval/vibe/fixture/`, plus
   the rules in `eval/vibe/sample-run/a01.diff` and `d03.diff` so there's
   ammo and coins to tune) exposes its tunables over the host protocol. Add
   unit tests, the content hash in the replay header, and a determinism test
   proving a mid-run tunable change replays identically.
3. **The editor shell.** The wasm game in a preview, the inspector bound to
   live tunables, and the control catalog working. Accessibility checked with
   an automated tool (e.g. axe) plus keyboard-only walkthroughs, and
   Playwright screenshots in light and dark mode. Chromium and Playwright are
   preinstalled; don't reinstall them.
4. **The chat with cards.** First with a mock agent that returns canned cards
   (for a01 and d03), so the whole UI works without model calls. Then a real
   one: a Claude API call that answers with a `show_card` tool call, used for
   requests a control already covers ("make bullets faster" moves the slider
   and shows the card). Don't build the code-writing agent loop in this
   session. That's the `eval/vibe` runner's territory; leave a clear seam
   for it.

## Constraints

- Follow `AGENTS.md`: one file per concern, curated Bevy features (each new
  one costs wasm bundle size), `wasm-bindgen` pinned exactly, the display
  module rules, and fonts through `ui/fonts.rs`.
- The wasm build needs `rustup target add wasm32-unknown-unknown` and the
  pinned `wasm-bindgen-cli`, as in `install.sh`. If the network blocks a
  download, tell me which host. Don't work around it.
- Don't push to `main` or open a PR until I ask. Commit on your own branch,
  with clear messages.
- When something can't be verified here (a real browser on a phone, the
  cabinet), say so rather than claiming it works.
