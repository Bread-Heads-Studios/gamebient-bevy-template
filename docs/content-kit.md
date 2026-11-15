# Content kit: editable game data

The template keeps the parts of a game a person (or the editor) should be
able to change in files under `content/`, apart from the Rust that decides
how the game behaves. Every file is optional; a missing file means the Rust
defaults. The files are embedded at build time (`src/game/content/files.rs`)
and the editor can replace them in a running game with `content.set`.

## The files

| File | Shape | Limit |
|------|-------|-------|
| `content/level.json` | `{"v":1,"things":[{"id","kind","at":[x,y],"size":[w,h]?}]}` | 2000 things, 256 KB |
| `content/tunables.json` | `{"v":1,"values":{"<tunable path>": value}}` | 64 KB |
| `content/sprites.json` | `{"v":1,"sprites":{"<kind>":{"frames"?,"fps"?,"scale"?,"anchor"?}}}` | 16 KB |
| `content/sprites/<kind>.png` | PNG, at most 256x256 | 64 KB each, 64 files |
| `content/audio/songs.json` | `{"v":1,"songs":{"<cue>":{...}}}` | 128 KB |
| `content/audio/sfx.json` | `{"v":1,"sfx":{"<SfxEvent name>": <Fx program>}}` | 64 KB |

`id` matches `^[a-z0-9_-]{1,32}$` and is unique within the level. Kind names
match `^[A-Za-z][A-Za-z0-9_-]{0,31}$`; sprite files are `<kind>.png`.
JSON Schemas (draft 2020-12) live in `content/schema/`
(`level`, `tunables`, `sprites`, `songs`, `sfx`); `build.rs` does not embed
them. Parsing and limits are in `src/game/content/model.rs`.

## Data or Rust

| Goes in `content/` | Stays in Rust |
|--------------------|---------------|
| Where Things are, how big (`level.json`) | What a kind does: its systems and components |
| Starting values of tunables (`tunables.json`) | Which numbers are tunable, their ranges (`tuning.rs`) |
| How a kind looks: pictures, frames (`sprites*`) | Kind registration (`thing_kind`) |
| Music and sound effects (`audio/*.json`) | The default songs and `bake`/`program` |

An agent asked to move, add or remove Things edits `content/level.json`; asked
to change how a kind behaves, it edits Rust.

## Adding a kind

Register it in `kinds_registration` (`src/game/mod.rs`) with
`thing_kind(name, KindProps { size, colour, on_spawn })`. Then `level.json`
can place it by name.

- Registration order is the kind's index (`Thing { kind }`). Never reorder
  between releases, exactly like tunables.
- `on_spawn` adds the kind's sim components (markers, bodies). It runs in
  the sim, so it must never depend on whether art exists: the verifier has no
  `ThingArt`, and a branch on it changes the archetype and desyncs replays.
  Art is added by `spawn_level` on a child entity, never by `on_spawn`.
- Order in `GamePlugin::build`: `tunables_registration` and
  `kinds_registration` both come **above** `ContentPlugin`. The plugin applies
  saved tunables and checks the embedded level's kinds when it is built, so a
  tunable or kind registered later is missed. A later `insert_resource` of a
  tunable resource also overwrites the saved values.
- Sprites and the default placeholder box are presentation: a sprite sits on
  a child of the Thing, and `content/sprites*` never touch the sim.

## `test_level()`

`content::level::test_level()` is the level the replay selftest and the named
checks run on (`LevelSource::Test`), built in Rust so a data save never
changes them. Replace it with your game's layout; it must only use
registered kinds (`test_level_uses_registered_kinds_and_is_valid` checks).
Those runs also reset tunables to the Rust defaults
(`saved::reset_to_defaults`), so neither `content/level.json` nor
`content/tunables.json` can invalidate the committed fixtures.

The rule for tests: a data save to `content/` (moving or adding Things, a
new or redrawn sprite, saved card values) must never fail fmt, clippy or
`cargo test`; only a Rust change may. So tests that play the game run on
built-in content (`LevelSource::Test`, `saved::reset_to_defaults`, and
`sprites::clear_sprites` with fixture PNGs), and the shipped files are
checked for validity only (`the_content_level_file_is_valid`,
`the_shipped_sprites_are_valid`), never for what they hold.

## What the replay hashes

Run content is the **level** and the **tunables**; presentation is not.

- The replay header's `content` is `tunables::hash::content_of_bytes`: it
  folds the level hash first, then every tunable start value and mid-run
  change.
- The level hash is the FNV-1a 64 of `content/level.json`'s raw bytes
  (`LevelHash`; `fnv1a64(b"test")` for `LevelSource::Test`).
- `replay::level_source_for(replay, content_level)` picks the verifier's
  level: the content level, unless the replay's `content` equals what the
  test level would give and not what the content level gives (the selftest
  fixtures).
- Sprites, `sprites.json`, songs and sfx never enter the sim or the hash.
  No sim system reads them.

A leaderboard accepts a run only when `content` equals the release's default
hash, so editing the level or a saved tunable changes the default for the next
release; it does not invalidate old releases' verifiers.

## File hash and `content.set`

Everywhere a file is named (the catalog's `files`, the UI, tests) its hash is
FNV-1a 64 over the **exact bytes**: offset `0xcbf29ce484222325`, prime
`0x100000001b3`, written as 16 lowercase hex digits (`files::fnv1a64`). Never
re-serialise JSON before hashing; serde_json and JavaScript order keys
differently.

In the editor build (`--features editor`) the host sends `content.set` with
`{"file": "<path under content/>", "text": "<exact file text>"}` (`.json`) or
`{"file", "png": "<base64>"}` (a sprite). The game answers `content` with
`{"file","ok","error"?,"hash"}`. `level.json` applies at the next run start
(send `run.restart`); `tunables.json` goes through `PendingTunes`; sprites and
audio apply in place. The protocol table is the module doc of
`src/game/tunables/protocol.rs`; `sound.preview` takes an event or cue name.

## Porting a game to the kit

1. Cues and layers: constructors take a name first
   (`Cue::loop_baked("name", f)`, `Layer::baked("name", f)`);
   `with_layers` takes `&[Layer]`. Every cue and layer name is unique
   snake_case.
2. Beside the per-game `bake`, add `program(event) -> Option<Fx>`: the chip
   program for an event, `None` for one that is not.
3. `EditorInfo.sounds` is now a `Vec<String>` filled by the protocol plugin
   (`audio::sound_names()`); do not fill it by hand.
4. Replace hand spawns of level entities with kinds: one `thing_kind` per
   kind, the positions go to `content/level.json`. `spawn_level` replaces the
   game's own spawn system.
5. Write `test_level()` and make the selftest and checks use it.
6. Regenerate the fixtures (`tests/fixtures/selftest*.gxr`; commands in
   `docs/replay-verification.md`), since the content hash now folds the level.
