# Look Packs Foundation (steps 0–1) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the template a data-driven "look" layer. A **Look Pack** (a `look.json` plus PNG and OGG files) can restyle a game's palette, strings, slot recipes, images and audio cues without touching the sim. Packs are:
- validated by a hardened loader;
- checked by a `look-check` CLI;
- live-reloaded during native development;
- proven by CI not to change replay checksums.

**Architecture:**
- **`src/look/`** is a new module, copied verbatim into games. It holds the schema-v1 manifest types, a pure validator with size, dimension and format limits, pack sources, launch-time selection and a `LookPlugin`. The sources are packs embedded at build time from `assets/looks/<id>/` and, on native only, a folder on disk.
- **`LookPlugin`** is render-side and never part of `GamePlugin`. Game code opts in in one of two ways:
  - by tagging entities with `LookColour` / `LookText`; a `PostUpdate` system fills them in from the active pack;
  - by reading `ActiveLook` directly.
- **The game's look contract** lives in `src/ui/theme.rs` as `theme::LOOK`. It lists the palette roles, string keys, slots, audio cues and bundled fonts a pack may set. The validator rejects anything outside it, so typos fail loudly.
- **Cosmetic tunables:** a new `Tune::cosmetic()` flag keeps editor colour and sound tunables out of replay start values, the content hash and the checksum.

**Tech Stack:** Rust 2024, Bevy 0.18, serde/serde_json (already deps), blake3 (already in `Cargo.lock` through Bevy, added as a direct dependency), web-sys (already a wasm dependency).

**Spec:** `../../../../docs/plans/2026-11-08-game-customization-assessment.md` in the workspace root (`colecovisiongx/docs/plans/`), §6 (Look Packs), §7 (D1–D6), §8.1 (D7, modding-lite), §10 steps 0–1.

## Global Constraints

- Work in `libs/gamebient-bevy-template` on a new branch `look-packs` cut from `main` (at `f5c88e6` or later). Never commit to `main`.
- **Must pass, exactly as CI runs it:** `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-targets --all-features`. Every task ends green on all three.
- **D1: a pack never touches the sim.**
  - `LookPlugin` is added in `src/main.rs` only, never in `GamePlugin`.
  - The pack ID never enters the `GXR2` content hash or the checksum.
  - The `GXR2` byte format does not change.
