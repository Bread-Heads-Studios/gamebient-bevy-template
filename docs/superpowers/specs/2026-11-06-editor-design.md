# Gamebient editor: design pass

**Date:** 2026-11-06 · **Status:** Proposed, milestone 1 of 4. No editor code
exists yet. Companion: `2026-11-06-editor-mockup.html` (clickable mockup of the
main screen).

Supersedes the fixed rule vocabulary of
`2026-11-01-natural-language-rules-design.md`. Its determinism section,
readbacks and "three layers, one game" still apply and are referenced below.

## Summary of the design

- **An agent writes rules, cards tune them.** A person types a request. A
  coding agent changes the game's Rust. With each change it emits a *card*:
  a plain-English readback and a few controls bound to *tunables*. Moving a
  control changes the running game at the next sim tick, with no model call
  and no rebuild.
- **The editor is a local web app** (Vite + React + TypeScript) with a small
  Rust backend, and ships as an **installable desktop app** (Tauri 2) that
  wraps both. It frames the game's wasm bundle in an iframe and talks to it
  over the existing `gamebient-input` host protocol, extended by one generic
  message (`gx:custom`). The backend holds the Claude API key and is the
  seam for the code-writing agent.
- **Tunables are reflected Rust resources** carrying labels, ranges and units
  as `bevy_reflect` custom attributes. The game reports them, accepts value
  changes at tick boundaries, folds them into the replay checksum, and
  records them in a new replay format `GXR2` with a content hash in the
  header.
- **Checks run in the browser.** Scenario checks are compiled into the editor
  build of the game and run headlessly inside the same wasm instance, so a
  control release gets a verdict in milliseconds.
- **Cards are data, validated against the game** before they render. A card
  that names a tunable the game doesn't expose is rejected with a plain
  message.

## Decisions taken, and the alternatives

