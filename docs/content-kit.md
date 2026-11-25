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
| `content/rules.json` | `{"v":1,"rules":[{"id","when","if"?,"do","source"?,"off"?}]}` | 64 rules, 32 KB |

`id` matches `^[a-z0-9_-]{1,32}$` and is unique within the level. Kind names
match `^[A-Za-z][A-Za-z0-9_-]{0,31}$`; sprite files are `<kind>.png`.
JSON Schemas (draft 2020-12) live in `content/schema/`
(`level`, `tunables`, `sprites`, `songs`, `sfx`, `rules`); `build.rs` does not embed
them. Parsing and limits are in `src/game/content/model.rs`.

## Data or Rust

| Goes in `content/` | Stays in Rust |
|--------------------|---------------|
| Where Things are, how big (`level.json`) | What a kind does: its systems and components |
| Starting values of tunables (`tunables.json`) | Which numbers are tunable, their ranges (`tuning.rs`) |
| How a kind looks: pictures, frames (`sprites*`) | Kind registration (`thing_kind`) |
| Music and sound effects (`audio/*.json`) | The default songs and `bake`/`program` |
| Rules the core vocabulary can say (`rules.json`) | Rules it cannot: the game's own systems |

An agent asked to move, add or remove Things edits `content/level.json`; asked
to change how a kind behaves, it edits Rust.

## Adding a kind

Register it in `kinds_registration` (`src/game/mod.rs`) with
`thing_kind(name, KindProps { size, colour, on_spawn, edit })`. Then
`level.json` can place it by name. `edit` is the kind's `EditRules`: whether
the level editor may resize it (`resizable`, its `size` is then written),
the most a level may hold (`max`, `None` for no limit) and whether the editor
may add it at all (`placeable`; set it to `false` for kinds spawned only at
runtime, such as bullets, which the catalog then leaves out). The template's Player
is `max: Some(1)`; a goal is usually one of a kind too, and a platform
resizable. The editor's grid step is the `LevelGrid` resource (default 20
world units); insert your own above `ContentPlugin` when your world units are
not pixels.

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
`the_shipped_sprites_are_valid`), never for what they hold. In code: start
from `replay::build_builtin_app()` (or call `replay::use_builtin_content` on
a verify app), and change a value with `tunables::index::bumped_in_range`,
which stays inside the tunable's range even when the value is at its end.

