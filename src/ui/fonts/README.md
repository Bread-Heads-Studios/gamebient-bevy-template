# Embedded fonts

Every face here is compiled into the binary with `include_bytes!` by
`src/ui/fonts.rs` and inserted into `Assets<Font>` at a fixed
`uuid_handle!` id, so nothing loads asynchronously and nothing pops in on
web. All are from [google/fonts](https://github.com/google/fonts) (fetched
2026-10-02, commit `9710da1`), licensed under the SIL Open Font License 1.1.
None of the three families declares a Reserved Font Name, so the cut,
subset files below may keep their names.

| File | Family, weight | Role | Source | Licence |
|---|---|---|---|---|
| `SpaceGrotesk-Bold.ttf` | Space Grotesk Bold (700) | `DISPLAY` (template) | `ofl/spacegrotesk/SpaceGrotesk[wght].ttf` | `SpaceGrotesk-OFL.txt` |
| `Inter-SemiBold.ttf` | Inter SemiBold (600, text optical size) | `BODY` (template) | `ofl/inter/Inter[opsz,wght].ttf` | `Inter-OFL.txt` |
| `Fraunces-Black.ttf` | Fraunces Black (900), 72pt optical size, Soft | `STUDIO` (studio logo, every game) | `ofl/fraunces/Fraunces[SOFT,WONK,opsz,wght].ttf` | `Fraunces-OFL.txt` |
| `Fraunces-Italic.ttf` | Fraunces Italic (400), 9pt optical size, Soft | `STUDIO_ITALIC` (studio logo, every game) | `ofl/fraunces/Fraunces-Italic[SOFT,WONK,opsz,wght].ttf` | `Fraunces-OFL.txt` |

Source URLs are under `https://raw.githubusercontent.com/google/fonts/main/`.

## How they were cut

All four families ship only as variable fonts, so each file is a static
instance at the named weight, then subset to Latin-1 plus common
punctuation (copy is ASCII-only by house rule; the extra range is
headroom). Kerning and the other layout features are kept.

```bash
pip install fonttools
fonttools varLib.instancer 'SpaceGrotesk[wght].ttf' wght=700 --static --update-name-table -o SpaceGrotesk-Bold.full.ttf
fonttools varLib.instancer 'Inter[opsz,wght].ttf' wght=600 opsz=14 --static --update-name-table -o Inter-SemiBold.full.ttf
fonttools varLib.instancer 'Fraunces[SOFT,WONK,opsz,wght].ttf' wght=900 opsz=72 SOFT=50 WONK=1 --static --update-name-table -o Fraunces-Black.full.ttf
fonttools varLib.instancer 'Fraunces-Italic[SOFT,WONK,opsz,wght].ttf' wght=400 opsz=9 SOFT=50 WONK=1 --static --update-name-table -o Fraunces-Italic.full.ttf
U="U+0020-007E,U+00A0-00FF,U+0131,U+0152-0153,U+02C6,U+02DA,U+02DC,U+2013-2014,U+2018-201A,U+201C-201E,U+2020-2022,U+2026,U+2030,U+2039-203A,U+2044,U+20AC,U+2122,U+2190-2193,U+2212"
for f in SpaceGrotesk-Bold Inter-SemiBold Fraunces-Black Fraunces-Italic; do
  pyftsubset $f.full.ttf --unicodes="$U" --layout-features='*' --name-IDs='*' \
    --name-legacy --name-languages='*' --notdef-outline --output-file=$f.ttf
done
```

`--update-name-table` only accepts axis values named in the font's `STAT`
table, which is why the italic uses `opsz=9` rather than an in-between size.

## Adding a face

1. Download the TTF and its `OFL.txt` (or Apache `LICENSE.txt`) from
   google/fonts. If the family is variable-only, cut a static instance as
   above; keep only the weights the game uses.
2. Drop both here, add a row to the table above, and add a `Face` const in
   `src/ui/fonts.rs` with a fresh UUID and its measured `advance_em`.
3. `cargo test` measures every face against the strings the UI fits
   (`fonts::tests`) and fails if the declared `advance_em` is too small.
