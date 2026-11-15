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

Folders in `assets/looks/` are built into the game, every bundle of it, so
each built-in pack adds its files to the download. Anything else is a folder
you point the game at while working on it (native only). Symlinks are
skipped there too: `build.rs` warns and leaves them out.

## Try one

| Where | How |
|---|---|
| Native, built-in | `GX_LOOK=night-shift cargo run` (or `cargo run -- --look night-shift`) |
| Native, your folder (live reload) | `cargo run -- --look ./my-packs/acme` — save `look.json` or a PNG and the game updates within half a second (except the menu title, which is fitted when the menu spawns and refreshes the next time it opens); a broken save is logged and the last good look stays |
| Web | `…/?look=night-shift` (built-in packs only) |
| Check | `cargo run --features look-check --bin look-check -- ./my-packs/acme` |

## look.json (schema v1 and v2)

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
| `v` | `1` or `2`. Use `2` only if the pack has a shape slot; see "Versions". |
| `id`, `name`, `creator` | Identity. `name` ≤ 32 chars, `creator` ≤ 48. |
| `kind` | `palette`, `skin`, `sponsor` or `genre`. A label only; any kind may use any field. |
| `palette` | Role → `#rrggbb` / `#rrggbbaa`. Roles are the game's (see its contract). Unset roles keep the game's colour. |
| `strings` | Key → printable ASCII, no longer than the game allows for that key. |
| `slots` | Slot → `{ "image": "x.png", "frames": 1, "fps": 0 }`, `{ "recipe": "<name>", "params": { … } }` or (v2) `{ "shape": [ … ] }`. The game lists which slots take images, which recipes (with which params) it can draw, and which take a shape. A slot has exactly one of the three forms. |
| `audio` | Cue → `.ogg`. Cues are `sfx.<sound>` and `music.<screen>`; the game lists them. A `music.<state>` track replaces that screen's music, including the template's baked layers. `music.howtoplay` only applies when HowToPlay has its own cue, because the template's Menu and HowToPlay share one. |
| `fonts` | `display` / `body` → one of the faces the game already ships. |
| `frame`, `end_card` | Cabinet marquee/bezel art and a sponsor end card. Checked now; shown by games from a later release. |
| `protagonist` | Reserved for bring-your-own characters. Ignored, with a warning. |

## Versions

`SCHEMA_VERSION` is 2. The rules, enforced by the validator:

- **v1 packs still load**, unchanged.
- **A newer `v` is refused**: `v: 3` on this game fails with `made for a newer
  version of this game (v3)`, and `v: 0` with `unknown schema version (0)`.
  A refused pack resolves to the game's own look, like any invalid pack.
  The version is read on its own before anything else, so a newer pack full
  of fields and primitives this game has never heard of still gets that one
  message, not a parse error.
- **Fields only arrive with a version bump.** A `shape` slot in a `v: 1` pack
  is an error (`shape slots need "v": 2`). Every manifest struct denies
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
| `extent` | Per-axis half-size `[x, y, z]` of the box the whole shape lives in, so a slot far from cubic is bounded tightly. A wide thin wall might use `[7.5, 0.5, 0.25]`: 15 wide, 1 tall, 0.5 deep. Each `at` component is bounded by its own axis (`-extent[i]..=extent[i]`); each `size` component is `0.001..=max(extent)` (sizes are not axis-aligned before `rot`, so the largest axis bounds them); then, once every other rule passes, the built mesh is checked per axis: `abs(x) <= extent[0]`, `abs(y) <= extent[1]`, `abs(z) <= extent[2]` for every vertex (so `at`, `size`, `scale` and `rot` together count). | `slots.player.shape[1]: size components are 0.001..=1.5`, `slots.wall.shape[0]: at y is 0.6; must be -0.5..=0.5`, `slots.wall.shape: reaches 0.800 on y; extent is 0.5` |

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

## Limits

| Thing | Limit |
|---|---|
| `look.json` | 64 KiB |
| Files | 48 including `look.json`; 16 MiB total |
| PNG | 2 MiB, 2048 × 2048 |
| OGG | Vorbis, 4 MiB, 180 s, mono or stereo, ≤ 48 kHz |
| Text | Printable ASCII; per-key length from the game |
| End-card URL | `https://`, ≤ 200 chars |

The game checks every pack against these before using a byte of it. A pack with
an error does not load (the game keeps its own look); warnings load and log.
Each pack has a `content_hash` (BLAKE3 over every file) that `look-check`
prints; on-chain looks will record it.

## The game's side: the contract

Each game declares what a pack may change in `src/ui/theme.rs` as `LOOK`:
palette roles with their default colours, string keys with defaults and
maximum lengths, slots with their recipes and parameter ranges, audio cues,
bundled fonts. The validator rejects anything not in it, so a typo fails
loudly. `look-check` prints the valid names in each error.

Game code uses a look in two ways:

- **Tag it** (no system parameters needed): put `LookColour::new("role")` next
  to a `TextColor`, `BackgroundColor`, `Sprite` or `MeshMaterial3d<StandardMaterial>`
  (`.alpha(a)`, `.emissive(strength)`), or `LookText("key")` next to a `Text`.
  On a text entity the tag colours the text only (its required, transparent
  `BackgroundColor` is left alone); elsewhere it colours whichever of
  `BackgroundColor`, `Sprite` or material is present.
  The look module fills them in on the frame they spawn and on every reload.
- **Ask it**: `Option<Res<ActiveLook>>` and `colour(role)`, `string(key)`,
  `slot(name)`, `recipe_number(slot, recipe, param)`; `Res<LookAssets>` for a
  pack's decoded images and audio overrides.

**The one rule:** nothing in the sim (`SimSet`, `OnEnter(Playing)` spawns,
anything the replay verifier runs) reads `ActiveLook` or `LookAssets`. Split
an entity into a sim spawn and a render-side dresser instead: the template's
`player::spawn_player` spawns the sim half and `player::dress_player`
(registered by `AssetsPlugin`, which the verifier never adds) gives it its
mesh from the pack. Art is sized to the sim's hitboxes, never the other way
round.

What the test catches, exactly: `tests/looks.rs`
(`no_built_in_pack_changes_a_replay`) replays the selftest headless under
each built-in pack and fails if the score, checksum or tick count moves. So
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
