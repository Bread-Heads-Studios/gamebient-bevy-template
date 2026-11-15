# Look Packs v2: Shapes as Data Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Look Pack schema v2 lets a pack describe meshes as lists of primitives (`"shape": [...]`), validated against per-slot budgets and applied render-side by swapping the mesh behind a game's existing handles. Schema versions are gated so v1 packs keep working and newer packs fail cleanly.

**Architecture:** All generic code lives in `src/look/` (copied verbatim into games). `manifest.rs` gains `Slot::Shape` and primitive types. `contract.rs` gains `ShapeSpec` per slot. A new `shape.rs` builds one merged `Mesh` and counts its vertices with no ECS. `validate.rs` enforces the version gate and the shape rules. A new `ShapeTargets` resource plus an `apply_shapes` system (registered by `LookPlugin`) rewrites `Assets<Mesh>` behind registered handles from a captured baseline. The template's own `player` slot accepts a shape, and a `sample-shapes` built-in pack demonstrates it.

**Tech Stack:** Rust 2024, Bevy 0.18 (`bevy_mesh` primitives: `Sphere`, `Cuboid`, `Cylinder`, `Cone`, `Torus`, `Capsule3d`, `Plane3d`; `Mesh::transformed_by`, `Mesh::merge`). No new crates.

**Spec:** `/Users/kelliott/Gamebient/colecovisiongx/docs/plans/2026-11-15-fae-wilds-design.md` §1, §2. Read it.

## Global Constraints

- **The default look is bit-identical.** With no pack, or a pack that doesn't set a shape slot, every mesh asset is unchanged: same attributes, same indices.
- **Values only.** `apply_shapes` writes `Assets<Mesh>` contents behind existing handles. It never adds or removes entities, components or children, and never reads or writes sim state. `LookPlugin` stays out of `GamePlugin`.
- **Versioning (F5):**
  - `SCHEMA_VERSION = 2`.
  - `v` must be in `1..=SCHEMA_VERSION`. Otherwise it's an error whose message contains exactly `made for a newer version of this game (v{N})` for `N > SCHEMA_VERSION`, or `unknown schema version` for 0.
  - A `shape` slot in a `v: 1` pack is an error: `shape slots need "v": 2`.
  - An invalid pack resolves to the default look, as today.
  - `deny_unknown_fields` stays on every manifest struct.
- **Primitives and `size` arity:**
  - `sphere [radius]`
  - `cuboid [x,y,z]`
  - `cylinder [radius,height]`
  - `cone [radius,height]`
  - `torus [ring_radius,tube_radius]`
  - `capsule [radius,length]`
  - `quad [x,z]`: two-sided, an XZ plane with both faces generated and normals ±Y.
- **Optional primitive fields:**
  - `at [x,y,z]`, default 0.
  - `rot [x,y,z]` in degrees, applied in XYZ Euler order.
  - `scale [x,y,z]`, default 1.
  - `detail`, an integer: default 12, allowed `3..=32`. It is the segment count around the primitive's axis. For spheres it also sets rings as `detail/2` (min 2). Torus uses `detail` for the ring and `detail/2` for the tube. Cuboid and quad ignore `detail`.
- **Shape validation:**
  - Every number is finite.
  - Each size component is in `[0.001, extent]`.
  - Each `at` component is in `[-extent, extent]`.
  - Each scale component is in `[0.01, 4.0]`.
  - The primitive count is `<= max_prims`.
  - `vertex_count(shape) <= max_vertices`.
  - Messages name the slot and the primitive index, e.g. `slots.player.shape[2]`.
