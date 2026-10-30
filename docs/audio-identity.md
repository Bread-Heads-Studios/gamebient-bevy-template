# Audio identity: Gamebient template

Every game fills in this form (see the fleet spec,
`docs/plans/2026-10-30-audio-identity-pass.md` in the workspace). The
template's own values are a reference pair, deliberately plain, that a port
replaces entirely.

| | |
|---|---|
| **Engine** | House chip (SN76489 model, `chip.rs`) via `tracker.rs` |
| **Key** | C major |
| **Tempo** | Menu 100 BPM · Play 128 BPM · Stings 100 BPM |
| **Voices** | `LEAD` singing square · `MAJ`/`MIN` fast-arp chords · `BASS` round square · `PLUCK` short square · `KICK` periodic noise · `HAT` white Div512 · `SNARE` white Div1024 |

## Cues

| State | Cue | Notes |
|---|---|---|
| StudioLogo | house sting (shared) | D major loaf-rising figure, 1.4 s. Same bytes in every game. |
| Menu, HowToPlay | `MENU` loop, 8 bars | "Title Card": lead over I–vi–IV–V, brushed hats. HowToPlay continues it. |
| Playing | `PLAY` loop, 16 bars, A A B A | "Run Loop": eighth-note bass, chord stabs. |
| Playing layer 0 | `PLAY_LAYER` | Plucked counter-line and driving hats; `LayerEvent::Start(0)` when a game's danger state begins. |
| GameOver | `game_over_sting()` | One bar descending G–E–D–C, 1.5 s tail, silence after. |
| DemoEnd | `demo_end_tag()` | The menu theme's first phrase, then silence. |

## SFX voice

| Event | Sound | Sting? |
|---|---|---|
| Confirm | C5 then G5 pluck (rising fifth) | no |
| Pause | G4 then C4 pluck (falling fifth) | no |
| ScreenSweep | C4→C6 glide with a white-noise shimmer | no |

## Mix

Loops peak −12 dBFS, stings and SFX −3 dBFS, then `MASTER_GAIN` 0.5. Stings
duck music to half for their length. No sound longer than 6 s except loops.

## Silence policy

None: every state has a cue. A game that chooses silence somewhere writes
the reason here.

## Sources and licences

All audio is generated in-process; no files ship.
