---
name: designing-cartridge-covers
description: Use when a Gamebient game needs its box cover (assets/cartridge.png) made or redone, when tools/cartridge-cover.svg still says PLACEHOLDER, when a cover looks like another game's cover, when capturing gameplay screenshots via the autopilot harness, or when customizing src/game/autopilot.rs for a new game
---

# Designing Cartridge Covers

## Overview

The cover is a 768x1024 PNG composed by `make-cartridge.sh` from a real
gameplay screenshot (captured by `cargo run --features autopilot`) and
`tools/cartridge-cover.svg`, rendered with `rsvg-convert`. The SVG that ships
in the template is a labelled placeholder. **Its arrangement is not a layout
to keep; the cover's composition, typeface, words, and drawings all come from
the game's own tone.** Two covers from this studio should never share a
skeleton.

Supporting files: [fonts.md](fonts.md) (what renders under rsvg on macOS),
[autopilot.md](autopilot.md) (customizing the capture harness).

## Process

1. **Capture.** `./make-cartridge.sh` runs the autopilot and writes
   `build/cartridge/shots/01…09`. If the bot does not yet play your game,
   fix `src/game/autopilot.rs` first ([autopilot.md](autopilot.md)). Read the
   shots as images; pick the beat with the most action.
2. **Write a design brief before touching SVG.** Five lines, from the game's
   README, spec, and in-game copy:
   - *Mood* in three words (deadpan bureaucratic; soda-fountain cheer; abyssal dread).
   - *Typeface voice* that mood implies (typewriter, chalk, chrome, serif field guide) → one or two faces from [fonts.md](fonts.md).
   - *Words*: the tagline and one sticker/stamp line, in the game's voice, not a generic "CATCH • STACK • DELIVER" cadence.
   - *Drawings*: two to five subjects of this game (its creature, its object, its arena) drawn as SVG paths.
   - *Composition*: a framing device the mood suggests (case file, chalkboard, treasure chart, versus split, depth chart, holo card, backglass, memo). Not the placeholder's header/panel/footer stack.
3. **Build the SVG.** 768x1024. Palette from the game's own colors (`src/`
   presentation code). The gameplay panel clips `shot.png` with `<image>`
   x/y/width/height tuned to frame the action, not the HUD. Every text run
   fits inside 768px.
4. **Fixed slots** (the only shelf constants; style and placement are yours):
   - a "Bread Heads Studios" credit
   - a "ColecoVision GX" credit
   - the player count (the `Players` attribute in `assets/info.json`; "1 PLAYER" or "2 PLAYERS")
   - the real-gameplay panel with a "real gameplay" tag
5. **Verify.** Render a probe of each font (see [fonts.md](fonts.md)), then
   `./make-cartridge.sh --skip-capture --beat <beat>`, Read
   `assets/cartridge.png`, and also judge it at thumbnail size: title legible,
   nothing clipped or overlapping, no placeholder text left. Iterate.
6. **Persist.** Set `BEAT=` in `make-cartridge.sh` to the beat you used and
   update its header comment. Confirm `file assets/cartridge.png` is PNG
   768 x 1024. Point `info.json` `image` at it (`generating-cartridge-metadata`).

## Common mistakes

| Mistake | Fix |
|---------|-----|
| Keeping the placeholder's header strip, footer text, and panel position and only restyling | Choose a composition from the brief; the placeholder is scaffolding to delete. |
| Copying a sibling game's cover (Tire Stack's tilted panel, ribbon, checker strip) | Reference siblings for mechanics only. If a stranger could not tell which game it is from the layout alone, redo it. |
| Fonts that silently render as Helvetica (Impact, DIN Condensed, Courier) | Probe first; [fonts.md](fonts.md) lists the traps. |
| Panel shows a menu, a letterbox band, or mostly HUD | Pick a play beat; zoom the clip onto the action. |
| "PLACEHOLDER COVER — DESIGN ME" still in the render | Delete the notice block. |
| Scratch files named `probe.png` in a shared scratchpad | Prefix with the game name, or work in `build/cartridge/`. |
