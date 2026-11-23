# Look Packs

A Look Pack restyles a game without touching how it plays: colours, words,
slot recipes and images, sounds. Scores, replays and the leaderboard are the
same under every pack (decision D1). A pack is data only: no code, no fonts,
nothing the game runs (decision D7, "modding-lite").

**Packs are not yet safe from anyone.** The validator checks sizes, counts,
names and file headers (PNG dimensions, the Vorbis identification header);
it does not decode a whole file. A PNG body is decoded with error handling,
but Bevy's audio decoder can panic on a malformed Vorbis body. Until a full
decode probe lands (step 5 of the Look Packs roadmap), take packs only from
allowlisted creators.

## A pack is a folder

```
assets/looks/<id>/
  look.json         required, schema v1 or v2
  *.png  *.ogg      optional, referenced from look.json
```

- Flat: no subfolders, no links, no fonts, no code.
- File names: lowercase letters, digits, `-` and `_`, then `.png` or `.ogg`.
- `<id>` is the folder name: `[a-z0-9-]{1,32}`.

Folders in `assets/looks/` are built into the game. Native bundles (Pi,
x86) embed every one. The web bundle embeds only packs of at most 64 KiB
(`look::split::WEB_EMBED_MAX_BYTES`); a larger one (audio, art) is left out,
listed in `look::pack::FETCHABLE` with its build-time content hash, and
fetched when `?look=<id>` names it. Only the files in the table are
requested, concurrently, from `assets/looks/<id>/<file>?h=<first 16 hex of
the build-time content hash>`. The URL is relative, so it is same-origin and
also resolves under the full bundle's `/full/<token>/` path (`build_web.sh`
copies `assets/` into `dist/` and `dist/full/`). The `?h=` query is a cache
buster: the files sit at stable unhashed URLs, so without it a browser's
cached copy from before a redeploy would fail the hash check. The game boots
on its own look; when all files are in, their content hash must equal the
build-time one and the validator must pass, then the pack is applied through
`look::swap::LookSwap` and `LookChanged` is sent, so appliers restyle live. A
big pack costs only the players who pick it. A fetch that fails (network,
HTTP status, hash or validation) logs one warning, applies nothing and keeps
the game's own look. Anything else is a folder you point the game at while
working on it (native only). Symlinks are skipped there too: the generator
(`src/look/embed.rs`, run by `build.rs`) warns and leaves them out. A
built-in pack whose folder name or a file name breaks the rules above
fails the build.

To try the fetch with small packs, lower the threshold for one local
build: `GX_WEB_EMBED_MAX=100 ./build.sh web` (bytes; testing only, never
for a shipping build), serve `dist/` and open `?mute=1&look=<id>`. The
console says `look: fetching <id>`, then `look: now wearing <id>`.

## Hosted looks (web)

A host page can dress a running web build in a pack it hosts, without a
redeploy. It sends:

    { "type": "gx:custom", "v": 1, "name": "look.set",
      "data": { "id": "fizzix", "base": "https://colecovisiongx.com/looks/voidrunner/<hash>/",
                "hash": "<content hash>", "files": ["look.json", "logo.png"] } }

and `look.clear` (no data) to go back to the game's own look. The game
answers every command with a `custom` event `look.applied`:
`{"id":"fizzix","ok":true}` or `{"id":"fizzix","ok":false,"reason":"hash"}`.
Reasons: `origin`, `caps`, `fetch`, `hash`, `unsupported` (native),
`invalid: <first problem>`.

Trust: the game fetches only under `HOSTED_ORIGIN`
(`https://colecovisiongx.com/looks/`, compiled in) and wears a pack only if
its content hash matches `hash` and it passes the validator, so anyone can
send the command but only published packs can load. `id` must be the
manifest's `id`.

Publishing is done from the website repo: `scripts/publish-look.mjs`.
To test against a local website, build with
`GX_LOOK_ORIGIN=http://localhost:3000/looks/ ./build.sh web`.

## Try one