- **Packs are data only:** a flat folder of `look.json` plus `.png` and `.ogg` files. No fonts, no code, no subfolders, no symlinks. Fonts are picked by name from faces the game already bundles.
- **Limits (validator constants, `src/look/validate.rs`):**

  | Limit | Value |
  |---|---|
  | `look.json` | ≤ 64 KiB |
  | Files per pack | ≤ 48 (including `look.json`) |
  | Pack size | ≤ 16 MiB total |
  | PNG | ≤ 2 MiB, ≤ 2048×2048 |
  | OGG | Vorbis only, ≤ 4 MiB, ≤ 180 s, ≤ 2 channels, ≤ 48 000 Hz |
  | Strings | printable ASCII, ≤ 64 chars (≤ the contract's `max_chars` for that key) |
  | Pack `id` | `[a-z0-9-]{1,32}` |
  | Keys | `[a-z0-9._-]{1,48}` |
  | File names | `[a-z0-9_-]{1,48}` plus `.png` or `.ogg` |

- **D4:** `protagonist` is reserved. It is parsed, produces a warning and is ignored.
- **Web** selects only built-in packs (`?look=<id>`). Fetching packs from a URL and on-chain looks (D3, D7) are step 5, **not this plan**.
- **No new crates except `blake3 = "1"`** (already in the lockfile).
- **Commit attribution:** every commit message ends with a blank line, then `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- **`GX_MUTE=1`** is already set for Claude's commands; add `?mute=1` when opening a web build.

**Out of scope (parsed and validated now, consumed later):**
- `fonts`, `frame` and `end_card` are consumed in steps 3–4.
- Recolouring `CardStyle` surfaces is decided per game in step 2.
- Voidrunner work is step 2.
- The editor UI for cosmetic tunables comes later.

---

## File Structure

| File | Status | Responsibility |
|---|---|---|
| `src/game/tunables/attr.rs` | modify | `Tune.cosmetic` field and `Tune::cosmetic()` builder |
| `src/game/tunables/index.rs` | modify | `run_starts()`: start values without cosmetic entries |
| `src/game/tunables/apply.rs` | modify | Cosmetic changes skip checksum and log; `begin_run_tunables` uses `run_starts` |
| `src/game/tunables/protocol.rs` | modify | `"cosmetic"` in `tunables.list` JSON |
| `src/game/tunables/mod.rs` | modify | Re-export `run_starts` |
| `src/game/replay/mod.rs` | modify | Doc comment: starts are non-cosmetic tunables |
| `tests/cosmetic_tunables.rs` | create | Cosmetic values never reach starts, log or checksum |
| `Cargo.toml` | modify | `blake3` dependency; `look-check` bin |
| `build.rs` | modify | Embeds `assets/looks/*/` into `$OUT_DIR/looks_embedded.rs` |
| `src/lib.rs` | modify | `pub mod look;` |
| `src/look/mod.rs` | create | Module doc, re-exports, `LookPlugin` |
| `src/look/manifest.rs` | create | Schema-v1 serde types and `parse` |
| `src/look/contract.rs` | create | `Contract`, `StringSpec`, `SlotSpec`, `RecipeSpec`, `ParamSpec`, `ParamKind` |
| `src/look/sniff.rs` | create | PNG IHDR and Ogg Vorbis header sniffers (no decoding) |
| `src/look/validate.rs` | create | Limits, `validate`, `Report`, `Problem`, `content_hash`, `parse_colour` |
| `src/look/pack.rs` | create | Embedded pack table access, `read_dir`, `dir_fingerprint` |
| `src/look/select.rs` | create | `Selection` from `--look` / `GX_LOOK` / `?look=` |
| `src/look/runtime.rs` | create | `ActiveLook` resource and its query API |
| `src/look/apply.rs` | create | `LookAssets`, `LookColour`, `LookText`, `LookChanged`, retint systems |
| `src/look/reload.rs` | create | Native hot reload for `Selection::Dir` |
| `src/ui/theme.rs` | modify | Game-owned contract `LOOK` (palette roles, strings, slots, audio, fonts) |
| `src/ui/menu.rs` | modify | Title from `ActiveLook`; colour tags |
| `src/game/mod.rs` | modify | Pause card tags |
| `src/game/player.rs` | modify | Player cuboid size and colour from the look |
| `src/game/audio/mod.rs` | modify | `music_key`; SFX and music overrides from `LookAssets` |
| `src/main.rs` | modify | Add `LookPlugin` |
| `src/bin/look_check.rs` | create | Validator CLI |
| `assets/looks/README.md` | create | One-paragraph pointer to the guide |
| `assets/looks/night-shift/look.json` | create | Palette-and-strings example pack |
| `assets/looks/sample-sponsor/look.json`, `logo.png` | create | Sponsor-style example pack (a fictional brand) |
| `tests/looks.rs` | create | Embedded packs validate; retint works; no pack changes the checksum; hot reload |
| `docs/look-packs.md` | create | Pack authoring guide (schema, limits, contract, workflow) |
| `AGENTS.md` | modify | Look Packs section |

---

### Task 1: Cosmetic tunables stay out of the replay

**Files:**
- Modify: `src/game/tunables/attr.rs` (the `Tune` struct and its `impl`, about lines 50–115)
- Modify: `src/game/tunables/index.rs` (after `snapshot`, about line 223)
- Modify: `src/game/tunables/apply.rs` (`apply_pending_tunes` about lines 46–83, `begin_run_tunables` about lines 87–96)
- Modify: `src/game/tunables/mod.rs:31`
- Modify: `src/game/tunables/protocol.rs` (the `json!` entry, about line 172, and its test at about line 353)
- Modify: `src/game/replay/mod.rs:13` (doc comment only)
- Test: `tests/cosmetic_tunables.rs`

**Interfaces:**
- Produces: `Tune.cosmetic: bool`, `Tune::cosmetic(self) -> Self` (const fn), `tunables::run_starts(world: &World) -> Vec<(u16, Value)>`.

- [ ] **Step 1: Branch**

```bash
cd libs/gamebient-bevy-template
git checkout main && git pull --ff-only
git checkout -b look-packs
```

- [ ] **Step 2: Write the failing test**

Create `tests/cosmetic_tunables.rs`:

```rust
//! A cosmetic tunable (`Tune::cosmetic`) changes only how the game looks:
//! it is not a start value, a mid-run change of it is not logged, and it
//! never moves the checksum. D1 of the customization assessment.

use bevy::color::Srgba;
use bevy::prelude::*;
use gamebient_game::game::replay::build_headless_app;
use gamebient_game::game::replay::selftest::SELFTEST_SEED;
use gamebient_game::game::sim::{Checksum, PendingSeed, SimTick};
use gamebient_game::game::states::GameState;
use gamebient_game::game::tunables::{
    self, Group, PendingTunes, TunableIndex, Tune, TuneLog, Value,
};

#[derive(Resource, Reflect, Clone, Debug)]
#[reflect(Resource, @Group::thing("paint", "Paint"))]
struct Paint {
    #[reflect(@Tune::colour("Coin colour").cosmetic())]
    coin: Srgba,
}

impl Default for Paint {
    fn default() -> Self {
        Self {
            coin: Srgba::rgb(1.0, 0.8, 0.1),
        }
    }
}

/// A headless app with `Paint` registered, in `Playing`, and the id of
/// `paint.coin`.
fn playing() -> (App, u16) {
    let mut app = build_headless_app();
    tunables::register::<Paint>(&mut app);
    let id = app
        .world()
        .resource::<TunableIndex>()
        .by_path("paint.coin")
        .expect("paint.coin is registered")
        .id;
    app.world_mut().resource_mut::<PendingSeed>().0 = Some(SELFTEST_SEED);
    app.update();
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Playing);
    app.update();
    (app, id)
}

fn run_to(app: &mut App, tick: u32) {
    while app.world().resource::<SimTick>().0 < tick {
        app.update();
    }
}

#[test]
fn a_cosmetic_tunable_is_not_a_start_value() {
    let (app, id) = playing();
    let log = app.world().resource::<TuneLog>();
    assert!(
        log.starts.iter().all(|(i, _)| *i != id),
        "cosmetic id {id} leaked into starts: {:?}",
        log.starts
    );
    assert!(
        !log.starts.is_empty(),
        "the template's gameplay tunable (run.spare) must still be recorded"
    );
}

#[test]
fn a_cosmetic_change_mid_run_applies_but_leaves_checksum_and_log_alone() {
    let (mut control, _) = playing();
    let (mut changed, id) = playing();
    run_to(&mut control, 10);
    run_to(&mut changed, 10);
    let red = Srgba::rgb(1.0, 0.0, 0.0);
    changed
        .world_mut()
        .resource_mut::<PendingTunes>()
        .0
        .push((id, Value::Colour(red)));
    run_to(&mut control, 30);
    run_to(&mut changed, 30);

    assert_eq!(changed.world().resource::<Paint>().coin, red, "the change applied");
    assert!(
        changed.world().resource::<TuneLog>().changes.is_empty(),
        "a cosmetic change is not replay content"
    );
    assert_eq!(
        changed.world().resource::<SimTick>().0,
        control.world().resource::<SimTick>().0
    );
    assert_eq!(
        changed.world().resource::<Checksum>().0,
        control.world().resource::<Checksum>().0,
        "a cosmetic change must not move the checksum"
    );
}
```

- [ ] **Step 3: Run it and confirm it fails**

Run: `cargo test --test cosmetic_tunables`

Expected: a compile error, `no method named cosmetic found for struct Tune`.

- [ ] **Step 4: Add the flag**

In `src/game/tunables/attr.rs`, add the field to `Tune` after `applies`:

```rust
    pub applies: Applies,
    /// Look-only (colours, sounds, sizes of art). Kept out of a run's start
    /// values, its content hash and its checksum, so changing it never
    /// makes a replay "a different game". See docs/look-packs.md.
    pub cosmetic: bool,
```

In `Tune::new`, after `applies: Applies::Tick,` add `cosmetic: false,`. After `next_run`, add:

```rust
    /// Marks the field as look-only; see [`Tune::cosmetic`](Self::cosmetic) field.
    pub const fn cosmetic(mut self) -> Self {
        self.cosmetic = true;
        self
    }
```

- [ ] **Step 5: Add `run_starts`**

In `src/game/tunables/index.rs`, directly after `pub fn snapshot`:

```rust
/// The start values a run records: every tunable except cosmetic ones
/// (`Tune::cosmetic`), which must never reach a replay or its hash.
/// `snapshot` stays the full set (the editor lists cosmetic values too).
pub fn run_starts(world: &World) -> Vec<(u16, Value)> {
    let idx = world.resource::<TunableIndex>().clone();
    idx.entries
        .iter()
        .filter(|e| !e.tune.cosmetic)
        .filter_map(|e| read(world, e).map(|v| (e.id, v)))
        .collect()
}
```

In `src/game/tunables/mod.rs`, change the `index` re-export to:

```rust
pub use index::{Entry, GroupEntry, TunableIndex, read, register, run_starts, snapshot, write};
```

- [ ] **Step 6: Keep cosmetic values out of the run**

In `src/game/tunables/apply.rs`:
- change the `use super::index::...` line to `use super::index::{TunableIndex, run_starts, write};`;
- in `begin_run_tunables`, replace `let starts = snapshot(world);` with `let starts = run_starts(world);`;
- in `apply_pending_tunes`, replace the `if let Some(tick) = tick { ... }` block with:

```rust
        if let Some(tick) = tick {
            // Cosmetic values change the look only: no checksum, no log.
            if !entry.tune.cosmetic {
                fold_value(&mut world.resource_mut::<Checksum>(), id, &value);
                world.resource_mut::<TuneLog>().changes.push(TuneChange {
                    tick,
                    id,
                    value: value.clone(),
                });
            }
        }
```

If clippy then flags `snapshot` as an unused import elsewhere in `apply.rs`, remove only that import.

- [ ] **Step 7: Expose the flag to the editor**

In `src/game/tunables/protocol.rs`, inside the `json!({ ... })` entry, after the `"applies": ...` line, add:

```rust
                "cosmetic": e.tune.cosmetic,
```

In the protocol test next to `assert_eq!(j["tunables"][0]["type"], entry.tune.kind.name());`, add:

```rust
        assert_eq!(j["tunables"][0]["cosmetic"], entry.tune.cosmetic);
```

In `src/game/replay/mod.rs` line 13, change "every tunable's start value, index order" to "every non-cosmetic tunable's start value, index order".

- [ ] **Step 8: Run the tests and confirm they pass**

Run: `cargo test --test cosmetic_tunables && cargo test --all-targets --all-features`

Expected: PASS, including the existing `tests/tunables.rs` and `tests/selftest.rs`.

- [ ] **Step 9: Lint and commit**

```bash
cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings
git add src/game/tunables src/game/replay/mod.rs tests/cosmetic_tunables.rs
git commit -m "feat(tunables): cosmetic flag keeps look-only values out of replays

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Manifest schema v1 and the contract types

**Files:**
- Create: `src/look/mod.rs`, `src/look/manifest.rs`, `src/look/contract.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Produces, in `crate::look::manifest`:
  - `SCHEMA_VERSION: u32 = 1`, `MANIFEST_FILE: &str = "look.json"`;
  - `Manifest { v, id, name, kind: LookKind, creator: Option<String>, palette: BTreeMap<String,String>, strings: BTreeMap<String,String>, slots: BTreeMap<String,Slot>, audio: BTreeMap<String,String>, fonts: BTreeMap<String,String>, frame: BTreeMap<String,String>, end_card: Option<EndCard>, protagonist: Option<serde_json::Value> }`;
  - `LookKind::{Palette, Skin, Sponsor, Genre}`;
  - `Slot::{Image(ImageSlot), Recipe(RecipeSlot)}`;
  - `ImageSlot { image: String, frames: u32, fps: f32 }`;
  - `RecipeSlot { recipe: String, params: BTreeMap<String, Param> }`;
  - `Param::{Bool(bool), Number(f64), Text(String)}`;
  - `EndCard { headline, body: Option<String>, url: Option<String>, image: Option<String> }`;
  - `fn parse(&[u8]) -> Result<Manifest, String>`.
- Produces, in `crate::look::contract`:
  - `Contract { palette: &'static [(&'static str, Color)], strings: &'static [StringSpec], slots: &'static [SlotSpec], audio: &'static [&'static str], fonts: &'static [&'static str] }`;
  - `StringSpec { key, default, max_chars: usize }`;
  - `SlotSpec { name, image: bool, recipes: &'static [RecipeSpec] }`;
  - `RecipeSpec { name, params: &'static [ParamSpec] }`;
  - `ParamSpec { name, kind: ParamKind }`;
  - `ParamKind::{Number { min: f64, max: f64 }, Bool, Text { max_chars: usize }, Colour}`;
  - methods `Contract::default_colour(&str) -> Option<Color>`, `default_string(&str) -> Option<&'static str>`, `string_spec(&str) -> Option<&StringSpec>`, `slot(&str) -> Option<&SlotSpec>`, and `SlotSpec::recipe(&str) -> Option<&RecipeSpec>`.

- [ ] **Step 1: Create the module skeleton**

`src/look/mod.rs`:

```rust
//! Look Packs: a game's look as data. A pack is a flat folder holding
//! `look.json` (schema v1, `manifest.rs`) plus `.png` / `.ogg` files. It
//! restyles palette roles, strings, slot recipes, images and audio cues
//! inside the limits the game's contract (`ui::theme::LOOK`) allows.
//!
//! **A pack never touches the sim.** `LookPlugin` is added in `main.rs`,
//! never in `GamePlugin`; sim systems never read `ActiveLook`. Replays,
//! checksums and leaderboards are identical under every pack
//! (`tests/looks.rs` proves it).
//!
//! This folder is copied verbatim into games. A game's own look surface is
//! `src/ui/theme.rs` (`LOOK`). Authoring guide: docs/look-packs.md.

pub mod contract;
pub mod manifest;

pub use contract::{Contract, ParamKind, ParamSpec, RecipeSpec, SlotSpec, StringSpec};
pub use manifest::{LookKind, Manifest, Param, Slot};
```

In `src/lib.rs`, add `pub mod look;` after `pub mod game;`.

- [ ] **Step 2: Write the failing tests**

`src/look/manifest.rs` (tests first; the types follow in Step 4):

```rust
//! `look.json`, schema v1. Pure data: parsing never touches Bevy, and
//! unknown fields are rejected so a typo fails instead of being ignored.

use std::collections::BTreeMap;

use serde::Deserialize;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_minimal_manifest_parses() {
        let m = parse(br#"{"v":1,"id":"x","name":"X","kind":"palette"}"#).unwrap();
        assert_eq!(m.v, 1);
        assert_eq!(m.kind, LookKind::Palette);
        assert!(m.palette.is_empty() && m.slots.is_empty() && m.protagonist.is_none());
    }

    #[test]
    fn slots_are_images_or_recipes() {
        let m = parse(
            br##"{"v":1,"id":"x","name":"X","kind":"skin","slots":{
                "a":{"image":"a.png","frames":4,"fps":8},
                "b":{"recipe":"cuboid","params":{"size":1.5,"glow":true,"tint":"#ff0000"}}
            }}"##,
        )
        .unwrap();
        assert_eq!(
            m.slots["a"],
            Slot::Image(ImageSlot { image: "a.png".into(), frames: 4, fps: 8.0 })
        );
        let Slot::Recipe(r) = &m.slots["b"] else { panic!("b is a recipe") };
        assert_eq!(r.recipe, "cuboid");
        assert_eq!(r.params["size"], Param::Number(1.5));
        assert_eq!(r.params["glow"], Param::Bool(true));
        assert_eq!(r.params["tint"], Param::Text("#ff0000".into()));
    }

    #[test]
    fn image_slot_defaults_to_one_still_frame() {
        let m = parse(br#"{"v":1,"id":"x","name":"X","kind":"skin","slots":{"a":{"image":"a.png"}}}"#)
            .unwrap();
        assert_eq!(
            m.slots["a"],
            Slot::Image(ImageSlot { image: "a.png".into(), frames: 1, fps: 0.0 })
        );
    }

    #[test]
    fn unknown_fields_are_rejected() {
        assert!(parse(br#"{"v":1,"id":"x","name":"X","kind":"palette","colours":{}}"#).is_err());
        assert!(parse(br#"{"v":1,"id":"x","name":"X","kind":"palette","slots":{"a":{"image":"a.png","zoom":2}}}"#).is_err());
    }

    #[test]
    fn protagonist_is_accepted_as_opaque_json() {
        let m = parse(br#"{"v":1,"id":"x","name":"X","kind":"skin","protagonist":{"avatar":"later"}}"#)
            .unwrap();
        assert!(m.protagonist.is_some());
    }

    #[test]
    fn unknown_kind_is_rejected() {
        assert!(parse(br#"{"v":1,"id":"x","name":"X","kind":"shader"}"#).is_err());
    }
}
```

- [ ] **Step 3: Run the tests and confirm they fail**

Run: `cargo test --lib look::manifest`

Expected: compile errors (`cannot find function parse`, `LookKind`, ...).

- [ ] **Step 4: Implement the manifest types**

Insert above the `#[cfg(test)]` block in `src/look/manifest.rs`:

```rust
pub const SCHEMA_VERSION: u32 = 1;
pub const MANIFEST_FILE: &str = "look.json";

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// Schema version; this build reads [`SCHEMA_VERSION`].
    pub v: u32,
    /// `[a-z0-9-]{1,32}`; equals the pack's folder name.
    pub id: String,
    pub name: String,
    pub kind: LookKind,
    #[serde(default)]
    pub creator: Option<String>,
    /// Palette role -> `#rrggbb` or `#rrggbbaa`.
    #[serde(default)]
    pub palette: BTreeMap<String, String>,
    /// String key -> printable ASCII text.
    #[serde(default)]
    pub strings: BTreeMap<String, String>,
    #[serde(default)]
    pub slots: BTreeMap<String, Slot>,
    /// Audio cue (`sfx.<event>`, `music.<state>`) -> `.ogg` file.
    #[serde(default)]
    pub audio: BTreeMap<String, String>,
    /// `display` / `body` -> a face the game bundles. Consumed in a later step.
    #[serde(default)]
    pub fonts: BTreeMap<String, String>,
    /// `marquee` / `bezel` -> `.png`. Consumed in a later step.
    #[serde(default)]
    pub frame: BTreeMap<String, String>,
    /// Sponsor end card. Consumed in a later step.
    #[serde(default)]
    pub end_card: Option<EndCard>,
    /// Reserved for a bring-your-own protagonist (D4): accepted, ignored.
    #[serde(default)]
    pub protagonist: Option<serde_json::Value>,
}

#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LookKind {
    Palette,
    Skin,
    Sponsor,
    Genre,
}

/// What fills a slot: an image (sprite games, logos) or a recipe the game
/// draws procedurally (most of the fleet: meshes plus materials).
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
pub enum Slot {
    Image(ImageSlot),
    Recipe(RecipeSlot),
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ImageSlot {
    pub image: String,
    /// Horizontal strip frame count; 1 = a still image.
    #[serde(default = "one")]
    pub frames: u32,
    #[serde(default)]
    pub fps: f32,
}

fn one() -> u32 {
    1
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RecipeSlot {
    /// A recipe name the game's contract lists for this slot.
    pub recipe: String,
    #[serde(default)]
    pub params: BTreeMap<String, Param>,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
pub enum Param {
    Bool(bool),
    Number(f64),
    Text(String),
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EndCard {
    pub headline: String,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub image: Option<String>,
}

pub fn parse(bytes: &[u8]) -> Result<Manifest, String> {
    serde_json::from_slice(bytes).map_err(|e| e.to_string())
}
```

- [ ] **Step 5: Implement the contract types**

`src/look/contract.rs`:

```rust
//! What a game lets a pack change. Each game declares one `Contract` in
//! `src/ui/theme.rs` (`LOOK`); the validator rejects anything outside it,
//! so a typo in a hand-written pack fails loudly instead of doing nothing.

use bevy::color::Color;

#[derive(Debug)]
pub struct Contract {
    /// Palette roles with the game's own colour for each.
    pub palette: &'static [(&'static str, Color)],
    pub strings: &'static [StringSpec],
    pub slots: &'static [SlotSpec],
    /// Audio cue names a pack may override (`sfx.<event>`, `music.<state>`).
    pub audio: &'static [&'static str],
    /// Names of fonts the game bundles; packs pick from these only.
    pub fonts: &'static [&'static str],
}

#[derive(Debug)]
pub struct StringSpec {
    pub key: &'static str,
    pub default: &'static str,
    /// Longest text that fits where the game draws it.
    pub max_chars: usize,
}

#[derive(Debug)]
pub struct SlotSpec {
    pub name: &'static str,
    /// Whether an image may fill the slot.
    pub image: bool,
    /// Recipes the game knows how to draw for this slot.
    pub recipes: &'static [RecipeSpec],
}

#[derive(Debug)]
pub struct RecipeSpec {
    pub name: &'static str,
    pub params: &'static [ParamSpec],
}

#[derive(Debug)]
pub struct ParamSpec {
    pub name: &'static str,
    pub kind: ParamKind,
}

#[derive(Debug)]
pub enum ParamKind {
    Number { min: f64, max: f64 },
    Bool,
    Text { max_chars: usize },
    /// `#rrggbb` / `#rrggbbaa`, or the name of a palette role.
    Colour,
}

impl Contract {
    pub fn default_colour(&self, role: &str) -> Option<Color> {
        self.palette.iter().find(|(r, _)| *r == role).map(|(_, c)| *c)
    }
    pub fn string_spec(&self, key: &str) -> Option<&StringSpec> {
        self.strings.iter().find(|s| s.key == key)
    }
    pub fn default_string(&self, key: &str) -> Option<&'static str> {
        self.string_spec(key).map(|s| s.default)
    }
    pub fn slot(&self, name: &str) -> Option<&SlotSpec> {
        self.slots.iter().find(|s| s.name == name)
    }
}

impl SlotSpec {
    pub fn recipe(&self, name: &str) -> Option<&RecipeSpec> {
        self.recipes.iter().find(|r| r.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const C: Contract = Contract {
        palette: &[("ink", Color::srgb(0.0, 0.0, 0.0))],
        strings: &[StringSpec { key: "pause", default: "TAKE FIVE", max_chars: 12 }],
        slots: &[SlotSpec {
            name: "player",
            image: false,
            recipes: &[RecipeSpec { name: "cuboid", params: &[] }],
        }],
        audio: &["sfx.confirm"],
        fonts: &["Inter SemiBold"],
    };

    #[test]
    fn lookups_find_declared_names_only() {
        assert_eq!(C.default_colour("ink"), Some(Color::srgb(0.0, 0.0, 0.0)));
        assert_eq!(C.default_colour("nope"), None);
        assert_eq!(C.default_string("pause"), Some("TAKE FIVE"));
        assert!(C.slot("player").and_then(|s| s.recipe("cuboid")).is_some());
        assert!(C.slot("player").and_then(|s| s.recipe("sphere")).is_none());
    }
}
```

In `src/look/mod.rs`, add `pub use manifest::{EndCard, ImageSlot, RecipeSlot};` to the re-exports.

- [ ] **Step 6: Run the tests and confirm they pass**

Run: `cargo test --lib look::`

Expected: PASS (7 tests).

- [ ] **Step 7: Lint and commit**

```bash
cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings
git add src/lib.rs src/look
git commit -m "feat(look): schema-v1 manifest and per-game contract types

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: The hardened validator and content hash

**Files:**
- Create: `src/look/sniff.rs`, `src/look/validate.rs`
- Modify: `src/look/mod.rs`, `Cargo.toml`

**Interfaces:**
- Consumes: `Manifest`, `parse`, `Slot`, `Param`, `MANIFEST_FILE`, `SCHEMA_VERSION` (Task 2); `Contract`, `ParamKind` (Task 2).
- Produces:
  - `sniff::png_size(&[u8]) -> Option<(u32, u32)>`;
  - `sniff::ogg_vorbis_info(&[u8]) -> Option<OggInfo { channels: u8, rate: u32, seconds: f32 }>`;
  - `validate::Files = BTreeMap<String, Vec<u8>>`;
  - `validate::validate(&Files, &Contract, expected_id: Option<&str>) -> Report`;
  - `Report { manifest: Option<Manifest>, problems: Vec<Problem>, content_hash: String }` with `ok()`, `errors()` and `Report::read_error(String) -> Report`;
  - `Problem { severity: Severity, at: String, message: String }`, `Severity::{Error, Warning}`;
  - `validate::content_hash(&Files) -> String` (64 hex characters);
  - `validate::parse_colour(&str) -> Option<Color>`;
  - module `validate::limits` with the constants below.

- [ ] **Step 1: Add the dependency**

In `Cargo.toml` `[dependencies]`, after `serde_json = "1"`:

```toml
# Look Pack content hash (src/look/validate.rs). Already in Cargo.lock
# through Bevy's asset processor, so no new download.
blake3 = "1"
```

- [ ] **Step 2: Write the failing sniffer tests**

`src/look/sniff.rs`:

```rust
//! Format checks that read headers only. Nothing here decodes pixels or
//! audio: a pack is untrusted input, so its claims are checked before
//! any decoder sees the bytes.

#[cfg(test)]
pub(crate) mod fixtures {
    /// The smallest bytes `png_size` accepts: signature + IHDR header.
    pub fn png(w: u32, h: u32) -> Vec<u8> {
        let mut b = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        b.extend_from_slice(&13u32.to_be_bytes());
        b.extend_from_slice(b"IHDR");
        b.extend_from_slice(&w.to_be_bytes());
        b.extend_from_slice(&h.to_be_bytes());
        b.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
        b
    }

    /// A first Ogg page carrying a Vorbis identification header, then a
    /// last page whose granule position is `samples`.
    pub fn ogg(channels: u8, rate: u32, samples: u64) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(b"OggS");
        b.extend_from_slice(&[0, 2]); // version, header type (BOS)
        b.extend_from_slice(&0u64.to_le_bytes()); // granule
        b.extend_from_slice(&[0; 12]); // serial, sequence, crc
        b.push(1); // one segment
        b.push(30); // segment length
        b.push(1);
        b.extend_from_slice(b"vorbis");
        b.extend_from_slice(&0u32.to_le_bytes()); // vorbis version
        b.push(channels);
        b.extend_from_slice(&rate.to_le_bytes());
        b.extend_from_slice(&[0; 14]); // rest of the id header
        b.extend_from_slice(b"OggS");
        b.extend_from_slice(&[0, 4]); // EOS page
        b.extend_from_slice(&samples.to_le_bytes());
        b.extend_from_slice(&[0; 13]);
        b
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::{ogg, png};
    use super::*;

    #[test]
    fn png_size_reads_the_ihdr() {
        assert_eq!(png_size(&png(128, 64)), Some((128, 64)));
        assert_eq!(png_size(b"GIF89a.................."), None);
        assert_eq!(png_size(&png(1, 1)[..20]), None, "truncated");
    }

    #[test]
    fn ogg_info_reads_channels_rate_and_length() {
        let info = ogg_vorbis_info(&ogg(2, 44_100, 44_100 * 3)).unwrap();
        assert_eq!((info.channels, info.rate), (2, 44_100));
        assert!((info.seconds - 3.0).abs() < 1e-3);
        assert!(ogg_vorbis_info(b"RIFF....WAVE").is_none());
        assert!(ogg_vorbis_info(&ogg(1, 0, 10)).is_none(), "zero rate");
    }
}
```

- [ ] **Step 3: Run the tests and confirm they fail**

First add `pub mod sniff;` to `src/look/mod.rs`.

Run: `cargo test --lib look::sniff`

Expected: compile errors (`cannot find function png_size`).

- [ ] **Step 4: Implement the sniffers**

Insert above `#[cfg(test)] pub(crate) mod fixtures` in `src/look/sniff.rs`:

```rust
pub const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// Width and height from a PNG's IHDR, or `None` if it is not a PNG.
pub fn png_size(b: &[u8]) -> Option<(u32, u32)> {
    if b.len() < 24 || b[..8] != PNG_SIGNATURE || &b[12..16] != b"IHDR" {
        return None;
    }
    let w = u32::from_be_bytes(b[16..20].try_into().ok()?);
    let h = u32::from_be_bytes(b[20..24].try_into().ok()?);
    Some((w, h))
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OggInfo {
    pub channels: u8,
    pub rate: u32,
    pub seconds: f32,
}

/// Channels, sample rate and length of an Ogg Vorbis file, or `None` if
/// the first page does not carry a Vorbis identification header. The
/// length is the last page's granule position over the rate.
pub fn ogg_vorbis_info(b: &[u8]) -> Option<OggInfo> {
    if b.len() < 28 || &b[..4] != b"OggS" {
        return None;
    }
    let start = 27 + usize::from(b[26]);
    let p = b.get(start..start + 16)?;
    if p[0] != 1 || &p[1..7] != b"vorbis" {
        return None;
    }
    let channels = p[11];
    let rate = u32::from_le_bytes(p[12..16].try_into().ok()?);
    if rate == 0 || channels == 0 {
        return None;
    }
    let last = b.windows(4).rposition(|w| w == b"OggS")?;
    let granule = u64::from_le_bytes(b.get(last + 6..last + 14)?.try_into().ok()?);
    Some(OggInfo {
        channels,
        rate,
        seconds: granule as f32 / rate as f32,
    })
}
```

Run: `cargo test --lib look::sniff`

Expected: PASS.

- [ ] **Step 5: Write the failing validator tests**

`src/look/validate.rs` (tests first; the code follows in Step 7):

```rust
//! Validates an untrusted pack against the limits and the game's
//! contract. Pure: no Bevy app, no decoding. `Severity::Error` blocks the
//! pack; a warning loads it and logs.

use std::collections::BTreeMap;

use bevy::color::{Color, Srgba};

use super::contract::{Contract, ParamKind};
use super::manifest::{self, MANIFEST_FILE, Manifest, Param, SCHEMA_VERSION, Slot};
use super::sniff;

#[cfg(test)]
mod tests {
    use super::super::contract::{ParamSpec, RecipeSpec, SlotSpec, StringSpec};
    use super::super::sniff::fixtures::{ogg, png};
    use super::*;

    const TEST: Contract = Contract {
        palette: &[
            ("ink", Color::srgb(0.0, 0.0, 0.0)),
            ("signal", Color::srgb(0.0, 1.0, 1.0)),
        ],
        strings: &[StringSpec { key: "pause", default: "TAKE FIVE", max_chars: 12 }],
        slots: &[SlotSpec {
            name: "player",
            image: true,
            recipes: &[RecipeSpec {
                name: "cuboid",
                params: &[
                    ParamSpec { name: "size", kind: ParamKind::Number { min: 0.25, max: 3.0 } },
                    ParamSpec { name: "tint", kind: ParamKind::Colour },
                ],
            }],
        }],
        audio: &["sfx.confirm", "music.menu"],
        fonts: &["Inter SemiBold"],
    };

    fn pack(json: &str) -> Files {
        BTreeMap::from([(MANIFEST_FILE.to_string(), json.as_bytes().to_vec())])
    }

    fn with(mut files: Files, name: &str, bytes: Vec<u8>) -> Files {
        files.insert(name.to_string(), bytes);
        files
    }

    fn errors(r: &Report) -> Vec<String> {
        r.errors().map(|p| format!("{}: {}", p.at, p.message)).collect()
    }

    const MIN: &str = r#"{"v":1,"id":"demo","name":"Demo","kind":"palette"}"#;

    #[test]
    fn a_minimal_pack_is_ok() {
        let r = validate(&pack(MIN), &TEST, Some("demo"));
        assert!(r.ok(), "{:?}", errors(&r));
        assert_eq!(r.content_hash.len(), 64);
    }

    #[test]
    fn missing_or_broken_manifest_is_an_error() {
        assert!(!validate(&Files::new(), &TEST, None).ok());
        assert!(!validate(&pack("{"), &TEST, None).ok());
        assert!(!validate(&pack(r#"{"v":2,"id":"demo","name":"D","kind":"palette"}"#), &TEST, None).ok());
    }

    #[test]
    fn the_id_must_be_well_formed_and_match_the_folder() {
        assert!(!validate(&pack(MIN), &TEST, Some("other")).ok());
        let bad = r#"{"v":1,"id":"Demo Pack","name":"D","kind":"palette"}"#;
        assert!(!validate(&pack(bad), &TEST, None).ok());
    }

    #[test]
    fn palette_roles_must_exist_and_colours_must_be_hex() {
        let ok = r##"{"v":1,"id":"demo","name":"D","kind":"palette","palette":{"ink":"#102030","signal":"#10203040"}}"##;
        assert!(validate(&pack(ok), &TEST, None).ok());
        let typo = r##"{"v":1,"id":"demo","name":"D","kind":"palette","palette":{"inc":"#102030"}}"##;
        assert!(errors(&validate(&pack(typo), &TEST, None))[0].contains("palette.inc"));
        let bad = r#"{"v":1,"id":"demo","name":"D","kind":"palette","palette":{"ink":"red"}}"#;
        assert!(!validate(&pack(bad), &TEST, None).ok());
    }

    #[test]
    fn strings_respect_keys_ascii_and_length() {
        let ok = r#"{"v":1,"id":"demo","name":"D","kind":"palette","strings":{"pause":"SNACK BREAK"}}"#;
        assert!(validate(&pack(ok), &TEST, None).ok());
        let long = r#"{"v":1,"id":"demo","name":"D","kind":"palette","strings":{"pause":"THIS IS FAR TOO LONG"}}"#;
        assert!(!validate(&pack(long), &TEST, None).ok());
        let unicode = r#"{"v":1,"id":"demo","name":"D","kind":"palette","strings":{"pause":"CAFÉ"}}"#;
        assert!(!validate(&pack(unicode), &TEST, None).ok());
        let unknown = r#"{"v":1,"id":"demo","name":"D","kind":"palette","strings":{"title":"X"}}"#;
        assert!(!validate(&pack(unknown), &TEST, None).ok());
    }

    #[test]
    fn recipes_and_params_are_checked_against_the_contract() {
        let ok = r##"{"v":1,"id":"demo","name":"D","kind":"skin","slots":{"player":{"recipe":"cuboid","params":{"size":1.5,"tint":"signal"}}}}"##;
        assert!(validate(&pack(ok), &TEST, None).ok(), "{:?}", errors(&validate(&pack(ok), &TEST, None)));
        for bad in [
            r#"{"v":1,"id":"demo","name":"D","kind":"skin","slots":{"enemy":{"recipe":"cuboid"}}}"#,
            r#"{"v":1,"id":"demo","name":"D","kind":"skin","slots":{"player":{"recipe":"sphere"}}}"#,
            r#"{"v":1,"id":"demo","name":"D","kind":"skin","slots":{"player":{"recipe":"cuboid","params":{"size":9}}}}"#,
            r#"{"v":1,"id":"demo","name":"D","kind":"skin","slots":{"player":{"recipe":"cuboid","params":{"spin":1}}}}"#,
            r#"{"v":1,"id":"demo","name":"D","kind":"skin","slots":{"player":{"recipe":"cuboid","params":{"tint":"mauve"}}}}"#,
        ] {
            assert!(!validate(&pack(bad), &TEST, None).ok(), "{bad}");
        }
    }

    #[test]
    fn image_files_must_exist_be_png_and_be_bounded() {
        let json = r#"{"v":1,"id":"demo","name":"D","kind":"skin","slots":{"player":{"image":"hero.png"}}}"#;
        assert!(!validate(&pack(json), &TEST, None).ok(), "missing file");
        assert!(validate(&with(pack(json), "hero.png", png(64, 64)), &TEST, None).ok());
        assert!(!validate(&with(pack(json), "hero.png", png(4096, 64)), &TEST, None).ok(), "too wide");
        assert!(!validate(&with(pack(json), "hero.png", b"not a png at all........".to_vec()), &TEST, None).ok());
    }

    #[test]
    fn audio_must_be_short_ogg_vorbis_for_a_known_cue() {
        let json = r#"{"v":1,"id":"demo","name":"D","kind":"skin","audio":{"sfx.confirm":"ding.ogg"}}"#;
        assert!(validate(&with(pack(json), "ding.ogg", ogg(1, 44_100, 22_050)), &TEST, None).ok());
        assert!(!validate(&with(pack(json), "ding.ogg", ogg(1, 44_100, 44_100 * 600)), &TEST, None).ok(), "too long");
        assert!(!validate(&with(pack(json), "ding.ogg", ogg(6, 44_100, 100)), &TEST, None).ok(), "5.1");
        let unknown = r#"{"v":1,"id":"demo","name":"D","kind":"skin","audio":{"sfx.boom":"ding.ogg"}}"#;
        assert!(!validate(&with(pack(unknown), "ding.ogg", ogg(1, 44_100, 100)), &TEST, None).ok());
    }

    #[test]
    fn file_names_cannot_escape_or_smuggle() {
        for name in ["../x.png", "sub/x.png", "X.PNG", "font.ttf", "script.js", ".hidden.png"] {
            let files = with(pack(MIN), name, png(8, 8));
            assert!(!validate(&files, &TEST, None).ok(), "{name}");
        }
    }

    #[test]
    fn fonts_are_bundled_names_only() {
        let ok = r#"{"v":1,"id":"demo","name":"D","kind":"genre","fonts":{"body":"Inter SemiBold"}}"#;
        assert!(validate(&pack(ok), &TEST, None).ok());
        let bad = r#"{"v":1,"id":"demo","name":"D","kind":"genre","fonts":{"body":"Comic Sans"}}"#;
        assert!(!validate(&pack(bad), &TEST, None).ok());
    }

    #[test]
    fn end_card_url_must_be_https() {
        let ok = r#"{"v":1,"id":"demo","name":"D","kind":"sponsor","end_card":{"headline":"THANKS","url":"https://example.com"}}"#;
        assert!(validate(&pack(ok), &TEST, None).ok());
        let bad = r#"{"v":1,"id":"demo","name":"D","kind":"sponsor","end_card":{"headline":"THANKS","url":"javascript:alert(1)"}}"#;
        assert!(!validate(&pack(bad), &TEST, None).ok());
    }

    #[test]
    fn protagonist_and_unused_files_warn_but_load() {
        let json = r#"{"v":1,"id":"demo","name":"D","kind":"skin","protagonist":{"avatar":"x"}}"#;
        let r = validate(&with(pack(json), "spare.png", png(8, 8)), &TEST, None);
        assert!(r.ok());
        assert_eq!(r.problems.iter().filter(|p| p.severity == Severity::Warning).count(), 2);
    }

    #[test]
    fn too_many_files_is_an_error() {
        let mut files = pack(MIN);
        for i in 0..limits::FILES_MAX {
            files.insert(format!("f{i}.png"), png(1, 1));
        }
        assert!(!validate(&files, &TEST, None).ok());
    }

    #[test]
    fn the_content_hash_covers_names_and_bytes() {
        let a = with(pack(MIN), "a.png", png(8, 8));
        let b = with(pack(MIN), "b.png", png(8, 8));
        let c = with(pack(MIN), "a.png", png(8, 9));
        assert_eq!(content_hash(&a), content_hash(&a.clone()));
        assert_ne!(content_hash(&a), content_hash(&b));
        assert_ne!(content_hash(&a), content_hash(&c));
    }
}
```

- [ ] **Step 6: Run the tests and confirm they fail**

First add `pub mod validate;` to `src/look/mod.rs`.

Run: `cargo test --lib look::validate`

Expected: compile errors (`cannot find type Files`, `validate`, ...).

- [ ] **Step 7: Implement the validator**

Insert between the `use` lines and `#[cfg(test)]` in `src/look/validate.rs`:

```rust
/// A pack's files by name, `look.json` included.
pub type Files = BTreeMap<String, Vec<u8>>;

pub mod limits {
    pub const MANIFEST_MAX: usize = 64 * 1024;
    /// Including `look.json`.
    pub const FILES_MAX: usize = 48;
    pub const PACK_MAX: usize = 16 * 1024 * 1024;
    pub const PNG_MAX: usize = 2 * 1024 * 1024;
    pub const PNG_DIM_MAX: u32 = 2048;
    pub const OGG_MAX: usize = 4 * 1024 * 1024;
    pub const OGG_SECONDS_MAX: f32 = 180.0;
    pub const OGG_CHANNELS_MAX: u8 = 2;
    pub const OGG_RATE_MAX: u32 = 48_000;
    pub const STRING_MAX: usize = 64;
    pub const NAME_MAX: usize = 32;
    pub const CREATOR_MAX: usize = 48;
    pub const ID_MAX: usize = 32;
    pub const KEY_MAX: usize = 48;
    pub const FILE_STEM_MAX: usize = 48;
    pub const FRAMES_MAX: u32 = 64;
    pub const FPS_MAX: f32 = 60.0;
    pub const URL_MAX: usize = 200;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Problem {
    pub severity: Severity,
    /// Where: `look.json`, `palette.ink`, `hero.png`, ...
    pub at: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct Report {
    pub manifest: Option<Manifest>,
    pub problems: Vec<Problem>,
    pub content_hash: String,
}

impl Report {
    pub fn ok(&self) -> bool {
        self.manifest.is_some() && self.errors().next().is_none()
    }
    pub fn errors(&self) -> impl Iterator<Item = &Problem> {
        self.problems.iter().filter(|p| p.severity == Severity::Error)
    }
    /// A pack that could not even be read (missing folder, too large).
    pub fn read_error(message: String) -> Self {
        Self {
            manifest: None,
            problems: vec![Problem { severity: Severity::Error, at: "pack".into(), message }],
            content_hash: String::new(),
        }
    }
}

/// BLAKE3 over every file, in name order: name length, name, byte length,
/// bytes. Domain-separated so it can never collide with another use.
pub fn content_hash(files: &Files) -> String {
    let mut h = blake3::Hasher::new();
    h.update(b"gx-look-v1\0");
    for (name, bytes) in files {
        h.update(&(name.len() as u64).to_le_bytes());
        h.update(name.as_bytes());
        h.update(&(bytes.len() as u64).to_le_bytes());
        h.update(bytes);
    }
    h.finalize().to_hex().to_string()
}

/// `#rrggbb` or `#rrggbbaa`, nothing looser.
pub fn parse_colour(s: &str) -> Option<Color> {
    if !s.starts_with('#') || !(s.len() == 7 || s.len() == 9) {
        return None;
    }
    Srgba::hex(&s[1..]).ok().map(Color::from)
}

fn is_text(s: &str, max: usize) -> bool {
    s.len() <= max && s.bytes().all(|c| (0x20..=0x7e).contains(&c))
}

fn is_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= limits::ID_MAX
        && s.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
}

fn is_key(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= limits::KEY_MAX
        && s.bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'.' | b'_' | b'-'))
}

/// `[a-z0-9_-]{1,48}.png|.ogg`: no dots in the stem, so no `..`, no
/// hidden files, no second extension.
fn is_file_name(s: &str) -> bool {
    let Some((stem, ext)) = s.rsplit_once('.') else {
        return false;
    };
    matches!(ext, "png" | "ogg")
        && !stem.is_empty()
        && stem.len() <= limits::FILE_STEM_MAX
        && stem
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'_' | b'-'))
}

struct Check<'a> {
    files: &'a Files,
    problems: Vec<Problem>,
    used: Vec<String>,
}

impl Check<'_> {
    fn error(&mut self, at: impl Into<String>, message: impl Into<String>) {
        self.problems.push(Problem { severity: Severity::Error, at: at.into(), message: message.into() });
    }
    fn warn(&mut self, at: impl Into<String>, message: impl Into<String>) {
        self.problems.push(Problem { severity: Severity::Warning, at: at.into(), message: message.into() });
    }
    /// `at` names `file`, which must be in the pack with extension `ext`.
    fn file_ref(&mut self, at: &str, file: &str, ext: &str) {
        if !file.ends_with(&format!(".{ext}")) {
            self.error(at, format!("expects a .{ext} file, got \"{file}\""));
        } else if !self.files.contains_key(file) {
            self.error(at, format!("\"{file}\" is not in the pack"));
        }
        self.used.push(file.to_string());
    }
    fn text(&mut self, at: &str, s: &str, max: usize) {
        if !is_text(s, max) {
            self.error(at, format!("must be printable ASCII, at most {max} characters"));
        }
    }
}

pub fn validate(files: &Files, contract: &Contract, expected_id: Option<&str>) -> Report {
    let mut c = Check { files, problems: Vec::new(), used: Vec::new() };
    let content_hash = content_hash(files);

    // --- The pack as a whole, and every file in it -------------------------
    if files.len() > limits::FILES_MAX {
        c.error("pack", format!("{} files; at most {}", files.len(), limits::FILES_MAX));
    }
    let total: usize = files.values().map(Vec::len).sum();
    if total > limits::PACK_MAX {
        c.error("pack", format!("{total} bytes; at most {}", limits::PACK_MAX));
    }
    for (name, bytes) in files {
        if name == MANIFEST_FILE {
            if bytes.len() > limits::MANIFEST_MAX {
                c.error(name.as_str(), format!("at most {} bytes", limits::MANIFEST_MAX));
            }
            continue;
        }
        if !is_file_name(name) {
            c.error(
                name.as_str(),
                "file names are lowercase letters, digits, - and _, ending .png or .ogg (no fonts, no folders)",
            );
            continue;
        }
        if name.ends_with(".png") {
            if bytes.len() > limits::PNG_MAX {
                c.error(name.as_str(), format!("at most {} bytes", limits::PNG_MAX));
            }
            match sniff::png_size(bytes) {
                None => c.error(name.as_str(), "is not a PNG"),
                Some((w, h)) if w == 0 || h == 0 || w > limits::PNG_DIM_MAX || h > limits::PNG_DIM_MAX => c.error(
                    name.as_str(),
                    format!("{w}x{h}; at most {0}x{0}", limits::PNG_DIM_MAX),
                ),
                Some(_) => {}
            }
        } else {
            if bytes.len() > limits::OGG_MAX {
                c.error(name.as_str(), format!("at most {} bytes", limits::OGG_MAX));
            }
            match sniff::ogg_vorbis_info(bytes) {
                None => c.error(name.as_str(), "is not an Ogg Vorbis file"),
                Some(i) => {
                    if i.channels > limits::OGG_CHANNELS_MAX {
                        c.error(name.as_str(), format!("{} channels; at most {}", i.channels, limits::OGG_CHANNELS_MAX));
                    }
                    if i.rate > limits::OGG_RATE_MAX {
                        c.error(name.as_str(), format!("{} Hz; at most {}", i.rate, limits::OGG_RATE_MAX));
                    }
                    if !(i.seconds.is_finite() && i.seconds <= limits::OGG_SECONDS_MAX) {
                        c.error(name.as_str(), format!("{:.1} s; at most {} s", i.seconds, limits::OGG_SECONDS_MAX));
                    }
                }
            }
        }
    }

    // --- The manifest ------------------------------------------------------
    let Some(raw) = files.get(MANIFEST_FILE) else {
        c.error(MANIFEST_FILE, "missing");
        return Report { manifest: None, problems: c.problems, content_hash };
    };
    let m = match manifest::parse(raw) {
        Ok(m) => m,
        Err(e) => {
            c.error(MANIFEST_FILE, e);
            return Report { manifest: None, problems: c.problems, content_hash };
        }
    };
    if m.v != SCHEMA_VERSION {
        c.error("v", format!("this game reads schema {SCHEMA_VERSION}, the pack is {}", m.v));
    }
    if !is_id(&m.id) {
        c.error("id", "lowercase letters, digits and -, at most 32");
    }
    if let Some(expected) = expected_id {
        if expected != m.id {
            c.error("id", format!("\"{}\" must match the folder name \"{expected}\"", m.id));
        }
    }
    c.text("name", &m.name, limits::NAME_MAX);
    if let Some(creator) = &m.creator {
        c.text("creator", creator, limits::CREATOR_MAX);
    }

    for (role, hex) in &m.palette {
        let at = format!("palette.{role}");
        if contract.default_colour(role).is_none() {
            let known: Vec<_> = contract.palette.iter().map(|(r, _)| *r).collect();
            c.error(at.as_str(), format!("unknown role; this game has {known:?}"));
        }
        if parse_colour(hex).is_none() {
            c.error(at, "colours are #rrggbb or #rrggbbaa");
        }
    }

    for (key, text) in &m.strings {
        let at = format!("strings.{key}");
        match contract.string_spec(key) {
            None => {
                let known: Vec<_> = contract.strings.iter().map(|s| s.key).collect();
                c.error(at, format!("unknown string; this game has {known:?}"));
            }
            Some(spec) => c.text(&at, text, spec.max_chars.min(limits::STRING_MAX)),
        }
    }

    for (name, slot) in &m.slots {
        let at = format!("slots.{name}");
        let Some(spec) = contract.slot(name) else {
            let known: Vec<_> = contract.slots.iter().map(|s| s.name).collect();
            c.error(at, format!("unknown slot; this game has {known:?}"));
            continue;
        };
        match slot {
            Slot::Image(i) => {
                if !spec.image {
                    c.error(at.as_str(), "this slot is drawn by a recipe, not an image");
                }
                c.file_ref(&at, &i.image, "png");
                if i.frames == 0 || i.frames > limits::FRAMES_MAX {
                    c.error(at.as_str(), format!("frames is 1..={}", limits::FRAMES_MAX));
                }
                if !(i.fps.is_finite() && (0.0..=limits::FPS_MAX).contains(&i.fps)) {
                    c.error(at.as_str(), format!("fps is 0..={}", limits::FPS_MAX));
                }
            }
            Slot::Recipe(r) => {
                let Some(recipe) = spec.recipe(&r.recipe) else {
                    let known: Vec<_> = spec.recipes.iter().map(|r| r.name).collect();
                    c.error(at, format!("unknown recipe \"{}\"; this slot has {known:?}", r.recipe));
                    continue;
                };
                for (pname, value) in &r.params {
                    let pat = format!("{at}.{pname}");
                    let Some(pspec) = recipe.params.iter().find(|p| p.name == pname.as_str()) else {
                        c.error(pat, "unknown parameter");
                        continue;
                    };
                    match (&pspec.kind, value) {
                        (ParamKind::Number { min, max }, Param::Number(n)) => {
                            if !(n.is_finite() && *n >= *min && *n <= *max) {
                                c.error(pat, format!("{n} is outside {min}..={max}"));
                            }
                        }
                        (ParamKind::Bool, Param::Bool(_)) => {}
                        (ParamKind::Text { max_chars }, Param::Text(s)) => c.text(&pat, s, *max_chars),
                        (ParamKind::Colour, Param::Text(s)) => {
                            if parse_colour(s).is_none() && contract.default_colour(s).is_none() {
                                c.error(pat, "a #rrggbb colour or a palette role");
                            }
                        }
                        (kind, _) => c.error(pat, format!("expects {kind:?}")),
                    }
                }
            }
        }
    }

    for (cue, file) in &m.audio {
        let at = format!("audio.{cue}");
        if !is_key(cue) || !contract.audio.contains(&cue.as_str()) {
            c.error(at.as_str(), format!("unknown cue; this game has {:?}", contract.audio));
        }
        c.file_ref(&at, file, "ogg");
    }

    for (role, face) in &m.fonts {
        let at = format!("fonts.{role}");
        if !matches!(role.as_str(), "display" | "body") {
            c.error(at.as_str(), "font roles are display and body");
        }
        if !contract.fonts.contains(&face.as_str()) {
            c.error(at, format!("not a bundled face; this game has {:?}", contract.fonts));
        }
    }

    for (part, file) in &m.frame {
        let at = format!("frame.{part}");
        if !matches!(part.as_str(), "marquee" | "bezel") {
            c.error(at.as_str(), "frame parts are marquee and bezel");
        }
        c.file_ref(&at, file, "png");
    }

    if let Some(card) = &m.end_card {
        c.text("end_card.headline", &card.headline, limits::STRING_MAX);
        if let Some(body) = &card.body {
            c.text("end_card.body", body, limits::STRING_MAX * 2);
        }
        if let Some(url) = &card.url {
            if !url.starts_with("https://") || !is_text(url, limits::URL_MAX) {
                c.error("end_card.url", format!("an https:// URL, at most {} characters", limits::URL_MAX));
            }
        }
        if let Some(image) = &card.image {
            c.file_ref("end_card.image", image, "png");
        }
    }

    if m.protagonist.is_some() {
        c.warn("protagonist", "reserved for a later schema; ignored");
    }

    let unused: Vec<String> = files
        .keys()
        .filter(|n| n.as_str() != MANIFEST_FILE && is_file_name(n) && !c.used.contains(n))
        .cloned()
        .collect();
    for name in unused {
        c.warn(name, "not referenced by look.json");
    }

    Report { manifest: Some(m), problems: c.problems, content_hash }
}
```

If clippy (Rust 2024) asks for a let-chain in `if let Some(expected) = expected_id { if ... }` (`collapsible_if`), apply its suggested form.

- [ ] **Step 8: Run the tests and confirm they pass**

Run: `cargo test --lib look::`

Expected: PASS (all `manifest`, `contract`, `sniff` and `validate` tests).

- [ ] **Step 9: Lint and commit**

```bash
cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings
git add Cargo.toml Cargo.lock src/look
git commit -m "feat(look): hardened pack validator, header sniffers and content hash

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Pack sources, selection, the template contract and example packs

**Files:**
- Modify: `build.rs`, `src/look/mod.rs`, `src/ui/theme.rs`
- Create: `src/look/pack.rs`, `src/look/select.rs`, `assets/looks/README.md`, `assets/looks/night-shift/look.json`, `assets/looks/sample-sponsor/look.json`, `assets/looks/sample-sponsor/logo.png`
- Test: `tests/looks.rs` (first test)

**Interfaces:**
- Consumes: `Files`, `limits`, `validate`, `Report` (Task 3); `Contract` and friends (Task 2).
- Produces:
  - `pack::embedded_ids() -> impl Iterator<Item = &'static str>`;
  - `pack::embedded_files(id: &str) -> Option<Files>`;
  - `pack::read_dir(&Path) -> Result<Files, String>` (native);
  - `pack::dir_fingerprint(&Path) -> u64` (native);
  - `select::Selection::{Default, Embedded(String), Dir(String)}` (a `Resource`, `Clone`, `PartialEq`, `Debug`);
  - `select::from_args_env(&[String], Option<&str>) -> Selection`, `select::from_query(&str) -> Selection`, `select::from_launch() -> Selection`;
  - `theme::LOOK: Contract`, `theme::PALETTE`, `theme::STRINGS`, `theme::SLOTS`, `theme::AUDIO_CUES`, `theme::FONTS`, `theme::PLAYER: Color`.

- [ ] **Step 1: Embed packs at build time**

In `build.rs`, add this function and call `embed_looks();` as the last line of `main`:

```rust
/// Writes `$OUT_DIR/looks_embedded.rs`: every `assets/looks/<id>/` folder
/// as `(id, [(file, bytes)])`, sorted, so built-in packs work offline on
/// cabinets and on web without a fetch. README.md and dotfiles are skipped.
fn embed_looks() {
    use std::fmt::Write as _;
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("looks_embedded.rs");
    let root = std::path::Path::new("assets/looks");
    println!("cargo:rerun-if-changed=assets/looks");
    let mut code = String::from(
        "/// Generated by build.rs from assets/looks/.\npub static EMBEDDED: &[(&str, &[(&str, &[u8])])] = &[\n",
    );
    let mut dirs: Vec<_> = std::fs::read_dir(root)
        .map(|rd| rd.filter_map(Result::ok).filter(|e| e.path().is_dir()).collect())
        .unwrap_or_default();
    dirs.sort_by_key(|e| e.file_name());
    for dir in dirs {
        let id = dir.file_name().to_string_lossy().to_string();
        println!("cargo:rerun-if-changed={}", dir.path().display());
        writeln!(code, "    ({id:?}, &[").unwrap();
        let mut files: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.path().is_file())
            .collect();
        files.sort_by_key(|e| e.file_name());
        for f in files {
            let name = f.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || name == "README.md" {
                continue;
            }
            let abs = std::fs::canonicalize(f.path()).unwrap();
            println!("cargo:rerun-if-changed={}", abs.display());
            writeln!(code, "        ({name:?}, include_bytes!({:?})),", abs.display().to_string()).unwrap();
        }
        code.push_str("    ]),\n");
    }
    code.push_str("];\n");
    std::fs::write(out, code).unwrap();
}
```

- [ ] **Step 2: Write the failing selection tests**

`src/look/select.rs`:

```rust
//! Which pack a launch asks for:
//! - native: `--look <id|dir>`, else `GX_LOOK=<id|dir>`;
//! - web: `?look=<id>`, built-in packs only.
//!
//! A value with a `/`, a `\` or a leading `.` is a folder on disk (native
//! only, live-reloaded). `default` or nothing means the game's own look.

use bevy::prelude::*;

#[derive(Resource, Debug, Clone, PartialEq, Eq, Default)]
pub enum Selection {
    #[default]
    Default,
    /// A pack built into this game (`assets/looks/<id>/`).
    Embedded(String),
    /// A pack folder on disk (native development; hot-reloaded).
    Dir(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn native_prefers_the_flag_over_the_env() {
        assert_eq!(from_args_env(&args(&["game"]), None), Selection::Default);
        assert_eq!(from_args_env(&args(&["game"]), Some("night-shift")), Selection::Embedded("night-shift".into()));
        assert_eq!(
            from_args_env(&args(&["game", "--look", "./packs/fae"]), Some("night-shift")),
            Selection::Dir("./packs/fae".into())
        );
        assert_eq!(from_args_env(&args(&["game", "--look", "default"]), None), Selection::Default);
        assert_eq!(from_args_env(&args(&["game"]), Some("")), Selection::Default);
    }

    #[test]
    fn web_takes_built_in_ids_only() {
        assert_eq!(from_query("?mute=1&look=night-shift"), Selection::Embedded("night-shift".into()));
        assert_eq!(from_query("?look=../../etc"), Selection::Default);
        assert_eq!(from_query("?look=https://evil.example/pack"), Selection::Default);
        assert_eq!(from_query("?mute=1"), Selection::Default);
    }
}
```

Add `pub mod select;` and `pub use select::Selection;` to `src/look/mod.rs`.

Run: `cargo test --lib look::select`

Expected: compile errors (`cannot find function from_args_env`).

- [ ] **Step 3: Implement selection**

Insert above `#[cfg(test)]` in `src/look/select.rs`:

```rust
fn classify(value: Option<String>, allow_dir: bool) -> Selection {
    match value {
        None => Selection::Default,
        Some(v) if v.is_empty() || v == "default" => Selection::Default,
        Some(v) if v.contains('/') || v.contains('\\') || v.starts_with('.') || v.contains(':') => {
            if allow_dir {
                Selection::Dir(v)
            } else {
                Selection::Default
            }
        }
        Some(v) => Selection::Embedded(v),
    }
}

pub fn from_args_env(args: &[String], env: Option<&str>) -> Selection {
    let flag = args.windows(2).find(|w| w[0] == "--look").map(|w| w[1].clone());
    classify(flag.or_else(|| env.map(str::to_string)), true)
}

pub fn from_query(search: &str) -> Selection {
    let value = search
        .trim_start_matches('?')
        .split('&')
        .find_map(|pair| pair.strip_prefix("look=").map(str::to_string));
    classify(value, false)
}

/// The selection this process was launched with.
pub fn from_launch() -> Selection {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let args: Vec<String> = std::env::args().collect();
        from_args_env(&args, std::env::var("GX_LOOK").ok().as_deref())
    }
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window()
            .and_then(|w| w.location().search().ok())
            .map_or(Selection::Default, |q| from_query(&q))
    }
}
```

Run: `cargo test --lib look::select`

Expected: PASS.

- [ ] **Step 4: Implement pack sources**

`src/look/pack.rs`:

```rust
//! Where pack bytes come from: built into the binary at compile time
//! (`build.rs` embeds `assets/looks/*/`), or, natively, a folder on disk.

#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;

use super::validate::Files;
#[cfg(not(target_arch = "wasm32"))]
use super::validate::limits;

mod generated {
    include!(concat!(env!("OUT_DIR"), "/looks_embedded.rs"));
}

pub fn embedded_ids() -> impl Iterator<Item = &'static str> {
    generated::EMBEDDED.iter().map(|(id, _)| *id)
}

pub fn embedded_files(id: &str) -> Option<Files> {
    generated::EMBEDDED.iter().find(|(i, _)| *i == id).map(|(_, files)| {
        files
            .iter()
            .map(|(name, bytes)| (name.to_string(), bytes.to_vec()))
            .collect()
    })
}

/// Reads a flat pack folder. Refuses subfolders and symlinks (an entry's
/// metadata does not follow links), and stops early on the file-count and
/// total-size limits so a huge folder is never read into memory.
#[cfg(not(target_arch = "wasm32"))]
pub fn read_dir(path: &Path) -> Result<Files, String> {
    let rd = std::fs::read_dir(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let mut files = Files::new();
    let mut total = 0usize;
    for entry in rd {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name == "README.md" {
            continue;
        }
        let meta = entry.metadata().map_err(|e| format!("{name}: {e}"))?;
        if !meta.is_file() {
            return Err(format!("{name}: packs are flat folders of files (no folders, no links)"));
        }
        if files.len() >= limits::FILES_MAX {
            return Err(format!("more than {} files", limits::FILES_MAX));
        }
        total += meta.len() as usize;
        if total > limits::PACK_MAX {
            return Err(format!("larger than {} bytes", limits::PACK_MAX));
        }
        let bytes = std::fs::read(entry.path()).map_err(|e| format!("{name}: {e}"))?;
        files.insert(name, bytes);
    }
    Ok(files)
}

/// Changes whenever a file in the folder is added, removed, resized or
/// rewritten (name, length and modification time of every entry).
#[cfg(not(target_arch = "wasm32"))]
pub fn dir_fingerprint(path: &Path) -> u64 {
    let mut h = blake3::Hasher::new();
    if let Ok(rd) = std::fs::read_dir(path) {
        let mut entries: Vec<_> = rd.filter_map(Result::ok).collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let Ok(meta) = e.metadata() else { continue };
            h.update(e.file_name().to_string_lossy().as_bytes());
            h.update(&meta.len().to_le_bytes());
            if let Some(d) = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            {
                h.update(&d.as_nanos().to_le_bytes());
            }
        }
    }
    u64::from_le_bytes(h.finalize().as_bytes()[..8].try_into().unwrap())
}
```

Add `pub mod pack;` to `src/look/mod.rs`, and add `pub use validate::{Files, Problem, Report, Severity};`.

- [ ] **Step 5: Declare the template's contract**

In `src/ui/theme.rs`:
- add `use crate::look::{Contract, ParamKind, ParamSpec, RecipeSpec, SlotSpec, StringSpec};` and `use crate::ui::fonts;` under the existing `use` lines;
- append to the end of the file:

```rust
// --- Look contract -------------------------------------------------------
// What a Look Pack may change in this game (docs/look-packs.md). Roles and
// keys are the game's own; keep defaults equal to the constants above.

/// The template player's colour (`game::player`).
pub const PLAYER: Color = Color::srgb(0.2, 0.8, 1.0);

pub const PALETTE: &[(&str, Color)] = &[
    ("ink", INK),
    ("night", NIGHT),
    ("dusk", DUSK),
    ("signal", SIGNAL),
    ("coral", CORAL),
    ("paper", PAPER),
    ("muted", MUTED),
    ("player", PLAYER),
];

pub const STRINGS: &[StringSpec] = &[
    StringSpec { key: "title", default: crate::ui::menu::TITLE, max_chars: 20 },
    StringSpec { key: "pause", default: PAUSE_WORD, max_chars: 12 },
    StringSpec { key: "pause.resume", default: PAUSE_RESUME, max_chars: 24 },
    StringSpec { key: "pause.quit", default: PAUSE_QUIT, max_chars: 24 },
];

pub const SLOTS: &[SlotSpec] = &[SlotSpec {
    name: "player",
    image: false,
    recipes: &[RecipeSpec {
        name: "cuboid",
        params: &[ParamSpec { name: "size", kind: ParamKind::Number { min: 0.25, max: 3.0 } }],
    }],
}];

/// `sfx.<SfxEvent::name>` and `music.<GameState, lower case>`.
pub const AUDIO_CUES: &[&str] = &[
    "sfx.confirm",
    "sfx.pause",
    "sfx.screensweep",
    "music.studiologo",
    "music.menu",
    "music.howtoplay",
    "music.playing",
    "music.gameover",
    "music.demoend",
];

pub const FONTS: &[&str] = &[
    fonts::DISPLAY.name,
    fonts::BODY.name,
    fonts::STUDIO.name,
    fonts::STUDIO_ITALIC.name,
];

pub const LOOK: Contract = Contract {
    palette: PALETTE,
    strings: STRINGS,
    slots: SLOTS,
    audio: AUDIO_CUES,
    fonts: FONTS,
};
```

If `fonts::DISPLAY.name` is rejected in a const context (because `Face` has a non-const field), replace `FONTS` with string literals: `&["Space Grotesk Bold", "Inter SemiBold", "Fraunces Black", "Fraunces Italic"]`. Then add a unit test in `theme.rs` that asserts they equal `fonts::ALL.map(|f| f.name)`.

Also append this test module to `theme.rs`. Add it to an existing `#[cfg(test)] mod tests` if one is present.

```rust
#[cfg(test)]
mod look_contract_tests {
    use super::*;
    use crate::game::audio::{MUSIC, SfxEvent};

    #[test]
    fn every_sound_the_game_plays_is_a_cue_a_pack_can_override() {
        for e in SfxEvent::ALL {
            let cue = format!("sfx.{}", e.name());
            assert!(AUDIO_CUES.contains(&cue.as_str()), "{cue} missing from AUDIO_CUES");
        }
        for (state, _) in MUSIC {
            let cue = format!("music.{}", format!("{state:?}").to_lowercase());
            assert!(AUDIO_CUES.contains(&cue.as_str()), "{cue} missing from AUDIO_CUES");
        }
    }

    #[test]
    fn string_defaults_fit_their_own_limits() {
        for s in STRINGS {
            assert!(s.default.len() <= s.max_chars, "{} default is longer than {}", s.key, s.max_chars);
        }
    }
}
```

- [ ] **Step 6: Add the example packs**

`assets/looks/README.md`:

```markdown
# Look Packs

Each folder here is a Look Pack built into the game (`build.rs` embeds them).
Pick one with `?look=<folder>` on web or `GX_LOOK=<folder>` / `--look <folder>` natively.
How to write one: `docs/look-packs.md`. Check one: `cargo run --bin look-check -- assets/looks/<folder>`.
```

`assets/looks/night-shift/look.json`:

```json
{
  "v": 1,
  "id": "night-shift",
  "name": "Night Shift",
  "kind": "palette",
  "creator": "Bread Heads Studios",
  "palette": {
    "ink": "#05070f",
    "night": "#0d1430",
    "dusk": "#1d3a5c",
    "signal": "#ffd24a",
    "coral": "#ff6fb1",
    "paper": "#f4f1e8",
    "muted": "#9aa6c2",
    "player": "#ffd24a"
  },
  "strings": { "pause": "LIGHTS LOW" }
}
```

`assets/looks/sample-sponsor/look.json`. The brand is fictional; never put a real brand in a template or public repo.

```json
{
  "v": 1,
  "id": "sample-sponsor",
  "name": "Acme Snacks Arcade",
  "kind": "sponsor",
  "creator": "Bread Heads Studios",
  "palette": {
    "ink": "#1a0f2e",
    "signal": "#ffc828",
    "coral": "#ff4f3a",
    "paper": "#fff8e7",
    "player": "#ff4f3a"
  },
  "strings": {
    "title": "ACME ARCADE",
    "pause": "SNACK BREAK",
    "pause.resume": "ESC: BACK TO IT",
    "pause.quit": "ENTER: CASH OUT"
  },
  "slots": { "player": { "recipe": "cuboid", "params": { "size": 1.4 } } },
  "end_card": {
    "headline": "THANKS FOR PLAYING",
    "body": "Brought to you by Acme Snacks, a made-up brand.",
    "url": "https://example.com",
    "image": "logo.png"
  }
}
```

Generate `logo.png`, a 128×64 checker in the pack colours:

```bash
python3 - <<'EOF'
import struct, zlib
w, h = 128, 64
def chunk(t, d):
    return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xffffffff)
raw = bytearray()
for y in range(h):
    raw.append(0)
    for x in range(w):
        on = (x // 16 + y // 16) % 2 == 0
        raw += bytes((255, 200, 40, 255) if on else (26, 15, 46, 255))
png = (b"\x89PNG\r\n\x1a\n"
       + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0))
       + chunk(b"IDAT", zlib.compress(bytes(raw), 9))
       + chunk(b"IEND", b""))
open("assets/looks/sample-sponsor/logo.png", "wb").write(png)
EOF
```

- [ ] **Step 7: Write the failing integration test**

Create `tests/looks.rs`:

```rust
//! Look Packs end to end: built-in packs validate against the game's
//! contract, tagged entities pick up a pack, no pack changes a replay, and
//! a pack folder reloads live.

use gamebient_game::look::{self, validate::validate};
use gamebient_game::ui::theme;

#[test]
fn every_built_in_pack_validates_against_the_contract() {
    let ids: Vec<_> = look::pack::embedded_ids().collect();
    assert!(
        ids.contains(&"night-shift") && ids.contains(&"sample-sponsor"),
        "build.rs did not embed assets/looks: {ids:?}"
    );
    for id in ids {
        let files = look::pack::embedded_files(id).unwrap();
        let r = validate(&files, &theme::LOOK, Some(id));
        assert!(r.ok(), "{id}: {:#?}", r.problems);
        assert!(
            r.problems.is_empty(),
            "{id} has warnings; built-in packs ship clean: {:#?}",
            r.problems
        );
    }
}
```

- [ ] **Step 8: Run the tests and confirm they pass**

Run: `cargo test --test looks && cargo test --lib`

Expected: PASS. If `every_built_in_pack_validates_against_the_contract` fails, its message names the exact field to fix in the pack JSON.

- [ ] **Step 9: Lint and commit**

```bash
cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings
git add build.rs src/look src/ui/theme.rs assets/looks tests/looks.rs
git commit -m "feat(look): embedded and folder pack sources, launch selection, template contract, example packs

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: `ActiveLook`, `LookPlugin` and the retint systems

**Files:**
- Create: `src/look/runtime.rs`, `src/look/apply.rs`
- Modify: `src/look/mod.rs`, `src/main.rs`
- Test: `tests/looks.rs` (more tests)

**Interfaces:**
- Consumes: `Selection`, `pack::*` (Task 4); `validate`, `Report`, `Files`, `parse_colour`, `Severity` (Task 3); `Contract`, `Manifest`, `Slot`, `Param` (Task 2); `theme::LOOK` (Task 4).
- Produces:
  - `ActiveLook` (Resource):
    - fields `contract: &'static Contract`, `manifest: Option<Manifest>`, `files: Files`, `content_hash: Option<String>`, `source: Selection`;
    - constructors and helpers `default_for(&'static Contract) -> Self`, `from_files(&'static Contract, Files, Selection, Option<&str>) -> Result<Self, Report>`, `resolve(&'static Contract, Selection) -> Self`, `load_dir(&'static Contract, &Path) -> Result<Self, Report>` (native);
    - queries `id() -> &str`, `colour(&str) -> Color`, `string(&str) -> &str`, `slot(&str) -> Option<&Slot>`, `recipe_param(&str, &str, &str) -> Option<&Param>`, `recipe_number(&str, &str, &str) -> Option<f64>`, `file(&str) -> Option<&[u8]>`.
  - `runtime::MISSING: Color`.
  - `runtime::log_report(&Report)`.
  - `apply::LookAssets` (Resource) with `image(&str) -> Option<Handle<Image>>` and `audio(&str) -> Option<Handle<AudioSource>>`.
  - `apply::build_assets(&ActiveLook, Option<&mut Assets<Image>>, Option<&mut Assets<AudioSource>>) -> LookAssets`.
  - `apply::LookColour { role, alpha, emissive }` with `new`, `alpha`, `emissive` and `resolve(&ActiveLook) -> Color`.
  - `apply::LookText(pub &'static str)`.
  - `apply::LookChanged` (Message).
  - `LookPlugin { contract: &'static Contract }`.
  - `LookSet` (SystemSet, in `PostUpdate`).

- [ ] **Step 1: Write the failing tests**

Append to `tests/looks.rs`:

```rust
use bevy::prelude::*;
use gamebient_game::look::{ActiveLook, LookColour, LookPlugin, LookText, Selection};
use gamebient_game::look::validate::parse_colour;

fn app_with(selection: Selection) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.insert_resource(selection);
    app.add_plugins(LookPlugin { contract: &theme::LOOK });
    app
}

#[test]
fn no_selection_is_the_games_own_look() {
    let app = app_with(Selection::Default);
    let look = app.world().resource::<ActiveLook>();
    assert_eq!(look.id(), "default");
    assert_eq!(look.colour("paper"), theme::PAPER);
    assert_eq!(look.string("pause"), theme::PAUSE_WORD);
}

#[test]
fn a_built_in_pack_overrides_only_what_it_sets() {
    let app = app_with(Selection::Embedded("night-shift".into()));
    let look = app.world().resource::<ActiveLook>();
    assert_eq!(look.id(), "night-shift");
    assert_eq!(look.colour("paper"), parse_colour("#f4f1e8").unwrap());
    assert_eq!(look.string("pause"), "LIGHTS LOW");
    assert_eq!(look.string("pause.quit"), theme::PAUSE_QUIT, "unset keys keep the game's text");
    assert!(look.content_hash.as_deref().is_some_and(|h| h.len() == 64));
}

#[test]
fn an_unknown_pack_falls_back_to_the_default() {
    let app = app_with(Selection::Embedded("no-such-pack".into()));
    assert_eq!(app.world().resource::<ActiveLook>().id(), "default");
}

#[test]
fn recipe_params_are_readable() {
    let app = app_with(Selection::Embedded("sample-sponsor".into()));
    let look = app.world().resource::<ActiveLook>();
    assert_eq!(look.recipe_number("player", "cuboid", "size"), Some(1.4));
    assert_eq!(look.recipe_number("player", "sphere", "size"), None);
}

#[test]
fn tagged_entities_take_the_pack_on_the_frame_they_spawn() {
    let mut app = app_with(Selection::Embedded("night-shift".into()));
    let text = app
        .world_mut()
        .spawn((Text::new(theme::PAUSE_WORD), TextColor(theme::PAPER), LookColour::new("paper"), LookText("pause")))
        .id();
    let veil = app
        .world_mut()
        .spawn((BackgroundColor(theme::INK), LookColour::new("ink").alpha(0.62)))
        .id();
    app.update();
    let w = app.world();
    assert_eq!(w.get::<TextColor>(text).unwrap().0, parse_colour("#f4f1e8").unwrap());
    assert_eq!(w.get::<Text>(text).unwrap().0, "LIGHTS LOW");
    assert_eq!(
        w.get::<BackgroundColor>(veil).unwrap().0,
        parse_colour("#05070f").unwrap().with_alpha(0.62)
    );
}
```

Run: `cargo test --test looks`

Expected: compile errors (`unresolved import gamebient_game::look::ActiveLook`, ...).

- [ ] **Step 2: Implement `ActiveLook`**

`src/look/runtime.rs`:

```rust
//! The pack this run is wearing, and the queries game code asks of it.
//! Every query falls back to the game's own look (the contract defaults),
//! so code can ask unconditionally.

use bevy::prelude::*;

use super::contract::Contract;
use super::manifest::{Manifest, Param, Slot};
use super::pack;
use super::select::Selection;
use super::validate::{Files, Report, Severity, parse_colour, validate};

/// Drawn for a palette role the contract does not declare: a programming
/// error, made loud on screen.
pub const MISSING: Color = Color::srgb(1.0, 0.0, 1.0);

#[derive(Resource, Debug)]
pub struct ActiveLook {
    pub contract: &'static Contract,
    pub manifest: Option<Manifest>,
    pub files: Files,
    pub content_hash: Option<String>,
    pub source: Selection,
    colours: Vec<(String, Color)>,
}

pub fn log_report(report: &Report) {
    for p in &report.problems {
        match p.severity {
            Severity::Error => error!("look pack: {}: {}", p.at, p.message),
            Severity::Warning => warn!("look pack: {}: {}", p.at, p.message),
        }
    }
}

impl ActiveLook {
    pub fn default_for(contract: &'static Contract) -> Self {
        Self {
            contract,
            manifest: None,
            files: Files::new(),
            content_hash: None,
            source: Selection::Default,
            colours: Vec::new(),
        }
    }

    /// Validates `files`; an invalid pack is returned as its report.
    pub fn from_files(
        contract: &'static Contract,
        files: Files,
        source: Selection,
        expected_id: Option<&str>,
    ) -> Result<Self, Report> {
        let report = validate(&files, contract, expected_id);
        if !report.ok() {
            return Err(report);
        }
        log_report(&report); // warnings only at this point
        let Report { manifest, content_hash, .. } = report;
        let manifest = manifest.expect("ok() implies a manifest");
        let colours = manifest
            .palette
            .iter()
            .filter_map(|(role, hex)| parse_colour(hex).map(|c| (role.clone(), c)))
            .collect();
        Ok(Self {
            contract,
            manifest: Some(manifest),
            files,
            content_hash: Some(content_hash),
            source,
            colours,
        })
    }

    /// The look for `selection`, or the default (with the reasons logged).
    pub fn resolve(contract: &'static Contract, selection: Selection) -> Self {
        let attempt = match &selection {
            Selection::Default => return Self::default_for(contract),
            Selection::Embedded(id) => match pack::embedded_files(id) {
                Some(files) => Self::from_files(contract, files, selection.clone(), Some(id)),
                None => {
                    warn!("look pack \"{id}\" is not built into this game; using the default look");
                    return Self::default_for(contract);
                }
            },
            #[cfg(not(target_arch = "wasm32"))]
            Selection::Dir(path) => Self::load_dir(contract, std::path::Path::new(path)),
            #[cfg(target_arch = "wasm32")]
            Selection::Dir(_) => return Self::default_for(contract),
        };
        attempt.unwrap_or_else(|report| {
            log_report(&report);
            warn!("look pack rejected; using the default look");
            Self::default_for(contract)
        })
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn load_dir(contract: &'static Contract, path: &std::path::Path) -> Result<Self, Report> {
        let id = path.file_name().and_then(|n| n.to_str()).map(str::to_string);
        let files = pack::read_dir(path).map_err(Report::read_error)?;
        Self::from_files(contract, files, Selection::Dir(path.display().to_string()), id.as_deref())
    }

    pub fn id(&self) -> &str {
        self.manifest.as_ref().map_or("default", |m| m.id.as_str())
    }

    pub fn colour(&self, role: &str) -> Color {
        if let Some((_, c)) = self.colours.iter().find(|(r, _)| r == role) {
            return *c;
        }
        self.contract.default_colour(role).unwrap_or_else(|| {
            warn_once!("look: palette role \"{role}\" is not in the game's contract");
            MISSING
        })
    }

    pub fn string(&self, key: &str) -> &str {
        self.manifest
            .as_ref()
            .and_then(|m| m.strings.get(key))
            .map(String::as_str)
            .or_else(|| self.contract.default_string(key))
            .unwrap_or(key)
    }

    pub fn slot(&self, name: &str) -> Option<&Slot> {
        self.manifest.as_ref()?.slots.get(name)
    }

    pub fn recipe_param(&self, slot: &str, recipe: &str, param: &str) -> Option<&Param> {
        match self.slot(slot)? {
            Slot::Recipe(r) if r.recipe == recipe => r.params.get(param),
            _ => None,
        }
    }

    pub fn recipe_number(&self, slot: &str, recipe: &str, param: &str) -> Option<f64> {
        match self.recipe_param(slot, recipe, param)? {
            Param::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub fn file(&self, name: &str) -> Option<&[u8]> {
        self.files.get(name).map(Vec::as_slice)
    }
}
```

- [ ] **Step 3: Implement assets, tags and retint**

`src/look/apply.rs`:

```rust
//! Getting a look onto the screen. Spawn code tags entities with
//! `LookColour` / `LookText` next to the game's own colour or text; the
//! systems here (in `PostUpdate`, so before the frame renders) replace
//! them from the active pack on the frame they spawn and again on every
//! `LookChanged` (hot reload). Untagged entities keep what spawn gave them.

use bevy::asset::RenderAssetUsages;
use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
use bevy::prelude::*;

use super::runtime::ActiveLook;

/// Sent after the active pack is replaced (native hot reload).
#[derive(Message, Debug, Clone, Copy)]
pub struct LookChanged;

/// Colours a `TextColor`, `BackgroundColor`, `Sprite` or `StandardMaterial`
/// on the same entity from palette role `role`.
#[derive(Component, Debug, Clone, Copy)]
pub struct LookColour {
    pub role: &'static str,
    pub alpha: Option<f32>,
    /// For a `StandardMaterial`: emissive = the colour times this.
    pub emissive: Option<f32>,
}

impl LookColour {
    pub const fn new(role: &'static str) -> Self {
        Self { role, alpha: None, emissive: None }
    }
    pub const fn alpha(mut self, alpha: f32) -> Self {
        self.alpha = Some(alpha);
        self
    }
    pub const fn emissive(mut self, strength: f32) -> Self {
        self.emissive = Some(strength);
        self
    }
    pub fn resolve(&self, look: &ActiveLook) -> Color {
        let c = look.colour(self.role);
        match self.alpha {
            Some(a) => c.with_alpha(a),
            None => c,
        }
    }
}

/// Replaces the entity's `Text` with string `key` of the active pack.
#[derive(Component, Debug, Clone, Copy)]
pub struct LookText(pub &'static str);

/// Decoded images (by file name) and audio overrides (by cue).
#[derive(Resource, Default, Debug)]
pub struct LookAssets {
    images: Vec<(String, Handle<Image>)>,
    audio: Vec<(String, Handle<AudioSource>)>,
}

impl LookAssets {
    pub fn image(&self, file: &str) -> Option<Handle<Image>> {
        self.images.iter().find(|(n, _)| n == file).map(|(_, h)| h.clone())
    }
    pub fn audio(&self, cue: &str) -> Option<Handle<AudioSource>> {
        self.audio.iter().find(|(n, _)| n == cue).map(|(_, h)| h.clone())
    }
}

/// Decodes the pack's PNGs and wraps its OGGs. Headless apps (the replay
/// verifier, tests) have no asset stores and get an empty set.
pub fn build_assets(
    look: &ActiveLook,
    images: Option<&mut Assets<Image>>,
    audio: Option<&mut Assets<AudioSource>>,
) -> LookAssets {
    let mut out = LookAssets::default();
    if let Some(images) = images {
        for (name, bytes) in look.files.iter().filter(|(n, _)| n.ends_with(".png")) {
            match Image::from_buffer(
                bytes,
                ImageType::Extension("png"),
                CompressedImageFormats::NONE,
                true,
                ImageSampler::Default,
                RenderAssetUsages::default(),
            ) {
                Ok(image) => out.images.push((name.clone(), images.add(image))),
                Err(e) => error!("look pack: {name}: {e}"),
            }
        }
    }
    if let (Some(audio), Some(manifest)) = (audio, look.manifest.as_ref()) {
        for (cue, file) in &manifest.audio {
            if let Some(bytes) = look.file(file) {
                let source = AudioSource { bytes: bytes.to_vec().into() };
                out.audio.push((cue.clone(), audio.add(source)));
            }
        }
    }
    out
}

pub fn load_assets(
    mut commands: Commands,
    look: Res<ActiveLook>,
    mut images: Option<ResMut<Assets<Image>>>,
    mut audio: Option<ResMut<Assets<AudioSource>>>,
) {
    commands.insert_resource(build_assets(&look, images.as_deref_mut(), audio.as_deref_mut()));
}

pub fn retint_text(
    look: Res<ActiveLook>,
    mut changed: MessageReader<LookChanged>,
    mut q: Query<(Ref<LookColour>, &mut TextColor)>,
) {
    let all = changed.read().count() > 0;
    for (tag, mut c) in &mut q {
        if all || tag.is_added() {
            c.0 = tag.resolve(&look);
        }
    }
}

pub fn retint_background(
    look: Res<ActiveLook>,
    mut changed: MessageReader<LookChanged>,
    mut q: Query<(Ref<LookColour>, &mut BackgroundColor)>,
) {
    let all = changed.read().count() > 0;
    for (tag, mut c) in &mut q {
        if all || tag.is_added() {
            c.0 = tag.resolve(&look);
        }
    }
}

pub fn retint_sprite(
    look: Res<ActiveLook>,
    mut changed: MessageReader<LookChanged>,
    mut q: Query<(Ref<LookColour>, &mut Sprite)>,
) {
    let all = changed.read().count() > 0;
    for (tag, mut s) in &mut q {
        if all || tag.is_added() {
            s.color = tag.resolve(&look);
        }
    }
}

/// Mutates the material asset itself: entities sharing a material share
/// the new colour. Give an entity its own material to colour it alone.
pub fn retint_material(
    look: Res<ActiveLook>,
    mut changed: MessageReader<LookChanged>,
    materials: Option<ResMut<Assets<StandardMaterial>>>,
    q: Query<(Ref<LookColour>, &MeshMaterial3d<StandardMaterial>)>,
) {
    let all = changed.read().count() > 0;
    let Some(mut materials) = materials else {
        return;
    };
    for (tag, handle) in &q {
        if !(all || tag.is_added()) {
            continue;
        }
        if let Some(m) = materials.get_mut(&handle.0) {
            let c = tag.resolve(&look);
            m.base_color = c;
            if let Some(s) = tag.emissive {
                m.emissive = LinearRgba::from(c) * s;
            }
        }
    }
}

pub fn retext(
    look: Res<ActiveLook>,
    mut changed: MessageReader<LookChanged>,
    mut q: Query<(Ref<LookText>, &mut Text)>,
) {
    let all = changed.read().count() > 0;
    for (tag, mut text) in &mut q {
        if all || tag.is_added() {
            let s = look.string(tag.0);
            if text.0 != s {
                text.0 = s.to_string();
            }
        }
    }
}
```

If `Image::from_buffer`'s 0.18 signature differs, match it from `docs.rs/bevy_image/0.18` (`Image::from_buffer`). Keep the arguments' meaning: PNG, no compressed formats, sRGB, default sampler, default usages. If `MeshMaterial3d` is not in the prelude under the feature set, import it from `bevy::pbr::MeshMaterial3d`.

- [ ] **Step 4: Implement the plugin**

Replace the re-exports in `src/look/mod.rs` with the full module list and plugin:

```rust
pub mod apply;
pub mod contract;
pub mod manifest;
pub mod pack;
#[cfg(not(target_arch = "wasm32"))]
pub mod reload;
pub mod runtime;
pub mod select;
pub mod sniff;
pub mod validate;

use bevy::prelude::*;

pub use apply::{LookAssets, LookChanged, LookColour, LookText};
pub use contract::{Contract, ParamKind, ParamSpec, RecipeSpec, SlotSpec, StringSpec};
pub use manifest::{EndCard, ImageSlot, LookKind, Manifest, Param, RecipeSlot, Slot};
pub use runtime::ActiveLook;
pub use select::Selection;
pub use validate::{Files, Problem, Report, Severity};

/// Retint and retext, in `PostUpdate` so a tag spawned this frame is
/// coloured before the frame renders.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct LookSet;

/// Picks the pack (a `Selection` resource inserted before this plugin
/// wins, else the launch: `--look`, `GX_LOOK`, `?look=`), validates it
/// and keeps it as `ActiveLook`. Add in `main.rs` only, never in
/// `GamePlugin`: the sim must not see it.
pub struct LookPlugin {
    pub contract: &'static Contract,
}

impl Plugin for LookPlugin {
    fn build(&self, app: &mut App) {
        let selection = app
            .world()
            .get_resource::<Selection>()
            .cloned()
            .unwrap_or_else(select::from_launch);
        let look = ActiveLook::resolve(self.contract, selection.clone());
        info!("look: {}", look.id());
        app.insert_resource(selection)
            .insert_resource(look)
            .init_resource::<LookAssets>()
            .add_message::<LookChanged>()
            .add_systems(Startup, apply::load_assets)
            .add_systems(
                PostUpdate,
                (
                    apply::retint_text,
                    apply::retint_background,
                    apply::retint_sprite,
                    apply::retint_material,
                    apply::retext,
                )
                    .in_set(LookSet),
            );
        #[cfg(not(target_arch = "wasm32"))]
        app.add_systems(Update, reload::hot_reload);
    }
}
```

Create a stub `src/look/reload.rs`, so this compiles; Task 6 replaces it:

```rust
//! Native hot reload of a pack folder (Task 6).

use bevy::prelude::*;

pub fn hot_reload() {}
```

- [ ] **Step 5: Wire it into the game**

In `src/main.rs`, add the plugin directly after `display::DisplayPlugin,` inside the plugin tuple:

```rust
            // Look Packs (src/look/): render-side only; never in GamePlugin.
            gamebient_game::look::LookPlugin {
                contract: &ui::theme::LOOK,
            },
```

- [ ] **Step 6: Run the tests and confirm they pass**

Run: `cargo test --test looks && cargo test --all-targets --all-features`

Expected: PASS.

- [ ] **Step 7: Lint and commit**

```bash
cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings
git add src/look src/main.rs tests/looks.rs
git commit -m "feat(look): ActiveLook, LookPlugin and tag-driven retint

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Native hot reload of a pack folder

**Files:**
- Modify: `src/look/reload.rs`
- Test: `tests/looks.rs` (one more test)

**Interfaces:**
- Consumes: `ActiveLook::load_dir`, `log_report` (Task 5); `build_assets`, `LookAssets`, `LookChanged` (Task 5); `pack::dir_fingerprint` (Task 4); `Selection::Dir` (Task 4).
- Produces: the `reload::hot_reload` system (native only). Every 0.5 s it reloads a changed `Selection::Dir` pack, and it keeps the previous look if the new files fail validation.

- [ ] **Step 1: Write the failing test**

Append to `tests/looks.rs`:

```rust
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn a_pack_folder_reloads_when_it_changes_and_survives_a_bad_edit() {
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

    let dir = std::env::temp_dir().join(format!("gx-look-reload-{}", std::process::id()));
    let pack = dir.join("hot");
    std::fs::create_dir_all(&pack).unwrap();
    let write = |pause: &str| {
        std::fs::write(
            pack.join("look.json"),
            format!(r#"{{"v":1,"id":"hot","name":"Hot","kind":"palette","strings":{{"pause":"{pause}"}}}}"#),
        )
        .unwrap();
    };
    write("ONE");

    let mut app = app_with(Selection::Dir(pack.display().to_string()));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(300)));
    let text = app.world_mut().spawn((Text::new("?"), LookText("pause"))).id();
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(app.world().get::<Text>(text).unwrap().0, "ONE");

    write("TWO PLUS"); // a different length, so the fingerprint changes on any filesystem
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(app.world().resource::<ActiveLook>().string("pause"), "TWO PLUS");
    assert_eq!(app.world().get::<Text>(text).unwrap().0, "TWO PLUS", "LookChanged retexts");

    std::fs::write(pack.join("look.json"), "{ not json").unwrap();
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(
        app.world().resource::<ActiveLook>().string("pause"),
        "TWO PLUS",
        "a broken edit keeps the last good look"
    );
    std::fs::remove_dir_all(&dir).ok();
}
```

Run: `cargo test --test looks a_pack_folder_reloads`

Expected: FAIL. The second assertion sees `"ONE"`, because the stub does nothing.

- [ ] **Step 2: Implement hot reload**

Replace `src/look/reload.rs`:

```rust
//! Native hot reload: with `--look <dir>` / `GX_LOOK=<dir>`, the folder is
//! polled every half second; a change that validates replaces the look and
//! sends `LookChanged`, a change that does not is logged and ignored.

