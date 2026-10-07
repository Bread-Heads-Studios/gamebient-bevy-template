# Demo Build and Gated Full Bundle Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** One source tree builds two wasm bundles: a public demo at `dist/` that ends at a per-game content cap with a `DEMO OVER` card and a `demo_end` host event, and the full game at `dist/full/`, which Vercel middleware serves only to a request carrying a valid website-signed entitlement token.

**Architecture:** A `demo` cargo feature compiles in one FixedUpdate system (`game::demo::end_demo`) that latches `RunOver` and moves the game to a new `GameState::DemoEnd` when `demo_reached` turns true; the sim, seed and replay layers are untouched. `build_web.sh` runs its bundle steps twice. A root `middleware.ts` (Vercel Routing Middleware) verifies Ed25519 tokens with WebCrypto, sets a `gx_full` cookie, and 403s everything else under `/full/`. Spec: `../../../../docs/plans/2026-10-07-demo-full-split-design.md` (workspace root). The website side (token signing, `/play`, `/cabinet`) is the website repo's plan `docs/superpowers/plans/2026-10-07-full-game-entitlement.md`.

**Tech Stack:** Rust 2024, Bevy 0.18, `gamebient-input` v0.4.0 (`HostEvent::Custom` already exists), wasm-bindgen 0.2.108, Vercel Routing Middleware (TypeScript, Web APIs only, no npm deps), Node 22 `node --test`.

## Global Constraints

- Feature name is exactly `demo`; off by default; never enabled for native, Pi, `verify`, `autopilot` or `record` builds. `cargo run --features demo` plays the demo natively.
- `GameState::DemoEnd` is a new variant after `GameOver`. Every `match` on `GameState` must handle it (treat it like `GameOver` unless stated).
- Host protocol unchanged. The cut emits `HostEvent::Custom { name: "demo_end", data: "{}" }`; `StateEvents` posts `State("DemoEnd")` by itself. No `gameover` event on a demo cut.
- No changes to `src/game/sim.rs`, `src/game/replay/`, `sim::begin_run`, `SimSet` membership or the recorder. `end_demo` runs in `FixedUpdate` **after** `SimSet`.
- On-screen copy ASCII only. Demo card: `DEMO OVER`, `OWN <TITLE> TO KEEP PLAYING`, `PRESS ENTER TO RESTART`. Title card tag: `DEMO`.
- Token: `base64url(payload) "." base64url(sig)`; signed bytes are the UTF-8 of the first part; payload `{h, w, g, exp}`; host compared case-insensitively; `exp` unix seconds. Cookie `gx_full`, `Path=/full`, `Secure`, `HttpOnly`, `SameSite=None`, `Max-Age=43200`. Env `GX_ENTITLEMENT_PUBKEY` = base64 of the raw 32-byte key. 403 responses carry `Cache-Control: no-store`.
- `dist/full/assets/` is a copy of `dist/assets/`; the full bundle references everything relatively.
- `verify.zip` is built once, from the full sim, at `dist/verify.zip` only.
- `cargo test --all-targets --all-features && cargo clippy --all-targets --all-features -- -D warnings && cargo fmt --all -- --check` must pass; `shellcheck tools/*.sh build_web.sh install.sh` must pass; `node --test tools/test_entitlement.mjs` must pass.
- Commit messages end with `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.

---

## File Structure

| File | Responsibility |
|---|---|
| `Cargo.toml` | `demo = []` feature. |
| `src/game/states.rs` | `GameState::DemoEnd`. |
| `src/game/demo.rs` | `DEMO`, `DEMO_CAP`, `demo_reached`, `end_demo` system, tests. The one file a port edits for its cut. |
| `src/game/mod.rs` | `pub mod demo;` and registering `end_demo` under the feature. |
| `src/ui/demo_end.rs` | `spawn_demo_end` card, `demo_tag` chip helper, copy constants. |
| `src/ui/mod.rs`, `src/ui/menu.rs` | Wire `DemoEnd`; `DEMO` tag on the title; `menu_input` routes `DemoEnd -> Menu`. |
| `build_web.sh` | `bundle()` function run for `dist` (demo) and `dist/full` (full). |
| `vercel.json` | No-cache headers for `/full/` and `/full/index.html`. |
| `tools/entitlement.mjs` | Pure token verification + cookie helpers (WebCrypto), shared by middleware and tests. |
| `tools/test_entitlement.mjs` | `node --test` suite. |
| `middleware.ts` | Vercel Routing Middleware for `/full/:path*`. |
| `.github/workflows/ci.yml` | Runs the Node test. |
| `assets/info.json` | `game_url` → `/full/`. |
| `AGENTS.md` | Demo feature, cut recipe, middleware env, production checks. |

---

### Task 1: Feature flag, `DemoEnd` state, `demo.rs` with a tested cut and system

**Files:**
- Modify: `Cargo.toml:22-38` (features)
- Modify: `src/game/states.rs:5-14`
- Create: `src/game/demo.rs`
- Modify: `src/game/mod.rs:3-14` (module list) and `:92-112` (FixedUpdate systems)

**Interfaces:**
- Produces: `game::demo::DEMO: bool`, `game::demo::DEMO_CAP: u32`, `game::demo::demo_reached(&GameData) -> bool`, `game::demo::end_demo` (system), `GameState::DemoEnd`.

- [ ] **Step 1: Add the feature and the state**

`Cargo.toml`, after the `verify` feature (line 38):
```toml
# Demo bundle: the run ends at `game::demo::demo_reached` with a DEMO OVER
# card and a `demo_end` host event. build_web.sh builds it into dist/ and the
# full game into dist/full/. Never for native, Pi, verify, autopilot or record.
demo = []
```

`src/game/states.rs`:
```rust
/// Top-level game flow.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GameState {
    /// "BREAD HEADS STUDIOS PRESENTS" boot screen.
    #[default]
    StudioLogo,
    Menu,
    /// Once-per-session instruction screen (see src/ui/how_to_play.rs).
    HowToPlay,
    Playing,
    GameOver,
    /// The demo bundle's content cap was reached (src/game/demo.rs). Only
    /// entered when the `demo` feature is on; the card is src/ui/demo_end.rs.
    DemoEnd,
}
```

- [ ] **Step 2: Write the failing tests**

Create `src/game/demo.rs` with only the tests first:

```rust
//! The demo bundle's content cap. TEMPLATE NOTE: a port changes `DEMO_CAP`
//! and `demo_reached` (and nothing else here) to the game's own progression:
//! "after level 2", "after hole 3", "after the first song". The cut fires the
//! moment the capped unit is complete; `end_demo` then ends the run exactly
//! the way `sim::end_run` does, through the fade into `GameState::DemoEnd`.
//!
//! Compiled in only with `--features demo`. The sim, seed and replay code
//! never see the flag: a demo run and a full run are identical up to the cut.

#[cfg(test)]
mod tests {
    use bevy::input::InputPlugin;
    use bevy::state::app::StatesPlugin;
    use bevy::time::TimeUpdateStrategy;
    use gamebient_input::HostEvent;

