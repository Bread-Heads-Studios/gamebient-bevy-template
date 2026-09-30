# Marquee and bezel art

Cabinets draw a frame around the game. A vertical cabinet shows a **marquee**
band above the game. Every cabinet shows a **bezel** behind the game, filling
the part of the panel the game does not use. Both are per-game PNGs rendered
from SVG by `tools/frame-art.sh` with `rsvg-convert`, the rasteriser the
cover uses. A game with no art of its own gets the shared ColecoVision GX
art, so a game is never blocked on this; a published game should have its own.

| File | Source | Size | Carries |
|---|---|---|---|
| `assets/marquee.png` | `tools/marquee.svg` | 1080x360 | the title treatment |
| `assets/bezel.png` | `tools/bezel.svg` | 1920x1920 | quiet texture in the game's palette |

Both are made from the game's finished cover, `tools/cartridge-cover.svg`.
Do the cover first.

## What the frame does to the art

The frame dims the art at runtime, the same way for every game. Draw at
full brightness. Never draw the art dim: it will be dimmed again.

| Element | Idle (menu, attract, game over) | In play and paused | Saturation |
|---|---|---|---|
| Bezel | x0.35 | x0.20 | x0.6 at all times |
| Marquee | x1.00 | x0.45 | unchanged |

The frame also owns every moving thing (the marquee's sweep, flashes and
pulses) and the score text. The art is a still image: no score, no "press
start", no animation frames.

## Where the bezel shows

The bezel is square. The frame cover-fits it to the display, crops it to
the centre, and draws the game over it.

```
          x 0       420              1500     1920
    y 0    +---------+----------------+---------+
           |  never  |    top arm     |  never  |
     420   +---------+----------------+---------+
           |  left   |     centre     |  right  |
           |  arm    |   1080x1080    |  arm    |
    1500   +---------+----------------+---------+
           |  never  |   bottom arm   |  never  |
    1920   +---------+----------------+---------+
```

- A landscape TV shows rows 420 to 1500: left arm, centre, right arm.
- A portrait TV shows columns 420 to 1500: top arm, centre, bottom arm. The
  marquee covers its top 360 rows.
- The four corners are never shown. Put nothing there that matters.
- The game covers the centre. How much of the centre stays visible depends
  on the game's ratio and the TV's orientation, and the frame may put a
  controls card or a score table there. So the central 1080x1080 is flat.

## Rules

Bezel:

1. **No detail inside the central 1080x1080** (x and y from 420 to 1500):
   one flat colour. `frame-art.sh` fails the render when the luma range
   there is above 8.
2. **Low contrast everywhere.** Darkest to brightest within 96 luma levels;
   no white, no black, no bright outlines. `frame-art.sh` fails the render
   otherwise. To quieten a drawing taken from the cover, fill it with one
   colour near the base colour and drop its outlines, or wrap it in a group
   with `opacity="0.3"`.
3. **No lettering, logos, faces or eyes.** Texture, pattern, and large soft
   shapes of the game's motifs.
4. **Detail lives in the arms and fades toward the centre.** Keep the
   template's `arms` mask and draw inside the masked group, so nothing forms
   an edge beside the game.
5. **Mid-tones from the cover's palette.** After x0.20 a dark colour is
   black and the bezel vanishes.
6. **Opaque.** The base rect covers the whole canvas.

Marquee:

1. **It is the title.** Take the cover's title treatment: same face, same
   colours, same stroke-below-fill-above stack. Make it as large as fits
   inside x 60 to 1020.
2. **Rows 288 to 360 hold no lettering.** The frame writes the score line
   over the marquee and needs a quiet strip. `tools/frame-art.sh` does not
   check this: it checks the size and the bezel only, so look at the render.
3. **It must read at x0.45.** Light lettering on a darker field. Check the
   dimmed preview below.
4. **At most two drawings from the cover**, at the sides, behind the title.
   No gameplay screenshot, no player count, no tagline paragraph.
5. **Opaque.** The background rect covers the whole canvas.

## Process

1. **Get the tooling.** If the game has no `tools/frame-art.sh`, run
   `libs/gamebient-bevy-template/tools/rollout-frame-art.sh games/<game>`
   from the workspace root. It copies the script, and the two placeholder
   SVGs only when the game has none.
2. **Read the cover.** Open `tools/cartridge-cover.svg` and note: the base
   palette (the `<stop>` colours of its main gradients), the title
   `<text>` elements with every attribute, the `<defs>` those elements use
   (gradients, filters), and two or three drawings that say which game
   this is.
