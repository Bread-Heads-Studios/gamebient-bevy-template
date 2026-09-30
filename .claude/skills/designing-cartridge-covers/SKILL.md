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
   x/y/width/height tuned to frame the action, not the HUD. The shot is the
   game's own ratio (`tools/game-size.sh`), so the `<image>`'s width:height
   must be that ratio too; see "Gameplay panel by ratio". Every text run
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

## Gameplay panel by ratio

Games are 4:3 (960x720), 1:1 (720x720) or 3:4 (720x960); none is 16:9. The
shot is captured at that ratio, and the panel shows it at that ratio. The
placeholder's slots, inside the 768x1024 cover:

| Ratio | Slot (`<image>` and clip rect) | Backing rect | Tag `translate` |
|---|---|---|---|
| 4:3 | x 64, y 424, 640 x 480 | x 56, y 416, 656 x 496 | (136, 904) |
| 1:1 | x 144, y 424, 480 x 480 | x 136, y 416, 496 x 496 | (216, 904) |
| 3:4 | x 204, y 424, 360 x 480 | x 196, y 416, 376 x 496 | (276, 904) |

A designed cover places its panel wherever the composition wants and may
clip it to any shape. Two things are fixed:

- The `<image>`'s `width:height` equals the game's ratio. To zoom onto the
  action, grow both together and move `x`/`y`; never change one alone.
- A portrait game gets a portrait panel and a landscape game a landscape
  one. A 3:4 shot in a wide window shows a sliver of the game.

A cover designed while the game was 16:9 has a 16:9 `<image>` (for example
`width="960" height="540"`). After the game changes ratio, re-capture
(`./make-cartridge.sh`), set the `<image>` to the new ratio, and reshape
the clip so it still frames the action.

## Common mistakes

| Mistake | Fix |
|---------|-----|
| Keeping the placeholder's header strip, footer text, and panel position and only restyling | Choose a composition from the brief; the placeholder is scaffolding to delete. |
| Copying a sibling game's cover (Tire Stack's tilted panel, ribbon, checker strip) | Reference siblings for mechanics only. If a stranger could not tell which game it is from the layout alone, redo it. |
| Fonts that silently render as Helvetica (Impact, DIN Condensed, Courier) | Probe first; [fonts.md](fonts.md) lists the traps. |
| Panel shows a menu, a letterbox band, or mostly HUD | Pick a play beat; zoom the clip onto the action. |
| Shot looks squashed, or has bars inside the panel | The `<image>` is still 16:9. Set its width:height to the game's ratio. |
| Shot shows a marquee or bezel around the game | It was captured without `GX_FRAME=off`. Use `./make-cartridge.sh`, which sets it. |
| "PLACEHOLDER COVER — DESIGN ME" still in the render | Delete the notice block. |
| Scratch files named `probe.png` in a shared scratchpad | Prefix with the game name, or work in `build/cartridge/`. |