| Question | Chosen | Alternatives considered |
|---|---|---|
| Where the editor runs | **Local-first, installable**: a Tauri 2 desktop app wrapping the web UI and a Rust backend; in development the same UI runs in a browser against the same backend | Hosted on colecovisiongx.com (needs server-side cargo builds and sandboxing; later). Pure browser, no server (can't hold an API key, can't rebuild). A Node sidecar (works in dev, but an installable app would have to ship Node). |
| Desktop shell | **Tauri 2** (system webview, ~10 MB, backend in Rust, the team's language) | Electron (bundled Chromium, ~200 MB, Node backend; most predictable WebGL2 but the heaviest install) |
| Protocol extension | **One generic host→game message `gx:custom`** in `gamebient-input` v0.5.0, mirroring the existing `custom` event; the tunables layer is named messages on top | Typed `gx:tune` / `gx:tunables` messages in the crate (crate learns game concepts; every layer needs a crate release). A second `postMessage` channel (rejected by the brief). |
| Where checks run | **In the game wasm**, as non-panicking scenario checks behind `--features editor` | `cargo test` in the sidecar per release (20 s+ per round trip, kills the "instant" feel). Both, with cargo as the authoritative gate on "Save" (deferred; the seam allows it). |
| Replay format | **`GXR2`**: `content` hash in the header, start values and mid-run changes in a tune log | Keep `GXR1` and fold values only into the checksum (a replay could not say *which* game it is; the verifier could never reproduce a mid-run change). |
| Frontend stack | **React 19 + TypeScript + Vite**, native form elements, no UI kit | Plain TS + Lit (smaller, but the team has no Lit code and the website is React). Svelte (same). |
| Agent in this session | **Mock agent, then a Claude "tune" agent** with a `show_card` tool | The code-writing loop (eval/vibe's territory; seam only). |

## Architecture

```
┌──────────────────────── browser ──────────────────────────────┐
│  Editor app (React)                                           │
│  ┌───────────┐  ┌──────────────┐  ┌───────────────────────┐   │
│  │ Preview   │  │ Chat + cards │  │ Inspector             │   │
│  │ <iframe>  │  │ history      │  │ Rules/Things/Counters │   │
│  └─────┬─────┘  └──────┬───────┘  └──────────┬────────────┘   │
│        │ postMessage   │ fetch /api/agent      │ (same store)  │
│  ┌─────▼──────────────────────────────────────▼────────────┐   │
│  │ host/  (protocol v1 + gx:custom)   store/ (tunables,    │   │
│  │ cards/ (schema + validator)        cards, pins, checks) │   │
│  └─────┬───────────────────────────────────────────────────┘   │
│        │                                                       │
│  ┌─────▼─────────────────────────────┐                         │
│  │ Game iframe: Coin Run wasm,       │  (served by backend,    │
│  │ built with --features editor      │   origin localhost)     │
│  └───────────────────────────────────┘                         │
└───────────────────────────────────────────────────────────────┘
          │ HTTP (127.0.0.1:<port>)
┌─────────▼────────────── Rust backend (crates/editor-server) ──┐
│ serves the UI and <game>/dist-editor/  ·  POST /api/agent      │
│ (Claude, tools)  ·  seam: POST /api/change → code agent →      │
│ rebuild → reload                                               │
└─────────┬──────────────────────────────────────────────────────┘
          │ embedded in
┌─────────▼────────────── Desktop app (crates/editor-app, Tauri) ┐
│ one window at http://127.0.0.1:<port>/ · key in the OS keychain│
│ · installers for macOS / Windows / Linux                       │
└────────────────────────────────────────────────────────────────┘
```

No database. The editor never sees Rust; the backend never sees UI.

### Installable app

Reducing friction is the point, so the local server is not something a
person starts by hand. The desktop app (Tauri 2) starts the backend on a
free 127.0.0.1 port and opens one window on it. Serving the UI from
`http://127.0.0.1:<port>/` rather than from the webview's own
`tauri://localhost` origin matters: the game's `gx.js` only answers hosts
whose origin matches `localhost` / `127.0.0.1`, so the app looks to the game
exactly like a browser tab does, and the same UI build runs unchanged in a
browser for development and for the Playwright suite.

- Backend: `crates/editor-server` (axum), with three jobs now (static
  files, `/api/agent`, `/api/game`) and the `/api/change` seam. The Claude
  key is read from the OS keychain (`keyring` crate) with an env-var
  override for development; the app's settings screen is where a person
  pastes it once.
- Game bundles: the app opens a game checkout (`GAME_DIR`) or a downloaded
  `dist-editor` bundle. Tuning needs no toolchain at all. The code-writing
  path needs Rust, the wasm target and the Claude Code CLI on the machine
  **or** a remote build service; the `/api/change` seam is an HTTP call so
  either backend can stand behind it. Which one ships to beginners is a
  later decision; the app detects what is installed and says so in plain
  words.
- Webview: WKWebView (macOS), WebView2 (Windows), WebKitGTK (Linux). All
  three run WebGL2; the Linux one is the least tested and is checked in
  milestone 5. Electron stays the fallback if a webview cannot run the
  game.

## Hosting the game

The website already frames a game and speaks the host protocol (reference:
`website/.../src/lib/gameHost.ts`, `gamebient-input/docs/host-protocol.md`).
The editor reuses the same handshake, the same origin rules (the crate allows
`localhost` hosts) and the same `gx:input` relay, in a framework-free
`host/protocol.ts` (parse, encode, reducer) plus a React hook around it.

**One crate change, `gamebient-input` v0.5.0:** a host→game message that
mirrors the existing game→host `custom` event.

```jsonc
{ "type": "gx:custom", "v": 1, "name": "tunables.set", "data": { "ammo.start": 8 } }
```

- `js/gx.js`: `case 'gx:custom'` queues `custom:<name>:<json>`; subject to the
  same origin allow-list as every other message.
- `src/host.rs`: `HostCommand::Custom { name: String, data: String }`
  (`data` is a JSON string; the crate does not parse it), `parse_command`
  arm, tests.
- `docs/host-protocol.md`: the message, and the rule that names are agreed
  out of band and unknown names are ignored. Existing hosts and games are
  unaffected (unknown types are already ignored on both sides).

Everything else is a **named layer** defined by the template, not the crate.

### The tunables layer (host protocol names)

| Direction | Message | When |
|---|---|---|
| host → game | `gx:custom` `tunables.list` | After hello; whenever the editor wants a fresh snapshot |
| game → host | `custom` `tunables` `{ game, build, content, tunables: [...], checks: [...], sounds: [...] }` | In answer to `tunables.list`, and once on `started` |
| host → game | `gx:custom` `tunables.set` `{ "<path>": value, ... }` | A control moved (sent on `input` while dragging, throttled to one per frame) |
| game → host | `custom` `tunable` `{ path, value, applied: "tick" \| "next_run", tick }` | After each set is applied |
| host → game | `gx:custom` `checks.run` `{ names: [...] }` | A control was released |
| game → host | `custom` `check` `{ name, ok, message }` | One per check, as each finishes |
| host → game | `gx:custom` `run.restart` | "Try it" / "Restart run" |
| host → game | `gx:custom` `sound.preview` `{ name }` | The sound picker's Play button |

A tunable entry in `tunables`:

```jsonc
{ "path": "things.bullet.speed", "group": "things.bullet", "label": "Bullet speed",
  "type": "f32", "min": 100, "max": 1200, "step": 10, "unit": "px/s",
  "applies": "tick",            // or "next_run"
  "value": 600, "default": 600 }
```

`type` is one of `f32 | i32 | u32 | bool | colour | choice | button | sound |
place | curve`; `choice`/`button`/`sound` entries carry `options`; `place`
carries `bounds` (the game size); `curve` carries `points` (count) and x/y
ranges.

## Tunables in the game

Lives in the template under `src/game/tunables/`, one file per concern:

| File | Owns |
|---|---|
| `attr.rs` | `Tune` and `Group`, the `bevy_reflect` custom attributes (`#[reflect(@Tune::int("Starting ammo", 0, 50))]`, `#[reflect(@Group::counter("ammo", "Ammo"))]`) |
| `value.rs` | `TunableValue` (the closed set of types), bit-exact `to_bits()` for the checksum, JSON encode/decode |
| `index.rs` | `TunableIndex`: the **ordered** list of registered tunable resources and their fields, built from `TypeRegistry` at startup. Order is registration order, never `TypeRegistry` iteration order (a hash map). `tunables::register::<T>(app)` is the one way in. |
| `apply.rs` | `PendingTunes` queue and `apply_pending_tunes`, chained in `SimSet` right after `sim::sync_run_clock`; folds `(id, bits)` into `Checksum`; records into the replay |
| `hash.rs` | `content_hash()`: FNV-1a over every tunable's `(id, bits)` in index order, plus every recorded mid-run change |
| `protocol.rs` | The named messages above: parses `HostCommand::Custom`, posts `HostEvent::Custom` |
| `checks.rs` | `Check { name, title, run: fn(&Scenario::Builder) -> Result<(), String> }`, `all()`, and the host handler that runs them (`--features editor`) |

Rules the implementation must keep:

1. **Apply only at a tick boundary.** `HostCommand`s arrive in `PreUpdate`;
   `queue_tunes` (an `Update` system) only pushes onto `PendingTunes`. The
   drain runs inside `SimSet`, so a value changes between two sim ticks and
   never during one. Outside `Playing`, the drain runs immediately (there is
   no run to record) and the values become the next run's start values.
2. **Every tunable field is the resource's real value.** Gameplay reads
   `Res<Ammo>` like any resource. No shadow copy, no `const`s. Coin Run's
   `PLAYER_SPEED`, `BULLET_SPEED`, `JUMP_STRENGTH`, `GRAVITY`, `STARTING_AMMO`
   etc. move into tunable resources.
3. **`applies: "next_run"`** marks fields read only at `OnEnter(Playing)` (a
   starting count). The game still accepts them mid-run; the card shows a
   "takes effect on the next run" chip and a Restart button.
4. **Reflection does the reading and writing.** `ReflectResource` +
   `reflect_path(field)` + `apply` from a `TunableValue`. Types are
   registered explicitly (`register_type`), because `reflect_auto_register`
   on wasm needs `load_type_registrations!` and gives no ordering anyway.
5. **Checks never panic.** On wasm32 a panic aborts the whole game. Checks
   return `Err(String)` through an `ensure!` macro; the `#[test]` wrapper in
   `checks.rs` turns `Err` into a failure. Each check builds its own
   `Scenario` with the *current* tunable values applied (`Scenario::with_tunables`).
6. **No new Bevy features, no crates outside the lockfile.** Reflection is
   already on; `serde`/`serde_json` are already in `Cargo.lock` through
   Bevy, so the host layer uses them.

### Replay format `GXR2`

```text
magic    b"GXR2"
build    u8 len + UTF-8
tick_hz  u16
seed     [u8; 32]
origin   u8
content  u64                       hash of start values + every mid-run change
ticks    u32
score    u64
checksum u64
starts   u16 count, then [id u16, bits u64]       start values (every tunable, index order)
tunes    u32 count, then [tick u32, id u16, bits u64]  mid-run changes, in apply order
runs     u32 count, then [held u16, latched u16, ax i8, ay i8, count u16]
```

- `begin_run` applies `starts`-equivalent values (the live ones), computes
  `content` over them, folds `content` into the fresh `Checksum`, and the
  recorder writes `starts` from the index.
- `apply_pending_tunes` folds each `(id, bits)` into the checksum on the tick
  it applies and appends to `tunes`; `content` is finalised at seal time over
  starts + tunes.
- The verifier (`replay::verify`) stages `starts` before entering `Playing`
  and feeds `tunes` from the feeder on their ticks. The verdict gains
  `"content": "<hex>"`. `matches` requires score, checksum **and** content to
  agree, so a replay recorded with one set of values cannot verify against
  another, and a header that lies about its values fails too.
- Leaderboard policy (website, not this session): accept a run only when
  `content` equals the release's canonical hash (the verifier exports
  `defaults_hash()`), which also rejects any run with mid-run changes.
- `tests/fixtures/selftest.gxr` and `selftest-wasm.gxr` are regenerated as
  `GXR2`. `windowed_shape.rs`, `archetype_order.rs`, `selftest.rs` keep
  running unchanged; a new `tests/tunables.rs` records a run that changes a
  value mid-run and asserts the replay verifies, and that the same inputs
  with a different value do not.

### Coin Run's first tunables

From the fixture plus a01 (coins) and d03 (ammo):

| Path | Type | Range | Source today |
|---|---|---|---|
| `player.speed` | f32 px/s | 60–600 | `things::PLAYER_SPEED` 240 |
| `player.jump_strength` | f32 px/s | 200–1200 | `player::JUMP_STRENGTH` 600 (unused until a jump exists) |
| `physics.gravity` | f32 px/s² | 300–3000 | `physics::GRAVITY` 1500 |
| `physics.max_fall` | f32 px/s | 200–2000 | `physics::MAX_FALL` 900 |
| `things.<kind>.colour` | colour | | `things::props` |
| `things.<kind>.size` | place-like w×h | 4–400 | `things::props` |
| `things.enemy.speed`, `things.bullet.speed` | f32 px/s | | `ENEMY_SPEED`, `BULLET_SPEED` |
| `lives.start` | u32 | 0–9 | `GameData::default` 3 |
| `ammo.start` | u32, next_run | 0–50 | `shooting::STARTING_AMMO` 5 |
| `ammo.fire_button` | button | A, B | hard-coded `secondary_just_pressed` |
| `coins.pickup_sound` | sound | `SfxEvent::ALL` | `Pickup` |

Checks: `coin_is_collected`, `coin_out_of_reach_stays`, `b_fires_and_uses_ammo`,
`no_ammo_no_shot`, `ammo_runs_out`, `bullets_leave_the_screen`. These are the
a01/d03 tests rewritten as non-panicking checks.

## Card schema

JSON Schema 2020-12, `$id` `https://gamebient.com/schemas/card/v1.json`,
shipped as `src/cards/schema.json` and validated with Ajv. Every card carries
`v: 1`; a card with another `v` is rejected before validation.

```jsonc
{
  "v": 1,
  "id": "card_01J…",                      // editor-assigned when missing
  "title": "Shooting uses ammo",          // ≤ 60 chars
  "readback": "Pressing B fires …",       // plain words, ≤ 400 chars
  "pin": { "kind": "rule", "name": "shooting" },   // rule | thing | counter
  "controls": [ /* 0–6 controls from the catalog */ ],
  "checks": [ "b_fires_and_uses_ammo", "no_ammo_no_shot" ],
  "changes": [ "Bullets now fly the way you face" ]   // optional "what changed" lines
}
```

Controls (discriminated on `kind`; every control has `bind`, `label`, optional
`help`):

| kind | extra fields | binds to |
|---|---|---|
| `slider` | `min`, `max`, `step?`, `unit?` | f32, i32, u32 |
| `number` | `min?`, `max?`, `step?`, `unit?` | f32, i32, u32 |
| `toggle` | `on?`, `off?` (labels) | bool |
| `colour` | `palette?` (array of `#rrggbb`) | colour |
| `choice` | `options` `[{ value, label }]` | choice |
| `button` | `options` ⊆ `["A","B"]` | button |
| `sound` | — (options come from the game) | sound |
| `place` | — (bounds come from the game) | place |
| `curve` | `presets?` | curve |

The full schema is written in milestone 3 (`src/cards/schema.json`) from this
table; `additionalProperties: false` everywhere; `bind` matches
`^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)+$`.

### Validation against the game

A card renders only when all of these hold; otherwise the chat shows a
one-line plain message ("I couldn't show the controls for this change; the
change itself is in.") and the developer log has the reason.

1. Schema-valid, `v` supported.
2. Every `bind` names a path in the live `TunableIndex`.
3. The control kind is allowed for the tunable's type (table above).
4. `min`/`max`/`step` lie inside the tunable's declared range; the card may
   narrow a range, never widen it. `step > 0`.
5. `choice`/`button` options are a subset of the tunable's options.
6. Every `checks` name exists in the game's check list.
7. No `bind` appears twice in one card; at most 6 controls.

## Control catalog

Built once in `src/controls/`, one file per control, each exporting the same
shape: `(props: { tunable, control, value, onInput, onCommit }) => JSX`.
`onInput` fires while dragging (throttled, sends `tunables.set`), `onCommit`
on release (sends `checks.run`). Native elements first; ARIA only where a
native element doesn't exist.

| Control | Element | Keyboard | Screen reader |
|---|---|---|---|
| Slider | `<input type=range>` + `<output>` | ←/→ step, PgUp/PgDn ×10, Home/End | `aria-valuetext` "600 pixels per second" |
| Number | `<input type=number inputmode=decimal>` | ↑/↓ step | invalid → `aria-invalid` + message |
| Toggle | `<button role=switch aria-checked>` | Space/Enter | on/off labels |
| Colour | `<input type=color>` + palette `role=radiogroup` + hex field | arrows in the palette | swatch names ("Coin yellow") |
| Choice | radio group ≤ 4 options, `<select>` above | arrows | option labels |
| Button picker | radio group "A (Z key)" / "B (X key)" | arrows | |
| Sound picker | `<select>` + Play button | arrows, Enter | "Play pickup" |
| Place picker | x/y number inputs **always present**, plus "Pick on stage": overlay on the preview, arrows nudge 1 px (Shift 10), Enter confirms, Esc cancels | full | live region announces coordinates |
| Curve | SVG with 4 handles each `role=slider`, plus a table of the points as number inputs, plus presets | arrows per handle | each handle labelled |

Modes: theme from `prefers-color-scheme` with a manual override; large text
(`html[data-text=large]`, root 125 %, layout in rem); reduced motion from
`prefers-reduced-motion` with an override (no card intro, no slider glow).
Focus rings are never removed. Every control has a visible label.

## The inspector

The chat is history; the inspector is the game as it is now. Three tabs:

- **Rules**: one entry per `pin` of kind `rule`, showing the latest card
  pinned there (older cards for the same pin stay in the chat).
- **Things**: one entry per tunable group of kind `thing` (Player, Coin,
  Bullet…), with an auto-generated card from the catalog (all of the group's
  tunables, default controls by type).
- **Counters**: groups of kind `counter` (lives, ammo, score).

Every tunable is reachable from the inspector even if no chat card named it.
Values shown are the game's reported values, so the chat card and the
inspector entry never disagree. Selecting a `thing` highlights it on the
stage (overlay over the iframe using the reported position of a sample
instance; display only).

## Checks

On `onCommit` the editor sends `checks.run` with the card's `checks`; the game
runs each in a fresh headless `App` inside the same wasm instance (the
verifier already proves `build_headless_app` works on wasm when stepped by
hand). A 120-tick scenario is a few milliseconds. Results update the card's
header: "✓ 3 checks pass" or "⚠ B never fires" with the check's message.
Warnings never block; they are advice.

Native CI runs the same checks through `#[test]`, so a check's meaning is
identical in the editor and in CI.

## Agent seam

`src/agent/types.ts`:

```ts
interface Agent {
  send(req: { text: string; game: GameSnapshot; history: Turn[] }): AsyncIterable<AgentEvent>;
}
type AgentEvent =
  | { kind: "text"; text: string }
  | { kind: "card"; card: unknown }            // validated by cards/validate.ts
  | { kind: "question"; question: string; options: string[] }
  | { kind: "cannot"; reason: string }
  | { kind: "rebuild"; status: "building" | "done" | "failed"; log?: string };
```

- `mock.ts`: canned replies for a01 and d03 (the sample run's readbacks as
  card text), no network.
- `tune.ts`: `POST /api/agent` → the backend calls the Claude Messages API
  with one tool, `show_card` (input schema = the card schema, plus
  `set: { "<path>": value }` for values the agent chose). The system prompt
  carries the tunable catalog and the current values; the agent is told it
  may only tune, never invent. Used for requests a control already covers
  ("make bullets faster" → `set things.bullet.speed`, card shown). The
  backend validates the card before returning it.
- `code.ts` (stub, this session): the same `Agent` interface, backed by
  `POST /api/change`, which the eval/vibe runner will implement: headless
  `claude -p` on the game checkout, gates, `tools/build_editor.sh`, then a
  `rebuild` event and the preview reloads. The runner's `READBACK:` becomes
  the card readback; the agent's `show_card` tool call becomes the card.

The API key lives in the backend (OS keychain, or `ANTHROPIC_API_KEY` in development), never in
the browser bundle.

## Frontend stack

**React 19 + TypeScript + Vite**, pnpm, no component library.

- The website is React 19; `gameHost.ts`'s reducer and parser port directly.
- Native form elements carry most of the accessibility; React's controlled
  inputs make `onInput`/`onCommit` and live regions straightforward.
- Tests: Vitest for the validator, reducer and catalog; Playwright with
  `@axe-core/playwright` for axe scans, keyboard-only walkthroughs and
  light/dark screenshots. Chromium is already cached under
  `~/Library/Caches/ms-playwright` (revisions up to 1243); the
  `@playwright/test` version is pinned to the one that uses a cached
  revision, with `PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1` in `.npmrc`/CI.

Plain TypeScript + Lit would be lighter and is a fine fallback; it is not
recommended because nothing on the project uses it.

## Repository layout (proposed `gamebient-editor`)

```
gamebient-editor/
  package.json  vite.config.ts  tsconfig.json  playwright.config.ts
  Cargo.toml                 (workspace: editor-server, editor-app)
  crates/editor-server/      axum backend: static files, /api/agent, /api/game, /api/change seam
  crates/editor-app/         Tauri 2 shell: starts the server, opens the window, keychain, installers
  src/
    main.tsx  App.tsx  store.ts
    host/      protocol.ts  useGameHost.ts  tunables.ts
    cards/     schema.json  validate.ts  Card.tsx
    controls/  Slider.tsx  Number.tsx  Toggle.tsx  Colour.tsx  Choice.tsx
               ButtonPick.tsx  SoundPick.tsx  Place.tsx  Curve.tsx  index.ts
    inspector/ Inspector.tsx
    chat/      Chat.tsx  Composer.tsx
    agent/     types.ts  mock.ts  tune.ts  code.ts
    preview/   Preview.tsx  StageOverlay.tsx
    a11y/      prefs.ts
  e2e/         axe.spec.ts  keyboard.spec.ts  screenshots.spec.ts
  docs/        (this spec, mockup, card schema notes)
```

The game under edit is a separate checkout, given as `GAME_DIR`. The backend
serves `$GAME_DIR/dist-editor/` (built by `tools/build_editor.sh` in the
game: `cargo build --features editor --target wasm32-unknown-unknown` +
`wasm-bindgen`, no `wasm-opt`, no brotli, no demo bundle).

**Coin Run** becomes a game checkout of its own: the template's `editor`
branch, `init-game.sh "Coin Run" Bread-Heads-Studios/coin-run`, the
`eval/vibe/fixture` overlay, then the a01 and d03 diffs. Assumed location
`games/coin-run` in the workspace, like every other game. The eval fixture
in `eval/vibe/` stays as it is.

## Milestones (as asked) and what proves each

| # | Deliverable | Proof |
|---|---|---|
| 1 | This spec, the mockup, the stack recommendation, the repo question | You open the mockup; you answer the repo question |
| 2 | `gamebient-input` v0.5.0 (`gx:custom`); template `editor` branch: `tunables/`, `GXR2`, checks, `build_editor.sh`; Coin Run with ammo and coins tunable | `cargo test --all-targets --all-features` green incl. `windowed_shape`, `archetype_order`, selftest fixtures; `tests/tunables.rs` mid-run change replays; `node tools/verify_fixture.mjs` on the wasm fixture; a conformance log of `tunables.list` → `tunables` |
| 3 | Editor shell: preview, inspector bound to live tunables, all nine controls | axe clean on the main screen; keyboard-only walkthrough spec passes; Playwright screenshots light/dark committed under `e2e/__screenshots__` |
| 4 | Chat with cards: mock agent (a01, d03), then the Claude tune agent | Mock: cards render, controls move the game, checks re-run, undo works. Real: "make bullets faster" moves the slider and shows a validated card; an invalid card from the model is rejected with the plain message |
| 5 | Installable app: Tauri shell around the backend and UI, key in the keychain, macOS installer (Windows/Linux in CI) | A fresh machine installs the `.dmg`, opens Coin Run, tunes a value and runs a check with no terminal |

## Not verifiable here

- A real phone and the cabinet (no device; the editor is desktop-first, the
  game preview is the same bundle the website would frame).
- Vercel deployment of an editor build (local only in this session).
- Windows and Linux installers and their webviews (macOS only here).
- Leaderboard acceptance of `GXR2` runs (website change; noted as policy).

## Owner decisions (2026-11-06)

1. Design approved. The editor lives in a new `gamebient-editor` repository.
2. The local server ships inside an installable desktop app (above).
3. Coin Run as its own repo at `games/coin-run` (assumed, not objected to).
4. `gamebient-input` v0.5.0 is a public-repo release; the editor branch pins
   it by tag once published, or by git rev until then.
