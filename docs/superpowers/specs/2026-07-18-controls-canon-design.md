# Gamebient Controls Canon + Cross-Repo Controls Pass

**Date:** 2026-07-18
**Status:** Approved
**Scope:** all six game repos (template, voidrunner, Hunted, pizza-pinball, BeerPong, Gravestone_Gauntlet) + ColecoVisionGXWebsite
**Builds on:** the presentation-kit rollout (spec `2026-07-10-presentation-kit-rollout-design.md`); all game work stacks on each repo's `feat/presentation-polish` branch.

## Goal

One control vocabulary across every game and every device — physical keyboard,
gamepad (including the 2-button DragonRise kiosk cabinet), and the website's
virtual/mobile controller — so any Gamebient game is immediately playable on
any surface, with identical system conventions (confirm, pause, quit).

## Why one fixed vocabulary

The website's virtual controller (`ColecoVisionGXWebsite/src/app/demo/[address]/MobileControls.tsx`)
is a fixed 9-button pad that injects synthetic key events into the game iframe
via `postMessage {type:"keyEvent", key, code, eventType}`:
D-pad→Arrow keys, **A→KeyZ**, **B→KeyX**, **Start→Enter**, Select→ShiftLeft,
Pause→Escape. The kiosk cabinet has a joystick and two action buttons. Neither
surface can adapt per game, so the games conform to one canon. A game is
mobile-compatible exactly when all of its actions are reachable through those
nine keys.

## The canon

NES-style semantics: **A = primary/jump, B = secondary/fire.**

| Logical action | Keyboard | Gamepad | Virtual pad / cabinet |
|---|---|---|---|
| Move / aim | Arrows AND WASD | Left stick AND D-pad | D-pad |
| **A** (primary) | **Z** (+ Space as gameplay alias) | **South OR North** (DragonRise pairing) | A |
| **B** (secondary) | **X** (+ Shift as gameplay alias) | **West OR East** (DragonRise pairing) | B |
| Confirm (menus / how-to-play / game over) | Enter, Space, Z | any face button | Start or A |
| Pause | Escape | Start | Pause |
| Quit (only while paused) | Enter | East | Start |

Rules:
1. Every gameplay action must be reachable in all three columns.
2. Gameplay systems read input ONLY through a `GameInput` resource (see below).
3. Menu/pause/how-to-play input stays in the presentation-kit systems but uses
   the same key/button sets as the canon rows above.
4. Keyboard aliases beyond the canon (legacy keys) may be kept per game; they
   must never be the ONLY binding for an action.
5. All user-visible control text (title-screen controls line, how-to-play
   blurbs, pause hints) must state canon bindings, ASCII only.

## `GameInput` unification

Canonical resource (template teaching code, `src/game/input.rs`):

```rust
#[derive(Resource, Default)]
pub struct GameInput {
    pub move_x: f32,                  // -1..1  (keyboard/D-pad digital, stick analog)
    pub move_y: f32,                  // -1..1
    pub primary_just_pressed: bool,   // A
    pub primary_held: bool,
    pub primary_just_released: bool,
    pub secondary_just_pressed: bool, // B
    pub secondary_held: bool,
    pub confirm_just_pressed: bool,
    pub pause_just_pressed: bool,
}
```

One `collect_input` system (First/PreUpdate) populates it from keyboard +
gamepads per the canon table (stick deadzone 0.2, digital sources sum then
clamp). Pure decision helpers where practical, unit-tested.

- **Template, voidrunner, pizza-pinball**: adopt this resource verbatim
  (new module; gameplay systems converted from raw reads).
- **Hunted, Gravestone_Gauntlet, BeerPong**: keep their existing `GameInput`
  structs and field names (same architecture already); only bindings inside
  their collect systems change where the per-game fit below requires.

## Per-game fit