| Where | How |
|---|---|
| Native, built-in | `GX_LOOK=night-shift cargo run` (or `cargo run -- --look night-shift`) |
| Native, your folder (live reload) | `cargo run -- --look ./my-packs/acme` — save `look.json` or a PNG and the game updates within half a second; a broken save is logged and the last good look stays |
| Web | `…/?look=night-shift` (built-in packs only) |
| Check | `cargo run --features look-check --bin look-check -- ./my-packs/acme` |

## look.json (schemas v1 to v3)

```json
{
  "v": 1,
  "id": "acme",
  "name": "Acme Snacks Arcade",
  "kind": "sponsor",
  "creator": "Acme Snacks",
  "palette": { "ink": "#1a0f2e", "signal": "#ffc828" },
  "strings": { "title": "ACME ARCADE", "pause": "SNACK BREAK" },
  "slots": { "player": { "recipe": "cuboid", "params": { "size": 1.4 } } },
  "audio": { "sfx.confirm": "crunch.ogg", "music.menu": "jingle.ogg" },
  "fonts": { "display": "Fraunces Black" },
  "frame": { "marquee": "marquee.png" },
  "end_card": { "headline": "THANKS FOR PLAYING", "url": "https://example.com", "image": "logo.png" }
}
```

| Field | Meaning |
|---|---|
| `v` | `1`, `2` or `3`. Use the lowest that has every field the pack uses: `2` for a shape slot, `3` for `glow`; see "Versions". |
| `id`, `name`, `creator` | Identity. `name` ≤ 32 chars, `creator` ≤ 48. |
| `kind` | `palette`, `skin`, `sponsor` or `genre`. A label only; any kind may use any field. |
| `palette` | Role → `#rrggbb` / `#rrggbbaa`. Roles are the game's (see its contract). Unset roles keep the game's colour. |
| `glow` | (v3) Role → `0.0`–`1.0`: how brightly a glowing part shines, as a share of the game's own (`1.0`). It only dims: a palette colour sets a glowing part's hue, never its brightness, so this is how a pack makes one darker. Only roles the game lists in its contract's `glow` are accepted. |
| `strings` | Key → printable ASCII, no longer than the game allows for that key. |
| `slots` | Slot → `{ "image": "x.png", "frames": 1, "fps": 0 }`, `{ "recipe": "<name>", "params": { … } }` or (v2) `{ "shape": [ … ] }`. The game lists which slots take images, which recipes (with which params) it can draw, and which take a shape. A slot has exactly one of the three forms. |
| `audio` | Cue → `.ogg`. Cues are `sfx.<sound>` and `music.<screen>`; the game lists them. A `music.<state>` track replaces that screen's music, including the template's baked layers. `music.howtoplay` only applies when HowToPlay has its own cue, because the template's Menu and HowToPlay share one. |
| `fonts` | `display` / `body` → one of the faces the game already ships. |
| `frame`, `end_card` | `frame`: cabinet marquee/bezel art (checked now; shown from a later release). `end_card`: a sponsor card shown on the game-over screen where the game places an `EndCardSlot`: image (112 px tall, at most 224 px wide, 2:1; a wider image is scaled down) beside headline, body and URL (plain text, never a link). Colours from the roles `endcard`, `endcardhead`, `endcardbody`, `endcardurl`. |
| `protagonist` | Reserved for bring-your-own characters. Ignored, with a warning. |

## End card

```json
"end_card": { "headline": "REFUEL WITH ACME", "body": "Use code ARCADE", "url": "https://example.com", "image": "qr.png" }
```

Only `headline` is required. The URL is shown as text; the game never opens it. Set the `endcard` palette role to give the card its own plate (transparent by default). See `docs/sponsor-kit/` for the sponsor kit.

To check a game's end card, wear `docs/sponsor-kit/end-card-sample/`: an end card (headline, body, URL and logo) and nothing else, so every game's contract accepts it. Run `cargo run -- --look <template>/docs/sponsor-kit/end-card-sample` from the game and lose a run. It is not a built-in pack (nothing builds or ships `docs/`), so never copy it into `assets/looks/`. The template's own `sample-sponsor` sets template roles, strings and a `player` slot, so other games refuse it.

## Versions

`SCHEMA_VERSION` is 3. v2 added `shape` slots; v3 added `glow`. The rules,
enforced by the validator:

- **v1 packs still load**, unchanged.
- **A newer `v` is refused**: `v: 4` on this game fails with `made for a newer
  version of this game (v4)`, and `v: 0` with `unknown schema version (0)`.
  A refused pack resolves to the game's own look, like any invalid pack.
  The version is read on its own before anything else, so a newer pack full
  of fields and primitives this game has never heard of still gets that one
  message, not a parse error.
- **Fields only arrive with a version bump.** A `shape` slot in a `v: 1` pack
  is an error (`shape slots need "v": 2`), and so is `glow` below `v: 3`
  (`glow needs "v": 3`). Every manifest struct denies
  unknown fields, so a field this game does not know fails instead of being
  ignored.

## Shapes (schema v2)

A shape slot describes a mesh as data: a list of primitives, merged into one
mesh by the game. A slot takes a shape only if the game's contract gives it a
budget (`shape: Some(ShapeSpec { … })`); otherwise it fails with `this slot
does not take a shape`. The template's `player` slot does.

```json
"slots": { "player": { "shape": [
  { "prim": "capsule", "size": [0.3, 0.6] },
  { "prim": "cone", "size": [0.3, 0.4], "at": [0, 0.75, 0] }
] } }
```

### Primitives

| `prim` | `size` | Notes |
|---|---|---|
| `sphere` | `[radius]` | |
| `cuboid` | `[x, y, z]` | Full edge lengths. Ignores `detail`. |
| `cylinder` | `[radius, height]` | |
| `cone` | `[radius, height]` | The tip keeps a zero normal; fine for `StandardMaterial`. |
| `torus` | `[ring_radius, tube_radius]` | |
| `capsule` | `[radius, length]` | `length` is the straight section between the hemispheres. |
| `quad` | `[x, z]` | A flat plane in XZ, two-sided (both faces generated, normals ±Y). Ignores `detail`. |

`size` must have exactly the listed number of components. Optional fields on
every primitive:

| Field | Meaning | Default |
|---|---|---|
| `at` | `[x, y, z]` position | `[0, 0, 0]` |
| `rot` | `[x, y, z]` degrees, applied in XYZ Euler order | `[0, 0, 0]` |
| `scale` | `[x, y, z]` | `[1, 1, 1]` |
| `detail` | Segments around the primitive's axis, integer `3..=32`. Spheres use `detail/2` rings (at least 2); a torus uses `detail` for the ring and `detail/2` (at least 3) for the tube. | `12` |

Each primitive is scaled, then rotated, then moved. Normals follow the
transform (inverse-transpose, renormalised), so a stretched `scale` still
shades correctly. `"shape": []` is valid: an empty mesh that draws nothing.

### Budgets

The game sets three numbers per slot (`ShapeSpec` in `src/ui/theme.rs`). The
template's `player` slot: `max_prims: 12`, `max_vertices: 3000`, `extent: [1.5, 1.5, 1.5]` (`ShapeSpec::cube(12, 3000, 1.5)`).

| Budget | Meaning | Error looks like |
|---|---|---|
| `max_prims` | Primitives in the list. | `slots.player.shape: 13 primitives; at most 12` |
| `max_vertices` | Total vertices of the merged mesh. | `slots.player.shape: 3366 vertices; budget 3000` |
| `extent` | Per-axis half-size `[x, y, z]` of the box the whole shape lives in, so a slot far from cubic is bounded tightly. A wide thin wall might use `[7.5, 0.5, 0.25]`: 15 wide, 1 tall, 0.5 deep. Each `at` component is bounded by its own axis (`-extent[i]..=extent[i]`); each `size` component is `0.001..=2 * max(extent)` (a cheap prefilter against absurd values: most sizes are full edges or lengths, only sphere/torus radii are half-sizes, and sizes are not axis-aligned before `rot`; the built-mesh check below is the real bound); then, once every other rule passes, the built mesh is checked per axis: `abs(x) <= extent[0]`, `abs(y) <= extent[1]`, `abs(z) <= extent[2]` for every vertex (so `at`, `size`, `scale` and `rot` together count). | `slots.player.shape[1]: size components are 0.001..=3`, `slots.wall.shape[0]: at y is 0.6; must be -0.5..=0.5`, `slots.wall.shape: reaches 0.800 on y; extent is 0.5` |