    use super::*;
    use crate::game::scoring::GameData;
    use crate::game::sim;
    use crate::game::states::GameState;
    use crate::game::GamePlugin;

    #[test]
    fn cut_is_reached_at_the_cap_and_not_before() {
        let mut data = GameData::default();
        data.score = DEMO_CAP - 1;
        assert!(!demo_reached(&data));
        data.score = DEMO_CAP;
        assert!(demo_reached(&data));
    }

    #[cfg(feature = "demo")]
    #[test]
    fn reaching_the_cap_ends_the_run_into_demo_end_and_tells_the_host() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(StatesPlugin)
            .add_plugins(InputPlugin)
            .insert_resource(TimeUpdateStrategy::ManualDuration(sim::tick_duration()))
            .add_plugins(GamePlugin { headless: true });
        app.update();
        app.world_mut().resource_mut::<NextState<GameState>>().set(GameState::Playing);
        app.update();
        app.update(); // one fixed tick under the manual clock
        assert!(!app.world().resource::<sim::RunOver>().0);

        app.world_mut().resource_mut::<GameData>().score = DEMO_CAP;
        app.update(); // end_demo runs after SimSet, latches RunOver, requests DemoEnd
        assert!(app.world().resource::<sim::RunOver>().0);
        app.update(); // the transition applies
        assert_eq!(*app.world().resource::<State<GameState>>().get(), GameState::DemoEnd);

        let events = app.world().resource::<Messages<HostEvent>>();
        let mut cursor = events.get_cursor();
        let custom: Vec<_> = cursor
            .read(events)
            .filter(|e| matches!(e, HostEvent::Custom { name, .. } if name == "demo_end"))
            .cloned()
            .collect();
        assert_eq!(custom.len(), 1, "exactly one demo_end event");
        assert!(matches!(&custom[0], HostEvent::Custom { data, .. } if data == "{}"));
    }

    #[cfg(feature = "demo")]
    #[test]
    fn the_cut_fires_once_even_if_the_score_stays_above_the_cap() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(StatesPlugin)
            .add_plugins(InputPlugin)
            .insert_resource(TimeUpdateStrategy::ManualDuration(sim::tick_duration()))
            .add_plugins(GamePlugin { headless: true });
        app.update();
        app.world_mut().resource_mut::<NextState<GameState>>().set(GameState::Playing);
        app.update();
        app.world_mut().resource_mut::<GameData>().score = DEMO_CAP + 50;
        for _ in 0..5 {
            app.update();
        }
        let events = app.world().resource::<Messages<HostEvent>>();
        let mut cursor = events.get_cursor();
        let n = cursor
            .read(events)
            .filter(|e| matches!(e, HostEvent::Custom { name, .. } if name == "demo_end"))
            .count();
        assert_eq!(n, 1);
    }
}
```

Add `pub mod demo;` to `src/game/mod.rs` after `pub mod audio;` (line 3).

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test --features demo demo::tests`
Expected: FAIL to compile: `DEMO_CAP`, `demo_reached` not found.

- [ ] **Step 4: Write the implementation**

Above the test module in `src/game/demo.rs`:

```rust
use bevy::prelude::*;
use gamebient_input::HostEvent;

use crate::game::scoring::GameData;
use crate::game::sim;
use crate::game::states::GameState;
use crate::ui::transition::ScreenFade;

/// True in the demo bundle. UI reads it to tag the title card.
pub const DEMO: bool = cfg!(feature = "demo");

/// TEMPLATE NOTE: the template has no levels, so its demo ends at a score.
/// A port replaces this with its own unit ("after level 2": `level >= 3`).
pub const DEMO_CAP: u32 = 1_000;

/// The cut, as a pure function of whatever the game already tracks.
/// TEMPLATE NOTE: a port reads its own progression resource instead of
/// `GameData` (change the `end_demo` parameter to match).
pub fn demo_reached(data: &GameData) -> bool {
    data.score >= DEMO_CAP
}

/// Ends the run at the cut: latches `RunOver` so this is the last simulated
/// tick (the sim freezes, like `sim::end_run`), tells the host, and leaves
/// `Playing` through the fade when there is one, or `NextState` when there
/// is not. Registered in `FixedUpdate` after `SimSet`, demo feature only.
pub fn end_demo(
    data: Res<GameData>,
    mut over: ResMut<sim::RunOver>,
    fade: Option<ResMut<ScreenFade>>,
    mut next: ResMut<NextState<GameState>>,
    mut out: MessageWriter<HostEvent>,
) {
    if over.0 || !demo_reached(&data) {
        return;
    }
    over.0 = true;
    out.write(HostEvent::Custom {
        name: "demo_end".into(),
        data: "{}".into(),
    });
    match fade {
        Some(mut fade) => {
            fade.request(GameState::DemoEnd);
        }
        None => next.set(GameState::DemoEnd),
    }
}
```

In `src/game/mod.rs`, after the `restore_tick_frame_while_paused` registration (ends line 122) and before the `OnExit(GameState::Playing)` one, add:

```rust
            // The demo bundle's content cap. After the sim so the capped
            // tick is simulated and recorded in full; `run_not_over` keeps it
            // from firing twice. Both modes: the headless verifier is never
            // built with `demo`, and the headless test app exercises it.
            .add_systems(
                FixedUpdate,
                demo::end_demo
                    .after(sim::SimSet)
                    .run_if(in_state(GameState::Playing).and(sim::run_not_over))
                    .run_if(|| cfg!(feature = "demo")),
            )
```

(Use `run_if(|| cfg!(feature = "demo"))` rather than `#[cfg]` on the builder chain so the method chain stays one expression; the system is a no-op in full builds and the constant condition is free.)

- [ ] **Step 5: Fix every non-exhaustive match**

