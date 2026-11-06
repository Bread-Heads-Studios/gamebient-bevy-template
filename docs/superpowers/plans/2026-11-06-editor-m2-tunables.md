# Editor milestone 2: tunables in the game — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Coin Run exposes its gameplay numbers as reflected tunables over the host protocol, applies changes at tick boundaries, records them in a `GXR2` replay with a content hash, and runs its scenario checks inside the wasm build.

**Architecture:** One generic host→game message (`gx:custom`) is added to `gamebient-input` (v0.5.0). The template gains `src/game/tunables/` (attributes, values, ordered index, tick-boundary apply, content hash, host layer, checks) and a `GXR2` replay format whose verifier reproduces mid-run changes. Coin Run is a new game repo built from the template's `editor` branch plus the `eval/vibe` fixture and the a01/d03 changes, with its constants turned into tunables and its tests rewritten as non-panicking checks.

**Tech Stack:** Rust 2024, Bevy 0.18 (`bevy_reflect` custom attributes, `ReflectResource`), `serde`/`serde_json` (already in the lockfile), wasm-bindgen 0.2.108, Node 24 for the wasm probes.

## Global Constraints

- Bevy pinned at **0.18**; check `docs/bevy-reference.md` before writing an API from memory.
- `wasm-bindgen` pinned **exactly** `=0.2.108`; CLI is 0.2.108 (already installed).
- Add no new Bevy features. New crates allowed only if already in `Cargo.lock` (`serde 1.0.228`, `serde_json 1.0.149` are).
- One file per concern; sim state changes only inside `sim::SimSet`; sim reads `TickInput`, `GameRng`, `RunClock` only.
- `sim.rs`, `chip.rs`, `tracker.rs`, `house.rs`, `fit.rs`, `card.rs`, `studio_logo.rs` are copied verbatim fleet-wide: **do not edit `src/game/sim.rs`** (add new systems in `tunables/` and chain them).
- Checks (`checks.rs`) must never panic: wasm32 aborts on panic.
- Every sim change regenerates `tests/fixtures/selftest.gxr` (`cargo run --features verify --bin verify -- --selftest --write tests/fixtures/selftest.gxr`) and the commit says so.
- Gates, run before every commit that touches Rust: `cargo fmt --all -- --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test --all-targets --all-features` (`GX_MUTE=1` is set for every command).
- Commit on branch `editor` (template worktree `.claude/worktrees/editor`), branch `custom-command` (gamebient-input), branch `main` of the new local Coin Run repo. Never push `main`, never open a PR. Pushing the gamebient-input feature branch is allowed (the template must pin a reachable rev).
- Shell scripts: portable BSD/GNU, shellchecked; a new tool gets `tools/test_<name>.sh`.
- Plain words in anything a person sees (labels, check messages).

---

## File map

**gamebient-input** (`libs/gamebient-input`, branch `custom-command`):
- Modify `src/host.rs`: `HostCommand::Custom { name, data }`, `parse_command` arm, tests.
- Modify `js/gx.js`: `case 'gx:custom'`.
- Modify `js/gx.test.mjs`: export-free test of the name guard via a new exported `gxCustomCommand(name, data)` helper.
- Modify `docs/host-protocol.md`, `README.md`, `Cargo.toml` (0.5.0).

**Template** (`editor` branch):
- Modify `Cargo.toml`: dep rev, `serde`, `serde_json`, feature `editor`.
- Create `src/game/tunables/mod.rs` (plugin wiring, `register`), `attr.rs`, `value.rs`, `index.rs`, `apply.rs`, `hash.rs`, `protocol.rs` (feature `editor`), `checks.rs`.
- Modify `src/game/mod.rs`: `pub mod tunables`, systems chained.
- Modify `src/game/replay/mod.rs` (GXR2, `Verdict.content`), `recorder.rs` (starts/tunes), `feeder.rs` (tunes), `selftest.rs` (no change expected).
- Create `tests/tunables.rs`.
- Create `tools/build_editor.sh`, `tools/test_build_editor.sh`.
- Modify `AGENTS.md`, `docs/replay-verification.md`, `docs/conventions.md`.
- Regenerate `tests/fixtures/selftest.gxr`, `tests/fixtures/selftest-wasm.gxr`.

**Coin Run** (`games/coin-run`, new local repo):
- Overlay `eval/vibe/fixture/`, apply `eval/vibe/sample-run/a01.diff` and `d03.diff` (not their fixture hunks).
- Create `src/game/tuning.rs` (the game's tunable resources), rewrite `coins.rs`, `shooting.rs`, `player.rs`, `physics.rs`, `things.rs`, `scoring.rs`, `ui/hud.rs` to read them.
- Create `src/game/checks_list.rs` (the game's `checks::all()` entries), modify `scenario.rs` (`tune`).
- Create `tools/editor_probe.mjs` (wasm conformance probe).

---

### Task 1: `gx:custom` in gamebient-input v0.5.0

**Files:**
- Modify: `libs/gamebient-input/src/host.rs`
- Modify: `libs/gamebient-input/js/gx.js`, `libs/gamebient-input/js/gx.test.mjs`
- Modify: `libs/gamebient-input/docs/host-protocol.md`, `README.md`, `Cargo.toml`

**Interfaces:**
- Produces: `HostCommand::Custom { name: String, data: String }` where `data` is a JSON text (`"null"` when the message had no `data`). Command string form `custom:<name>:<json>`.

- [ ] **Step 1: Write the failing Rust tests** (append to `mod tests` in `src/host.rs`)

```rust
    #[test]
    fn parses_custom_command() {
        assert_eq!(
            parse_command("custom:tunables.set:{\"ammo.start\":8}"),
            Some(HostCommand::Custom {
                name: "tunables.set".into(),
                data: "{\"ammo.start\":8}".into()
            })
        );
        assert_eq!(
            parse_command("custom:run.restart:null"),
            Some(HostCommand::Custom {
                name: "run.restart".into(),
                data: "null".into()
            })
        );
        // The JSON may itself contain colons; only the first two split.
        assert_eq!(
            parse_command("custom:a:{\"k\":\"v:w\"}"),
            Some(HostCommand::Custom {
                name: "a".into(),
                data: "{\"k\":\"v:w\"}".into()
            })
        );
        assert_eq!(parse_command("custom:"), None);
        assert_eq!(parse_command("custom:no-json"), None);
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd libs/gamebient-input && cargo test parses_custom_command`
Expected: compile error, no `Custom` variant.

- [ ] **Step 3: Implement**

In `HostCommand` add, after `Seed`:

```rust
    /// A named host→game message (`gx:custom`). The crate does not interpret
    /// it: `name` is agreed between a host and a game out of band (the
    /// editor's `tunables.*`, `checks.run`, `run.restart`), and `data` is the
    /// message's `data` field as JSON text (`"null"` when absent). Games
    /// ignore names they do not know.
    Custom { name: String, data: String },
```

In `parse_command`, before the `match s`:

```rust
    if let Some(rest) = s.strip_prefix("custom:") {
        let (name, data) = rest.split_once(':')?;
        if name.is_empty() {
            return None;
        }
        return Some(HostCommand::Custom {
            name: name.to_string(),
            data: data.to_string(),
        });
    }
```

Update the doc comment on `parse_command` to list `custom:<name>:<json>`.

- [ ] **Step 4: JS side.** In `js/gx.js` add after the `gx:set` case:

```js
    case 'gx:custom': {
      const cmd = gxCustomCommand(data.name, data.data);
      if (cmd) state.commands.push(cmd);
      return;
    }
```

and near the exports:

```js
// The command string for a `gx:custom` message, or null when the name is
// not a dotted lower-case identifier. `data` is re-serialised so the Rust
// side always receives valid JSON text (`null` when absent).
export function gxCustomCommand(name, data) {
  if (typeof name !== 'string' || !/^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)*$/.test(name)) return null;
  let json;
  try { json = JSON.stringify(data === undefined ? null : data); } catch (_) { return null; }
  if (json === undefined) return null;
  return 'custom:' + name + ':' + json;
}
```

Append to `js/gx.test.mjs`:

```js
import { gxCustomCommand } from './gx.js';

test('gx:custom becomes a custom:<name>:<json> command', () => {
  assert.equal(gxCustomCommand('tunables.set', { 'ammo.start': 8 }), 'custom:tunables.set:{"ammo.start":8}');
  assert.equal(gxCustomCommand('run.restart', undefined), 'custom:run.restart:null');
  assert.equal(gxCustomCommand('Bad Name', {}), null);
  assert.equal(gxCustomCommand(42, {}), null);
});
```

(Merge the import with the existing `import { GX_PAD_BAND, gxComputeFit } from './gx.js';` line.)

- [ ] **Step 5: Docs and version.** In `docs/host-protocol.md`, under "Host → game", add:

```jsonc
{ "type": "gx:custom", "v": 1, "name": "tunables.set", "data": { "ammo.start": 8 } }
```

with the paragraph: "`gx:custom` is the host→game twin of the `custom` event. `name` is a dotted lower-case identifier agreed out of band (the Gamebient editor uses `tunables.list`, `tunables.set`, `checks.run`, `run.restart`); `data` is any JSON value. A game ignores names it does not know; the message is subject to the same origin rules as every other." Set `version = "0.5.0"` in `Cargo.toml`; add a line to the README's protocol summary.

- [ ] **Step 6: Run all gates**

Run: `cd libs/gamebient-input && cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check && node --test js/gx.test.mjs`
Expected: all pass.

- [ ] **Step 7: Commit on a branch and push the branch**

```bash
cd libs/gamebient-input
git switch -c custom-command
git add -A
git commit -m "v0.5.0: gx:custom, a named host->game message

Twin of the custom event: HostCommand::Custom { name, data } with data as
JSON text. The crate does not interpret names; the editor defines its
tunables layer on top. Unknown names are ignored.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
git push -u origin custom-command
git rev-parse HEAD   # <REV> for Task 2
```

---

### Task 2: Template deps, feature flag, attributes and values

**Files:**
- Modify: `Cargo.toml` (template worktree)
- Create: `src/game/tunables/mod.rs`, `src/game/tunables/attr.rs`, `src/game/tunables/value.rs`
- Modify: `src/game/mod.rs` (add `pub mod tunables;`)

**Interfaces:**
- Produces:
  - `tunables::attr::Tune { label, kind: Kind, min, max, step, unit, applies: Applies }` with constructors `Tune::float(label, min, max)`, `Tune::int(label, min, max)`, `Tune::toggle(label)`, `Tune::colour(label)`, `Tune::choice(label)`, `Tune::button(label)`, `Tune::sound(label)`, `Tune::place(label)`, `Tune::curve(label)`, builders `.step(f64)`, `.unit(&'static str)`, `.next_run()`.
  - `tunables::attr::Group { path: &'static str, label: &'static str, kind: GroupKind }` with `Group::thing(path, label)`, `Group::counter(..)`, `Group::rule(..)`.
  - `tunables::attr::Button { A, B }` (unit enum, `Reflect`).
  - `tunables::value::Value` enum: `Float(f32) | Int(i64) | Bool(bool) | Colour(Srgba) | Place(Vec2) | Choice(String) | Curve([f32; 4])`, with `kind() -> Kind`, `to_bytes() -> Vec<u8>`, `from_bytes(kind, &[u8]) -> Option<Value>`, `to_json() -> serde_json::Value`, `from_json(kind, &serde_json::Value) -> Option<Value>`.

- [ ] **Step 1: Cargo.toml**

```toml
# [features] — add:
# Editor build: answers the editor's tunables layer over the host protocol
# and runs scenario checks in the browser (src/game/tunables/protocol.rs,
# checks.rs). tools/build_editor.sh builds it. Never for shipping bundles.
editor = []

# [dependencies] — change the gamebient-input line to:
gamebient-input = { git = "https://github.com/Bread-Heads-Studios/gamebient-input", rev = "<REV>" }
# and add (both already in Cargo.lock through Bevy; no new download):
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

Run: `cargo build` — expected to succeed and update `Cargo.lock` for the new rev only.

- [ ] **Step 2: Write the failing value tests** in `src/game/tunables/value.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_round_trip_every_kind() {
        let all = [
            Value::Float(600.5),
            Value::Int(-3),
            Value::Bool(true),
            Value::Colour(Srgba::new(1.0, 0.85, 0.2, 1.0)),
            Value::Place(Vec2::new(-12.5, 300.0)),
            Value::Choice("B".into()),
            Value::Curve([0.0, 0.25, 0.75, 1.0]),
        ];
        for v in all {
            let b = v.to_bytes();
            assert_eq!(Value::from_bytes(v.kind(), &b), Some(v.clone()), "{v:?}");
        }
        assert_eq!(Value::from_bytes(Kind::Float, &[1, 2]), None);
    }

    #[test]
    fn json_round_trip_and_rejects_wrong_shape() {
        let v = Value::Colour(Srgba::new(1.0, 0.0, 0.5, 1.0));
        assert_eq!(v.to_json(), serde_json::json!("#ff0080"));
        assert_eq!(Value::from_json(Kind::Colour, &v.to_json()), Some(v));
        assert_eq!(Value::from_json(Kind::Int, &serde_json::json!(7)), Some(Value::Int(7)));
        assert_eq!(Value::from_json(Kind::Int, &serde_json::json!(7.5)), None);
        assert_eq!(Value::from_json(Kind::Place, &serde_json::json!([1, 2])), Some(Value::Place(Vec2::new(1.0, 2.0))));
        assert_eq!(Value::from_json(Kind::Bool, &serde_json::json!("yes")), None);
    }

    #[test]
    fn float_bytes_are_bit_exact() {
        let a = Value::Float(0.1 + 0.2).to_bytes();
        let b = Value::Float(0.3).to_bytes();
        assert_ne!(a, b, "the checksum must see the last bit");
    }
}
```

- [ ] **Step 3: Run to verify it fails**: `cargo test tunables::value` → compile error (module missing).

- [ ] **Step 4: Implement `attr.rs`**

```rust
//! The reflection attributes that make a resource field a tunable:
//! `#[reflect(@Tune::float("Bullet speed", 100.0, 1200.0).unit("px/s"))]`
//! on a field, `#[reflect(@Group::thing("bullet", "Bullet"))]` on the
//! resource. Opaque reflect values (stored by `TypeId`, read back with
//! `get_attribute::<Tune>()`); nothing here is sim state.

use bevy::prelude::*;

/// What kind of control a field takes and how its value is encoded.
#[derive(Reflect, Clone, Copy, Debug, PartialEq, Eq)]
#[reflect(opaque)]
pub enum Kind {
    Float,
    Int,
    Bool,
    Colour,
    Choice,
    Button,
    Sound,
    Place,
    Curve,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Float => "f32",
            Kind::Int => "i32",
            Kind::Bool => "bool",
            Kind::Colour => "colour",
            Kind::Choice => "choice",
            Kind::Button => "button",
            Kind::Sound => "sound",
            Kind::Place => "place",
            Kind::Curve => "curve",
        }
    }
}