use std::path::PathBuf;

use bevy::prelude::*;

use super::apply::{LookAssets, LookChanged, build_assets};
use super::pack::dir_fingerprint;
use super::runtime::{ActiveLook, log_report};
use super::select::Selection;

pub fn hot_reload(
    time: Res<Time>,
    mut timer: Local<Option<Timer>>,
    mut last: Local<Option<u64>>,
    mut look: ResMut<ActiveLook>,
    mut assets: ResMut<LookAssets>,
    mut images: Option<ResMut<Assets<Image>>>,
    mut audio: Option<ResMut<Assets<AudioSource>>>,
    mut changed: MessageWriter<LookChanged>,
    selection: Res<Selection>,
) {
    let Selection::Dir(path) = selection.as_ref() else {
        return;
    };
    let timer = timer.get_or_insert_with(|| Timer::from_seconds(0.5, TimerMode::Repeating));
    if !timer.tick(time.delta()).just_finished() {
        return;
    }
    let path = PathBuf::from(path);
    let fp = dir_fingerprint(&path);
    let Some(prev) = *last else {
        *last = Some(fp);
        return;
    };
    if prev == fp {
        return;
    }
    *last = Some(fp);
    match ActiveLook::load_dir(look.contract, &path) {
        Ok(next) => {
            *look = next;
            *assets = build_assets(&look, images.as_deref_mut(), audio.as_deref_mut());
            changed.write(LookChanged);
            info!("look: reloaded {}", look.id());
        }
        Err(report) => {
            log_report(&report);
            warn!("look: kept the previous pack; fix the errors above");
        }
    }
}
```

Note: this reads `Selection` (the launch choice), not `ActiveLook.source`, because a rejected first load leaves `ActiveLook` at default while the folder should still be watched.

- [ ] **Step 3: Run the tests and confirm they pass**

Run: `cargo test --test looks`

Expected: PASS. If the first assertion fails because the 0.5 s timer never fires under `MinimalPlugins`, raise the loop counts (each `update` advances 300 ms).

- [ ] **Step 4: Lint and commit**

```bash
cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings
git add src/look/reload.rs tests/looks.rs
git commit -m "feat(look): live-reload a pack folder on native

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: The template wears the look (UI, player, audio) and no pack changes a replay

