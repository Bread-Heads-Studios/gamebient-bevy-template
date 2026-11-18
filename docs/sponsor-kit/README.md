# Sponsor kit

A pitch pack shows a brand inside a game before any deal is signed. It is a Look Pack with `"kind": "sponsor"`: one `look.json` and a few PNG files. It has no code. The full pack format is in [`docs/look-packs.md`](../look-packs.md).

## What a brand can change

- Palette colours.
- Strings, such as the title.
- Logo art in the game's image slots.
- Glow, dimmed only.
- The end card: a headline, a line of text, a URL and an image, shown on the game-over screen. The image draws 112 px tall and at most 224 px wide (2:1); a wider image is scaled down.
- Shapes in the slots the game's contract lists.

A pack cannot change gameplay, hitboxes, scores or timing. The replay test proves this: a run replays to the same score with or without a pack.

## Steps

1. Copy `starter/` to the game's `assets/looks/<id>/`.
2. Set `id` in `look.json` to the folder name.
3. Fill in every `TODO:`. The pack will not load until none are left, and the URL counts too.
4. Change the colours to the game's own roles. Read `src/ui/theme.rs` (`PALETTE` and `STRINGS`) for the keys. A key the game does not declare is rejected.
5. Draw the logo as an SVG. Render it to a PNG:
   `tools/render_logo.sh logo.svg assets/looks/<id>/logo.png 128`
   This needs `rsvg-convert` (librsvg).
6. If you want a QR code, make one. It needs Python and the packages in `tools/requirements-kit.txt`. Install them in a virtual environment:
   `pip install -r tools/requirements-kit.txt`
   Then make the code:
   `tools/make_qr.py https://example.com/offer assets/looks/<id>/qr.png`
   Then set `end_card.image` to `qr.png`.
7. Run `cargo test --test looks`.

`art/logo.svg` is a placeholder you can copy and edit.

## Preview

- Native: `cargo run -- --look <id>` or `GX_LOOK=<id> cargo run`.
- Web: open `?look=<id>&mute=1`.

## Limits

The limits are in `src/look/validate.rs`, module `limits`. They cover the file count, PNG size and dimensions, and string lengths. A pack of 64 KiB or less is built into the web bundle. A larger pack is fetched when a player asks for it with `?look=<id>`.

## Before you pitch

- Search the web and trademark records for the brand name.
- Do not use a real brand's marks unless you have a licence.
- Keep the URL honest. Use `https://example.com/...` until a sponsor signs.
- Leave no `TODO:` in the pack.

## Example

Voidrunner has a full pack in `assets/looks/fizzix/`.
