# Gamebient Game: game brief

One page that says what this game is. Fill it in before the first gameplay
code and keep it current; it is the source the cover, the store description
and `assets/info.json` are written from (the `designing-cartridge-covers`
and `generating-cartridge-metadata` skills read it). Replace every `TBD`;
keep each field to the length given.

Status: TBD (draft / building / shipped)

## Pitch

TBD. One excited sentence a player would repeat to a friend.

## Core loop

TBD. The two to four steps the player repeats, as verbs:
1. ...
2. ...

## Goal and fail state

- **Goal:** TBD. What the player is trying to do; what the score counts.
- **Fail state:** TBD. How a run ends. (Higher or lower score is better?
  That is `HIGHER_SCORE_IS_BETTER`.)

## Controls

TBD. One line on the Gamebient canon (stick, A, B, START); see "Controls" in
`docs/conventions.md`.

## MVP

The short list that makes it this game. Seven items at most: if it is
longer, move the ones the core loop works without to "Out of scope".

1. TBD

## Out of scope

TBD. What this game deliberately does not build (cut MVP items go here).

## Build order

TBD. Riskiest or least proven part first, so a bad idea fails early.

## Demo cut

TBD. Where the demo bundle stops, in the game's own unit ("after level 2").
This becomes `DEMO_CAP` in `src/game/demo.rs`.

## Voice

- **Who it is for and what they feel:** TBD (one line).
- **Mood in three words:** TBD (the cover and title use these).
- **Art and audio direction:** TBD (one line).
- **Reference game:** TBD. The closest shipped game, and the one part of it
  this game keeps.
