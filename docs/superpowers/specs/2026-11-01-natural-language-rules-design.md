# Natural-language rules: vocabulary v0 and eval set

**Date:** 2026-11-01 · **Status:** Draft, for discussion. Nothing here is implemented.

## Goal

Test whether a beginner can make a Gamebient game by typing rules in plain
English ("if the player touches a coin, they collect it") before building
anything. The editor sends each sentence to a model. The model translates it
into a small, fixed **rule vocabulary**, and a deterministic interpreter runs
that vocabulary inside `SimSet`. The model is only called while editing. The
shipped game, the replay verifier and the leaderboard never call it.

This spec defines:

1. **The vocabulary**: 22 verbs (9 triggers and 13 actions), 3 conditions
   and 10 thing properties.
2. **The model's output contract**: what a translation may return.
3. **The eval set** (`2026-11-01-natural-language-rules-eval.jsonl`): 51
   sentences with gold answers.
4. **How to score it**, and the bar that justifies building the editor.

## Three layers, one game

| Who | Edits | Stored as |
|---|---|---|
| Beginner | The sentence | `source` text on each rule, kept verbatim |
| Intermediate | Rule cards and thing properties | The vocabulary below |
| Expert | Rust | Eject: the vocabulary mapped to template-convention files |

The structured rules are the source of truth. The sentence is kept so the
editor can show it and re-translate on request, but the editor **never
re-translates on its own**: model output isn't reproducible, and a game must
not change when it's reopened.

## Scope of v0

These limits come from the platform, and every one of them makes the
vocabulary smaller.

- **One fixed screen** (`display.rs` size). No camera, no scrolling levels.
- **One player**, using the controls canon: move, **A** (primary/jump),
  **B** (secondary/fire). Start, Select and Pause belong to the system
  and can't be used in rules.