| Game | Move | A (Z / Space · South/North) | B (X / Shift · West/East) | Changes required |
|---|---|---|---|---|
| **template** | 4-way (player example) | documented, unused | documented, unused | new `GameInput` + stick/D-pad movement + conventions "Controls" section (the canon table above) |
| **voidrunner** | 4-way ship | shoot | shoot (same action) | adopt `GameInput`; add North/West/East/X so both cabinet buttons shoot |
| **Hunted** | turn/forward | sprint (already Z · S/N) | fire/interact (already X · W/E) | none to bindings — already canonical; doc text only |
| **Gravestone_Gauntlet** | left/right | **flip** (Z NEW + legacy Space/W/↑ · S/N) | **fire** (X NEW + Shift · W/E) | **Z changes meaning fire→flip** (flag in PR); X becomes fire |
| **pizza-pinball** | (D-pad only for plunger) | **left flipper** (Z + ← · S/N + LeftTrigger) | **right flipper** (X + → · W/E + RightTrigger) | full gamepad support (new); **launch = hold ↓ (D-pad/stick down) or Space, release to fire**; `/`+Shifts stay as legacy aliases; **delete the bespoke touch overlay** from index.html |
| **BeerPong** | aim: X=angle, Y=pitch (stick/arrows/WASD) | **throw: hold to charge, release to throw** (replaces Q/E power) | unused | charge-throw mechanic: power meter fills while A held (visible on the existing aim UI), release throws at current power; Q/E removed; how-to-play + HUD text updated |

pizza-pinball note: gameplay ignores D-pad left/right (flippers are A/B); the
launch-on-↓ gesture reads as a plunger pull and is held/released, matching the
existing Space charge semantics. Shoulder buttons (LeftTrigger/RightTrigger,
i.e. L1/R1) are standard pinball aliases on full gamepads.

BeerPong charge-throw: pressing A begins charge from 0, power ramps to max
over ~1.2 s and holds (no oscillation), release throws; a short grace
(<80 ms) tap throws at a sensible minimum. Tuning values live as constants
with the existing aim constants. The old power display becomes the live
charge meter.

## Website changes (branch off current `main`)

1. **Cabinet gamepad relay** (`src/lib/gamepad.ts`, used by `/cabinet`):
   - ADD: Start button → `Enter` (button index **verified on the physical
     cabinet at implementation time** — standard pads report 9; DragonRise
     boards vary; do not assume). The user is adding a physical Start button.
   - ADD: buttons 2 and 3 relay `KeyX` (B) so a second action button reaches
     games regardless of encoder enumeration; existing mappings
     (axes→arrows, 0→Space, 1→KeyZ, 4→Escape, 12 reserved) unchanged.
2. **DesktopControls** reference component: verify labels match the canon
   (A=Z, B=X, Enter=Start, Esc=Pause — believed already correct; fix if not).
3. No virtual-pad redesign; no per-game layouts. Select stays as-is (sends
   Shift; games treat Shift only as a B-alias).

## Out of scope

- Physical cabinet wiring itself (user is adding the Start button; relay index
  gets verified then).
- Select-button semantics (nothing uses it; revisit if a game earns it).
- Any input remapping UI, touch redesigns, or per-game virtual layouts.

## Delivery

- Game repos: branch `feat/controls-pass` **based on `feat/presentation-polish`**,
  one stacked PR per repo (base = the presentation branch; retarget after those
  merge). BeerPong stays local.
- Website: branch `feat/controls-pass` off `main`, own PR.
- Template lands first (canonical `GameInput` + conventions doc), then the
  five games in parallel, website independent.

## Error handling / invariants

- `GameInput` is reset/collected every frame; no stale just-pressed flags
  across state transitions (collect runs unconditionally; consumers are
  state-gated — same pattern as Hunted today).
- Charge states (pizza launch, BeerPong throw) must reset on pause-quit and
  run restart (extend the existing per-run reset systems).
- The keyEvent bridge in each game's index.html is untouched (it already
  delivers the virtual pad's keys).

## Testing

- Unit: canon decision helpers (digital+analog merge, charge ramp math,
  just-released edge through pause — extending the pause-safe release test
  pizza-pinball already has).
- Runtime (headless Chrome, extends the existing driver): per game, drive the
  full flow **plus input probes through the postMessage keyEvent protocol**
  (the exact path mobile uses): e.g. pizza — hold ↓ then release, assert ball
  launches (screenshot delta); BeerPong — hold Z, screenshot charge meter,
  release, assert throw; GG — Z flips, X fires. Gates: cargo test / clippy
  -D warnings / fmt per repo; website `pnpm lint` + build.
