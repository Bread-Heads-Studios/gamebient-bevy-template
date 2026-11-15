# Look Packs: fetch large built-in packs on demand (web) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Large built-in packs no longer inflate every web download. On wasm, `build.rs` embeds only small packs. Any larger built-in pack is fetched from the game's own site when `?look=<id>` selects it, checked against a build-time content hash, and applied through the same path as native hot reload. Native builds keep embedding everything.

**Why (owner decision, 2026-11-15):** Voidrunner's Fae Wilds pack (9 OGGs and a title PNG, about 1.65 MB) grew the brotli wasm from about 4.7 MB to 6.3 MB for every player. F2 ("no degradation") rules that out.

**Architecture:**
- **`build.rs` (wasm target only):** `CARGO_CFG_TARGET_ARCH == "wasm32"` splits built-in packs. Packs with total size ≤ `WEB_EMBED_MAX_BYTES` (64 KiB) stay embedded. Larger packs go into a generated `FETCHABLE` table holding the id, file names and build-time `content_hash`.
- **Shipping:** `build_web.sh` already copies `assets/` (including `assets/looks/<id>/`) into `dist/` and `dist/full/`, so the files are served same-origin.
- **Selection:** `select::from_query` returns `Selection::Fetch(id)` for a fetchable id on wasm.
- **`LookPlugin`:** it starts with the default look and kicks off a fetch of exactly the listed files from `assets/looks/<id>/<file>` (relative URL). On completion it builds `Files`, requires `validate::content_hash(&files)` to equal the table's hash and the validator to pass, then swaps `ActiveLook`, rebuilds `LookAssets` and sends `LookChanged`. That is the same apply step native hot reload uses, factored out of `reload.rs` into a shared function. Any failure (network, hash or validation) logs once and keeps the default look.

**Tech Stack:** Bevy 0.18, `web-sys` (already a wasm dependency; add features such as `Request`, `Response`, `RequestInit`, `Headers`), and `wasm-bindgen-futures` and `js-sys`. Both are already in `Cargo.lock` via Bevy, so adding them as direct wasm-only dependencies pins existing versions and adds no new crate to the build. Confirm this in `Cargo.lock` before adding. If either is NOT already in the lockfile, stop and report.

## Global Constraints

- **Native is unchanged:** all built-in packs stay embedded. `cargo test` runs natively, and every existing test passes unchanged.
- **Web, default:** with no `?look=` (or a small embedded pack), behaviour and download are as before minus the large packs' bytes. Report the `dist/` brotli wasm size before and after for Voidrunner-sized evidence, using the template's `sample-*` packs plus a test-only large fixture if needed.
- **Security:**
  - Fetch only ids in the build-time `FETCHABLE` table, only their listed file names, and only same-origin relative URLs under `assets/looks/<id>/`.
  - The fetched files' `content_hash` must equal the build-time hash. Never apply a pack that fails the hash or the validator.
  - Arbitrary `?look=<url>` stays unsupported. That's step 5.
- **D1 still holds:** the fetch and apply happen in presentation systems only. The sim never sees it.
- **No blocking:** the fetch is async. The game boots on the default look and switches when the pack arrives, through the existing `LookChanged` path that every applier already handles.
- `src/look/` stays generic: no game names.
- **CI:**
  - `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings` and `cargo test`;
  - `cargo clippy --target wasm32-unknown-unknown -- -D warnings` (or `cargo check --target wasm32-unknown-unknown`) must pass;
  - `./build.sh web` must pass;
  - `bash tools/test_rollout_replay.sh --no-game` if module lines change.
- Commit trailer: `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

---

### Task 1: Build-time split, the `FETCHABLE` table and a shared apply step

**Files:** `build.rs`, `src/look/pack.rs` (the embedded/fetchable API), `src/look/select.rs` (`Selection::Fetch`), `src/look/reload.rs` and a new shared module, e.g. `src/look/swap.rs` (`fn swap_look(world/commands, contract, files, expected_id, expected_hash) -> Result<(), Report>`, used by native hot reload and the web fetch), and tests.

**Steps:**
- [ ] `build.rs`:
  - When building for wasm32, compute each built-in pack's total size and `content_hash`. The hash must match `validate::content_hash`, the same domain and format. Duplicate the tiny hashing in `build.rs`, or include the hashing source via `#[path]` so there's a single source of truth; prefer the single source.
  - Emit `EMBEDDED` (small packs) and `FETCHABLE: &[(&str id, &[&str] files, &str content_hash)]` (large packs).
  - On native, all packs stay embedded and `FETCHABLE` is empty.
  - Make the split policy a pure function, `fn web_embeds(total_bytes) -> bool` with a const threshold, that can be unit-tested. A `build.rs` can't be unit-tested directly, so mirror the policy in `pack.rs` with a test, or include it via `#[path]`.
- [ ] `select.rs`: `from_query` maps an id found in `pack::fetchable(id)` to `Selection::Fetch(id)`, and an embedded id to `Embedded` as today. The mapping is testable natively by passing in the tables.
- [ ] A shared swap step: refactor native `reload.rs` so its "files changed → validate → replace ActiveLook → rebuild LookAssets → send LookChanged" logic is one function both paths call, plus an optional expected-hash check.
- [ ] Tests (native):
  - the split policy at the threshold boundaries;
  - `from_query` with an embedded, a fetchable and an unknown id;
  - the swap step applies a valid Files set and sends `LookChanged`;
  - it refuses on hash mismatch, keeping the default and sending no `LookChanged`;
  - it refuses on validation failure;
  - native hot reload still passes its existing tests.
- [ ] Commit: `feat(look): split large built-in packs out of the web build`

### Task 2: The web fetch

**Files:** `src/look/fetch.rs` (cfg wasm32), `src/look/mod.rs` (`LookPlugin` starts the fetch for `Selection::Fetch`), `Cargo.toml` (wasm-only deps and features).

**Steps:**
- [ ] Use `web_sys::window().fetch_with_str` (or a `Request`), `wasm_bindgen_futures::spawn_local` and `JsFuture` to fetch each listed file as bytes (`array_buffer` → `Uint8Array` → `Vec<u8>`). Resolve the URL relative to the page (`assets/looks/<id>/<file>`), matching how Bevy resolves `assets/` on web.
- [ ] Hand the completed `Files` to the ECS through a channel (e.g. `crossbeam-channel` if already a dependency, or `std::sync::mpsc` / `Arc<Mutex<Option<..>>>` wrapped in a Resource). A presentation `Update` system polls it once and calls the shared swap step with the expected hash.
- [ ] Failure handling: any fetch error, non-200 response or hash or validation failure logs one `warn!` with the reason and keeps the default look.
- [ ] Natively testable seam: the polling system and the swap step are covered by Task 1's tests with injected Files. Here, add a test, native-compiled with a fake source, that the polling system applies once and then stops.
- [ ] Verify: `cargo check --target wasm32-unknown-unknown`, `./build.sh web`, then serve `dist/` and open `?mute=1&look=<a fetchable test pack>`.
  - The template has no large pack, so add a test-only route: temporarily set the threshold low in a local build, or add an env override `GX_WEB_EMBED_MAX` read by `build.rs` for testing only. Document it.
  - The look must switch after the fetch, and the network panel must show the pack files fetched.
- [ ] Commit: `feat(look): fetch large built-in packs on demand on the web`

### Task 3: Docs

- [ ] `docs/look-packs.md`: the web embedding rule (threshold, the fetch path, the hash check, the failure fallback, boot on default and switch on arrival, the test override) and `AGENTS.md`. Verify the claims against the code.
- [ ] Commit: `docs(look): built-in packs on the web`