Independent of the game: each `scale` component is `0.01..=4`, every number
must be finite, and `detail` is `3..=32`. Errors name the slot and the
primitive index (`slots.player.shape[2]`).

Vertices per primitive, with `d` = `detail`:

| `prim` | Vertices |
|---|---|
| `sphere` | `(d + 1) * (max(d / 2, 2) + 1)` |
| `cuboid` | `24` |
| `cylinder` | `4d + 2` |
| `cone` | `2d + 1` |
| `torus` | `(d + 1) * (max(d / 2, 3) + 1)` |
| `capsule` | `2d + (d + 1) * 2h`, `h = max(d / 4, 2)` |
| `quad` | `8` |

`look-check` prints the total for each shape slot of a valid pack:

```
$ cargo run --features look-check --bin look-check -- assets/looks/sample-shapes
assets/looks/sample-shapes: shape player: 220 vertices (budget 3000)
assets/looks/sample-shapes: ok, content_hash 2d032c899e93dd27083ce5a230fa4b3b123ec41acc77a7d056c836e29f2cee35
```

(`night-shift` and `sample-sponsor` set no shape, so they print only the `ok`
line.) That 220 is the capsule at the default detail (24 + 13 * 6 = 102), the
cone (25), two cuboids (48), and the sphere at detail 8 (9 * 5 = 45). If a
shape is over budget `look-check` fails with the error above instead. Lower
`detail` first: it is the cheapest way to save vertices.

### Worked example: `sample-shapes`

`assets/looks/sample-shapes/look.json` ("Rocket Run") is a v2 pack whose
`player` slot is a rocket: a capsule body, a cone nose at `y = 0.75`, two fin
cuboids at `x = ±0.36`, and a small sphere window at `detail: 8`. Try it:
`GX_LOOK=sample-shapes cargo run`, or `?look=sample-shapes` on web.

### How a game adopts shapes

1. **Give the slot a budget**: set `shape: Some(ShapeSpec { max_prims,
   max_vertices, extent })` on its `SlotSpec` in `LOOK` (`ShapeSpec::cube(max_prims,
   max_vertices, e)` for square slots; `extent` is `[x, y, z]`). Pick `extent` per axis from
   the art's real size (the sim's hitbox is the ceiling, since art is sized to
   it) and `max_vertices` from what the platform can draw many of; a Pi 4 in
   browser mode is the weak target.
2. **Use the shape**, either way:
   - **Handle-based art.** The game already spawns entities that draw
     `Handle<Mesh>`s it created (the Voidrunner pattern: the player ship,
     wraiths, sentinels, pickups and projectiles all share meshes built at
     startup). From render-side code
     (`AssetsPlugin`, never the sim), call
     `ShapeTargets::register("slot_name", mesh_handle)`. `apply_shapes`
     (registered by `LookPlugin`) then rewrites the mesh asset behind that
     handle whenever the pack changes, so every entity drawing it changes
     with no new entities, components or children. `register` is valid at any
     time, including for meshes created after startup, and registering the
     same handle twice is a no-op.
   - **Mesh built at spawn.** For a one-off like the template's `player`,
     match `look.slot("player")` for `Slot::Shape(s)` and call
     `build_mesh(&s.shape)` (see `player_mesh` in `src/game/player.rs`).
3. **Only handles whose mesh is in main-world memory.** Targets must keep
   `RenderAssetUsages::MAIN_WORLD` (the `Mesh` default); the baseline is read
   from it. A target registered before its asset exists waits for it, and
   stays listed if it never arrives. A registered mesh whose asset is removed
   after it was baselined (first seen) is forgotten.
4. **Registered meshes are static.** The baseline is captured once, at first
   sight. A game that edits a registered mesh at runtime will see its old
   baseline written back whenever the look returns to the default; animate
   transforms or materials instead, or register a different handle.

With no pack, or a pack that sets no shape for the slot, nothing is written
and the mesh is bit-identical to the game's own. When a pack with a shape is
swapped for one without, the baseline captured at first sight is written back.

### The values-only rule