**Files:**
- Modify: `src/game/mod.rs` (pause overlay, about lines 330–390), `src/ui/menu.rs` (`spawn_menu`, about lines 63–100), `src/game/player.rs` (`spawn_player`), `src/game/audio/mod.rs` (`play_sfx` about lines 114–129, `music_director` about lines 435–500)
- Test: `tests/looks.rs` (checksum test), `src/game/audio/mod.rs` (unit test)

**Interfaces:**
- Consumes: `ActiveLook`, `LookAssets`, `LookColour`, `LookText`, `LookPlugin`, `Selection` (Tasks 4–5); `theme::PLAYER` (Task 4).
- Produces: `audio::music_key(&GameState) -> String` (`"music.<state lower case>"`).

- [ ] **Step 1: Write the failing tests**

Append to `tests/looks.rs`:

```rust
use gamebient_game::game::replay::selftest::record_scripted_run;
use gamebient_game::game::replay::{build_verify_app, run_verify_app, verify};

/// D1: every pack replays to the same score, checksum and content hash.
/// In the template nothing in the sim reads a look, so this guards the
/// plugin; in a game it fails the moment a sim system starts reading one.
#[test]
fn no_built_in_pack_changes_a_replay() {
    let replay = record_scripted_run();
    let base = verify(&replay);
    assert!(base.matches, "the selftest replay must verify plainly first: {base:?}");
    for id in look::pack::embedded_ids() {
        let mut app = build_verify_app(&replay);
        app.insert_resource(Selection::Embedded(id.to_string()));
        app.add_plugins(LookPlugin { contract: &theme::LOOK });
        assert_eq!(app.world().resource::<ActiveLook>().id(), id, "pack {id} did not load");
        let v = run_verify_app(&mut app, &replay);
        assert_eq!(
            (v.score, v.checksum, v.content, v.ticks),
            (base.score, base.checksum, base.content, base.ticks),
            "pack {id} changed the sim"
        );
        assert!(v.matches);
    }
}
```