3. **Marquee.** In `tools/marquee.svg`: replace the placeholder title with
   the cover's title elements, copying the `<defs>` they reference. Titles
   on the cover are often stacked on two lines for a 768-wide page; on the
   marquee put them on one line, or two short lines, and recompute the
   size: a run of N glyphs at font-size S is about `0.8 x N x S` wide in a
   heavy face, and must be at most 960. Replace the background with the
   cover's palette. Delete the placeholder notice and the word PLACEHOLDER
   from the header comment.
4. **Bezel.** In `tools/bezel.svg`: set the base rect and the arm fill to
   two close mid-tones from the cover's palette. Replace the grid pattern
   with a pattern from the cover if it has one (the cover's `<pattern>`
   copies across as it is). Replace the four circles with the game's motifs
   drawn flat, one per arm, inside the masked group. Delete the word
   PLACEHOLDER from the header comment.
5. **Render.** `tools/frame-art.sh`. Fix what it refuses.
6. **Look at it as the cabinet shows it.** These write previews into
   `build/frame-art/`; Read each one.

   ```bash
   # landscape TV, idle, with a 4:3 game in place
   ffmpeg -v error -y -i assets/bezel.png -vf "crop=1920:1080:0:420,hue=s=0.6,lutrgb=r='val*0.35':g='val*0.35':b='val*0.35',drawbox=x=240:y=0:w=1440:h=1080:color=black:t=fill" build/frame-art/preview-landscape-idle.png
   # portrait TV, in play, with a 3:4 game in place and the marquee band blanked
   ffmpeg -v error -y -i assets/bezel.png -vf "crop=1080:1920:420:0,hue=s=0.6,lutrgb=r='val*0.20':g='val*0.20':b='val*0.20',drawbox=x=0:y=0:w=1080:h=360:color=gray:t=fill,drawbox=x=0:y=420:w=1080:h=1440:color=black:t=fill" build/frame-art/preview-portrait-play.png
   # marquee in play
   ffmpeg -v error -y -i assets/marquee.png -vf "lutrgb=r='val*0.45':g='val*0.45':b='val*0.45'" build/frame-art/preview-marquee-play.png
   ```

   For a 1:1 game use `drawbox=x=420:y=0:w=1080:h=1080` (landscape) and
   `drawbox=x=0:y=600:w=1080:h=1080` (portrait); for a 3:4 game in
   landscape `drawbox=x=555:y=0:w=810:h=1080`; for a 4:3 game in portrait
   `drawbox=x=0:y=735:w=1080:h=810`.

   Judge: the title reads in the dimmed marquee; the bezel is visible but
   your eye stays on the black game area; nothing in the bezel looks like
   a game object.
7. **Persist.** `file assets/marquee.png assets/bezel.png` reports 1080 x
   360 and 1920 x 1920. Commit the two SVGs and the two PNGs.
   `tools/store-assets.sh` then writes `properties.marquee` and
   `properties.bezel` into `assets/info.json` (`generating-cartridge-metadata`).

## Fonts

The traps in [fonts.md](fonts.md) apply here too. Impact, DIN Condensed,
plain Courier, Cooper Black, Hoefler Text, Big Caslon and Superclarendon
render as Helvetica without warning, which also changes the width of the
title. A title that fitted the cover in its real face can overflow the
marquee in Helvetica. Probe every face before relying on it. `<textPath>`
is dropped, `word-spacing` is ignored, and text never wraps.

## Common mistakes

| Mistake | Fix |
|---------|-----|
| Drawing the bezel dark because it must not distract | The frame does the dimming. Draw mid-tones at full brightness. |
| Shrinking the cover onto the bezel | The cover is a poster. The bezel is wallpaper: take its palette and motifs, leave its lettering and screenshot. |
| Key art in the middle of the bezel | The game covers it. Detail goes in the arms; the central 1080x1080 stays flat. |
| Art that matters in a corner | The corners are never shown. |
| Title in the bezel | It belongs on the marquee. Horizontal cabinets have a physical marquee. |
| Score, "INSERT COIN" or "PRESS START" drawn into the marquee | The frame writes live text. The art is still. |
| Lettering in the marquee's bottom 72 rows | Move it up; that strip is for the score line. |
| "PLACEHOLDER MARQUEE — DESIGN ME" still in the render | Delete the notice block. |
| Editing `assets/bezel.png` in an image editor | Edit `tools/bezel.svg` and re-render, or the next render discards the change. |