Shapes obey the same one rule as everything else in a pack. `apply_shapes`
writes the contents of `Assets<Mesh>` behind handles that already exist; it
never adds or removes entities, components or children, and never reads or
writes sim state. Hitboxes, spawn positions and collision stay the game's
own, so a shape changes how a thing looks, never how it plays. `LookPlugin`
stays out of `GamePlugin`; the headless replay verifier has no `Assets<Mesh>`
and skips shapes entirely.

## The editor (`look.preview`)

An editor build (`--features editor`, `tools/build_editor.sh`) lets the
editor's Looks tab dress the running game in the pack it is writing:

| Direction | Message | Meaning |
|---|---|---|
| editor → game | `look.preview {id, files: {"look.json": {text}, "<name>.png": {png: base64}, "<name>.ogg": {ogg: base64}}}` | Validate this pack and wear it. |
| editor → game | `look.preview.clear {}` | Go back to the game's own look. |
| game → editor | `look {id, ok, errors: [string], hash, warnings: [string]}` | The validator's verdict, the pack's content hash when `ok`, and its warnings. |

- These are not the hosted looks' `look.set` / `look.clear` (above, which
  editor builds answer too, with `look.applied`). The two share one rule:
  every look command of either kind, accepted or refused, supersedes a fetch
  in flight, so whichever command came last is the look the game wears. A
  preview sent while a hosted (or `?look=`) pack is still downloading wins,
  and that pack's late arrival is dropped; a hosted `look.set` after a
  preview replaces it once its files arrive.

- The game runs this page's validator on the files as given, with `id` as
  the folder name. A valid pack is worn through `look::swap::LookSwap` and
  `LookChanged` is sent, so appliers restyle live. An invalid one changes
  nothing: the last good look stays, and `errors` lists each problem as
  `<where>: <what>` (`palette.coin: colours are #rrggbb or #rrggbbaa`).
  `hash` is the same BLAKE3 content hash `look-check` prints; `null` when
  not `ok`. `warnings` (beside the spec's fields) lists the validator's
  warnings on a pack it took, in the same form (an unreferenced file, say).
  A payload that is not JSON answers `{"id": "", "ok": false, "errors":
  ["pack: not JSON"], ...}`. The name and hash come from the look the game
  wears after the validator accepted it; nothing reads the bytes first.
  `look.preview.clear` answers `{"id": "default", "ok": true, "errors": [], "hash": null, "warnings": []}`.
- The catalog (`tunables`) carries `looks`: the contract as data (palette
  roles with their default colours, string keys with defaults and maximum
  lengths, image slots, audio cues, and `end_card` when the game declares
  the four `endcard*` roles), the built-in packs plus the ones `look.preview`
  wore this session, and the validator's limits (`validate::limits`,
  `split::WEB_EMBED_MAX_BYTES`). Code: `src/look/editor.rs`.
- Looks stay cosmetic: `tests/looks_template.rs`
  (`a_look_preview_pack_does_not_change_a_replay`) replays the selftest under a
  `look.preview` pack that sets every role and key.
- **Drafts are not built in.** The editor saves a pack the game took
  cleanly (`ok`, no warnings) to `assets/looks/<id>/`, where `build.rs`
  embeds it. A pack it refused, or took with warnings, is still saved, so no
  work is lost, but to `looks-drafts/<id>/` at the top of the repo: same flat
  layout, never read by `build.rs`, the embedder, `build_web.sh` (it ships
  `assets/` only) or the tests. So a draft never turns `cargo test`, CI or a
  release red, and `tests/looks.rs` keeps holding every built-in pack to
  "ships clean". The editor picks the place from the game's verdict on
  exactly the files it saves; a pack whose verdict changes moves between
  the two folders in that save's one commit, so it is never in both, and
  the two share one id namespace. Try a draft by hand with
  `cargo run -- --look looks-drafts/<id>`.
- Its store listing is `looks-listing/<id>.json` (not read by the game).

## Limits

| Thing | Limit |
|---|---|
| `look.json` | 64 KiB |
| Files | 48 including `look.json`; 16 MiB total |
| PNG | 2 MiB, 2048 × 2048 |
| OGG | Vorbis, 4 MiB, 180 s, mono or stereo, ≤ 48 kHz |
| Text | Printable ASCII; per-key length from the game |
| End-card URL | `https://`, ≤ 200 chars |