In `src/game/audio/mod.rs`, inside its existing `#[cfg(test)] mod tests`, add:

```rust
    #[test]
    fn music_keys_name_the_state_in_lower_case() {
        assert_eq!(music_key(&GameState::GameOver), "music.gameover");
        assert_eq!(music_key(&GameState::Menu), "music.menu");
    }
```

Run: `cargo test --test looks no_built_in_pack && cargo test --lib music_keys`

Expected: the first passes already (the plugin is sim-free). The second fails to compile (`cannot find function music_key`). The replay test is the guard for every later change in this task: re-run it after each step.

- [ ] **Step 2: Audio overrides**

In `src/game/audio/mod.rs`, add above `pub fn music_director`:

```rust
/// The Look Pack cue name for a state's music: `music.<state, lower case>`.
pub fn music_key(state: &GameState) -> String {
    format!("music.{}", format!("{state:?}").to_lowercase())
}
```

In `play_sfx`, add the parameter `look: Option<Res<crate::look::LookAssets>>,`. Replace the spawn line with:

```rust
        let handle = look
            .as_ref()
            .and_then(|l| l.audio(&format!("sfx.{}", event.name())))
            .unwrap_or_else(|| handle.clone());
        commands.spawn((AudioPlayer(handle), PlaybackSettings::DESPAWN));
```