/// When a change takes effect: at the next sim tick, or only when the next
/// run starts (a starting count, read on `OnEnter(Playing)`).
#[derive(Reflect, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[reflect(opaque)]
pub enum Applies {
    #[default]
    Tick,
    NextRun,
}

/// Field attribute. Ranges are hard limits a card may narrow, never widen.
#[derive(Reflect, Clone, Debug, PartialEq)]
#[reflect(opaque)]
pub struct Tune {
    pub label: &'static str,
    pub kind: Kind,
    pub min: f64,
    pub max: f64,
    pub step: f64,
    pub unit: &'static str,
    pub applies: Applies,
}

impl Tune {
    const fn new(label: &'static str, kind: Kind, min: f64, max: f64, step: f64) -> Self {
        Self { label, kind, min, max, step, unit: "", applies: Applies::Tick }
    }
    pub const fn float(label: &'static str, min: f64, max: f64) -> Self {
        Self::new(label, Kind::Float, min, max, 1.0)
    }
    pub const fn int(label: &'static str, min: i64, max: i64) -> Self {
        Self::new(label, Kind::Int, min as f64, max as f64, 1.0)
    }
    pub const fn toggle(label: &'static str) -> Self { Self::new(label, Kind::Bool, 0.0, 1.0, 1.0) }
    pub const fn colour(label: &'static str) -> Self { Self::new(label, Kind::Colour, 0.0, 1.0, 0.0) }
    pub const fn choice(label: &'static str) -> Self { Self::new(label, Kind::Choice, 0.0, 0.0, 0.0) }
    pub const fn button(label: &'static str) -> Self { Self::new(label, Kind::Button, 0.0, 0.0, 0.0) }
    pub const fn sound(label: &'static str) -> Self { Self::new(label, Kind::Sound, 0.0, 0.0, 0.0) }
    /// `min`/`max` bound both axes (game pixels, origin at the centre).
    pub const fn place(label: &'static str, min: f64, max: f64) -> Self {
        Self::new(label, Kind::Place, min, max, 1.0)
    }
    /// Four y-values in `min..=max` at x = 0, 1/3, 2/3, 1.
    pub const fn curve(label: &'static str, min: f64, max: f64) -> Self {
        Self::new(label, Kind::Curve, min, max, 0.01)
    }
    pub const fn step(mut self, step: f64) -> Self { self.step = step; self }
    pub const fn unit(mut self, unit: &'static str) -> Self { self.unit = unit; self }
    pub const fn next_run(mut self) -> Self { self.applies = Applies::NextRun; self }
}

/// What a group of tunables describes, for the inspector's tabs.
#[derive(Reflect, Clone, Copy, Debug, PartialEq, Eq)]
#[reflect(opaque)]
pub enum GroupKind {
    Thing,
    Counter,
    Rule,
}

/// Resource attribute: the group every field of the resource belongs to.
/// `path` is the prefix of each field's tunable path (`ammo` → `ammo.start`).
#[derive(Reflect, Clone, Debug, PartialEq)]
#[reflect(opaque)]
pub struct Group {
    pub path: &'static str,
    pub label: &'static str,
    pub kind: GroupKind,
}

impl Group {
    pub const fn thing(path: &'static str, label: &'static str) -> Self {
        Self { path, label, kind: GroupKind::Thing }
    }
    pub const fn counter(path: &'static str, label: &'static str) -> Self {
        Self { path, label, kind: GroupKind::Counter }
    }
    pub const fn rule(path: &'static str, label: &'static str) -> Self {
        Self { path, label, kind: GroupKind::Rule }
    }
}

/// The canon action buttons, as a tunable value (`Kind::Button`).
#[derive(Reflect, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Button {
    A,
    #[default]
    B,
}

impl Button {
    /// Whether this button was just pressed on this tick.
    pub fn just_pressed(self, input: &gamebient_input::GameInput) -> bool {
        match self {
            Button::A => input.primary_just_pressed,
            Button::B => input.secondary_just_pressed,
        }
    }
}
```

If `#[reflect(opaque)]` on an enum with `&'static str` fields fails to compile (opaque needs `Clone` only; `Debug`/`PartialEq` registrations are optional), drop the `PartialEq` derive from `Tune`/`Group` and compare fields in tests.

- [ ] **Step 5: Implement `value.rs`**

```rust
//! A tunable's value: the closed set of types a control can bind to, with
//! a bit-exact byte encoding (what the checksum and the replay carry) and a
//! JSON form (what the host protocol carries).

use bevy::color::Srgba;
use bevy::prelude::*;
use serde_json::{Value as Json, json};

use super::attr::Kind;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Float(f32),
    Int(i64),
    Bool(bool),
    Colour(Srgba),
    Place(Vec2),
    /// A unit variant's name (choice, button, sound).
    Choice(String),
    Curve([f32; 4]),
}

impl Value {
    pub fn kind(&self) -> Kind {
        match self {
            Value::Float(_) => Kind::Float,
            Value::Int(_) => Kind::Int,
            Value::Bool(_) => Kind::Bool,
            Value::Colour(_) => Kind::Colour,
            Value::Place(_) => Kind::Place,
            Value::Choice(_) => Kind::Choice,
            Value::Curve(_) => Kind::Curve,
        }
    }

    /// Little-endian, bit-exact. Floats via `to_bits`, so two values that
    /// print alike but differ in the last bit encode differently.
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            Value::Float(f) => f.to_bits().to_le_bytes().to_vec(),
            Value::Int(i) => i.to_le_bytes().to_vec(),
            Value::Bool(b) => vec![u8::from(*b)],
            Value::Colour(c) => [c.red, c.green, c.blue, c.alpha]
                .iter()
                .flat_map(|f| f.to_bits().to_le_bytes())
                .collect(),
            Value::Place(p) => [p.x, p.y].iter().flat_map(|f| f.to_bits().to_le_bytes()).collect(),
            Value::Choice(s) => s.as_bytes().to_vec(),
            Value::Curve(c) => c.iter().flat_map(|f| f.to_bits().to_le_bytes()).collect(),
        }
    }

    pub fn from_bytes(kind: Kind, b: &[u8]) -> Option<Value> {
        fn f32s<const N: usize>(b: &[u8]) -> Option<[f32; N]> {
            if b.len() != N * 4 {
                return None;
            }
            let mut out = [0f32; N];
            for (i, chunk) in b.chunks_exact(4).enumerate() {
                out[i] = f32::from_bits(u32::from_le_bytes(chunk.try_into().ok()?));
            }
            Some(out)
        }
        Some(match kind {
            Kind::Float => Value::Float(f32s::<1>(b)?[0]),
            Kind::Int => Value::Int(i64::from_le_bytes(b.try_into().ok()?)),
            Kind::Bool => Value::Bool(*b.first()? != 0),
            Kind::Colour => {
                let [r, g, bl, a] = f32s::<4>(b)?;
                Value::Colour(Srgba::new(r, g, bl, a))
            }
            Kind::Place => {
                let [x, y] = f32s::<2>(b)?;
                Value::Place(Vec2::new(x, y))
            }
            Kind::Choice | Kind::Button | Kind::Sound => Value::Choice(core::str::from_utf8(b).ok()?.to_string()),
            Kind::Curve => Value::Curve(f32s::<4>(b)?),
        })
    }

    pub fn to_json(&self) -> Json {
        match self {
            Value::Float(f) => json!(f),
            Value::Int(i) => json!(i),
            Value::Bool(b) => json!(b),
            Value::Colour(c) => json!(hex(c)),
            Value::Place(p) => json!([p.x, p.y]),
            Value::Choice(s) => json!(s),
            Value::Curve(c) => json!(c),
        }
    }

    pub fn from_json(kind: Kind, j: &Json) -> Option<Value> {
        Some(match kind {
            Kind::Float => Value::Float(j.as_f64()? as f32),
            Kind::Int => Value::Int(j.as_i64()?),
            Kind::Bool => Value::Bool(j.as_bool()?),
            Kind::Colour => Value::Colour(from_hex(j.as_str()?)?),
            Kind::Place => {
                let a = j.as_array()?;
                if a.len() != 2 {
                    return None;
                }
                Value::Place(Vec2::new(a[0].as_f64()? as f32, a[1].as_f64()? as f32))
            }
            Kind::Choice | Kind::Button | Kind::Sound => Value::Choice(j.as_str()?.to_string()),
            Kind::Curve => {
                let a = j.as_array()?;
                if a.len() != 4 {
                    return None;
                }
                let mut c = [0f32; 4];
                for (i, v) in a.iter().enumerate() {
                    c[i] = v.as_f64()? as f32;
                }
                Value::Curve(c)
            }
        })
    }
}

/// `#rrggbb` (alpha is always 1 for a tunable colour).
fn hex(c: &Srgba) -> String {
    let q = |f: f32| (f.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", q(c.red), q(c.green), q(c.blue))
}

fn from_hex(s: &str) -> Option<Srgba> {
    let s = s.strip_prefix('#')?;
    if s.len() != 6 {
        return None;
    }
    let p = |i: usize| u8::from_str_radix(&s[i..i + 2], 16).ok().map(|v| f32::from(v) / 255.0);
    Some(Srgba::new(p(0)?, p(2)?, p(4)?, 1.0))
}
```

Note the colour JSON round trip test compares `Srgba` after quantising to 8 bits; use `1.0, 0.0, 0.5` → `#ff0080` → `0.50196…`: adjust the test to compare `to_json` of the parsed value instead of the `Srgba` (`assert_eq!(Value::from_json(Kind::Colour, &v.to_json()).unwrap().to_json(), v.to_json())`).

- [ ] **Step 6: `mod.rs` skeleton**

```rust
//! Tunables: gameplay numbers that live in `Reflect` resources, carry
//! labels and ranges as attributes, and can be changed by the editor at a
//! sim-tick boundary. See docs/superpowers/specs/2026-11-06-editor-design.md.

pub mod attr;
pub mod value;

pub use attr::{Applies, Button, Group, GroupKind, Kind, Tune};
pub use value::Value;
```

Add `pub mod tunables;` to `src/game/mod.rs` (alphabetical, after `things`).

- [ ] **Step 7: Run tests**: `cargo test tunables::` → PASS. Then gates. Commit:

```bash
git add Cargo.toml Cargo.lock src/game/mod.rs src/game/tunables
git commit -m "Tunables: attributes and values

Tune/Group reflect attributes, the Button canon enum and the closed Value
set with bit-exact bytes and a JSON form. Pins gamebient-input at the
gx:custom rev; serde/serde_json were already in the lockfile.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 3: The ordered tunable index with reflective read and write

**Files:**
- Create: `src/game/tunables/index.rs`
- Modify: `src/game/tunables/mod.rs`

**Interfaces:**
- Produces:
  - `pub struct Entry { pub id: u16, pub path: String, pub group: usize, pub field: &'static str, pub type_id: TypeId, pub tune: Tune, pub options: Vec<String>, pub default: Value }`
  - `pub struct GroupEntry { pub path: &'static str, pub label: &'static str, pub kind: GroupKind }`
  - `#[derive(Resource, Default)] pub struct TunableIndex { pub entries: Vec<Entry>, pub groups: Vec<GroupEntry> }` with `by_path(&str) -> Option<&Entry>`, `by_id(u16) -> Option<&Entry>`.
  - `pub fn register<T: Resource + Reflect + Typed + GetTypeRegistration + Default>(app: &mut App)` — the one way to add a tunable resource.
  - `pub fn read(world: &World, entry: &Entry) -> Option<Value>` and `pub fn write(world: &mut World, entry: &Entry, value: &Value) -> bool` (false when the kind or option is wrong; never panics).
  - `pub fn snapshot(world: &World) -> Vec<(u16, Value)>` in index order.

- [ ] **Step 1: Failing tests** (in `index.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::tunables::attr::{Button, Group, Tune};

    #[derive(Resource, Reflect, Default)]
    #[reflect(Resource, @Group::counter("ammo", "Ammo"))]
    struct Ammo {
        #[reflect(@Tune::int("Starting ammo", 0, 50).next_run())]
        start: u32,
        #[reflect(@Tune::button("Fire button"))]
        fire: Button,
        /// Not a tunable: no attribute.
        secret: f32,
    }

    #[derive(Resource, Reflect)]
    #[reflect(Resource, @Group::thing("bullet", "Bullet"))]
    struct Bullet {
        #[reflect(@Tune::float("Bullet speed", 100.0, 1200.0).unit("px/s"))]
        speed: f32,
        #[reflect(@Tune::colour("Colour"))]
        colour: bevy::color::Srgba,
    }
    impl Default for Bullet {
        fn default() -> Self {
            Self { speed: 600.0, colour: bevy::color::Srgba::WHITE }
        }
    }

    fn app() -> App {
        let mut app = App::new();
        app.init_resource::<TunableIndex>();
        register::<Ammo>(&mut app);
        register::<Bullet>(&mut app);
        app
    }

    #[test]
    fn index_is_in_registration_order_with_paths_and_options() {
        let app = app();
        let idx = app.world().resource::<TunableIndex>();
        let paths: Vec<&str> = idx.entries.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, ["ammo.start", "ammo.fire", "bullet.speed", "bullet.colour"]);
        assert_eq!(idx.entries[1].options, ["A", "B"]);
        assert_eq!(idx.entries[0].default, Value::Int(0));
        assert_eq!(idx.entries[2].tune.unit, "px/s");
        assert_eq!(idx.groups.len(), 2);
        assert_eq!(idx.by_path("bullet.speed").unwrap().id, 2);
    }

    #[test]
    fn read_and_write_through_reflection() {
        let mut app = app();
        let idx = app.world().resource::<TunableIndex>().clone();
        let speed = idx.by_path("bullet.speed").unwrap();
        assert_eq!(read(app.world(), speed), Some(Value::Float(600.0)));
        assert!(write(app.world_mut(), speed, &Value::Float(900.0)));
        assert_eq!(app.world().resource::<Bullet>().speed, 900.0);
        let fire = idx.by_path("ammo.fire").unwrap();
        assert!(write(app.world_mut(), fire, &Value::Choice("A".into())));
        assert_eq!(app.world().resource::<Ammo>().fire, Button::A);
        assert!(!write(app.world_mut(), fire, &Value::Choice("C".into())), "unknown option");
        assert!(!write(app.world_mut(), speed, &Value::Int(1)), "wrong kind");
        assert_eq!(app.world().resource::<Bullet>().speed, 900.0, "untouched after a bad write");
        let snap = snapshot(app.world());
        assert_eq!(snap[2], (2, Value::Float(900.0)));
    }
}
```

`TunableIndex` needs `Clone` (derive it; `Entry` derives `Clone`).

- [ ] **Step 2: Run** `cargo test tunables::index` → fails to compile.

- [ ] **Step 3: Implement `index.rs`**

```rust
//! The ordered list of every tunable the game exposes, built from
//! reflection at registration time. **Order is registration order**, never
//! `TypeRegistry` iteration order (a hash map): the index position is the
//! `id` the replay and the checksum use, so it must be the same in every
//! build of the same source.

use core::any::TypeId;

use bevy::ecs::reflect::ReflectResource;
use bevy::prelude::*;
use bevy::reflect::{
    DynamicEnum, DynamicVariant, GetPath, GetTypeRegistration, ReflectRef, TypeInfo, Typed,
};

use super::attr::{Group, GroupKind, Kind, Tune};
use super::value::Value;

#[derive(Clone, Debug)]
pub struct Entry {
    pub id: u16,
    pub path: String,
    /// Index into `TunableIndex::groups`.
    pub group: usize,
    pub field: &'static str,
    pub type_id: TypeId,
    pub tune: Tune,
    /// Variant names for choice-like kinds; empty otherwise.
    pub options: Vec<String>,
    pub default: Value,
}

#[derive(Clone, Debug)]
pub struct GroupEntry {
    pub path: &'static str,
    pub label: &'static str,
    pub kind: GroupKind,
}

#[derive(Resource, Default, Clone, Debug)]
pub struct TunableIndex {
    pub entries: Vec<Entry>,
    pub groups: Vec<GroupEntry>,
}

impl TunableIndex {
    pub fn by_path(&self, path: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.path == path)
    }
    pub fn by_id(&self, id: u16) -> Option<&Entry> {
        self.entries.get(usize::from(id))
    }
}

/// Registers `T` as a tunable resource: reflection registration, the
/// resource itself (`Default`), and one index entry per `Tune` field. Call
/// it from the game's plugin in a fixed order.
pub fn register<T: Resource + Reflect + Typed + GetTypeRegistration + Default>(app: &mut App) {
    app.register_type::<T>();
    app.init_resource::<T>();
    let TypeInfo::Struct(info) = T::type_info() else {
        panic!("tunable resources are structs: {}", T::type_path());
    };
    let group = info
        .get_attribute::<Group>()
        .unwrap_or_else(|| panic!("{} has no #[reflect(@Group…)]", T::type_path()))
        .clone();
    let world = app.world_mut();
    let group_ix = {
        let mut idx = world.resource_mut::<TunableIndex>();
        idx.groups.push(GroupEntry { path: group.path, label: group.label, kind: group.kind });
        idx.groups.len() - 1
    };
    let mut pending = Vec::new();
    for field in info.iter() {
        let Some(tune) = field.get_attribute::<Tune>() else { continue };
        pending.push((field.name(), tune.clone(), field.type_info().cloned()));
    }
    for (name, tune, field_info) in pending {
        let options = match (&tune.kind, field_info) {
            (Kind::Choice | Kind::Button | Kind::Sound, Some(TypeInfo::Enum(e))) => {
                e.variant_names().iter().map(|s| s.to_string()).collect()
            }
            _ => Vec::new(),
        };
        let id = world.resource::<TunableIndex>().entries.len() as u16;
        let entry = Entry {
            id,
            path: format!("{}.{}", group.path, name),
            group: group_ix,
            field: name,
            type_id: TypeId::of::<T>(),
            tune,
            options,
            default: Value::Bool(false), // replaced below
        };
        let default = read(world, &entry)
            .unwrap_or_else(|| panic!("{}: field type does not match Tune kind", entry.path));
        world
            .resource_mut::<TunableIndex>()
            .entries
            .push(Entry { default, ..entry });
    }
}

fn with_field<R>(world: &World, entry: &Entry, f: impl FnOnce(&dyn PartialReflect) -> Option<R>) -> Option<R> {
    let registry = world.resource::<AppTypeRegistry>().clone();
    let registry = registry.read();
    let rr = registry.get(entry.type_id)?.data::<ReflectResource>()?;
    let res = rr.reflect(world).ok()?;
    let field = res.reflect_path(entry.field).ok()?;
    f(field)
}

/// The field's current value, or `None` when its Rust type does not fit
/// the attribute's kind.
pub fn read(world: &World, entry: &Entry) -> Option<Value> {
    with_field(world, entry, |field| {
        Some(match entry.tune.kind {
            Kind::Float => Value::Float(*field.try_downcast_ref::<f32>()?),
            Kind::Int => {
                if let Some(v) = field.try_downcast_ref::<u32>() {
                    Value::Int(i64::from(*v))
                } else {
                    Value::Int(i64::from(*field.try_downcast_ref::<i32>()?))
                }
            }
            Kind::Bool => Value::Bool(*field.try_downcast_ref::<bool>()?),
            Kind::Colour => Value::Colour(*field.try_downcast_ref::<bevy::color::Srgba>()?),
            Kind::Place => Value::Place(*field.try_downcast_ref::<Vec2>()?),
            Kind::Curve => Value::Curve(*field.try_downcast_ref::<[f32; 4]>()?),
            Kind::Choice | Kind::Button | Kind::Sound => {
                let ReflectRef::Enum(e) = field.reflect_ref() else { return None };
                Value::Choice(e.variant_name().to_string())
            }
        })
    })
}

/// Writes `value` into the field. Returns false, changing nothing, when the
/// kind does not match, an option is unknown, or a number is out of the
/// attribute's range.
pub fn write(world: &mut World, entry: &Entry, value: &Value) -> bool {
    if value.kind() != entry.tune.kind
        && !(matches!(value, Value::Choice(_)) && matches!(entry.tune.kind, Kind::Button | Kind::Sound))
    {
        return false;
    }
    let (min, max) = (entry.tune.min, entry.tune.max);
    match value {
        Value::Float(f) if f64::from(*f) < min || f64::from(*f) > max => return false,
        Value::Int(i) if (*i as f64) < min || (*i as f64) > max => return false,
        Value::Choice(s) if !entry.options.iter().any(|o| o == s) => return false,
        _ => {}
    }
    let registry = world.resource::<AppTypeRegistry>().clone();
    let registry = registry.read();
    let Some(rr) = registry.get(entry.type_id).and_then(|r| r.data::<ReflectResource>()) else {
        return false;
    };
    let Ok(mut res) = rr.reflect_mut(world) else { return false };
    let Ok(field) = res.reflect_path_mut(entry.field) else { return false };
    match value {
        Value::Float(f) => set(field, *f),
        Value::Int(i) => {
            if let Some(slot) = field.try_downcast_mut::<u32>() {
                let Ok(v) = u32::try_from(*i) else { return false };
                *slot = v;
                true
            } else {
                let Ok(v) = i32::try_from(*i) else { return false };
                set(field, v)
            }
        }
        Value::Bool(b) => set(field, *b),
        Value::Colour(c) => set(field, *c),
        Value::Place(p) => set(field, *p),
        Value::Curve(c) => set(field, *c),
        Value::Choice(name) => {
            field.try_apply(&DynamicEnum::new(name.clone(), DynamicVariant::Unit)).is_ok()
        }
    }
}

fn set<T: Reflect>(field: &mut dyn PartialReflect, v: T) -> bool {
    match field.try_downcast_mut::<T>() {
        Some(slot) => {
            *slot = v;
            true
        }
        None => false,
    }
}

/// Every tunable's current value, in index order.
pub fn snapshot(world: &World) -> Vec<(u16, Value)> {
    let idx = world.resource::<TunableIndex>().clone();
    idx.entries
        .iter()
        .filter_map(|e| read(world, e).map(|v| (e.id, v)))
        .collect()
}
```

API notes to verify while implementing (0.18): `StructInfo::iter()` yields `&NamedField` with `name()`, `type_info()` (`Option<&'static TypeInfo>`), `get_attribute`; `EnumInfo::variant_names()`; `Enum::variant_name()`; `PartialReflect::try_apply`; `GetPath` is implemented for `dyn Reflect`. If `field.type_info()` is `None` for a type without `Typed`, fall back to looking the field's `type_id()` up in the registry.

- [ ] **Step 4: Export from `mod.rs`**: `pub mod index; pub use index::{Entry, GroupEntry, TunableIndex, read, register, snapshot, write};`

- [ ] **Step 5: Run** `cargo test tunables::` → PASS. Gates. Commit "Tunables: ordered index with reflective read and write".

---

### Task 4: Apply at tick boundaries, fold into the checksum, keep a tune log

**Files:**
- Create: `src/game/tunables/apply.rs`, `src/game/tunables/hash.rs`
- Modify: `src/game/tunables/mod.rs` (`TunablesPlugin`), `src/game/mod.rs` (chain + plugin)

**Interfaces:**
- Produces:
  - `#[derive(Resource, Default)] pub struct PendingTunes(pub Vec<(u16, Value)>)` — anyone (host layer, feeder, tests) pushes here.
  - `#[derive(Resource, Default, Clone)] pub struct TuneLog { pub starts: Vec<(u16, Value)>, pub changes: Vec<TuneChange> }`, `pub struct TuneChange { pub tick: u32, pub id: u16, pub value: Value }`.
  - `pub fn apply_pending_tunes(world: &mut World)` — exclusive; in `SimSet` right after `sim::sync_run_clock`; also in `Update` with `run_if(not(in_state(Playing)))`.
  - `pub fn begin_run_tunables(world: &mut World)` — `OnEnter(Playing)` chained after `sim::begin_run`: snapshot → `TuneLog.starts`, fold `hash::content(&log)` into `Checksum`.
  - `hash::fold_value(sum: &mut Checksum, id: u16, value: &Value)`, `hash::content(log: &TuneLog) -> u64`.
  - `TunablesPlugin` (added by `GamePlugin` in both modes).
  - `#[derive(Message)] pub struct TuneApplied { pub id: u16, pub value: Value, pub tick: Option<u32> }` (tick `None` outside a run) for the host layer to report.

- [ ] **Step 1: Failing tests** in `apply.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::replay::build_headless_app;
    use crate::game::sim::{Checksum, PendingSeed, SimTick, seed_bytes};
    use crate::game::states::GameState;
    use crate::game::tunables::{TunableIndex, read};

    fn playing() -> App {
        let mut app = build_headless_app();
        app.world_mut().resource_mut::<PendingSeed>().0 = Some(seed_bytes(1));
        app.update();
        app.world_mut().resource_mut::<NextState<GameState>>().set(GameState::Playing);
        app.update();
        app
    }

    #[test]
    fn a_change_lands_on_the_next_tick_and_is_logged() {
        let mut app = playing();
        let entry = app.world().resource::<TunableIndex>().entries.first().cloned()
            .expect("the template registers at least one tunable (Demo)");
        let before = read(app.world(), &entry).unwrap();
        let next = bump(&before);
        app.world_mut().resource_mut::<PendingTunes>().0.push((entry.id, next.clone()));
        assert_eq!(read(app.world(), &entry), Some(before), "not applied until the tick runs");
        app.update();
        assert_eq!(read(app.world(), &entry), Some(next.clone()));
        let log = app.world().resource::<TuneLog>();
        let tick = app.world().resource::<SimTick>().0;
        assert_eq!(log.changes, vec![TuneChange { tick, id: entry.id, value: next }]);
        assert_eq!(log.starts.len(), app.world().resource::<TunableIndex>().entries.len());
    }

    #[test]
    fn a_change_moves_the_checksum() {
        let mut a = playing();
        let mut b = playing();
        let entry = a.world().resource::<TunableIndex>().entries.first().cloned().unwrap();
        let v = bump(&read(a.world(), &entry).unwrap());
        b.world_mut().resource_mut::<PendingTunes>().0.push((entry.id, v));
        for _ in 0..5 {
            a.update();
            b.update();
        }
        assert_ne!(a.world().resource::<Checksum>().0, b.world().resource::<Checksum>().0);
    }

    #[test]
    fn outside_a_run_a_change_applies_at_once_and_becomes_a_start_value() {
        let mut app = build_headless_app();
        app.update();
        let entry = app.world().resource::<TunableIndex>().entries.first().cloned().unwrap();
        let v = bump(&read(app.world(), &entry).unwrap());
        app.world_mut().resource_mut::<PendingTunes>().0.push((entry.id, v.clone()));
        app.update();
        assert_eq!(read(app.world(), &entry), Some(v.clone()));
        app.world_mut().resource_mut::<NextState<GameState>>().set(GameState::Playing);
        app.update();
        assert_eq!(app.world().resource::<TuneLog>().starts[usize::from(entry.id)].1, v);
        assert!(app.world().resource::<TuneLog>().changes.is_empty());
    }

    fn bump(v: &Value) -> Value {
        match v {
            Value::Float(f) => Value::Float(f + 1.0),
            Value::Int(i) => Value::Int(i + 1),
            Value::Bool(b) => Value::Bool(!b),
            other => other.clone(),
        }
    }
}
```

The template needs at least one tunable for these tests: register a `Demo` tunable resource in the template (`src/game/tunables/demo_tunables.rs` is wrong — one file per concern; put it in `src/game/demo.rs`? That file is the demo cut). Instead add `src/game/tuning.rs` to the template: a `#[reflect(Resource, @Group::rule("run", "Run"))] pub struct RunTuning { #[reflect(@Tune::int("Selftest spare", 0, 10))] pub spare: u32 }` with a comment "TEMPLATE NOTE: replace with the game's own tunables". Keep its default `1` and fold nothing else (the sim does not read it; the checksum folds it through the tune log only).

- [ ] **Step 2: Run** → compile errors.

- [ ] **Step 3: Implement `hash.rs`**

```rust
//! Content hash: FNV-1a over the tune log, so a replay names which game it
//! was played on. Reuses `sim::Checksum`'s fold so the two agree bit for bit.

use crate::game::sim::Checksum;

use super::apply::TuneLog;
use super::value::Value;

/// Folds `(id, value)` as the checksum sees it: the id, the byte length,
/// then the bytes in 8-byte little-endian chunks (zero padded).
pub fn fold_value(sum: &mut Checksum, id: u16, value: &Value) {
    let bytes = value.to_bytes();
    sum.fold(u64::from(id));
    sum.fold(bytes.len() as u64);
    for chunk in bytes.chunks(8) {
        let mut word = [0u8; 8];
        word[..chunk.len()].copy_from_slice(chunk);
        sum.fold(u64::from_le_bytes(word));
    }
}

/// Hash of the start values and every mid-run change, in order.
pub fn content(log: &TuneLog) -> u64 {
    let mut sum = Checksum::default();
    for (id, v) in &log.starts {
        fold_value(&mut sum, *id, v);
    }
    for c in &log.changes {
        sum.fold(u64::from(c.tick));
        fold_value(&mut sum, c.id, &c.value);
    }
    sum.0
}
```

- [ ] **Step 4: Implement `apply.rs`**

```rust
//! Where a tunable change lands: the queue anyone may push to, the one
//! system that drains it (inside `SimSet`, so a value changes between two
//! ticks and never during one), and the log a replay carries.

use bevy::prelude::*;

use crate::game::sim::{Checksum, SimTick};
use crate::game::states::GameState;

use super::hash::{content, fold_value};
use super::index::{TunableIndex, snapshot, write};
use super::value::Value;

#[derive(Resource, Default, Debug)]
pub struct PendingTunes(pub Vec<(u16, Value)>);

#[derive(Clone, Debug, PartialEq)]
pub struct TuneChange {
    pub tick: u32,
    pub id: u16,
    pub value: Value,
}

/// Start values and mid-run changes of the current (or last) run.
#[derive(Resource, Default, Clone, Debug, PartialEq)]
pub struct TuneLog {
    pub starts: Vec<(u16, Value)>,
    pub changes: Vec<TuneChange>,
}

/// Reported after a change is applied (host layer, tests).
#[derive(Message, Debug, Clone, PartialEq)]
pub struct TuneApplied {
    pub id: u16,
    pub value: Value,
    /// The sim tick it applied on; `None` outside a run.
    pub tick: Option<u32>,
}

/// Drains [`PendingTunes`] in order. In `SimSet` (right after
/// `sim::sync_run_clock`) during a run; in `Update` outside one.
pub fn apply_pending_tunes(world: &mut World) {
    let pending: Vec<(u16, Value)> = core::mem::take(&mut world.resource_mut::<PendingTunes>().0);
    if pending.is_empty() {
        return;
    }
    let playing = *world.resource::<State<GameState>>().get() == GameState::Playing;
    let idx = world.resource::<TunableIndex>().clone();
    for (id, value) in pending {
        let Some(entry) = idx.by_id(id) else { continue };
        if !write(world, entry, &value) {
            continue;
        }
        let tick = playing.then(|| world.resource::<SimTick>().0);
        if let Some(tick) = tick {
            fold_value(&mut world.resource_mut::<Checksum>(), id, &value);
            world.resource_mut::<TuneLog>().changes.push(TuneChange { tick, id, value: value.clone() });
        }
        world.write_message(TuneApplied { id, value, tick });
    }
}

/// `OnEnter(Playing)`, chained after `sim::begin_run`: the run's start
/// values become the log's `starts`, and their hash is the first thing the
/// fresh checksum folds.
pub fn begin_run_tunables(world: &mut World) {
    let starts = snapshot(world);
    let log = TuneLog { starts, changes: Vec::new() };
    let c = content(&log);
    world.resource_mut::<Checksum>().fold(c);
    *world.resource_mut::<TuneLog>() = log;
}
```

- [ ] **Step 5: `mod.rs` plugin and wiring**

```rust
pub mod apply;
pub mod hash;
pub mod index;
// (attr, value as before)

pub use apply::{PendingTunes, TuneApplied, TuneChange, TuneLog, apply_pending_tunes, begin_run_tunables};

/// Registered by `GamePlugin` in both modes. Games register their tunable
/// resources *after* this plugin, in a fixed order, with [`register`].
pub struct TunablesPlugin;

impl Plugin for TunablesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<index::TunableIndex>()
            .init_resource::<apply::PendingTunes>()
            .init_resource::<apply::TuneLog>()
            .add_message::<apply::TuneApplied>()
            .add_systems(
                Update,
                apply::apply_pending_tunes.run_if(not(in_state(crate::game::states::GameState::Playing))),
            );
    }
}
```

In `src/game/mod.rs`:
- `.add_plugins(tunables::TunablesPlugin)` right after `host::HostBridgePlugin`;
- then `tunables::register::<tuning::RunTuning>(app);` (new `pub mod tuning;`), with the comment "Game tunables, registered here in a fixed order (their index is replay data)";
- in the `OnEnter(Playing)` chain insert `tunables::begin_run_tunables` right after `sim::begin_run` (before `replay::recorder::begin_recording`);
- in the `SimSet` chain insert `tunables::apply_pending_tunes` right after `sim::sync_run_clock`.

- [ ] **Step 6: Run** `cargo test tunables::` → PASS. Then `cargo test` — `selftest` tests now fail (checksum folds the content hash). Regenerate: `cargo run --features verify --bin verify -- --selftest --write tests/fixtures/selftest.gxr`. Gates green. Commit:

"Tunables: tick-boundary apply, tune log, content hash in the checksum. Regenerates tests/fixtures/selftest.gxr (the checksum now folds the content hash). The wasm fixture is regenerated in a later commit with the GXR2 format."

(`selftest-wasm.gxr` cannot be checked natively; `build_web.sh` would fail until Task 5 regenerates it. Say so in the commit.)

---

### Task 5: Replay format GXR2

**Files:**
- Modify: `src/game/replay/mod.rs`, `recorder.rs`, `feeder.rs`
- Modify: `tools/verify_fixture.mjs` (print `content`), `docs/replay-verification.md`

**Interfaces:**
- Produces: `Replay { build, tick_hz, seed, origin, content: u64, ticks, score, checksum, starts: Vec<(u16, Vec<u8>)>, tunes: Vec<TuneRecord>, runs }`, `TuneRecord { tick: u32, id: u16, bytes: Vec<u8> }`, `MAGIC = b"GXR2"`, `DecodeError::BadValue`, `Verdict { ..., content: u64 }` and JSON `"content":"<decimal>"`. `ReplayFeeder::new` takes tunes; `feed_tick` pushes due tunes into `PendingTunes`.

- [ ] **Step 1: Failing tests** (extend `mod tests` in `replay/mod.rs`; update `sample()` to fill `content: 7, starts: vec![(0, vec![1,0,0,0,0,0,0,0])], tunes: vec![TuneRecord{tick: 3, id: 0, bytes: vec![2,0,0,0,0,0,0,0]}]`)

```rust
    #[test]
    fn gxr2_round_trips_starts_and_tunes() {
        let r = sample();
        let bytes = r.encode();
        assert_eq!(&bytes[..4], b"GXR2");
        assert_eq!(Replay::decode(&bytes).unwrap(), r);
    }

    #[test]
    fn gxr1_is_rejected() {
        assert_eq!(Replay::decode(b"GXR1\x00").err(), Some(DecodeError::BadMagic));
    }

    #[test]
    fn verdict_json_carries_content_as_a_string() {
        let v = Verdict { score: 1, checksum: 2, content: u64::MAX, ticks: 3, ended: Ended::Cap, matches: false };
        assert!(v.to_json().contains("\"content\":\"18446744073709551615\""));
    }
```

- [ ] **Step 2: Run** → compile errors.

- [ ] **Step 3: Implement the format.** Header doc at the top of `replay/mod.rs` becomes the GXR2 layout from the spec. Code changes:

```rust
pub const MAGIC: &[u8; 4] = b"GXR2";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TuneRecord { pub tick: u32, pub id: u16, pub bytes: Vec<u8> }

// Replay gains: pub content: u64, pub starts: Vec<(u16, Vec<u8>)>, pub tunes: Vec<TuneRecord>
// DecodeError gains: BadValue  (a value longer than 255 bytes, or more than 4096 starts/tunes)

// encode, after `checksum`:
        out.extend_from_slice(&(self.starts.len() as u16).to_le_bytes());
        for (id, b) in &self.starts {
            out.extend_from_slice(&id.to_le_bytes());
            out.push(b.len().min(255) as u8);
            out.extend_from_slice(&b[..b.len().min(255)]);
        }
        out.extend_from_slice(&(self.tunes.len() as u32).to_le_bytes());
        for t in &self.tunes {
            out.extend_from_slice(&t.tick.to_le_bytes());
            out.extend_from_slice(&t.id.to_le_bytes());
            out.push(t.bytes.len().min(255) as u8);
            out.extend_from_slice(&t.bytes[..t.bytes.len().min(255)]);
        }
// (content goes right after origin, before ticks; mirror it in decode.)
// decode: cap starts at 4096 and tunes at 4096 (BadValue) before reading them.
```

Field order in `encode`/`decode`: magic, build, tick_hz, seed, origin, **content**, ticks, score, checksum, **starts**, **tunes**, runs.

`Verdict` gains `content: u64`; `to_json` emits `"content":"{}"` as a decimal string beside `checksum`. `run_verify_app` sets `content: app.world().resource::<TuneLog>()` → `hash::content(..)` and `matches: score == replay.score && checksum == replay.checksum && content == replay.content`.

`build_verify_app`: after `ReplayFeeder::new(replay)`, stage the start values:

```rust
    {
        let idx = app.world().resource::<TunableIndex>().clone();
        let mut pending = app.world_mut().resource_mut::<PendingTunes>();
        for (id, bytes) in &replay.starts {
            if let Some(entry) = idx.by_id(*id)
                && let Some(v) = Value::from_bytes(entry.tune.kind, bytes)
            {
                pending.0.push((*id, v));
            }
        }
    }
```

(They apply in the first `app.update()` of `run_verify_app`, in `Update`, before `Playing` is entered.)

- [ ] **Step 4: Recorder.** `empty()` fills `content: 0, starts: Vec::new(), tunes: Vec::new()`. `begin_recording` becomes `pub fn begin_recording(seed: Res<RunSeed>, log: Res<TuneLog>, mut rec: ResMut<ReplayRecorder>)` and sets `rec.replay.starts = log.starts.iter().map(|(id, v)| (*id, v.to_bytes())).collect()` after `begin`. `seal_run` takes `log: Res<TuneLog>` and before sealing sets `rec.replay.tunes = log.changes.iter().map(|c| TuneRecord { tick: c.tick, id: c.id, bytes: c.value.to_bytes() }).collect(); rec.replay.content = hash::content(&log);`. Recorder unit tests: update the `tick()` helper and add `assert_eq!(r.content, 0)` where relevant; add a test that `seal` carries `tunes` through.

- [ ] **Step 5: Feeder.** `ReplayFeeder` gains `tunes: Vec<TuneRecord>, tune_at: usize, kinds: Vec<Kind>` — no: keep the feeder free of the index; store `tunes` and let `feed_tick` take `Res<TunableIndex>`:

```rust
pub fn feed_tick(
    mut feeder: ResMut<ReplayFeeder>,
    mut acc: ResMut<InputAccumulator>,
    tick: Res<SimTick>,
    idx: Res<TunableIndex>,
    mut pending: ResMut<PendingTunes>,
) {
    // The tick about to be simulated is SimTick + 1 (advance_tick runs first in SimSet).
    let next = tick.0 + 1;
    while let Some(t) = feeder.tunes.get(feeder.tune_at)
        && t.tick <= next
    {
        if let Some(entry) = idx.by_id(t.id)
            && let Some(v) = Value::from_bytes(entry.tune.kind, &t.bytes)
        {
            pending.0.push((t.id, v));
        }
        feeder.tune_at += 1;
    }
    // ... existing input feed unchanged
}
```

Feeder unit test: a replay with one tune at tick 2 → after `feed_tick` is called with `SimTick(1)`, `PendingTunes` holds it, and `tune_at == 1`.

- [ ] **Step 6: `tools/verify_fixture.mjs`** already prints the whole JSON; no change. Docs: update the format block and the verdict description (`content`), add a "Tunables" paragraph under determinism rules (rule 11: "Tunable values are run content: every change applies in `SimSet`, folds into `Checksum`, and is carried by the replay; the verifier reproduces it. A leaderboard accepts only runs whose `content` equals the release's default hash."). Also update `docs/conventions.md` "Sim determinism" with one line pointing at rule 11.

- [ ] **Step 7: Regenerate fixtures and run gates.**

```bash
cargo run --features verify --bin verify -- --selftest --write tests/fixtures/selftest.gxr
tools/build_verify.sh && node tools/verify_fixture.mjs --record tests/fixtures/selftest-wasm.gxr && node tools/verify_fixture.mjs tests/fixtures/selftest-wasm.gxr && node tools/verify_fixture.mjs tests/fixtures/selftest.gxr
cargo fmt --all -- --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test --all-targets --all-features
```

Expected: both verifier runs print `"matches":true` with a `"content"` field. Commit: "Replay GXR2: content hash, start values and mid-run tune log. Regenerates both selftest fixtures."

---

### Task 6: Determinism test for mid-run changes

**Files:**
- Create: `tests/tunables.rs`

- [ ] **Step 1: Write the test** (it must fail if Task 5's feeder is disabled; verify that by commenting the `while let` out once, then restore)

```rust
//! A tunable change made mid-run is replay content: the verifier reproduces
//! it, and the same inputs under a different value do not verify.

use bevy::prelude::*;
use gamebient_game::game::replay::recorder::ReplayRecorder;
use gamebient_game::game::replay::selftest::{SELFTEST_SEED, script};
use gamebient_game::game::replay::{Replay, build_headless_app, verify};
use gamebient_game::game::sim::{PendingSeed, SimTick};
use gamebient_game::game::states::GameState;
use gamebient_game::game::tunables::{PendingTunes, TunableIndex, Value, read};

const TICKS: u32 = 300;
const CHANGE_AT: u32 = 120;

/// Records the selftest script and, on tick `change_at`, queues `value`
/// for the first tunable. `None` records a plain run.
fn record(change_at: Option<u32>, value: Value) -> Replay {
    let mut app = build_headless_app();
    app.add_systems(PreUpdate, script.before(gamebient_input::input::accumulate_input));
    app.world_mut().resource_mut::<PendingSeed>().0 = Some(SELFTEST_SEED);
    app.update();
    app.world_mut().resource_mut::<NextState<GameState>>().set(GameState::Playing);
    app.update();
    let id = app.world().resource::<TunableIndex>().entries[0].id;
    while app.world().resource::<SimTick>().0 < TICKS {
        if Some(app.world().resource::<SimTick>().0) == change_at {
            app.world_mut().resource_mut::<PendingTunes>().0.push((id, value.clone()));
        }
        app.update();
    }
    app.world_mut().resource_mut::<NextState<GameState>>().set(GameState::GameOver);
    app.update();
    app.world().resource::<ReplayRecorder>().last_run().cloned().unwrap()
}

fn bumped(app_value: &Value) -> Value {
    match app_value {
        Value::Int(i) => Value::Int(i + 1),
        Value::Float(f) => Value::Float(f + 1.0),
        Value::Bool(b) => Value::Bool(!b),
        other => other.clone(),
    }
}

fn first_default() -> Value {
    let app = build_headless_app();
    let e = app.world().resource::<TunableIndex>().entries[0].clone();
    read(app.world(), &e).unwrap()
}

#[test]
fn a_mid_run_change_replays_identically() {
    let v = bumped(&first_default());
    let replay = record(Some(CHANGE_AT), v);
    assert_eq!(replay.tunes.len(), 1, "one change recorded");
    assert_eq!(replay.tunes[0].tick, CHANGE_AT + 1, "applied on the tick after it was queued");
    let verdict = verify(&replay);
    assert!(verdict.matches, "{verdict:?} vs {} / {} / {}", replay.score, replay.checksum, replay.content);
    assert_eq!(verdict.content, replay.content);
}

#[test]
fn the_same_inputs_under_another_value_do_not_verify() {
    let plain = record(None, first_default());
    let changed = record(Some(CHANGE_AT), bumped(&first_default()));
    assert_ne!(plain.content, changed.content);
    assert_ne!(plain.checksum, changed.checksum);
    // Lie: claim the changed run's result with the plain run's tune log.
    let mut forged = changed.clone();
    forged.tunes.clear();
    forged.content = plain.content;
    let v = verify(&forged);
    assert!(!v.matches, "a replay stripped of its change must not verify: {v:?}");
}

#[test]
fn different_start_values_are_different_content() {
    let mut app = build_headless_app();
    app.update();
    let e = app.world().resource::<TunableIndex>().entries[0].clone();
    let v = bumped(&read(app.world(), &e).unwrap());
    app.world_mut().resource_mut::<PendingTunes>().0.push((e.id, v));
    app.update();
    app.world_mut().resource_mut::<NextState<GameState>>().set(GameState::Playing);
    app.update();
    app.world_mut().resource_mut::<NextState<GameState>>().set(GameState::GameOver);
    app.update();
    let tweaked = app.world().resource::<ReplayRecorder>().last_run().cloned().unwrap();
    let plain = record(None, first_default());
    assert_ne!(tweaked.content, plain.content);
    assert_ne!(tweaked.starts, plain.starts);
}
```

- [ ] **Step 2: Run** `cargo test --test tunables` → PASS. Watch it fail: temporarily make `feed_tick` skip tunes, run, see `a_mid_run_change_replays_identically` fail, restore.

- [ ] **Step 3: Gates, commit** "tests/tunables.rs: a mid-run tunable change replays identically".

---

### Task 7: The host layer (`tunables.list`, `tunables.set`, `run.restart`) behind `editor`

**Files:**
- Create: `src/game/tunables/protocol.rs`
- Modify: `src/game/tunables/mod.rs`, `src/game/mod.rs` (`RestartPending` handling), `src/game/host.rs` (nothing: `HostCommand::Custom` falls through the existing `match` — add a `HostCommand::Custom { .. } => {}` arm so the match stays exhaustive)

**Interfaces:**
- Produces (feature `editor`): systems `answer_tunables_list`, `queue_tunable_sets`, `report_tune_applied`, `restart_run`; wire JSON exactly as in the spec table. `pub fn catalog_json(world: &World, checks: &[&str], sounds: &[&str]) -> serde_json::Value`. `#[derive(Resource, Default)] pub struct EditorInfo { pub checks: Vec<&'static str>, pub sounds: Vec<&'static str> }` the game fills.

- [ ] **Step 1: Failing test** (in `protocol.rs`, `#[cfg(all(test, feature = "editor"))]`)

```rust
    #[test]
    fn list_set_and_report_over_host_messages() {
        use gamebient_input::{HostCommand, HostEvent};
        let mut app = build_headless_app();
        app.update();
        app.world_mut().write_message(HostCommand::Custom { name: "tunables.list".into(), data: "null".into() });
        app.update();
        let events = drain_custom(&mut app);
        let (name, data) = &events[0];
        assert_eq!(name, "tunables");
        let j: serde_json::Value = serde_json::from_str(data).unwrap();
        assert_eq!(j["tunables"][0]["path"], "run.spare");
        assert_eq!(j["tunables"][0]["type"], "i32");
        assert_eq!(j["tunables"][0]["value"], 1);
        assert!(j["content"].is_string());

        app.world_mut().write_message(HostCommand::Custom { name: "tunables.set".into(), data: "{\"run.spare\":4}".into() });
        app.update();
        let events = drain_custom(&mut app);
        assert_eq!(events[0].0, "tunable");
        let j: serde_json::Value = serde_json::from_str(&events[0].1).unwrap();
        assert_eq!(j["path"], "run.spare");
        assert_eq!(j["value"], 4);
        assert_eq!(j["applied"], "now");
        assert_eq!(app.world().resource::<crate::game::tuning::RunTuning>().spare, 4);
    }
```

with `drain_custom` reading `Messages<HostEvent>` and collecting `(name, data)` from `HostEvent::Custom`.

- [ ] **Step 2: Implement.** `catalog_json` builds `{ game, build: env!("GX_BUILD_ID"), content: "<decimal of hash::content(TuneLog)>", groups: [{path,label,kind}], tunables: [{path, group, label, type, min, max, step, unit, applies: "tick"|"next_run", options?, value, default}], checks: [...], sounds: [...] }`. Systems in `Update`:

```rust
pub fn handle_custom_commands(
    mut commands: MessageReader<HostCommand>,
    world_idx: Res<TunableIndex>,
    mut pending: ResMut<PendingTunes>,
    mut restart: ResMut<RestartPending>,
    mut list_requested: Local<bool>,
) { /* match name: "tunables.list" => *list_requested = true; "tunables.set" => parse object, for each (path, json) look up entry, Value::from_json(kind, v), push; "run.restart" => restart.0 = true; _ => {} */ }
```

Because `catalog_json` needs `&World`, make the list answer an exclusive system `answer_tunables_list(world: &mut World)` that checks a `ListRequested(bool)` resource set by the handler, builds the JSON and writes `HostEvent::Custom { name: "tunables".into(), data: json.to_string() }`. `report_tune_applied` maps `TuneApplied` → `HostEvent::Custom { name: "tunable", data: {path, value, applied: "tick"|"now", tick} }` (`"now"` when `tick` is `None`). Also post the catalog once on `OnEnter(Playing)` (spec: "once on started").

`run.restart`: `RestartPending(bool)` resource (always compiled, in `tunables/apply.rs`? No — new concern, put it in `protocol.rs` behind `editor`): in `Update`, when set: if state is `Playing` → `next.set(GameState::Menu)` and keep the flag; `OnEnter(Menu)` system `resume_restart`: if flag → clear and `next.set(GameState::Playing)`. If state is `Menu`/`GameOver`/others → `next.set(GameState::Playing)` directly and clear.

Register everything in `TunablesPlugin` under `#[cfg(feature = "editor")]`, plus `app.init_resource::<EditorInfo>()`.

- [ ] **Step 3: Run** `cargo test --features editor tunables::protocol` → PASS. Gates with `--all-features` (which includes `editor`). Commit "Tunables host layer: tunables.list/set, run.restart (feature editor)".

---

### Task 8: Checks that never panic, runnable from the host

**Files:**
- Create: `src/game/tunables/checks.rs`
- Modify: `src/game/tunables/mod.rs`, `src/game/tunables/protocol.rs` (`checks.run`)

**Interfaces:**
- Produces: `pub struct Check { pub name: &'static str, pub title: &'static str, pub run: fn(&Tunes) -> Result<(), String> }`, `pub type Tunes = Vec<(String, Value)>` (path → value), `pub fn run_all(checks: &[Check], names: &[&str], tunes: &Tunes) -> Vec<(String, Result<(), String>)>`, macro `ensure!(cond, "message {}", x)`, a `#[derive(Resource, Default)] pub struct CheckList(pub &'static [Check])` the game fills, and the `checks.run` host handler emitting one `check` event per result `{ name, ok, message }`.

- [ ] **Step 1: Failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn passes(_: &Tunes) -> Result<(), String> { Ok(()) }
    fn fails_when_zero(t: &Tunes) -> Result<(), String> {
        let v = t.iter().find(|(p, _)| p == "ammo.start").map(|(_, v)| v.clone());
        ensure!(v != Some(Value::Int(0)), "B never fires with 0 ammo");
        Ok(())
    }
    const CHECKS: &[Check] = &[
        Check { name: "a", title: "A passes", run: passes },
        Check { name: "b", title: "B fires", run: fails_when_zero },
    ];

    #[test]
    fn run_all_reports_each_named_check_and_unknown_names() {
        let tunes = vec![("ammo.start".to_string(), Value::Int(0))];
        let out = run_all(CHECKS, &["a", "b", "zzz"], &tunes);
        assert_eq!(out[0], ("a".into(), Ok(())));
        assert_eq!(out[1], ("b".into(), Err("B never fires with 0 ammo".into())));
        assert_eq!(out[2].1, Err("no check named zzz".into()));
    }
}
```

- [ ] **Step 2: Implement** `checks.rs` with the types above, the macro (`#[macro_export] macro_rules! ensure`), `run_all` (a name not found yields `Err("no check named …")`), and a `#[test] fn every_registered_check_passes_at_defaults()` that the game's test suite inherits: iterates `CheckList` from a headless app and asserts each `Ok`, printing the message otherwise. Host handler in `protocol.rs` (`checks.run` → `{names}`; `tunes` = current `snapshot` mapped to paths; one `HostEvent::Custom { name: "check", data: {name, ok, message} }` per result, in `Update` — a check builds its own headless `App`, which is allowed from a non-sim system).

- [ ] **Step 3: Run**, gates, commit "Checks: a non-panicking scenario check registry runnable from the host".

---

### Task 9: `tools/build_editor.sh` and the docs

**Files:**
- Create: `tools/build_editor.sh`, `tools/test_build_editor.sh`
- Modify: `AGENTS.md` (a "Tunables and the editor" section under "How to add things"), `docs/conventions.md`, `.claude/rules/sim.md` (one line: tunables apply only through `PendingTunes`)

- [ ] **Step 1: Script**

```bash
#!/usr/bin/env bash
# Builds the editor bundle: the game with `--features editor` as a web
# bundle into dist-editor/, without wasm-opt, brotli, demo split or the
# cartridge fetch (the editor serves this directory locally; speed over
# size). Usage: tools/build_editor.sh
set -euo pipefail
cd "$(dirname "$0")/.."
rustup target add wasm32-unknown-unknown 2>/dev/null || true
command -v wasm-bindgen >/dev/null || { echo "wasm-bindgen-cli missing (see install.sh)" >&2; exit 1; }
cargo build --locked --profile wasm-release --target wasm32-unknown-unknown --features editor
wasm=$(find target/wasm32-unknown-unknown/wasm-release -maxdepth 1 -name '*.wasm' ! -name 'verify.wasm')
[ "$(printf '%s\n' "$wasm" | wc -l | tr -d ' ')" -eq 1 ] || { echo "ambiguous .wasm in target (cargo clean?)" >&2; exit 1; }
rm -rf dist-editor && mkdir -p dist-editor
wasm-bindgen --out-dir dist-editor --out-name gamebient-game --target web "$wasm"
cp index.html dist-editor/
[ -d assets ] && cp -r assets dist-editor/assets
echo "dist-editor/ ready: serve it from a localhost origin (the editor does)."
```

`tools/test_build_editor.sh`: shellcheck the script and assert it refuses when `wasm-bindgen` is absent (`PATH=/usr/bin:/bin tools/build_editor.sh` exits 1 with the message). Add `/dist-editor` to `.gitignore`. Run `shellcheck tools/build_editor.sh tools/test_build_editor.sh`.

- [ ] **Step 2: Docs.** AGENTS.md "How to add things" bullet:

"**A tunable:** a field of a `Reflect` resource with `#[reflect(@Tune::float("Label", min, max).unit("px/s"))]`, on a resource marked `#[reflect(Resource, @Group::thing("bullet", "Bullet"))]`, registered with `tunables::register::<T>(app)` in `GamePlugin` in a fixed order (the index is replay data: never reorder registrations between releases). Gameplay reads the resource; changes arrive through `tunables::PendingTunes` and land in `SimSet` (`tunables::apply_pending_tunes`), fold into the checksum and ride in the replay (`GXR2`). `tools/build_editor.sh` builds the editor bundle; see `docs/superpowers/specs/2026-11-06-editor-design.md`."

Also the gotcha: "`#[reflect(@…)]` attribute values are opaque reflect types; a tunable field's Rust type must match its `Tune` kind (`f32`, `u32`/`i32`, `bool`, `Srgba`, `Vec2`, `[f32; 4]`, or a unit enum) or `register` panics at startup, which `cargo test` catches."

- [ ] **Step 3: Gates (`tools/test_build_editor.sh`, shellcheck), commit** "tools/build_editor.sh and tunables docs".

---

### Task 10: Coin Run repository

**Files:** new repo `games/coin-run`.

- [ ] **Step 1: Create it from the template's `editor` branch**

```bash
cd /Users/kelliott/Gamebient/colecovisiongx/games
git clone --branch editor /Users/kelliott/Gamebient/colecovisiongx/libs/gamebient-bevy-template coin-run
cd coin-run
git remote remove origin          # local only; the owner creates Bread-Heads-Studios/coin-run
./init-game.sh "Coin Run" Bread-Heads-Studios/coin-run
rm -rf eval                       # the eval harness is the template's, not a game's
cp -R ../../libs/gamebient-bevy-template/.claude/worktrees/editor/eval/vibe/fixture/. .
git apply --exclude='tests/fixtures/*' ../../libs/gamebient-bevy-template/.claude/worktrees/editor/eval/vibe/sample-run/a01.diff
git apply --exclude='tests/fixtures/*' ../../libs/gamebient-bevy-template/.claude/worktrees/editor/eval/vibe/sample-run/d03.diff
```

Expected conflicts to resolve by hand: `init-game.sh` renamed `gamebient_game` → `coin_run` in the template files but the fixture overlay and the diffs still say `gamebient_game`/`Gamebient Game` (the diffs touch `src/game/mod.rs`, `scenario.rs`, `scoring.rs`, `things.rs`, `ui/hud.rs`, `AGENTS.md`). Simplest order: overlay and apply the diffs **before** `init-game.sh`, then run `init-game.sh` last. Do that instead:

```bash
git clone --branch editor <template-worktree> coin-run && cd coin-run && git remote remove origin
cp -R <template-worktree>/eval/vibe/fixture/. .
git apply --exclude='tests/fixtures/*' <template-worktree>/eval/vibe/sample-run/a01.diff
git apply --exclude='tests/fixtures/*' <template-worktree>/eval/vibe/sample-run/d03.diff
rm -rf eval
./init-game.sh "Coin Run" Bread-Heads-Studios/coin-run
```

The `fixture/src/game/mod.rs` was written against BASE's template `mod.rs`; the `editor` branch's `mod.rs` adds `TunablesPlugin`, `tuning`, `begin_run_tunables`, `apply_pending_tunes`. Re-apply those four edits to the overlaid `mod.rs` by hand (the fixture file replaces the template one).

- [ ] **Step 2: Build and test.** `cargo build && cargo test --all-targets --all-features`. Regenerate `tests/fixtures/selftest.gxr`; build the verifier and regenerate `selftest-wasm.gxr`. Expected: green (a01/d03 tests included, now reading the ammo through `GameData`).

- [ ] **Step 3: Commit** `git add -A && git commit -m "Coin Run: template editor branch + eval fixture + a01 coins + d03 ammo"`. Also commit `.claude/launch.json`? No, that is the workspace's. Record the template commit used in `docs/game-brief.md` ("Built from gamebient-bevy-template editor@<sha>").

---

### Task 11: Coin Run's tunables and checks

**Files:**
- Create: `src/game/tuning.rs` (replace the template's placeholder `RunTuning` with Coin Run's resources), `src/game/checks_list.rs`
- Modify: `src/game/things.rs`, `player.rs`, `physics.rs`, `shooting.rs`, `coins.rs`, `scoring.rs`, `scenario.rs`, `ui/hud.rs`, `src/game/mod.rs`

**Interfaces:**
- `tuning::PlayerTuning { speed: f32, jump_strength: f32 }` group `thing("player","Player")`; `PhysicsTuning { gravity: f32, max_fall: f32 }` group `rule("physics","Falling")`; `ThingTuning` per kind? One resource per kind is many files; instead `ThingColours { player, coin, enemy, bullet, goal, spike, platform: Srgba }` group `thing("colours","Colours")` and `ThingSpeeds { enemy: f32, bullet: f32 }` group `thing("speeds","Speeds")`; `LivesTuning { start: u32 }` group `counter("lives","Lives")` (next_run); `AmmoTuning { start: u32 (next_run), fire_button: Button }` group `rule("ammo","Ammo")`; `CoinTuning { pickup_sound: PickupSound }` group `rule("coins","Coins")` where `PickupSound` is a unit enum mirroring `SfxEvent::{Pickup, Hit, Jump, Shoot, Win, Lose}` with `fn sfx(self) -> SfxEvent`.
- `Scenario::tune(&mut self, path: &str, value: Value) -> &mut Self` (queues into `PendingTunes` before `start`; applied by the `Update` drain at `start`'s first update).
- `checks_list::CHECKS: &[Check]` with the six checks from the spec.

- [ ] **Step 1: Write the checks first** (`src/game/checks_list.rs`), as the failing tests — each check is a function `fn(&Tunes) -> Result<(), String>` using `Scenario`:

```rust
//! Coin Run's scenario checks: what a card can ask the editor to re-run.
//! Each builds its own stage, applies the tunable values it is handed, and
//! returns a plain-words message instead of panicking (checks run inside the
//! wasm game, where a panic is fatal). `cargo test` runs them all at the
//! defaults through `tunables::checks`.

use crate::ensure;
use crate::game::audio::SfxEvent;
use crate::game::scenario::{Buttons, FLOOR_TOP, START, Scenario};
use crate::game::tunables::checks::{Check, Tunes};

fn stage(tunes: &Tunes) -> Scenario {
    let mut s = Scenario::new();
    for (path, v) in tunes {
        s.tune(path, v.clone());
    }
    s
}

fn coin_is_collected(t: &Tunes) -> Result<(), String> {
    let mut s = stage(t);
    s.place("Coin", START.x + 40.0, FLOOR_TOP + 10.0).start();
    s.hold(Buttons::RIGHT, 30);
    ensure!(s.count("Coin") == 0, "the coin was not picked up");
    ensure!(s.coins() == 1, "the coin counter stayed at {}", s.coins());
    ensure!(s.score() == 0, "picking up a coin changed the score");
    Ok(())
}

fn coin_out_of_reach_stays(t: &Tunes) -> Result<(), String> {
    let mut s = stage(t);
    s.place("Coin", START.x + 300.0, FLOOR_TOP + 10.0).start();
    s.hold(Buttons::RIGHT, 30);
    ensure!(s.count("Coin") == 1, "a coin far away was picked up");
    Ok(())
}

fn fire_button(t: &Tunes) -> Buttons {
    match t.iter().find(|(p, _)| p == "ammo.fire_button") {
        Some((_, crate::game::tunables::Value::Choice(b))) if b == "A" => Buttons::A,
        _ => Buttons::B,
    }
}

fn b_fires_and_uses_ammo(t: &Tunes) -> Result<(), String> {
    let mut s = stage(t);
    s.start();
    let ammo = s.ammo();
    ensure!(ammo > 0, "{} never fires with 0 ammo", button_name(fire_button(t)));
    s.press(fire_button(t)).tick(1);
    ensure!(s.count("Bullet") == 1, "pressing {} fired no bullet", button_name(fire_button(t)));
    ensure!(s.ammo() == ammo - 1, "a shot did not use one ammo");
    ensure!(s.sounds().iter().any(|(_, e)| *e == SfxEvent::Shoot), "no shoot sound");
    Ok(())
}

fn no_ammo_no_shot(t: &Tunes) -> Result<(), String> {
    let mut s = stage(t);
    s.set_ammo(0).press(fire_button(t)).tick(1);
    ensure!(s.count("Bullet") == 0, "a bullet was fired with no ammo");
    Ok(())
}

fn ammo_runs_out(t: &Tunes) -> Result<(), String> {
    let mut s = stage(t);
    s.start();
    let ammo = s.ammo();
    for _ in 0..ammo + 3 {
        s.press(fire_button(t)).tick(1);
    }
    ensure!(s.ammo() == 0, "ammo did not run out");
    ensure!(s.count("Bullet") as u32 == ammo, "more bullets than ammo");
    Ok(())
}

fn bullets_leave_the_screen(t: &Tunes) -> Result<(), String> {
    let mut s = stage(t);
    s.start();
    ensure!(s.ammo() > 0, "no ammo, so no bullet to watch");
    s.press(fire_button(t)).tick(1);
    ensure!(s.count("Bullet") == 1, "no bullet was fired");
    let flew = s.hold_until(Buttons::NONE, 600, |s| s.count("Bullet") == 0);
    ensure!(flew, "a bullet never leaves the screen (is its speed too low?)");
    Ok(())
}

fn button_name(b: Buttons) -> &'static str {
    if b == Buttons::A { "A" } else { "B" }
}

pub const CHECKS: &[Check] = &[
    Check { name: "coin_is_collected", title: "A coin is collected", run: coin_is_collected },
    Check { name: "coin_out_of_reach_stays", title: "A coin out of reach stays", run: coin_out_of_reach_stays },
    Check { name: "b_fires_and_uses_ammo", title: "The fire button fires and uses ammo", run: b_fires_and_uses_ammo },
    Check { name: "no_ammo_no_shot", title: "No ammo, no shot", run: no_ammo_no_shot },
    Check { name: "ammo_runs_out", title: "Ammo runs out", run: ammo_runs_out },
    Check { name: "bullets_leave_the_screen", title: "Bullets leave the screen", run: bullets_leave_the_screen },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::tunables::Value;

    #[test]
    fn checks_warn_in_plain_words_when_ammo_is_zero() {
        let tunes = vec![("ammo.start".to_string(), Value::Int(0))];
        let r = (CHECKS[2].run)(&tunes);
        assert_eq!(r, Err("B never fires with 0 ammo".into()));
    }

    #[test]
    fn checks_follow_the_fire_button() {
        let tunes = vec![("ammo.fire_button".to_string(), Value::Choice("A".into()))];
        assert_eq!((CHECKS[2].run)(&tunes), Ok(()));
    }
}
```

- [ ] **Step 2: Run** → compile errors (`tune`, `ensure` path, tuning resources).

- [ ] **Step 3: `tuning.rs`** with the resources listed in Interfaces (attributes per the spec's table: player.speed 60–600 px/s step 10, default 240; player.jump_strength 200–1200, 600; physics.gravity 300–3000 px/s², 1500; physics.max_fall 200–2000, 900; speeds.enemy 0–600, 120; speeds.bullet 100–1200 px/s step 10, 600; lives.start 0–9 next_run, 3; ammo.start 0–50 next_run, 5; ammo.fire_button Button::B; coins.pickup_sound PickupSound::Pickup; colours.* as in `things::props`). Register in `src/game/mod.rs` in this order: `PlayerTuning, PhysicsTuning, ThingSpeeds, ThingColours, LivesTuning, AmmoTuning, CoinTuning`. Fill `EditorInfo { checks: CHECKS names, sounds: ["pickup","hit","jump","shoot","win","lose"] }` and `CheckList(CHECKS)` under `#[cfg(feature = "editor")]` (the `CheckList` resource is always inserted so the `every_registered_check_passes_at_defaults` test runs natively).

- [ ] **Step 4: Readers.** `player::control_player` takes `Res<PlayerTuning>` (`speed`); `physics::step_bodies` takes `Res<PhysicsTuning>`; `shooting::shoot` takes `Res<AmmoTuning>` (`fire_button.just_pressed(&input.input)`, note `TickInput.input` is the `GameInput`) and `Res<ThingSpeeds>` (`bullet`); `coins::collect_coins` takes `Res<CoinTuning>` → `sfx.write(tuning.pickup_sound.sfx())`; `reset_game_data` (in `mod.rs`) takes `Res<LivesTuning>` and `Res<AmmoTuning>` to set `lives`/`ammo` at run start (this is the `next_run` semantics; `GameData::default()` keeps 3/5 for tests that never enter `Playing`). `things::props` keeps sizes; colours come from `ThingColours` in `setup_thing_art` (an `Update`-side read of a tunable is fine: colours are presentation). Delete the constants `PLAYER_SPEED`, `ENEMY_SPEED`, `BULLET_SPEED`, `JUMP_STRENGTH`, `AIR_JUMPS`, `GRAVITY`, `MAX_FALL`, `STARTING_AMMO`; `hud.rs` reads `data.ammo` only. **Presentation must not read a tunable the sim writes mid-run and then feed it back** — colours are read-only presentation, fine.

- [ ] **Step 5: `Scenario::tune`**

```rust
    /// Sets a tunable before the run starts (applied at `start`).
    pub fn tune(&mut self, path: &str, value: Value) -> &mut Self {
        let id = self.app.world().resource::<TunableIndex>().by_path(path).map(|e| e.id);
        if let Some(id) = id {
            self.app.world_mut().resource_mut::<PendingTunes>().0.push((id, value));
        }
        self
    }
```

`Scenario::start` already calls `app.update()` once before `Playing`, which drains the queue.

- [ ] **Step 6: Run** `cargo test --all-targets --all-features` → the a01/d03 unit tests and the new checks pass; regenerate `selftest.gxr` and `selftest-wasm.gxr` (the sim reads resources now; values equal the old constants so the fixture *should* be byte-identical except for the content hash — it will differ because the checksum folds content). Gates. Commit "Coin Run: tunables for player, physics, things, lives, ammo and coins; checks for a01 and d03. Regenerates both fixtures."

---

### Task 12: Editor wasm build and the conformance probe

**Files:**
- Create (Coin Run): `tools/editor_probe.mjs`, `tools/editor_probe.html`

- [ ] **Step 1: Build** `tools/build_editor.sh` in `games/coin-run` → `dist-editor/`.

- [ ] **Step 2: Playwright package.** Chromium revisions cached: 1140, 1217, 1234, 1243. Find the `playwright-core` version that uses one of them without downloading browsers:

```bash
cd games/coin-run && mkdir -p tools/probe && cd tools/probe
for v in $(npm view playwright-core versions --json | node -e 'const a=JSON.parse(require("fs").readFileSync(0));console.log(a.filter(v=>/^1\.(5[5-9]|6\d)\.\d+$/.test(v)).reverse().join(" "))'); do
  npm pack playwright-core@$v --silent >/dev/null 2>&1 && tar -xzf playwright-core-$v.tgz package/browsers.json && rev=$(node -e 'const b=require("./package/browsers.json").browsers.find(b=>b.name==="chromium");console.log(b.revision)') && echo "$v chromium-$rev" && rm -rf package playwright-core-$v.tgz
done
```

Pick the newest version whose chromium revision is cached; `PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1 npm init -y && npm i -D playwright-core@<v>` in `tools/probe/` (gitignore `node_modules`; commit `package.json`). If npm cannot reach the registry, stop and report the host (`registry.npmjs.org`).

- [ ] **Step 3: Probe page** `tools/editor_probe.html`: an iframe to `/dist-editor/index.html`, a script that answers `gx:hello` with `{type:"gx:hello",v:1,hostHasControls:true}`, then on the game's hello sends `gx:input` with `buttons: 64` (Start) to clear the click-to-start gate, waits for `ready`, sends `gx:custom tunables.list`, logs every `gx:event` as `console.log("GX " + JSON.stringify(e.data))`.

- [ ] **Step 4: Probe script** `tools/editor_probe.mjs`: starts a Node `http` static server for the game dir on `127.0.0.1:0`, launches `chromium` from `playwright-core` with `executablePath` under `~/Library/Caches/ms-playwright/chromium-<rev>/chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing` (or `--headless=new` flags; use `args: ['--use-gl=angle','--use-angle=swiftshader','--enable-unsafe-swiftshader']` so WebGL2 works headless), opens the probe page, collects `GX` console lines, and asserts in order:
  1. a `tunables` event whose `tunables` has `ammo.start` with value 5 and `speeds.bullet` with value 600;
  2. after posting `gx:custom tunables.set {"speeds.bullet": 900}`, a `tunable` event `{path:"speeds.bullet", value:900}`;
  3. after `gx:custom checks.run {names:["b_fires_and_uses_ammo","bullets_leave_the_screen"]}`, two `check` events with `ok: true`;
  4. after `tunables.set {"ammo.start": 0}` and `checks.run {names:["b_fires_and_uses_ammo"]}`, a `check` event with `ok: false` and message `"B never fires with 0 ammo"`;
  5. after `run.restart`, a `state` event `Playing` then a `tunables` event.
  Prints each step as `ok <step>` and exits 1 on the first miss, with the last 20 console lines. Run: `node tools/editor_probe.mjs`.

- [ ] **Step 5: Commit** the probe and its page; commit `tools/probe/package.json`. Record the Playwright version and revision in `docs/game-brief.md`? No — in the template's `docs/superpowers/specs/2026-11-06-editor-design.md` "Frontend stack" paragraph (one line: "`playwright-core@<v>` uses cached chromium-<rev>").

- [ ] **Step 6: Show it working.** Also open `dist-editor` through the desktop app's browser pane with the probe page (workspace `.claude/launch.json` config `coin-run-editor`: `python3 -m http.server 8766 --directory games/coin-run`) and screenshot the game running with the console log of the `tunables` event visible in `read_console_messages`.

---

## Self-review

- **Spec coverage:** hosting/gx:custom (Task 1), tunables rules 1–6 (Tasks 2–4, 8), GXR2 + verifier + content (Task 5), determinism test (Task 6), host layer names (Task 7), checks (Task 8, 11), build_editor + docs (Task 9), Coin Run with ammo and coins (Tasks 10–11), conformance log (Task 12). Leaderboard policy is documented only (spec says website-side, not this session).
- **Type consistency:** `Value` (not `TunableValue`) everywhere; `Kind` names match `value.rs` and `protocol.rs` `type` strings via `Kind::name()`; `TuneLog.starts: Vec<(u16, Value)>` while `Replay.starts: Vec<(u16, Vec<u8>)>` (bytes) — conversion in the recorder and in `build_verify_app`. `checks::Tunes` is path-keyed (the host layer maps ids → paths with `snapshot` + index).
- **Spec amendment:** `serde_json` is used in the game (it is already in the lockfile); the spec's "no serde_json" line is replaced by "no new crates outside the lockfile".