Run: `cargo check --all-targets --all-features 2>&1 | grep -B2 -A6 "non-exhaustive"`
For each `match` on `GameState` reported (expected in `src/game/audio.rs` `music_director` and possibly `src/frame/`), add `GameState::DemoEnd` to the same arm as `GameState::GameOver`.

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test --features demo demo::tests && cargo test demo::tests`
Expected: PASS: 3 tests with the feature, 1 without.

- [ ] **Step 7: Full checks and commit**

Run: `cargo test --all-targets --all-features && cargo clippy --all-targets --all-features -- -D warnings && cargo fmt --all -- --check`
Expected: clean.

```bash
git add Cargo.toml src/game/states.rs src/game/demo.rs src/game/mod.rs src/game/audio.rs
git commit -m "feat(demo): demo feature, GameState::DemoEnd and the end_demo cut

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```
(Add any other file Step 5 touched.)

---

### Task 2: `DEMO OVER` card, title tag, menu routing

**Files:**
- Create: `src/ui/demo_end.rs`
- Modify: `src/ui/mod.rs:4-13` (modules) and `:65-70` (Menu / Game Over wiring)
- Modify: `src/ui/menu.rs:106-110` (after the tagline) and `:263-267` (`menu_input` targets)
- Test: `src/ui/demo_end.rs` (unit test on copy helpers), `src/ui/menu.rs` (existing tests, if any, stay green)

**Interfaces:**
- Consumes: `game::demo::DEMO`, `card`, `card_with`, `chip_node`, `intro`, `theme::{CARD, BADGE, RAIL, CHIP, CORAL, MUTED, PAPER, SIGNAL, TITLE_SHADOW}`, `fonts::{BODY, DISPLAY}`, `menu::{MenuRoot, TITLE, PROMPT_PULSE, best_label}`, `spawn_backdrop`.
- Produces: `ui::demo_end::spawn_demo_end` (system), `ui::demo_end::own_line(title: &str) -> String`, `ui::demo_end::spawn_demo_tag(parent)`, constants `DEMO_HEADLINE`, `RESTART_PROMPT`, `FULL_GAME_LINE`.

- [ ] **Step 1: Write the failing test**

Create `src/ui/demo_end.rs` with the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn own_line_names_the_game_in_caps_and_ascii() {
        assert_eq!(own_line("Gamebient Game"), "OWN GAMEBIENT GAME TO KEEP PLAYING");
        assert!(own_line(crate::ui::menu::TITLE).is_ascii());
    }

    #[test]
    fn copy_is_ascii() {
        for s in [DEMO_HEADLINE, RESTART_PROMPT, FULL_GAME_LINE, DEMO_TAG] {
            assert!(s.is_ascii(), "{s}");
        }
    }
}
```

Add `pub mod demo_end;` to `src/ui/mod.rs` after `pub mod card;`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test demo_end::tests`
Expected: FAIL to compile (`own_line` etc. missing).

- [ ] **Step 3: Write the card**

Above the tests in `src/ui/demo_end.rs`:

```rust
//! The demo bundle's end card (`GameState::DemoEnd`): the run stopped at the
//! content cap, not at a game over. Hosts put the buy button next to it; this
//! card names no price and no URL.

use bevy::prelude::*;

use crate::game::scoring::GameData;
use crate::ui::backdrop::spawn_backdrop;
use crate::ui::card::{S1, S2, S4, S6, card, card_with, chip_node, intro};
use crate::ui::fonts::{BODY, DISPLAY};
use crate::ui::menu::{MenuRoot, PROMPT_PULSE, TITLE, best_label};
use crate::ui::theme::{self, BADGE, CHIP, CORAL, MUTED, PAPER, RAIL, SIGNAL, TITLE_SHADOW};

pub const DEMO_HEADLINE: &str = "DEMO OVER";
pub const RESTART_PROMPT: &str = "PRESS ENTER TO RESTART";
/// TEMPLATE NOTE: name the unit the full game has more of ("MORE HOLES",
/// "EVERY SONG", "ALL FIVE SHIFTS").
pub const FULL_GAME_LINE: &str = "THE FULL GAME HAS MORE LEVELS";
/// The chip on the title card in the demo bundle.
pub const DEMO_TAG: &str = "DEMO";

/// "OWN <TITLE> TO KEEP PLAYING".
pub fn own_line(title: &str) -> String {
    format!("OWN {} TO KEEP PLAYING", title.to_ascii_uppercase())
}

/// The DEMO chip; `spawn_menu` calls it under the tagline when `demo::DEMO`.
pub fn spawn_demo_tag(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn(card_with(&BADGE, chip_node()))
        .with_child((Text::new(DEMO_TAG), BODY.font(16.0), TextColor(CORAL)));
}

pub fn spawn_demo_end(mut commands: Commands, data: Res<GameData>) {
    let own = own_line(TITLE);
    let score = data.score.to_string();
    let best = best_label(data.high_score);
    commands
        .spawn((
            MenuRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(S4),
                ..default()
            },
        ))
        .with_children(|root| {
            spawn_backdrop(root, true);
            root.spawn((card(&theme::CARD), intro())).with_children(|card| {
                card.spawn((
                    Text::new(DEMO_HEADLINE),
                    DISPLAY.fitted(DEMO_HEADLINE, 40.0),
                    TextColor(CORAL),
                ));
                card.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(S2),
                    padding: UiRect::horizontal(Val::Px(S6)),
                    ..default()
                })
                .with_children(|body| {
                    body.spawn((
                        Text::new(own.clone()),
                        DISPLAY.fitted(&own, 32.0),
                        TextColor(PAPER),
                        TextShadow {
                            offset: Vec2::new(0.0, 4.0),
                            color: TITLE_SHADOW,
                        },
                    ));
                    body.spawn((
                        Text::new(FULL_GAME_LINE),
                        BODY.fitted(FULL_GAME_LINE, 18.0),
                        TextColor(SIGNAL),
                    ));
                    body.spawn((Text::new("SCORE"), BODY.font(16.0), TextColor(MUTED)));
                    body.spawn((Text::new(score), DISPLAY.font(64.0), TextColor(PAPER)));
                });
                card.spawn(card_with(&CHIP, chip_node()))
                    .with_child((Text::new(best), BODY.font(16.0), TextColor(SIGNAL)));
            });
            root.spawn(card_with(
                &RAIL,
                Node {
                    padding: UiRect::axes(Val::Px(S4), Val::Px(S2)),
                    margin: UiRect::top(Val::Px(S1)),
                    ..default()
                },
            ))
            .with_child((
                Text::new(RESTART_PROMPT),
                BODY.fitted(RESTART_PROMPT, 24.0),
                TextColor(PAPER),
                PROMPT_PULSE,
            ));
        });
}
```

(If `ChildSpawnerCommands` is not the type `with_children` closures receive in this Bevy version, use whatever type `spawn_backdrop`'s first parameter has in `src/ui/backdrop.rs`; match it exactly.)

- [ ] **Step 4: Wire the state and the title tag**

`src/ui/mod.rs`, after the Game Over lines (68-69):
```rust
            .add_systems(OnEnter(GameState::DemoEnd), demo_end::spawn_demo_end)
            .add_systems(OnExit(GameState::DemoEnd), menu::despawn_menu)
```

`src/ui/menu.rs`:
- Add `use crate::game::demo;` and `use crate::ui::demo_end::spawn_demo_tag;` to the imports.
- In `spawn_menu`, right after the tagline spawn (line 110, inside the `lockup` closure), add:
```rust
                if demo::DEMO {
                    spawn_demo_tag(lockup);
                }
```
- In `menu_input`, change the target match to:
```rust
    let target = match state.get() {
        GameState::Menu => Some(start_target(seen.0)),
        GameState::GameOver | GameState::DemoEnd => Some(GameState::Menu),
        _ => None,
    };
```
- In `spawn_game_over`, after the best/new-best chip (line 225), add the full-game reminder in the demo bundle:
```rust
                    if demo::DEMO {
                        card.spawn(card_with(&CHIP, chip_node())).with_child((
                            Text::new(crate::ui::demo_end::FULL_GAME_LINE),
                            BODY.font(14.0),
                            TextColor(MUTED),
                        ));
                    }
