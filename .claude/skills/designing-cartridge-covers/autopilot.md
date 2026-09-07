# Customizing the autopilot harness

`src/game/autopilot.rs` (`--features autopilot`) plays the game through the
real input path and saves screenshots at fixed beats. The template version
already walks logo → title → how-to-play → run → pause → game over; three
functions at the bottom of the file are game-specific.

## The beat contract

| File | Moment |
|------|--------|
| `01-studio-logo` … `03-how-to-play` | the boot screens, before any input |
| `04-early-play` | ~3 s into the run |
| `05-mid-play` | ~30 s in, the bot mid-action |
| `06-<moment>` | the signature moment: the payoff event players remember (a delivery, a K.O., a combo banner, a level clear). Rename `SIGNATURE_BEAT`. |
| `07-late-play` | ~48 s in |
| `08-pause` | the pause overlay over live play |
| `09-game-over` | the real game-over screen |

Whole tour ≤ ~90 s; the app exits with `AppExit::Success`. Captures are
1280x720 by default and 1920x1080 with `AUTOPILOT_SCALE=1.5`
(`make-cartridge.sh` sets this).

## What to replace

1. **`drive_bot`** — read the world (nearest target, hazards, timers) and set
   `virt.set_held(...)` for directions, `tap(&mut virt, Buttons::A)` for
   presses, `virt.axis` for analog steering. Play well enough that the HUD
   shows a score and things are happening in shots 05 and 07. If the game
   already has a playtest bot (`harness`, `playtest`, `--sim`), move its
   policy into a plain module both can call rather than duplicating it.
2. **`signature_moment_ready`** — return true when the payoff is on screen
   (an event fired this frame, a banner entity exists, a counter advanced).
   Add a short delay if the banner animates in.
3. **`force_game_over`** — trip the game's real losing condition (zero lives,
   expire the clock, drain health) so the real game-over systems run. Only
   request the state directly if the game has no losing path.

## Input notes

- Write `VirtualInput` in `PreUpdate` **before**
  `gamebient_input::input::collect_input` (already ordered in the plugin), so
  the frame's snapshot carries the injected buttons like a touch-overlay tap.
- A held direction that must read as an *edge* (menus, lane presses) should be
  held for ~0.15 s then released, or latched via `tap`.
- Games that do not use gamebient-input: inject `ButtonInput<KeyCode>`
  presses upstream of the game's own collector instead.
- Use `Time<Real>` for the script clock if the game slows or pauses virtual
  time on death.
- Never tap Start/B while paused in games where that quits to the title.

## Keep it out of shipping builds

All references live under `#[cfg(feature = "autopilot")]`. CI runs clippy with
`--all-features`, so the harness must be lint-clean; `cargo check` with default
features must still build without it.