Any text containing `TODO:` is an error, so an unfinished copy of the sponsor-kit starter cannot load.

The game checks every pack against these before using a byte of it. A pack with
an error does not load (the game keeps its own look); warnings load and log.
Each pack has a `content_hash` (BLAKE3 over every file) that `look-check`
prints; on-chain looks will record it.

## The game's side: the contract

Each game declares what a pack may change in `src/ui/theme.rs` as `LOOK`:
palette roles with their default colours, string keys with defaults and
maximum lengths, slots with their recipes and parameter ranges, audio cues,
bundled fonts, and `glow`: the palette roles whose glow a pack may dim. The
validator rejects anything not in it, so a typo fails loudly. `look-check`
prints the valid names in each error.

List a role in `glow` only where the game multiplies the part's emissive by
`look.glow(role)` (always `0.0..=1.0`); a `LookColour::new(role).emissive(s)`
tag already does. Emissive roles usually set the hue only, so a dark hex
does not dim them; `glow` is how a pack turns a bright part down without
ever turning one up.

Game code uses a look in two ways:

- **Tag it** (no system parameters needed): put `LookColour::new("role")` next
  to a `TextColor`, `BackgroundColor`, `Sprite`, `MeshMaterial2d<ColorMaterial>`
  or `MeshMaterial3d<StandardMaterial>` (`.alpha(a)`, `.emissive(strength)`;
  emissive is for `StandardMaterial` only), or `LookText("key")` next to a
  `Text`. On a text entity the tag colours the text only (its required,
  transparent `BackgroundColor` is left alone); elsewhere it colours
  whichever of `BackgroundColor`, `Sprite` or material is present.
  The look module fills them in on the frame they spawn and on every reload.
  A material tag recolours the material **asset**, so every entity drawing
  that handle changes with it, tagged or not; when tagged entities with
  different roles share one handle, the last one the query visits wins, in
  no set order. Give each role its own material (2D games often share one
  `ColorMaterial` per colour, which is fine as long as one colour is one
  role). Only the material's colour is written: a translucent `.alpha()`
  needs a material that blends (`ColorMaterial::default()` does;
  `ColorMaterial::from(opaque colour)` is opaque).
- **Ask it**: `Option<Res<ActiveLook>>` and `colour(role)`, `string(key)`,
  `slot(name)`, `glow(role)`, `recipe_number(slot, recipe, param)`; `Res<LookAssets>` for a
  pack's decoded images and audio overrides.

## Helpers: what to reach for

`LookColour` and `LookText` cover a text's colour and words, a flat
`BackgroundColor`, a `Sprite` and a material's base colour. For the rest,
`src/look/` has one tested helper each; a port uses these and writes no
game-local copy. All are tags (spawn code puts them next to the game's
own values; `LookPlugin` applies them in `LookSet` on the frame they
spawn and on every `LookChanged`, which `look.clear` and
`look.preview.clear` also send, so a clear puts the role defaults back).