```

- [ ] **Step 5: Run tests and the game**

Run: `cargo test demo_end::tests && cargo test --all-targets --all-features && cargo clippy --all-targets --all-features -- -D warnings && cargo fmt --all -- --check`
Expected: clean.

Run: `GX_MUTE=1 cargo run --features demo` and play until the score passes 1000 (the template scores on movement/pickups; if reaching 1000 takes longer than a minute, temporarily lower `DEMO_CAP` locally, do not commit that). Expected: fade to the `DEMO OVER` card, Enter returns to the title, title shows the `DEMO` chip. Then `GX_MUTE=1 cargo run`: no chip, no cut.

- [ ] **Step 6: Commit**

```bash
git add src/ui/demo_end.rs src/ui/mod.rs src/ui/menu.rs
git commit -m "feat(demo): DEMO OVER card, title tag, DemoEnd routing

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 3: Token verification module and its Node tests

**Files:**
- Create: `tools/entitlement.mjs`
- Create: `tools/test_entitlement.mjs`
- Modify: `.github/workflows/ci.yml:64` (after "Cutter tests")

**Interfaces:**
- Produces: `verifyToken(token, publicKeyB64, { host, now }) -> Promise<{ok: true, payload} | {ok: false, reason}>`, `cookieHeader(token, maxAge = 43200) -> string`, `readCookie(cookieHeader, name) -> string | null`, `MAX_AGE = 43200`.

- [ ] **Step 1: Write the failing tests**

```js
// tools/test_entitlement.mjs
// node --test tools/test_entitlement.mjs
import { test } from "node:test";
import assert from "node:assert/strict";
import { webcrypto } from "node:crypto";
import { MAX_AGE, cookieHeader, readCookie, verifyToken } from "./entitlement.mjs";

const subtle = webcrypto.subtle;
const enc = new TextEncoder();
const b64url = (bytes) =>
  Buffer.from(bytes).toString("base64").replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");

async function keypair() {
  const kp = await subtle.generateKey({ name: "Ed25519" }, true, ["sign", "verify"]);
  const raw = new Uint8Array(await subtle.exportKey("raw", kp.publicKey));
  return { priv: kp.privateKey, pubB64: Buffer.from(raw).toString("base64") };
}

async function mint(priv, payload) {
  const part = b64url(enc.encode(JSON.stringify(payload)));
  const sig = new Uint8Array(await subtle.sign({ name: "Ed25519" }, priv, enc.encode(part)));
  return `${part}.${b64url(sig)}`;
}

const HOST = "voidrunner-theta.vercel.app";
const NOW = 1_800_000_000;

test("a good token verifies for its host before expiry", async () => {
  const { priv, pubB64 } = await keypair();
  const token = await mint(priv, { h: HOST, w: "W", g: "G", exp: NOW + 60 });
  const r = await verifyToken(token, pubB64, { host: HOST.toUpperCase(), now: NOW });
  assert.equal(r.ok, true);
  assert.deepEqual(r.payload, { h: HOST, w: "W", g: "G", exp: NOW + 60 });
});

test("expired, wrong host, wrong key, tampered and malformed tokens fail", async () => {
  const { priv, pubB64 } = await keypair();
  const other = await keypair();
  const good = await mint(priv, { h: HOST, w: "W", g: "G", exp: NOW + 60 });
  assert.equal((await verifyToken(good, pubB64, { host: HOST, now: NOW + 60 })).reason, "expired");
  assert.equal((await verifyToken(good, pubB64, { host: "x.vercel.app", now: NOW })).reason, "host");
  assert.equal((await verifyToken(good, other.pubB64, { host: HOST, now: NOW })).reason, "signature");
  const [part, sig] = good.split(".");
  const tampered = `${b64url(enc.encode(JSON.stringify({ h: "x.vercel.app", w: "W", g: "G", exp: NOW + 60 })))}.${sig}`;
  assert.equal((await verifyToken(tampered, pubB64, { host: "x.vercel.app", now: NOW })).reason, "signature");
  assert.equal((await verifyToken("nodot", pubB64, { host: HOST, now: NOW })).reason, "format");
  assert.equal((await verifyToken(`${part}.!!!`, pubB64, { host: HOST, now: NOW })).reason, "format");
  const badShape = await mint(priv, { h: HOST, w: "W", exp: NOW + 60 });
  assert.equal((await verifyToken(badShape, pubB64, { host: HOST, now: NOW })).reason, "payload");
});

test("cookie helpers", () => {
  const c = cookieHeader("abc.def");
  assert.equal(c, `gx_full=abc.def; Path=/full; Max-Age=${MAX_AGE}; Secure; HttpOnly; SameSite=None`);
  assert.equal(MAX_AGE, 43200);
  assert.equal(readCookie("a=1; gx_full=abc.def; b=2", "gx_full"), "abc.def");
  assert.equal(readCookie("a=1", "gx_full"), null);
  assert.equal(readCookie(null, "gx_full"), null);
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `node --test tools/test_entitlement.mjs`
Expected: FAIL, cannot find `./entitlement.mjs`.

- [ ] **Step 3: Write the module**

```js
// tools/entitlement.mjs
// Verifies a ColecoVision GX full-game entitlement token. Web APIs only
// (TextEncoder, atob, crypto.subtle) so the same file runs in Vercel
// middleware and under `node --test`. Mirror of the website's
// src/lib/entitlement/payload.ts + sign.ts.

export const MAX_AGE = 43_200;
export const COOKIE = "gx_full";

const B64URL = /^[A-Za-z0-9_-]+$/;