In `music_director`, add the parameter `look: Option<Res<crate::look::LookAssets>>,`. Replace `let handle = cache.handle(&mut sources, &asset_server, &cue.source);` with:

```rust
    // A pack may replace a state's own music (not a mid-state switch).
    let pack_track = current
        .switched
        .is_none()
        .then(|| look.as_ref().and_then(|l| l.audio(&music_key(state.get()))))
        .flatten();
    let handle = pack_track.unwrap_or_else(|| cache.handle(&mut sources, &asset_server, &cue.source));
```

- [ ] **Step 3: Pause card and menu title**

In `src/game/mod.rs`, in the pause overlay spawn, add a `LookColour` or `LookText` next to each themed component. Import `use crate::look::{LookColour, LookText};` at the top of the file.
- `BackgroundColor(theme::INK.with_alpha(0.62)),` → add `LookColour::new("ink").alpha(0.62),`.
- The `PAUSE_WORD` text tuple → add `LookText("pause"), LookColour::new("paper"),`.
- `BackgroundColor(theme::CORAL),` (the rule) → add `LookColour::new("coral"),`.
- The `PAUSE_RESUME` tuple → add `LookText("pause.resume"), LookColour::new("paper"),`.
- The `PAUSE_QUIT` tuple → add `LookText("pause.quit"), LookColour::new("muted"),`.

