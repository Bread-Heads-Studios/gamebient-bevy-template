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
  look.json         required, schema v1
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

## look.json (schema v1)

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
| `v` | Always `1` for now. |
| `id`, `name`, `creator` | Identity. `name` ≤ 32 chars, `creator` ≤ 48. |
| `kind` | `palette`, `skin`, `sponsor` or `genre`. A label only; any kind may use any field. |
| `palette` | Role → `#rrggbb` / `#rrggbbaa`. Roles are the game's (see its contract). Unset roles keep the game's colour. |
| `strings` | Key → printable ASCII, no longer than the game allows for that key. |
| `slots` | Slot → `{ "image": "x.png", "frames": 1, "fps": 0 }` or `{ "recipe": "<name>", "params": { … } }`. The game lists which slots take images and which recipes (with which params) it can draw. |
| `audio` | Cue → `.ogg`. Cues are `sfx.<sound>` and `music.<screen>`; the game lists them. A `music.<state>` track replaces that screen's music, including the template's baked layers. `music.howtoplay` only applies when HowToPlay has its own cue, because the template's Menu and HowToPlay share one. |
| `fonts` | `display` / `body` → one of the faces the game already ships. |
| `frame`, `end_card` | Cabinet marquee/bezel art and a sponsor end card. Checked now; shown by games from a later release. |
| `protagonist` | Reserved for bring-your-own characters. Ignored, with a warning. |

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