function fromBase64Url(s) {
  const b64 = s.replace(/-/g, "+").replace(/_/g, "/") + "=".repeat((4 - (s.length % 4)) % 4);
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

function fromBase64(s) {
  const bin = atob(s);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

function decodePayload(part) {
  let parsed;
  try {
    parsed = JSON.parse(new TextDecoder().decode(fromBase64Url(part)));
  } catch {
    return null;
  }
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) return null;
  const keys = Object.keys(parsed);
  if (keys.length !== 4) return null;
  const { h, w, g, exp } = parsed;
  if (typeof h !== "string" || typeof w !== "string" || typeof g !== "string") return null;
  if (typeof exp !== "number" || !Number.isFinite(exp)) return null;
  return { h, w, g, exp };
}

/**
 * @param {string} token  "<base64url payload>.<base64url signature>"
 * @param {string} publicKeyB64  base64 of the raw 32-byte Ed25519 public key
 * @param {{host: string, now: number}} ctx  request host and unix seconds
 */
export async function verifyToken(token, publicKeyB64, ctx) {
  if (typeof token !== "string") return { ok: false, reason: "format" };
  const parts = token.split(".");
  if (parts.length !== 2 || !B64URL.test(parts[0]) || !B64URL.test(parts[1])) return { ok: false, reason: "format" };
  const [part, sigB64] = parts;
  const sig = fromBase64Url(sigB64);
  if (sig.length !== 64) return { ok: false, reason: "format" };
  let key;
  try {
    key = await crypto.subtle.importKey("raw", fromBase64(publicKeyB64), { name: "Ed25519" }, false, ["verify"]);
  } catch {
    return { ok: false, reason: "key" };
  }
  const valid = await crypto.subtle.verify({ name: "Ed25519" }, key, sig, new TextEncoder().encode(part));
  if (!valid) return { ok: false, reason: "signature" };
  const payload = decodePayload(part);
  if (!payload) return { ok: false, reason: "payload" };
  if (payload.h.toLowerCase() !== ctx.host.toLowerCase()) return { ok: false, reason: "host" };
  if (!(ctx.now < payload.exp)) return { ok: false, reason: "expired" };
  return { ok: true, payload };
}

export function cookieHeader(token, maxAge = MAX_AGE) {
  return `${COOKIE}=${token}; Path=/full; Max-Age=${maxAge}; Secure; HttpOnly; SameSite=None`;
}

export function readCookie(cookieHeaderValue, name) {
  if (!cookieHeaderValue) return null;
  for (const pair of cookieHeaderValue.split(";")) {
    const [k, ...v] = pair.trim().split("=");
    if (k === name) return v.join("=");
  }
  return null;
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `node --test tools/test_entitlement.mjs`
Expected: PASS (3 tests). If `generateKey` throws `NotSupportedError`, the Node on PATH is older than 18.4; use `~/.nvm/versions/node/v22*/bin/node`.

- [ ] **Step 5: CI**

In `.github/workflows/ci.yml`, after the "Cutter tests" step (line 64-66) add:
```yaml
      - name: Entitlement middleware tests
        run: node --test tools/test_entitlement.mjs
```
(The runner's default Node is 20+, which has WebCrypto Ed25519.)

- [ ] **Step 6: Commit**

```bash
git add tools/entitlement.mjs tools/test_entitlement.mjs .github/workflows/ci.yml
git commit -m "feat(full): entitlement token verifier (WebCrypto) with node tests

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 4: Vercel middleware for `/full/`

**Files:**
- Create: `middleware.ts`
- Modify: `vercel.json:27-38` (add `/full/` no-cache entries)

**Interfaces:**
- Consumes: Task 3's `verifyToken`, `cookieHeader`, `readCookie`, `COOKIE`.
- Produces: `/full/?t=<token>` → 302 to `/full/` with the cookie; `/full/*` with a valid cookie → pass through; otherwise 403 HTML, `Cache-Control: no-store`.

- [ ] **Step 1: Write the middleware**

```ts
// middleware.ts
// Vercel Routing Middleware: gates the full-game bundle under /full/ behind
// a ColecoVision GX entitlement token (docs: AGENTS.md "Demo and full
// bundles"). Runs before the edge cache. No npm dependencies: Web APIs only.
import { COOKIE, cookieHeader, readCookie, verifyToken } from "./tools/entitlement.mjs";

export const config = {
  matcher: ["/full/:path*"],
};

const FORBIDDEN_HTML = `<!doctype html><meta charset="utf-8"><title>Full game</title>
<style>body{margin:0;min-height:100vh;display:grid;place-items:center;background:#0a0a0f;color:#e8e4df;font:16px/1.5 system-ui,sans-serif;text-align:center}a{color:#00d4ff}</style>
<div><p>This is the full game.</p><p>Launch it from <a href="https://colecovisiongx.com/play">colecovisiongx.com/play</a>.</p></div>`;

function forbidden(reason: string): Response {
  return new Response(FORBIDDEN_HTML, {
    status: 403,
    headers: {
      "content-type": "text/html; charset=utf-8",
      "cache-control": "no-store",
      "x-gx-entitlement": reason,
    },
  });
}

export default async function middleware(request: Request): Promise<Response | undefined> {
  const pubkey = process.env.GX_ENTITLEMENT_PUBKEY;
  if (!pubkey) return forbidden("unconfigured");
  const url = new URL(request.url);
  const ctx = { host: url.host, now: Math.floor(Date.now() / 1000) };

  // Entry: the website frames /full/?t=<token>. Verify, set the cookie, and
  // redirect to the clean URL so the bundle's relative asset loads carry it.
  const t = url.searchParams.get("t");
  if (t) {
    const r = await verifyToken(t, pubkey, ctx);
    if (!r.ok) return forbidden(r.reason);
    url.searchParams.delete("t");
    return new Response(null, {
      status: 302,
      headers: {
        location: url.pathname + url.search,
        "set-cookie": cookieHeader(t),
        "cache-control": "no-store",
      },
    });
  }

  const cookie = readCookie(request.headers.get("cookie"), COOKIE);
  if (!cookie) return forbidden("missing");
  const r = await verifyToken(cookie, pubkey, ctx);
  if (!r.ok) return forbidden(r.reason);
  return undefined; // fall through to the static file
}
```

- [ ] **Step 2: Headers for the full bundle's index**

In `vercel.json`, append to `headers` (after the `/index.html` entry):
```json
    {
      "source": "/full",
      "headers": [{ "key": "Cache-Control", "value": "public, max-age=0, must-revalidate" }]
    },
    {
      "source": "/full/",
      "headers": [{ "key": "Cache-Control", "value": "public, max-age=0, must-revalidate" }]
    },
    {
      "source": "/full/index.html",
      "headers": [{ "key": "Cache-Control", "value": "public, max-age=0, must-revalidate" }]
    }
```
(The existing `/(.*).wasm`, `/(.*).wasm.br` and `/(.*).js` entries already match under `/full/`.)

- [ ] **Step 3: Local syntax check**

Run: `node --check middleware.ts 2>/dev/null || npx --yes esbuild middleware.ts --bundle --format=esm --platform=neutral --outfile=/dev/null`
Expected: esbuild bundles without error (TypeScript parses; the `.mjs` import resolves).

- [ ] **Step 4: Commit**

```bash
git add middleware.ts vercel.json
git commit -m "feat(full): Vercel middleware gates /full/ behind the entitlement cookie

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 5: Two-bundle `build_web.sh`

**Files:**
- Modify: `build_web.sh` (whole file)
- Modify: `assets/info.json:20` (`game_url`)
- Test: `tools/test_game_size.sh` and `shellcheck` stay green; a manual build inspection.

**Interfaces:**
- Produces: `dist/` (demo bundle + `assets/` + `verify.zip`), `dist/full/` (full bundle + `assets/` copy). Both `index.html`s reference hashed names relatively.

- [ ] **Step 1: Rewrite the script**

Replace `build_web.sh` with:

```bash
#!/bin/bash
set -euo pipefail

echo "Building Gamebient Game for Web (WASM)..."

# Ensure wasm target is installed
rustup target add wasm32-unknown-unknown 2>/dev/null || true

# Check for wasm-bindgen
if ! command -v wasm-bindgen &> /dev/null; then
    echo "Installing wasm-bindgen-cli..."
    cargo install wasm-bindgen-cli
fi
# wasm-opt is required; fail loudly if missing.
if ! command -v wasm-opt &> /dev/null; then
    echo "ERROR: wasm-opt not found. Install binaryen (e.g. 'npm install -g wasm-opt')." >&2
    exit 1
fi

hash8() {
    if command -v sha256sum &> /dev/null; then sha256sum "$1"; else shasum -a 256 "$1"; fi | cut -c1-8
}

# bundle OUT FEATURES: builds one web bundle into OUT. The demo (`--features
# demo`) goes to dist/, the full game to dist/full/; Vercel middleware gates
# the latter. Both are built from this same commit, so a demo run and a full
# run are identical up to the demo's cut.
bundle() {
    local out="$1" features="$2"
    echo "== bundle: ${out} (features: ${features:-none})"

    # --locked: Cargo.lock is committed, and this same script is the Vercel
    # build command, so a deploy must use the exact dependency versions CI
    # tested (wasm-bindgen in particular, whose CLI install.sh pins separately).
    if [ -n "$features" ]; then
        cargo build --locked --profile wasm-release --target wasm32-unknown-unknown --features "$features"
    else
        cargo build --locked --profile wasm-release --target wasm32-unknown-unknown
    fi

    # Find the compiled .wasm by glob (cargo names it after the bin target) and
    # force deterministic output names with --out-name. verify.wasm is excluded
    # by name: tools/build_verify.sh builds the replay verifier into the same
    # directory. Anything else unexpected in there is a hard error.
    mkdir -p "$out"
    local wasm
    wasm=$(find target/wasm32-unknown-unknown/wasm-release -maxdepth 1 -name '*.wasm' ! -name 'verify.wasm')
    if [ -z "$wasm" ]; then
        echo "ERROR: no game .wasm found in target/wasm32-unknown-unknown/wasm-release/" >&2
        exit 1
    fi
    if [ "$(printf '%s\n' "$wasm" | wc -l | tr -d ' ')" -ne 1 ]; then
        echo "ERROR: more than one candidate .wasm in target/wasm32-unknown-unknown/wasm-release/:" >&2
        printf '%s\n' "$wasm" >&2
        echo "Remove the stale ones (or 'cargo clean') so the deployed bundle is unambiguous." >&2
        exit 1
    fi
    wasm-bindgen --out-dir "$out" --out-name gamebient-game --target web "$wasm"

    cp index.html "$out/"

    echo "Optimizing WASM with wasm-opt..."
    # Explicitly allow the WASM extensions enabled in .cargo/config.toml.
    wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
        "$out/gamebient-game_bg.wasm" -o "$out/gamebient-game_bg.wasm"

    # Content-hash the immutable assets; index.html stays unhashed (no-cache).
    rm -f "$out"/gamebient-game_bg.????????.wasm "$out"/gamebient-game_bg.????????.wasm.br "$out"/gamebient-game_bg.????????.wasm.gz \
          "$out"/gamebient-game.????????.js "$out"/gamebient-game.????????.js.br "$out"/gamebient-game.????????.js.gz \
          "$out"/gamebient-game_bg.wasm.br "$out"/gamebient-game_bg.wasm.gz "$out"/gamebient-game.js.br "$out"/gamebient-game.js.gz
    local wasm_hash js_hash wasm_out js_out
    wasm_hash=$(hash8 "$out/gamebient-game_bg.wasm")
    js_hash=$(hash8 "$out/gamebient-game.js")
    wasm_out="gamebient-game_bg.${wasm_hash}.wasm"
    js_out="gamebient-game.${js_hash}.js"
    mv "$out/gamebient-game_bg.wasm" "$out/${wasm_out}"
    mv "$out/gamebient-game.js" "$out/${js_out}"
    sed -i.bak "s#gamebient-game_bg\.wasm#${wasm_out}#g" "$out/${js_out}" && rm -f "$out/${js_out}.bak"
    sed -i.bak -e "s#\./gamebient-game_bg\.wasm#./${wasm_out}#g" -e "s#\./gamebient-game\.js#./${js_out}#g" "$out/index.html" && rm -f "$out/index.html.bak"
    echo "Hashed assets: ${wasm_out}, ${js_out}"

    # Brotli-compress the wasm with Node's zlib (no separate brotli binary).
    if command -v node &> /dev/null; then
        echo "Brotli-compressing WASM..."
        node -e "const fs=require('fs'),zlib=require('zlib');const src=fs.readFileSync('${out}/${wasm_out}');const o=zlib.brotliCompressSync(src,{params:{[zlib.constants.BROTLI_PARAM_QUALITY]:11}});fs.writeFileSync('${out}/${wasm_out}.br',o);console.log('  '+src.length+' -> '+o.length+' bytes ('+(o.length*100/src.length).toFixed(1)+'%)')"
    else
        echo "WARNING: node not found; skipping brotli compression." >&2
    fi
}

# Fresh output: the full bundle lives inside dist/, so stale files from a
# previous layout must not survive.
rm -rf dist
bundle dist demo
bundle dist/full ""

# Game assets (Bevy expects assets/ relative to the page); the full bundle gets
# its own copy so nothing resolves across the /full/ boundary.
if [ -d assets ]; then
    rm -rf dist/assets dist/full/assets
    cp -r assets dist/assets
    cp -r assets dist/full/assets
fi

# The Pi cartridge binary (~45 MB) is fetched from the latest GitHub release so
# binary_url keeps resolving at the same Vercel path. Non-fatal.
bash fetch-cartridge.sh || true

# The headless replay verifier, once, from the full sim: dist/verify.zip is
# what properties.verify_url names. Built last so the `find` above never sees
# verify.wasm before it is excluded by name.
bash tools/build_verify.sh
cp dist-verify.zip dist/verify.zip

echo ""
echo "Build complete! Demo in dist/, full game in dist/full/ (gated by middleware.ts)."
echo "To test locally: cd dist && python3 -m http.server 8080  (full: /full/ is open locally; only Vercel runs the middleware)"
```

Check `fetch-cartridge.sh`: it writes into `dist/assets/`; if it also expects `dist/assets` to exist before running, the order above (assets copied first) satisfies it. Read it and confirm, then leave it alone.

- [ ] **Step 2: Shellcheck and build**

Run: `shellcheck build_web.sh && bash build_web.sh`
Expected: builds twice; `dist/index.html`, `dist/full/index.html`, hashed `.wasm`/`.js` in both, `dist/assets`, `dist/full/assets`, `dist/verify.zip`. The two hashed wasm names differ (the demo has the extra system).

Run: `ls dist dist/full && tools/test_game_size.sh`
Expected: as above; size test green.

Run: `cd dist && python3 -m http.server 8080` then open `http://localhost:8080/?mute=1` and `http://localhost:8080/full/?mute=1`. Expected: the root shows the `DEMO` chip on the title; `/full/` does not.

- [ ] **Step 3: `info.json`**

`assets/info.json`: `"game_url": "https://gamebient-game.vercel.app/full/"` (keep `demo_url` as the root).

- [ ] **Step 4: Commit**

```bash
git add build_web.sh assets/info.json
git commit -m "feat(build): demo bundle at dist/, full bundle at dist/full/

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 6: Preview deploy proves the gate

**Files:** none new. This task verifies Tasks 3 to 5 on Vercel before any game ports.

- [ ] **Step 1: Push a branch and let Vercel build a preview**

```bash
git push -u origin HEAD
```
Open the preview URL Vercel posts (`vercel ls gamebient-bevy-template --scope breadheads` or the PR check). Set `GX_ENTITLEMENT_PUBKEY` on the template's Vercel project for Preview + Production (`vercel env add GX_ENTITLEMENT_PUBKEY preview --scope breadheads` from the repo dir; value from the website's `node scripts/entitlement-keygen.mjs` output, the public line) and redeploy the preview.

- [ ] **Step 2: Probe**

```bash
P=https://<preview-host>
curl -sI "$P/" | head -1                      # 200
curl -sI "$P/full/" | grep -E "HTTP|x-gx"     # 403, x-gx-entitlement: missing
curl -sI "$P/full/?t=garbage" | grep -E "HTTP|x-gx"   # 403, format
```
Mint a token for the preview host with the website repo (private key from the same keygen run):
```bash
cd ../../website/ColecoVisionGXWebsite
GX_ENTITLEMENT_PRIVATE_KEY=<priv> node -e '
const {signEntitlement}=await import("./src/lib/entitlement/sign.ts").catch(()=>null) ?? {};
' 2>/dev/null || true
```
If importing the TS module from Node is awkward, add a one-off script `scripts/entitlement-sign.mjs` to the website repo (pure Node: same PKCS8 key, `crypto.sign(null, Buffer.from(part), key)`, prints the token) and commit it there as a debugging aid.
```bash
T=<token for h=<preview-host>>
curl -sI "$P/full/?t=$T" | grep -E "HTTP|location|set-cookie"   # 302, location: /full/, set-cookie: gx_full=...
curl -sI -H "cookie: gx_full=$T" "$P/full/" | head -1            # 200
curl -sI -H "cookie: gx_full=$T" "$P/full/$(curl -s -H "cookie: gx_full=$T" "$P/full/" | grep -o 'gamebient-game_bg\.[0-9a-f]*\.wasm' | head -1)" | head -1   # 200
```
Open `$P/full/?t=$T&mute=1` in a browser: the full game loads without the `DEMO` chip.

- [ ] **Step 3: If the middleware did not run**

Symptoms: `/full/` returns 200 without a cookie. Check the Vercel build log for "middleware"; if the file was ignored, add `"framework": null` is already the case per the workspace memory; try `export const config = { matcher: ["/full/:path*"], runtime: "nodejs" }` (Node runtime middleware), redeploy, re-probe. If `importKey` logged `NotSupportedError` on the Edge runtime, the same `runtime: "nodejs"` change fixes it. Record which variant worked in `AGENTS.md` (Task 7). If neither works, stop and report; do not ship an open `/full/`.

- [ ] **Step 4: Record the result**

No commit unless Step 3 changed `middleware.ts`; then:
```bash
git add middleware.ts
git commit -m "fix(full): middleware runtime that verifies Ed25519 on Vercel

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 7: `AGENTS.md` and the port recipe

**Files:**
- Modify: `AGENTS.md` ("How to add things" gets a subsection; "Build / CI / release model" gets a paragraph; "Gotchas" gets two bullets)

- [ ] **Step 1: Document**

Under "How to add things" add:

```md
### Demo and full bundles

Every web deploy carries two bundles from one commit: the **demo** at `/`
(built with `--features demo`) and the **full game** at `/full/`, which
`middleware.ts` serves only to a request carrying a valid ColecoVision GX
entitlement (a 12 h Ed25519 token the website signs for an owner; public key
in the Vercel env `GX_ENTITLEMENT_PUBKEY`). Native, Pi, `verify`, `autopilot`
and `record` builds are always the full game.

The cut lives in `src/game/demo.rs` and nowhere else:

- `DEMO_CAP` and `demo_reached(..)` express the cap in the game's own unit
  ("after level 2" is `level >= 3`, checked when the next unit begins).
  Change the `end_demo` parameter to whatever resource holds that unit.
- `end_demo` runs in `FixedUpdate` after `SimSet`, latches `sim::RunOver`
  (the sim freezes, as on `sim::end_run`), posts `HostEvent::Custom
  { name: "demo_end" }`, and fades into `GameState::DemoEnd`.
- `src/ui/demo_end.rs` draws `DEMO OVER` / `OWN <TITLE> TO KEEP PLAYING`;
  set `FULL_GAME_LINE` to name what the full game has more of. The title
  card shows a `DEMO` chip in the demo bundle (`demo::DEMO`).
- `assets/info.json`: `demo_url` is the host root, `game_url` is
  `<host>/full/`.

Port checklist: feature in `Cargo.toml`; `DemoEnd` in `states.rs` plus every
`match` on `GameState`; `demo.rs` with the game's cut and tests;
`ui/demo_end.rs`, `ui/mod.rs`, `ui/menu.rs` edits; `build_web.sh`,
`middleware.ts`, `tools/entitlement.mjs`, `tools/test_entitlement.mjs`,
`vercel.json` headers and the CI step copied from the template; `game_url`;
`GX_ENTITLEMENT_PUBKEY` on the game's Vercel project; then release and run the
production checks below.

Production checks after a release: `/` plays and stops at the cut with the
card; `curl -sI https://<host>/full/` is 403; `/play/<pda>` as an owner on
colecovisiongx.com runs past the cut; the cabinet paired to that wallet does
too; a `/play` run still lands on the verified leaderboard.
```

Under "Build / CI / release model" add: "`build_web.sh` runs `bundle` twice (demo, then full); `dist/verify.zip` is built once from the full sim. CI runs `node --test tools/test_entitlement.mjs`."

Under "Gotchas" add:
```md
- **`/full/` is open when served locally** (`python3 -m http.server`): only
  Vercel runs `middleware.ts`. Never mirror `dist/full/` anywhere else.
- **`cargo test --all-features` includes `demo`.** A test that scores past
  `DEMO_CAP` in a windowed or headless `GamePlugin` app lands in `DemoEnd`,
  not `GameOver`; keep fixtures below the cap or run them without the feature.
```

- [ ] **Step 2: Lint docs and commit**

Run: `tools/test_aspect_docs.sh` (the stale-size lint must still pass).

```bash
git add AGENTS.md
git commit -m "docs: demo and full bundles, port checklist, production checks

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

- [ ] **Step 3: PR**

```bash
gh pr create --title "Demo build and gated full bundle" --body "$(cat <<'EOF'
Template side of the demo/full split (spec: workspace docs/plans/2026-10-07-demo-full-split-design.md).

- `demo` feature: `end_demo` cut after SimSet, `GameState::DemoEnd`, `DEMO OVER` card, `DEMO` title chip, `demo_end` host event.
- `build_web.sh` emits the demo at dist/ and the full game at dist/full/.
- `middleware.ts` + `tools/entitlement.mjs` gate /full/ behind the website's Ed25519 entitlement (cookie `gx_full`).
- AGENTS.md port checklist and production checks.

Preview verified: /full/ 403 without a token, 302 + cookie with one, wasm loads with the cookie.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
gh pr checks --watch
```

---

### Task 8: Pilot port, Voidrunner (`games/voidrunner`)

**Files (in the Voidrunner repo):**
- Modify: `Cargo.toml`, `src/game/states.rs`, `src/game/mod.rs`, `src/ui/mod.rs`, `src/ui/menu.rs`, `build_web.sh`, `vercel.json`, `.github/workflows/ci.yml`, `assets/info.json`, `AGENTS.md`
- Create: `src/game/demo.rs`, `src/ui/demo_end.rs`, `middleware.ts`, `tools/entitlement.mjs`, `tools/test_entitlement.mjs`

**Interfaces:**
- Consumes: Voidrunner's `level::LevelManager { current_level: u32, .. }` (`src/game/level.rs:27`), advanced by `level::advance_level` on each corridor leg; `is_boss_level(3)` is true, so level 3 is the first Leviathan.
- Produces: the demo ends the moment level 3 begins.

- [ ] **Step 1: Branch and apply the template's generic pieces**

```bash
cd games/voidrunner && git checkout main && git pull --ff-only && git checkout -b demo-full-split
```
Copy from the template at its merged commit: `middleware.ts`, `tools/entitlement.mjs`, `tools/test_entitlement.mjs`, the `vercel.json` header entries, the CI step, the `demo` feature stanza in `Cargo.toml`, the `DemoEnd` variant, the `build_web.sh` `bundle()` structure (keep Voidrunner's crate name where the template says `gamebient-game`; diff the two scripts first since games drift), `src/ui/demo_end.rs` (adjust imports to Voidrunner's `ui` modules: it has `card.rs`, `fonts.rs`, `theme.rs`, `menu.rs`; `backdrop` may be `title_scene`; match what `spawn_game_over` in `src/ui/menu.rs` uses), and the `ui/mod.rs` + `menu.rs` edits.

- [ ] **Step 2: Write the failing cut test**

`src/game/demo.rs` test:
```rust
    #[test]
    fn demo_ends_when_the_first_boss_level_begins() {
        assert!(!demo_reached(1));
        assert!(!demo_reached(2));
        assert!(demo_reached(3));
        assert!(demo_reached(4));
    }
```
Run: `cargo test demo::tests` → FAIL to compile.

- [ ] **Step 3: Implement the cut**

```rust
/// Levels 1 and 2 (corridor + open space each) are the demo. Level 3 is the
/// first Void Leviathan: the full game starts there.
pub const DEMO_LEVELS: u32 = 2;

pub fn demo_reached(current_level: u32) -> bool {
    current_level > DEMO_LEVELS
}

pub fn end_demo(
    level: Res<LevelManager>,
    mut over: ResMut<sim::RunOver>,
    fade: Option<ResMut<ScreenFade>>,
    mut next: ResMut<NextState<GameState>>,
    mut out: MessageWriter<HostEvent>,
) {
    if over.0 || !demo_reached(level.current_level) {
        return;
    }
    over.0 = true;
    out.write(HostEvent::Custom { name: "demo_end".into(), data: "{}".into() });
    match fade {
        Some(mut fade) => {
            fade.request(GameState::DemoEnd);
        }
        None => next.set(GameState::DemoEnd),
    }
}
```
Register it in `src/game/mod.rs` `FixedUpdate` after `sim::SimSet` with `.run_if(in_state(GameState::Playing).and(sim::run_not_over)).run_if(|| cfg!(feature = "demo"))` (Voidrunner's `mod.rs` uses the same `SimSet`; find it with `grep -n "SimSet" src/game/mod.rs`). `FULL_GAME_LINE` = `"THE FULL GAME HAS THE LEVIATHAN AND BEYOND"`.

Since `advance_level` runs on entering the corridor phase, `current_level` becomes 3 at the start of level 3 and the cut fires on that tick, before the player sees the boss corridor. Verify by reading `phase.rs:76-85`; if the level increments only after the first corridor tick, the cut still fires within one tick.

- [ ] **Step 4: Checks, play, build**

Run: `cargo test --all-targets --all-features && cargo clippy --all-targets --all-features -- -D warnings && cargo fmt --all -- --check && node --test tools/test_entitlement.mjs && shellcheck tools/*.sh build_web.sh install.sh`
Then `GX_MUTE=1 cargo run --features demo`: clear level 2 (the autopilot's beat timing in `src/game/autopilot.rs` shows how long that takes); expect the `DEMO OVER` card as level 3 starts. `bash build_web.sh`; serve `dist` and open `/?mute=1` (DEMO chip) and `/full/?mute=1` (no chip).

- [ ] **Step 5: `info.json`, env, PR**

`assets/info.json`: `"game_url": "https://voidrunner-theta.vercel.app/full/"`.
```bash
vercel env add GX_ENTITLEMENT_PUBKEY production --scope breadheads   # paste the public key
vercel env add GX_ENTITLEMENT_PUBKEY preview --scope breadheads
git add -A && git commit -m "feat(demo): demo bundle ends before the Leviathan; full game gated at /full/

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
git push -u origin HEAD
gh pr create --title "Demo/full split: demo ends before the first Leviathan" --body "…🤖 Generated with [Claude Code](https://claude.com/claude-code)"
gh pr checks --watch
```
On the preview: `curl -sI https://<preview>/full/` is 403; the root plays.

- [ ] **Step 6: Merge, release, redeploy, verify production** (controller session, not a subagent)

Per the store-refresh brief: merge, `git tag v0.3.0 && git push origin v0.3.0`, watch `release.yml`, `vercel redeploy <prod url> --scope breadheads`. Then:
```bash
curl -s https://voidrunner-theta.vercel.app/assets/info.json | grep game_url   # /full/
curl -sI https://voidrunner-theta.vercel.app/full/ | head -1                     # 403
```
On colecovisiongx.com: `/demo/<pda>` plays and shows the buy strip when level 3 would begin; `/play/<pda>` signed in as the publisher wallet reaches the Leviathan; the paired cabinet does too; a `/play` run posts to the verified leaderboard.

---

## Fleet rollout (after the pilot)

Each remaining game is Task 8 with its own cut (table in the spec, section 3) in waves of three or four, subagent per game, controller merges and releases. The cut functions:

| Game | `demo_reached` | Reads |
|---|---|---|
| Gravestone Gauntlet | `wave > 5` | wave counter |
| Grand Theft Otto | `round > 2` | round counter |
| Grand Theft Auto-Reply | `shift > 1` | shift index |
| Pizza Pinball | `level > 2` | level counter |
| Ladder Legend | `song_index > 0` on song start | track index |
| Beat Bender | `song_index > 0` on song start | song index |
| Sundae Shooter | `level > 3` | level counter |
| Tire Stack | `shift > 1` | shift index |
| Cannonball Putt | `hole > 3` | hole index |
| Dive Rise | `day > 2` | day counter |
| Pack The Ripper | `packs_opened >= 10` at the next pack | packs opened |
| Dough.io | `bakes >= 3` | bakes completed |
| Attic Excavator, Gulper, Hunted | `level > 2` | level counter |
| Table Titans | match ended once | match end |
| Moleman Racing (unpublished) | `lap > 1` on the first track | lap counter |