In `src/ui/menu.rs`:
- add `use crate::look::{ActiveLook, LookColour};`;
- change the signature to `pub fn spawn_menu(mut commands: Commands, data: Res<GameData>, look: Option<Res<ActiveLook>>)`;
- at the top of the body, add `let title = look.as_deref().map_or(TITLE, |l| l.string("title"));`;
- in the title tuple, use `Text::new(title)` and `DISPLAY.fitted(title, 88.0)`, and add `LookColour::new("paper"),` after `TextColor(PAPER),`.

The title is fitted at spawn, so a hot-reloaded title shows the next time the menu opens. The doc says so.

- [ ] **Step 4: The player recipe**

In `src/game/player.rs`, add the parameter `look: Option<Res<crate::look::ActiveLook>>,` to `spawn_player`. Replace the `insert((Mesh3d(...), MeshMaterial3d(...)))` call with:

```rust
        // Look only: size and colour of the mesh, never the Transform or
        // anything the sim reads (tests/looks.rs checks every pack).
        let size = look
            .as_deref()
            .and_then(|l| l.recipe_number("player", "cuboid", "size"))
            .unwrap_or(1.0) as f32;
        commands.entity(entity).insert((
            Mesh3d(meshes.add(Cuboid::new(size, size, size))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: crate::ui::theme::PLAYER,
                ..default()
            })),
            crate::look::LookColour::new("player"),
        ));
```