- **The vertex count matches what gets built:** `shape::vertex_count` must equal the built mesh's vertex count for every primitive at every allowed `detail`. A test asserts this.
- **`"shape": []`** builds a valid empty mesh: a TriangleList with zero vertices and zero indices, with position, normal and UV attributes present and empty.
- **Mesh layout:** `build_mesh` output is a TriangleList with `ATTRIBUTE_POSITION`, `ATTRIBUTE_NORMAL`, `ATTRIBUTE_UV_0` and u32 indices, with `RenderAssetUsages::default()`. Normals are transformed by the inverse-transpose of each primitive's transform and renormalised.
- **CI:** `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test`. Also `bash tools/test_rollout_replay.sh --no-game` if `src/main.rs` or `src/lib.rs` module lines change, and shellcheck if you touch shell. `./build.sh web` must still pass.
- `GX_MUTE=1` for native runs.
- Commit trailer: `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

---

### Task 1: Schema v2 types, contract `ShapeSpec`, version gate

**Files:**
- Modify: `src/look/manifest.rs`: `SCHEMA_VERSION = 2`, `Slot::Shape(ShapeSlot)`, `ShapeSlot { shape: Vec<Primitive> }`, `Primitive { prim: Prim, size: Vec<f32>, at: Option<[f32;3]>, rot: Option<[f32;3]>, scale: Option<[f32;3]>, detail: Option<u32> }`, and `Prim` (lowercase serde: sphere, cuboid, cylinder, cone, torus, capsule, quad). `deny_unknown_fields` on the new structs. Check that the untagged `Slot` still resolves `image`, `recipe` and `shape` unambiguously; add a test.
- Modify: `src/look/contract.rs`: `SlotSpec` gains `pub shape: Option<ShapeSpec>`, with `ShapeSpec { max_prims: usize, max_vertices: usize, extent: f32 }`. Update every `SlotSpec` literal in the repo with `shape: None`, except the template's `player` slot in `src/ui/theme.rs`, which gets `Some(ShapeSpec { max_prims: 12, max_vertices: 3000, extent: 1.5 })`.
- Modify: `src/look/validate.rs`: the version gate (messages exactly as in Global Constraints) and the "shape slots need v 2" rule. A shape in a slot whose spec has `shape: None` is an error: `this slot does not take a shape`. Shape contents are only parse-checked here; Task 3 adds the numeric rules.
- Tests (in-module): a v1 pack still validates; `v: 3` is refused with the exact message; `v: 0` is refused; a shape in v1 is an error; a shape in a non-shape slot is an error; a v2 pack with a shape parses; unknown fields in a primitive are refused.
- [ ] TDD: write the tests, watch them fail, implement, watch them pass. Also run the full `cargo test`, `tests/looks.rs` included.
- [ ] Commit: `feat(look): schema v2 shape slots and the version gate`

### Task 2: `shape.rs`: mesh builder and vertex count

**Files:** Create `src/look/shape.rs` and add `pub mod shape;` with re-exports in `src/look/mod.rs`.

**Interfaces:**
- `pub const DEFAULT_DETAIL: u32 = 12; pub const DETAIL_RANGE: RangeInclusive<u32> = 3..=32;`
- `pub fn vertex_count(shape: &[Primitive]) -> usize`
- `pub fn build_mesh(shape: &[Primitive]) -> Mesh`
- `pub fn primitive_transform(p: &Primitive) -> Transform`

Each primitive is built with Bevy's primitive meshers at the clamped detail, transformed (`scale`, then `rot`, then `at`), and merged into one mesh. A `quad` is a `Plane3d` mesh plus a copy with reversed winding and negated normals.

`vertex_count` must be computed from the same parameters, without building, so the validator stays cheap. If a Bevy mesher's vertex count can't be derived simply for a primitive, `vertex_count` may build that primitive's mesh and count it. Document which ones do and keep it correct: correctness over cleverness.

**Tests:**
- For every `Prim` at details 3, 12 and 32, `vertex_count(&[p]) == build_mesh(&[p]).count_vertices()`.
- A two-primitive shape counts as the sum.
- `build_mesh(&[])` is an empty TriangleList with the three attributes present.
- `at` moves the AABB centre.
- `rot` of 90° about Z maps a +X extent to +Y.
- Normals are unit length after a non-uniform scale.
- A quad has both up and down normals.
- An out-of-range `detail` is clamped, and the count reflects the clamp.

- [ ] TDD, then commit: `feat(look): build a mesh from a primitive list`

### Task 3: Validator shape rules and `look-check` vertex counts

**Files:** `src/look/validate.rs` and `src/bin/look_check.rs`.

**Rules:**
- The numeric rules from Global Constraints.
- `size` arity per `Prim` is checked: a wrong length is an error naming the expected arity.
- The budget uses `shape::vertex_count`.
- `Report` gains a way to carry per-slot vertex counts for valid shape slots, e.g. `pub shapes: Vec<(String, usize)>`.
- `look-check` prints `shape <slot>: <n> vertices (budget <max>)` for each.

**Tests:**
- One test per rule: arity, non-finite (JSON can't encode NaN, so check out-of-range values in place of NaN), size over the extent, `at` outside the extent, scale bounds, too many primitives, vertex budget exceeded (a fine sphere repeated), `detail` out of range (an error, not a silent clamp, in validation).
- The message includes the `slots.<name>.shape[i]` path.
- A valid shape reports its vertex count.

- [ ] TDD, then commit: `feat(look): validate shapes against the slot budget`

### Task 4: `ShapeTargets` + `apply_shapes`, template player, `sample-shapes` pack

**Files:**
- Create `src/look/shapes_apply.rs`, or add to `apply.rs` if it stays small; prefer a new file.
- Modify `src/look/mod.rs` so `LookPlugin` inits `ShapeTargets` and adds `apply_shapes` in `LookSet`.
- Modify `src/game/player.rs`: `dress_player` uses the pack's shape for `player` if present, else the cuboid recipe as today.
- Add `assets/looks/sample-shapes/look.json`.
- Tests in `tests/looks.rs`.

**Interfaces:**
- `#[derive(Resource, Default)] pub struct ShapeTargets { .. }` with `pub fn register(&mut self, slot: &'static str, mesh: Handle<Mesh>)`. Registering the same handle twice is a no-op.
- `apply_shapes` (PostUpdate, `LookSet`):
  - For each registered target not yet baselined, it clones the current mesh asset as the baseline, then applies.
  - On `LookChanged` it re-applies all targets.
  - "Apply" means: if the active look's slot is a `Slot::Shape`, write `build_mesh(shape)` into `Assets<Mesh>` behind the handle; otherwise write the baseline back if the asset differs.
  - It never touches entities.
  - Baselines are keyed by `AssetId`; drop entries whose asset no longer exists.

