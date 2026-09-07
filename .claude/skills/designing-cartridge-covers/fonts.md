# Fonts under rsvg-convert (macOS)

rsvg-convert shapes text with pango against the system font list. `fc-match`
resolving a name does **not** mean it renders: several faces silently fall
back to Helvetica, which also changes text width and overflows the page.

## Always probe first

```bash
cat > build/cartridge/<game>-probe.svg <<'EOF'
<svg xmlns="http://www.w3.org/2000/svg" width="900" height="120">
  <text x="10" y="40" font-family="Phosphate" font-size="34">Phosphate ABC abc</text>
  <text x="10" y="80" font-family="Helvetica Neue" font-size="34">Helvetica Neue ABC abc (control)</text>
</svg>
EOF
rsvg-convert build/cartridge/<game>-probe.svg -o build/cartridge/<game>-probe.png
```

Read the PNG. If a line looks like the control line, the face fell back.

## Verified on this studio's Mac

| Voice | Faces that render |
|-------|-------------------|
| Chunky arcade / display | Phosphate, Arial Black, Krungthep, Gill Sans Ultra Bold (`font-weight="900"`), Orbitron |
| Condensed poster | Helvetica Neue or Futura with `font-stretch="condensed" font-weight="900"`, Avenir Next Heavy |
| Slab / signage | Rockwell, Copperplate (bold and light) |
| Serif / field guide / heraldry | Didot, Baskerville (italic and bold), Georgia, Optima |
| Carved / pirate / fantasy | Trattatello, Luminari, Herculanum, Papyrus |
| Hand-lettered / chalk / marker | Marker Felt (`font-weight="bold"` selects the Wide face), Chalkduster, Chalkboard SE, Bradley Hand, Noteworthy |
| Script | Snell Roundhand (bold) |
| Typewriter / mono / corporate | American Typewriter, 'Courier New', Menlo |
| Pixel | Press Start 2P (installed here; check with `fc-list`) |

## Silent fallbacks (avoid)

Impact, DIN Condensed, plain Courier (use 'Courier New'), Cooper Black (not
installed), Hoefler Text, Big Caslon, Superclarendon, "Phosphate Inline" /
"Phosphate Solid" family names. Avenir Next Condensed loses its condensed
width.

## Other rsvg text behaviour

- `word-spacing` is ignored; space words with `<tspan dx="…">`.
- Whitespace between adjacent `<tspan>`s collapses; put multi-colour word rows
  in separate `<text>` elements.
- Always give a fallback stack (`font-family="Rockwell, Georgia, serif"`) so a
  machine without the face degrades legibly.
- Text does not wrap. Long titles need explicit line breaks and a size check.
- `<textPath>` (text on a curve) is silently dropped by rsvg-convert 2.62;
  the text vanishes. Use straight text, or rotate a `<text>` per word.
- `paint-order="stroke"` is unverified here; for outlined titles stack two
  copies of the `<text>` (stroke below, fill above) as the template covers do.