- [ ] **Step 5: Run everything**

Run: `cargo test --all-targets --all-features`

Expected: PASS, including `no_built_in_pack_changes_a_replay`, `tests/selftest.rs` and `tests/archetype_order.rs`.

If `archetype_order` fails because `LookColour` split an archetype, read that test's doc comment. It needs a sim-owned sort key, not a render component, so the fix belongs in the test's expectations as its doc describes. Do not remove the tag.

- [ ] **Step 6: See it run**

```bash
GX_LOOK=night-shift cargo run
```

Expected: the log line `look: night-shift`. Press Enter to play and Escape to pause: the pause card reads `LIGHTS LOW` and the player is yellow. Then:

```bash
GX_LOOK=sample-sponsor cargo run
```

Expected: the menu title reads `ACME ARCADE`, and the player is a larger red cube.

Also open the web build with `./build.sh web && (cd dist && python3 -m http.server 8080)`, then visit `http://localhost:8080/?mute=1&look=night-shift`. Expected: the same look in the browser.

- [ ] **Step 7: Lint and commit**

```bash
cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings
git add src/game src/ui/menu.rs tests/looks.rs
git commit -m "feat(look): template pause card, title, player and audio follow the pack

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 8: `look-check` CLI, authoring guide and AGENTS.md

**Files:**
- Create: `src/bin/look_check.rs`, `docs/look-packs.md`
- Modify: `Cargo.toml`, `AGENTS.md`

**Interfaces:**
- Consumes: `pack::read_dir` (Task 4), `validate`, `Severity` (Task 3), `theme::LOOK` (Task 4).
- Produces: `cargo run --bin look-check -- <dir>...`. It prints each problem, then `ok, content_hash <hex>`. Exit code 0 means every pack is clean of errors, 1 means at least one has errors, 2 is a usage error.

- [ ] **Step 1: Add the binary**

In `Cargo.toml`, after the `verify` `[[bin]]`:

```toml
[[bin]]
# Validates Look Packs against this game's contract (docs/look-packs.md).
name = "look-check"
path = "src/bin/look_check.rs"
```

`src/bin/look_check.rs`:

```rust
//! `cargo run --bin look-check -- <pack folder>...`
//! Checks Look Packs against this game's contract (`ui::theme::LOOK`) and
//! the pack limits, prints every problem and each pack's content hash.

//!
//! `build_web.sh` and the editor and verify builds compile every bin for
//! wasm32, where there is no filesystem, so the CLI is native-only and the
//! wasm build gets an empty `main`.

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::process::ExitCode {
    use std::path::Path;
    use std::process::ExitCode;

    use gamebient_game::look::pack;
    use gamebient_game::look::validate::{Severity, validate};
    use gamebient_game::ui::theme;

    let dirs: Vec<String> = std::env::args().skip(1).collect();
    if dirs.is_empty() {
        eprintln!("usage: look-check <pack folder>...");
        return ExitCode::from(2);
    }
    let mut failed = false;
    for d in &dirs {
        let path = Path::new(d.trim_end_matches('/'));
        let id = path.file_name().and_then(|n| n.to_str());
        let files = match pack::read_dir(path) {
            Ok(f) => f,
            Err(e) => {
                println!("{d}: error: pack: {e}");
                failed = true;
                continue;
            }
        };
        let report = validate(&files, &theme::LOOK, id);
        for p in &report.problems {
            let tag = match p.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
            };
            println!("{d}: {tag}: {}: {}", p.at, p.message);
        }
        if report.ok() {
            println!("{d}: ok, content_hash {}", report.content_hash);
        } else {
            failed = true;
        }
    }
    if failed { ExitCode::FAILURE } else { ExitCode::SUCCESS }
}
```

Confirm the wasm build still compiles every bin: `cargo build --target wasm32-unknown-unknown --profile wasm-release`. Expected: success. `build_web.sh` builds without `--bin`.

- [ ] **Step 2: Check it by hand**

```bash
cargo run --bin look-check -- assets/looks/night-shift assets/looks/sample-sponsor
```

Expected: two `ok, content_hash <64 hex>` lines, exit code 0. Then:

```bash
mkdir -p /tmp/gx-bad && printf '{"v":1,"id":"gx-bad","name":"Bad","kind":"palette","palette":{"inc":"#ff0000"}}' > /tmp/gx-bad/look.json
cargo run --bin look-check -- /tmp/gx-bad; echo "exit $?"
```

Expected: `error: palette.inc: unknown role; this game has [...]` and `exit 1`.

- [ ] **Step 3: Write the authoring guide**

Create `docs/look-packs.md`:

````markdown
# Look Packs

A Look Pack restyles a game without touching how it plays: colours, words,
slot recipes and images, sounds. Scores, replays and the leaderboard are the
same under every pack (decision D1). A pack is data only, so it is safe to
take from other people (decision D7, "modding-lite").

## A pack is a folder

```
assets/looks/<id>/
  look.json         required, schema v1
  *.png  *.ogg      optional, referenced from look.json
```

- Flat: no subfolders, no links, no fonts, no code.
- File names: lowercase letters, digits, `-` and `_`, then `.png` or `.ogg`.
- `<id>` is the folder name: `[a-z0-9-]{1,32}`.

Folders in `assets/looks/` are built into the game. Anything else is a folder
you point the game at while working on it.

## Try one

| Where | How |
|---|---|
| Native, built-in | `GX_LOOK=night-shift cargo run` (or `cargo run -- --look night-shift`) |
| Native, your folder (live reload) | `cargo run -- --look ./my-packs/acme` — save `look.json` or a PNG and the game updates within half a second; a broken save is logged and the last good look stays |
| Web | `…/?look=night-shift` (built-in packs only) |
| Check | `cargo run --bin look-check -- ./my-packs/acme` |

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
| `audio` | Cue → `.ogg`. Cues are `sfx.<sound>` and `music.<screen>`; the game lists them. |
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
  The look module fills them in on the frame they spawn and on every reload.
- **Ask it**: `Option<Res<ActiveLook>>` and `colour(role)`, `string(key)`,
  `slot(name)`, `recipe_number(slot, recipe, param)`; `Res<LookAssets>` for a
  pack's decoded images and audio overrides.

**The one rule:** nothing in the sim (`SimSet`, anything the replay verifier
runs) reads `ActiveLook` or `LookAssets`. `tests/looks.rs`
(`no_built_in_pack_changes_a_replay`) fails if one does. Art is sized to the
sim's hitboxes, never the other way round.

Editor tunables that only change the look are marked `.cosmetic()` (for
example `#[reflect(@Tune::colour("Coin colour").cosmetic())]`), which keeps
them out of a replay's start values, content hash and checksum.
````

- [ ] **Step 4: Point AGENTS.md at it**

In `AGENTS.md`, add a section after the presentation-kit bullets (the ones that end with "...`fonts.rs`, `theme.rs`, `backdrop.rs` and the screens are the game's own."):

```markdown
## Look Packs

- `src/look/` (copied verbatim into games) loads a **Look Pack**: a flat folder of `look.json` plus `.png` / `.ogg`, which restyles palette, strings, slot recipes or images, and audio cues. Built-in packs live in `assets/looks/<id>/` and are embedded by `build.rs`. Guide: `docs/look-packs.md`.
- The game's look contract is `theme::LOOK` in `src/ui/theme.rs`: palette roles, string keys with max lengths, slots, recipes, audio cues and fonts. Add a role or key there before a pack can use it.
- Tag spawned entities with `LookColour::new("role")` / `LookText("key")` instead of reading the pack in spawn code where you can.
- **Never read `ActiveLook` or `LookAssets` from a sim system.** `LookPlugin` goes in `main.rs`, never `GamePlugin`. `tests/looks.rs` replays the selftest under every pack and must stay green.
- Look-only editor tunables are `.cosmetic()`.
- Try a pack: `GX_LOOK=<id> cargo run`, `cargo run -- --look ./folder` (live reload), `?look=<id>` on web. Check one: `cargo run --bin look-check -- <folder>`.
```

- [ ] **Step 5: Run everything, lint and commit**

```bash
cargo fmt --all -- --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test --all-targets --all-features
git add Cargo.toml src/bin/look_check.rs docs/look-packs.md AGENTS.md
git commit -m "feat(look): look-check CLI and pack authoring guide

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

- [ ] **Step 6: Push and open the PR**

```bash
git push -u origin look-packs
gh pr create --repo Bread-Heads-Studios/gamebient-bevy-template --base main --head look-packs \
  --title "Look Packs foundation: cosmetic tunables, pack loader, validator, live reload" \
  --body "Steps 0–1 of the customization assessment (colecovisiongx docs/plans/2026-11-08-game-customization-assessment.md).

- Tune::cosmetic keeps look-only tunables out of GXR2 starts, content hash and checksum
- src/look/: schema-v1 Look Packs, hardened validator (limits, header sniffing, BLAKE3 content hash), embedded + folder sources, --look / GX_LOOK / ?look= selection, native live reload, tag-driven retint
- theme::LOOK contract; template pause card, title, player and audio follow the pack
- look-check CLI, docs/look-packs.md, two example packs (night-shift, sample-sponsor)
- tests/looks.rs: every pack validates, and no pack changes a replay

🤖 Generated with [Claude Code](https://claude.com/claude-code)"
```

---

## Self-review notes (for the executor)

- **Spec coverage:**

  | Spec item | Task |
  |---|---|
  | Step 0 (the cosmetic-hash rule) | Task 1 |
  | Schema v1 slots (palette, recipes, images, strings, fonts, audio, frame, end card, reserved `protagonist`) | Tasks 2–3 |
  | Hardened loader and validator | Task 3 |
  | Embedded packs, `--look` / `GX_LOOK` / `?look=` | Task 4 |
  | Plugin and retint | Task 5 |
  | Live reload | Task 6 |
  | Template adoption and the checksum-equality test (D1) | Task 7 |
  | `look-check` and the authoring guide (D5: hand-authored sponsor demos) | Task 8 |

  Fonts, frame and end card are validated but not yet drawn. That is deliberate (steps 3–4).
- **Wasm builds compile every `[[bin]]`** (`build_web.sh` has no `--bin`). Anything native-only (`pack::read_dir`, `reload`) stays behind `cfg(not(target_arch = "wasm32"))`, including in `look-check`.
- **Watch these API spots against Bevy 0.18:** `Image::from_buffer` arguments, `AudioSource { bytes }`, the `MeshMaterial3d` import path, the `warn_once!` macro, and the `BorderColor` shape (not used). The plan's tests pin the behaviour, so adjust the call and not the test.
- **Determinism tests are the gate.** If any change makes `tests/selftest.rs`, `tests/tunables.rs` or `no_built_in_pack_changes_a_replay` fail, stop and find the sim read. Never relax those tests.