**Player:**
- The template player isn't handle-based: `dress_player` adds `meshes.add(..)` per spawn.
- Make it use `build_mesh(shape)` when the active look's `player` slot is a shape, otherwise the existing cuboid path.
- This keeps the template's working example simple. `ShapeTargets` is exercised by its own tests.

**The `sample-shapes` pack:** `v: 2`, `kind: "skin"`, a player shape of about 4–6 primitives (a small rocket: capsule body, cone nose, two fin cuboids), and a palette override for `player`. It must validate clean.

**Tests:**
- A registered handle under the default look is bit-identical after a frame: compare positions, normals, UVs and indices.
- A pack with a shape for that slot replaces the asset with `build_mesh(shape)`, and an unregistered handle is untouched.
- Switching back to the default restores the baseline exactly.
- A hot-reload `LookChanged` to a different shape applies the new one.
- `sample-shapes` dresses the player with the shape's vertex count.
- The existing `every_built_in_pack_validates_against_the_contract` and `no_built_in_pack_changes_a_replay` pass with the new pack included.
- `the_player_is_dressed_from_the_pack_on_the_frame_it_spawns` still passes.

- [ ] TDD, then commit: `feat(look): apply pack shapes behind registered mesh handles`

### Task 5: Docs

**Files:** `docs/look-packs.md`, `assets/looks/README.md` and `AGENTS.md`.

**Content:**
- The v2 format: the primitive table and arities, optional fields, `"shape": []`.
- The version rules: v1 still loads, a newer `v` is refused, and fields only arrive with a version bump.
- Budgets: what `max_prims`, `max_vertices` and `extent` mean, and how to read the `look-check` vertex output.
- How a game adopts it: `ShapeTargets::register` for handle-based art (with the Voidrunner pattern as the motivating example), or `build_mesh` at spawn.
- The values-only rule. `sample-shapes` as the worked example.

Every claim must be checked against the code, and the `look-check` command run on each built-in pack.

- [ ] Commit: `docs(look): shapes as data (schema v2)`

## Verification (controller)

- `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
- `./build.sh web`; `bash tools/test_rollout_replay.sh --no-game`
- Browser: `?mute=1&look=sample-shapes` shows the rocket player.