`tools/test_extreme_content.sh` (run in CI) proves the rule: it copies the
repo to a scratch folder, saves every tunable at its max and then at its min
(the file comes from the ignored unit test `write_extreme_tunables`, so it
follows the game's own tunables), moves one Thing and removes one, and runs
`cargo test --all-targets --all-features` there, in its own target dir
(`target/extreme-content`). Run it after adding a test that plays the game.

## What the replay hashes

Run content is the **level**, the **rules** and the **tunables**; presentation is not.

- The replay header's `content` is `tunables::hash::content_of_bytes`: it
  folds the run key first (`hash::with_rules(level hash, rules hash)`), then
  every tunable start value and mid-run change. A file with no rules hashes
  as 0, and `with_rules(level, 0)` is the level hash, so runs without rules
  keep the hash they always had.
- The level hash is the FNV-1a 64 of `content/level.json`'s raw bytes
  (`LevelHash`; `fnv1a64(b"test")` for `LevelSource::Test`).
- `replay::level_source_for(replay, content_level)` picks the verifier's
  level: the content level, unless the replay's `content` equals what the
  test level would give and not what the content level gives (the selftest
  fixtures).
- Sprites, `sprites.json`, songs and sfx never enter the sim or the hash;
  `rules.json` does.
  No sim system reads them.

A leaderboard must accept a run only when it was played on the release's
default content (so test-level runs and runs on edited content stay off it).
The verifier reports both halves: `default_content`, the hash its own
embedded `content/` gives a run that edits nothing, and
`is_default_content`, the run's level, start values and changes compared
with those defaults exactly. The website refuses a run unless both say it
is the default. Editing the level or a saved
tunable changes the default for the next release; it does not invalidate old
releases' verifiers.

## Rules as data

`content/rules.json` holds rules a card can say: a trigger (`start`,
`every`, `button`, `touch`, `counter`, `timer`), conditions (`compare`,
`chance`) and actions (`add`, `set`, `remove`, `spawn`, `respawn`, `jump`,
`move`, `sound`, `timer`, `end_run`). `src/game/rules/model.rs` checks a
file whole, with one sentence per refusal ("This game has no counter called
gems."); `run.rs` runs it in `SimSet` after physics and the game's own
rules (`run_rules`), deterministically: file order, `SpawnOrder` pairs,
`GameRng` draws, `RunClock` ticks, queued spawns and removals, a 4096-action
cap per tick, counters and timers folded into the `Checksum`.

A game declares what rules may name in `src/game/rule_table.rs` (its own
file): `COUNTERS` (each a `GameData` field with a label and a range), `JUMP`
and `STOP` (hooks into its physics; `jump` respects the game's grounded
check), `HALF_SCREEN`, and `test_rules()`, the rules the selftest and the
named checks run under `LevelSource::Test`. Kinds come from `thing_kind`
and sounds from `SfxEvent`. Everything else in `src/game/rules/` is copied
verbatim (`tools/frame-accept.sh --check-copies`), and the editor carries
byte-identical copies of `model.rs` and `cases.json`, so the game, the
editor and its server accept and refuse the same files.

`content.set rules.json` checks the file whole and swaps it in: a run in
progress keeps going on the new rules from the next tick (unchanged rules
keep their edge memory; timers whose names are gone are dropped). The
catalog's `rules` key carries the vocabulary the editor builds its
dropdowns from.

## Checks as data

`content/checks.json` holds plain-English checks: a sentence (`source`)
and a small staged test that should always pass. `given` builds the scene
(`level` `"empty"` or `"current"`, `player.at`, up to 16 Things placed `at`
a point or `beside` the player, starting values in `set`), `do` plays up
to 16 steps (`hold`, `press`, `wait`, `until`; at most 1800 ticks in all)
and `expect` holds 1 to 8 expectations (`counter`, `change`, `count`,
`outcome`, `sound`; `"ever": true` means "at some tick"). Comparisons are
the rules' `<`, `<=`, `=`, `>=`, `>`.

`src/game/checks_data/model.rs` checks a file whole, with one sentence per
refusal; `run.rs` runs one check on a fresh headless `Scenario`
(`src/game/scenario/`) with a fixed seed, under the game's current rules,
tunables and level, and answers in plain words ("lives went from 3 to 3;
expected a change of -1"). The stage comes from `rule_table::STAGE` (the
player's kind and start, the floor, how far `beside` is), counters from
`COUNTERS`, sounds from `SfxEvent`. `scenario/` and `checks_data/` are
copied verbatim (`tools/frame-accept.sh --check-copies`); the editor carries
byte-identical copies of `model.rs`, `cases.json` and the schema.

A data check runs whole in one frame, and a long check (up to 1800 ticks)
can take a noticeable fraction of a second in the browser; a name already
waiting in the queue is not queued again.

The editor runs checks by name (`data:<id>`, one per frame, through
`checks.run`); `content.set checks.json` swaps the file in. The test
`data_checks_pass` runs every check that is on in the committed file under
the committed content, so the release gate (which runs `cargo test`)
refuses a game with a failing check. Its message names the failing checks
by sentence and carries one `GX-CHECK-FAILED {json}` line each for the
editor. Turning a check off (`"off": true`) is the way past it.

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

Audio overrides are presentation data. A `songs.json` song becomes a
tracker `Song`, whose data is `&'static` (`tracker.rs` is the same in every
game), so it is leaked, through an intern table in `content::audio`:
identical data is leaked once, and an editing session's memory grows only
with genuinely new versions of a song (`tests/audio_intern.rs`). `sfx.json`
programs are owned `Fx` values and leak nothing. The full owned-data fix is
a later fleet change.

## The level editor

The editor build lets a creator move, add, remove, duplicate and resize the
Things in `content/level.json` on the running game.

- `GameState::Editing` (`src/game/content/edit.rs`) despawns the run and
  shows the current level for display only: `EditThing` entities with the
  kit's art and no `on_spawn` components or `SpawnOrder`. Presentation
  dressing still runs, so it looks like the game: the art children carry the
  `EditArt` marker and `player::dress_player` dresses the shown Player's. **No `SimSet`
  system runs in Editing**, so replays, `RunClock`, the selftest and the
  checks never see it. Never give a sim system a run condition that lets it
  run in Editing.
- A separate orthographic `EditCamera` shows the level, fitted to its bounds
  on entry; the game's own cameras are switched off while Editing.
- Each kind's `EditRules` (`resizable`, `max`) and the `LevelGrid` resource
  reach the editor through the catalog's `level`.
- Host messages (`gx:custom`): `level.edit {on}` enters Editing or leaves it
  for a normal, replayable run with the current level; `level.camera {at,
  zoom}` moves the edit camera; the game answers `level.view {at, zoom,
  canvas}` (zoom is CSS pixels per world unit, canvas is the `#game`
  rectangle in the frame's CSS pixels) and `level.mode {editing}` on every
  state change.
- While Editing, `content.set` of `level.json` respawns the shown level at
  once. Outside Editing it still applies at the next run start.

## Time travel

In editor builds (`--features editor`), `src/game/time/` lets the editor pause a run, step it, scrub back through it and "Try again from here". It has no save-states. A seek starts the run again in place (the `BeginRun` schedule, on the run's own start values, level, rules and seed) and re-simulates the recording, 2000 ticks a frame, with nothing drawn or played until it lands. Two edit rules follow from that:
- A setting or rules change made while paused applies on the next tick played, and is recorded there.
- A level edit becomes the run's level from its first tick. A branch replays the earlier moves on it, and the editor is told if they played out differently (`time.diverged`).

A game lists the counters its timeline marks in `rule_table::MARKS` (the template: `lives` falling). The folder is copied verbatim: `tools/frame-accept.sh --check-copies` checks it. Any use of the timeline marks the run edited, and an edited run never reaches a leaderboard (docs/replay-verification.md, "Edited runs").

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