| Need | Use | Notes |
|---|---|---|
| A node's border | `LookBorder::new(role)` (`.alpha(a)`) | Uniform borders only: it writes all four sides of `BorderColor`, so a top-only accent would become four coloured sides even under the default look. |
| Gradient stops | `LookGradient::two(LookTint::new(top), LookTint::new(bottom))`, `LookGradient::new([Some(t), None, ..])`, `LookGradient::role(role, &[alphas])` | Stop `i` of every gradient in the `BackgroundGradient`; `None` keeps the spawned colour. Positions and shape untouched. |
| A card (`card_with`) | `LookColour::new(fill)` + `LookBorder` + `LookGradient::two` | The roles' defaults must equal the `CardStyle` literals. |
| A glow that is a multiple of the base | `LookColour::new(role).emissive(s)` | Emissive = colour × s × `glow`. |
| A glow that is **not** a multiple of the base | `LookColour::new(role)` + `LookGlow::new(role, game_emissive)`, **the same role on both** | Default look: exactly `game_emissive`. Pack: the pack's colour for the role × the game's emissive-to-default brightness ratio, × the pack's `glow` for the role, so one palette entry recolours base and glow together and one `glow` entry dims it. List the role in the contract's `glow`. 3D only (compiled out under `gx_look_no_pbr`). Never combine with `.emissive(s)` on one material. A separate glow role (`LookGlow::new("rail_glow", ..)`) is the exception, only for a part whose glow a pack should recolour independently of its body: the contract then lists `rail_glow` in `palette` and `glow`, and a pack must set and dim `rail_glow` itself (setting `rail` alone leaves the glow in the game's hue). |
| A fitted line whose words a pack sets | `LookText(key)` + `face.look_fit(key, max)` or `face.look_fit_in(key, max, box_px)` + the face's `TextFont` | `refit_text` sets `font_size`: the game's own string keeps exactly its `face.fitted(default, max)` size (with `look_fit_in` too: the box never re-fits the default, even where the default overflows it); any other is sized at the face's `widest_em` against the line (or box), never above the default's size, so any string up to `max_chars` fits. Only for a line whose whole text is `look.string(key)`: a composed line (`"OWN " + title`, a formatted score) gets the bare string's size and overflows, so size it from a game system. |
| A colour a game system paints from state | No tag. The system reads `look.colour(role)` / `look.mix(from, to, t)` and re-runs on a look change. | See pattern 1 below. |
| A shared material that sim entities draw | An anchor entity (below). | |

### The four review patterns

Every port review in the first wave found one of these. Check each.

1. **Static tags on system-written colours.** A heart that goes grey when
   lost, a rarity rim, combo heat: a game system writes the colour from
   state. A static `LookColour` on it repaints it the role's flat colour on
   every `LookChanged`, so a look change mid-run draws lost hearts as lit.
   Leave such entities untagged; the game system reads the role from
   `Option<Res<ActiveLook>>` and also re-runs when the look changes
   (`look.is_changed()`, or a `LookChanged` reader), writing exactly its
   old colour under the default look. The same goes for every tag, not only
   `LookColour`: no `LookBorder` on a border a system highlights (focus,
   hover, selection), no `LookGradient` on a gradient a system animates,
   no `LookFit` on a size a system animates (a score pop); each tag
   rewrites its value on every `LookChanged`.
2. **Fit-at-spawn-only.** A line sized at spawn from the pack's string
   keeps that size when a hosted `look.set`, a reload or a clear changes
   the words. Use `LookFit`, or rebuild the screen on `LookChanged`.
3. **Fit width vs container.** A line fitted against the view but drawn in
   a card, plank or rail overflows the box with a pack's longer word. Give
   it `look_fit_in(key, max, interior)` (width less padding and border):
   a pack's words fit the box, and the game's own string keeps exactly
   its `fitted` size, so the default look cannot change. If the game's own
   string overflows its box too, that is a default-look bug: fix the
   default (shorter words, a smaller `max`) as a deliberate, screenshotted
   change, not through the tag.
4. **Widest-glyph `max_chars`.** `max_chars` counts characters, and
   `Face::advance_em` is an average: twelve `W`s are far wider than twelve
   average glyphs. `LookFit` sizes a pack's words at `Face::widest_em`, the
   widest printable-ASCII glyph (every character the validator accepts,
   so `@` and `%` count, not only A–Z and 0–9). For a fixed-size line, set
   `max_chars` so `max_chars × widest_em × size` fits the box, and test it
   against the TTF.

### Anchor entities for shared materials

A material handle that sim spawns draw (every tire, every scoop flavour)
cannot be tagged on those entities: sim spawns never carry look tags.
Spawn one persistent anchor per handle from render-side code
(`AssetsPlugin` / `UiPlugin`, once the handles exist):

```rust
for (role, handle) in FLAVOR_ROLES.iter().zip(&assets.flavor_materials) {
    commands.spawn((LookColour::new(role), MeshMaterial3d(handle.clone())));
}
// One role for base and glow: a pack's "rail" recolours both, and its
// glow {"rail": 0.5} dims the glow.
commands.spawn((
    LookColour::new("rail"),
    LookGlow::new("rail", RAIL_EMISSIVE),
    MeshMaterial3d(assets.rail_material.clone()),
));
```

The material retint rewrites the asset, so every mesh drawing the handle
follows, from the first frame and on every change. The anchor has no
mesh, so it draws nothing, and no `GameEntity` / `StateScoped`, so it lives
across runs. One role per handle: two anchors with different roles on one
handle race. 2D games use `MeshMaterial2d<ColorMaterial>` the same way.
A colour derived from a role (a tint shift, a mix) cannot be an anchor;
write it from a render-side system that compares before `get_mut`.

**The one rule:** nothing in the sim (`SimSet`, `OnEnter(Playing)` spawns,
anything the replay verifier runs) reads `ActiveLook` or `LookAssets`. Split
an entity into a sim spawn and a render-side dresser instead: the template's
`content::level::spawn_level` spawns the Player Thing's sim half (art on a
child) and `player::dress_player` (registered by `AssetsPlugin`, which the
verifier never adds) gives that art child its mesh from the pack, leaving a
`content/sprites/Player.png` sprite alone. Art is sized to the sim's hitboxes, never the other way
round.