- **2D side view or top-down**, chosen by giving things gravity or not.
- **Global counters only.** There is no per-instance state ("each enemy
  takes 3 hits"). This is the most likely first addition. The eval set
  includes it as a refusal (`f06`) so we can measure how often people ask.

## Vocabulary v0

### Things

A **thing** is a kind (Player, Coin, Enemy…), and a level holds placed
instances of it. Kinds have properties, and the inspector edits them
directly. A sentence that describes how something *is*, rather than what
*happens*, becomes a property change, not a rule.

| Property | Type | Meaning |
|---|---|---|
| `color` | colour | Fill colour |
| `size` | w × h px | Collision and draw box |
| `speed` | px/s | Default speed for `move` / `move_toward` / `spawn … moving` |
| `control` | `none` \| `stick` \| `stick_x` | Driven by the move input. `stick_x` is a platformer, `stick` is top-down |
| `solid` | bool | Things with `gravity` or `control` can't pass through it |
| `gravity` | bool | Falls, and lands on solid things |
| `jump_strength` | px/s | Upward speed given by `jump` |
| `air_jumps` | int | Extra jumps allowed in the air (1 = double jump) |
| `bounces` | bool | Reverses direction on hitting a solid thing or the screen edge |
| `wraps` | bool | Leaving one screen edge brings it back on the opposite edge |

### Counters

A counter is a named integer with `start`, `hud` (shown or not) and
`leaderboard` (exactly one counter is the score submitted for
verification). The game file starts with `score` (leaderboard) and `lives`.

### Rules

```
rule = { when: Trigger, if: [Condition], do: [Action] }
```

**Triggers (9).** `self` and `other` are the instances involved.

| Trigger | Fields | Fires | `self` / `other` |
|---|---|---|---|
| `start` | | On the run's first tick | none |
| `every` | `ticks` | When `RunClock.ticks % ticks == 0` and `ticks > 0` | none |
| `button` | `button` (A, B, UP, DOWN, LEFT, RIGHT), `edge` (pressed, held, released) | Per `TickInput` edge | none |
| `touch` | `a`, `b` (kinds), `side` (any, top, bottom, left, right, not_top) | On the first tick an `a` overlaps a `b`. `side` is the side of `b` that `a` hit | `a` / `b` |
| `value` | `left`, `op`, `right` (Values) | On the tick the comparison becomes true | none |
| `leaves_screen` | `kind`, `edge` (any, top, bottom, left, right) | When an instance's box fully leaves the screen | the instance |
| `timer_done` | `timer` | When a timer started by `timer` reaches zero | none |
| `spawned` | `kind` | When an instance is placed at start or by `spawn` | the instance |
| `removed` | `kind` | When an instance is removed, at its last position | the instance |

**Conditions (3).** All must hold.

| Condition | Fields |
|---|---|
| `compare` | `left`, `op` (`==` `!=` `<` `<=` `>` `>=`), `right` (Values) |
| `chance` | `percent` (1–99). Drawn from `GameRng` |
| `holding` | `button` |

**Actions (13).** Each runs in list order.

| Action | Fields | Effect |
|---|---|---|
| `spawn` | `kind`, `at` (Place), optional `moving` {`dir`, `speed`} | New instance |
| `remove` | `target` | Removes it, firing `removed` |
| `add` | `counter`, `amount` (may be negative) | |
| `set` | `counter`, `value` | |
| `move` | `target`, `dir` (up, down, left, right, facing, reverse, stop), optional `speed` | Sets ongoing motion |
| `move_toward` | `target`, `toward` (Target), optional `speed` | Ongoing straight-line homing. Horizontal only for things with gravity. Never pathfinds |
| `jump` | `target`, optional `strength` | Only works when standing on something solid or with `air_jumps` left |
| `teleport` | `target`, `to` (Place) | |
| `sound` | `name` | Presentation only |
| `timer` | `name`, `ticks` | Starts or restarts a named timer |
| `end_run` | `result` (win, lose) | `sim::end_run`, which leads to `GameOver` |
| `effect` | `kind` (shake, flash, marquee), optional `ticks`, `color` | Presentation only. `marquee` writes `frame::Highlight` |
| `tune` | `thing`, `property`, `op` (set, add, multiply), `value` | Changes a kind's property during the run, for every instance |

**Targets:**

- `self` or `other`
- `Kind`: every instance of that kind
- `nearest:Kind`: the instance nearest to `self`, or to the Player when
  there is no `self`

**Places:**

- `self`, `other`, or `Kind` (a single-instance kind)
- `start`: where `self` was placed in the level
- `random`, `random_top` or `random_edge`, drawn from `GameRng`
- `{x, y}` in game pixels, with the origin at the screen centre

**Values:**

- a number
- a counter name
- `count:Kind`
- `Kind.x` or `Kind.y` (single-instance kinds)

### Determinism (why novices can't break verification)

The interpreter is one system in the `SimSet` chain. On each tick it:

- evaluates rules in rule-list order and instances in `SpawnOrder`
- draws `chance` and random places from `GameRng` in that same order
- applies spawns and removals at the end of the tick, so `other` stays
  valid until the rule finishes
- reads time only from `RunClock`. Seconds are turned into ticks (× 60,
  rounded) when a rule is translated, so the game file holds only ticks
- folds every counter (in declaration order) and every instance position
  (in `SpawnOrder`) into the `Checksum`

`sound` and `effect` are messages handled in `Update`, so they never enter
sim state.

The game file's hash goes into the replay header and the verifier's build
identity. A tweaked game is a different game.

## The model's output contract

The model must answer through one tool call, using one of three shapes.
Schema validity is enforced, so the interpreter never sees anything it
can't run.

```jsonc
// 1. A translation. `changes` may be empty ("it already does that").
{ "kind": "changes",
  "changes": [ /* operations below */ ],
  "readback": "When the Player touches a Coin: the Coin disappears and Coins goes up by 1." }

// 2. A question. One question per turn; `options` are clickable.
{ "kind": "clarify",
  "question": "Should spikes take one life or end the game?",
  "options": ["Take one life and go back to the start", "End the game"] }

// 3. An honest no.
{ "kind": "cannot",
  "reason": "Rules can't find a path around platforms yet.",
  "nearest": [ /* optional changes that get partway */ ],
  "eject": true }
```

Operations:

- `add_rule` {`id`?, `rule`, `source`}
- `edit_rule` {`id`, `rule`}
- `remove_rule` {`id`}
- `set_property` {`thing`, `property`, `value`}
- `create_thing` {`name`, `properties`}
- `create_counter` {`name`, `start`}
- `set_counter` {`counter`, `field`, `value`}

The model's context is the whole game file: every thing, property, counter
and rule, with their ids. It needs that to resolve names, edit rules
instead of duplicating them, and notice conflicts.

The **readback** is the beginner-facing part. It must:

- use the game's own names and plain words
- name every number the model chose for itself ("10 ammo", "jump strength
  800")
- name every assumption it made ("I took 'collect' to mean count it")
- warn about conflicts with existing rules

## Eval set

`2026-11-01-natural-language-rules-eval.jsonl` has one case per line. Every
case starts from the same fixture scene. Context cases add rules on top of
it with `scene_extra`.

**Fixture: "Coin Run"**, a 4:3 side-view platformer, 960 × 720.

| Thing | Properties |
|---|---|
| Player | `control: stick_x`, `speed: 240`, `gravity: true`, `jump_strength: 600`, `air_jumps: 0` |
| Platform | `solid: true`, `gravity: false` |
| Coin | `gravity: true` (the editor's default for new things) |
| Enemy | `speed: 120`, `gravity: true` |
| Bullet | `speed: 600`, `gravity: false` |
| Goal, Spike | `gravity: false` |

- **Counters:** `score` (leaderboard, start 0), `lives` (start 3),
  `coins` (start 0).
- **Sounds:** `pickup`, `hit`, `jump`, `shoot`, `win`, `lose`.
- **Rules:** none.

**Categories:**

| Code | Category | Count | Expected outcome |
|---|---|---|---|
| a | Clear single rules, including canon and typo cases | 17 | `changes` |
| b | Numbers, units and time | 6 | `changes` |
| c | Properties, not rules | 5 | `changes` (`set_property`) |
| d | Several actions or rules in one sentence | 4 | `changes` |
| e | Ambiguous | 6 | `clarify` |
| f | Outside the vocabulary | 7 | `cannot` |
| g | Depends on existing rules or names | 6 | mixed |

**Case fields:**

- `id`, `category`, `text`
- `scene_extra` (optional)
- `expect`: the response kind
- `gold`: changes, or a question's required options
- `accept`: listed acceptable alternatives
- `optional`: presentation-only actions that may be added or left out
- `must`: readback or behaviour checks a human grader ticks

## Scoring

A case **passes** when the response kind matches `expect` and either:

- the changes equal `gold` or one of the `accept` alternatives after
  normalising, or
- for `clarify` and `cannot`, the response meets every `must` line.

**Normalising:**

- `touch(a, b)` equals `touch(b, a)` with `self` and `other` swapped
- leaving out a field equals giving its default
- actions in a `do` list are compared as a set, except two `move`s on the
  same target
- numbers must match exactly, unless a `must` line gives a range

**Grading rules:**

- **Unrequested sim changes fail the case.** Adding score nobody asked for
  is changing the game. Adding presentation (`sound`, `effect`) is fine
  when it's listed in `optional`.
- **Every passing `changes` case also needs a readback** that the writer
  would agree with. Grade it 1/0 by a person, against `must`.

**Metrics and the bar to build the editor:**

| Metric | Over | Bar |
|---|---|---|
| Intent match | a, b, c, d, and the `changes` cases in g | ≥ 90% |
| Asks when it should | e and the `clarify` cases in g | ≥ 80% |
| Asks when it shouldn't (`clarify` on a clear case) | a–d | ≤ 10% |
| Honest refusal | f | ≥ 90% |
| Fakes it (returns `changes` that don't do what was asked, for an f case) | f | ≤ 5% |
| Schema-valid | all | 100% (enforced by the tool schema) |

Run each case 5 times with sampling on, and report the pass rate per case
as well as the mean. A rule that translates correctly 3 times in 5 is a
bad editor experience, even if the average looks fine.

## Known weaknesses of this eval

- **Claude wrote these sentences, not beginners.** A model-written test set
  makes models look good: the phrasing is too tidy, and the hard cases
  are the ones a model would think of. Before deciding anything, replace
  at least half the set with sentences from real first-time users, for
  example by asking playtesters to describe a game they'd like to make,
  one rule per line, with no examples shown.
- **One fixture game** biases the set towards platformer verbs. Add a
  top-down shooter fixture next.
- **Readback grading is subjective.** Use two graders, and keep a case
  only where they agree.
- **Single-turn only.** Multi-turn flows (answer a `clarify`, then
  translate) need their own cases once the single-turn bar is met.

## Next steps

1. Gather real beginner sentences. Re-balance the set.
2. Write the tool schema from this spec. Run the eval through a small
   script: the game file as context, one sentence, one tool call.
3. Write an automatic structural grader for the `gold` and `accept`
   comparison. Readback grading stays human.
4. Only if the bar is met: build the interpreter as a `SimSet` system
   behind an `editor` feature, with its own replay fixture.