What the test catches, exactly: `tests/looks.rs`
(`no_built_in_pack_changes_a_replay`) replays the selftest headless under
each built-in pack and fails if the score, checksum, end or tick count
moves (the template's `tests/looks_template.rs` adds the GXR2 content hash). So
it catches a sim read **whose result differs under some built-in pack's
values**. It never catches a read behind a render-only branch (the verifier
has no asset stores, so an `Option<Res<Assets<…>>>` branch never runs
there), and it cannot catch a read of a role, key or slot no built-in pack
sets, because then every pack gives the game's own default. When you port
this, add a built-in pack that sets **every** palette role, string key and
slot your game reads, to a value unlike the default.

Editor tunables that only change the look are marked `.cosmetic()` (for
example `#[reflect(@Tune::colour("Coin colour").cosmetic())]`), which keeps
them out of a replay's start values, content hash and checksum.

## Porting into a game

Copy `src/look/` from **template main**, byte for byte, `editor.rs`
included; never edit it in the game, and never diff against another game's
copy (Voidrunner's has drifted from main and is not a reference). The
template's `AGENTS.md` ("Look Packs", **Porting**) lists the Cargo and
`build.rs` lines. Two of them depend on the game:

- **2D without `bevy_pbr`.** The `StandardMaterial` retint needs `bevy_pbr`.
  A game whose Bevy features leave it out adds one line to `build.rs`'s
  `main`, before `look_embed::embed_looks();`:

  ```rust
  println!("cargo::rustc-cfg=gx_look_no_pbr");
  ```

  It is a build-script cfg, not a Cargo feature, because CI builds with
  `--all-features` and the switch has to stay off for 3D games. `embed.rs`
  declares the cfg, so a 3D game that never sets it gets no warning.
  `tools/test_look_2d.sh` (in CI) proves `src/look/` builds this way with
  no `bevy_pbr` in the graph.
- **The editor.** `src/look/editor.rs` is behind `feature = "editor"`, and
  it needs the template's tunables layer (`game::content`). `embed.rs`
  declares the feature value for the `unexpected_cfgs` lint, so a game
  without the editor needs nothing. A game must not declare `editor = []`
  unless it has the tunables layer: CI's `--all-features` would then build
  `editor.rs` and fail.

Tests: copy `tests/looks.rs` unchanged (only the crate name in its `use`
lines changes). It is generic over the game's contract and built-in packs,
and uses only `look::*`, `ui::theme` (`LOOK`, `PALETTE`, `STRINGS`) and the
replay API every template game has; it needs one built-in pack that changes
a palette role and a string key. Do not copy `tests/looks_template.rs`: it
tests the template's player slot, shapes, sprites and editor. Game-specific
look tests (a game-side retint, say) go in a file of the game's own, so
`tests/looks.rs` can be recopied when the template changes it. Check the
end card with `docs/sponsor-kit/end-card-sample/` (see "End card").

Copy `look_contract_tests` from the template's `src/ui/theme.rs`
unchanged (end-card roles, end-card column fit, string defaults); it uses
only `crate::look`, `theme` and `ui::fit::ui_width`. The template's
`look_contract_game_tests` (every sound is a cue a pack can override) reads
the template's own audio module: write a game's own version against its
sound list, or leave it out while `AUDIO_CUES` is empty.
