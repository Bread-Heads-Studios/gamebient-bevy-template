//! Runs one data check (`model::DataCheck`) on a fresh headless stage
//! (`game::scenario::Scenario`) with a fixed seed ([`SEED`]), under the
//! game's current rules, tunables and level ([`CheckContext`]), and answers
//! `Ok(())` or a plain-words `Err`. It never panics: everything it is
//! handed was checked by `model::check_file` against this game's names,
//! and a name the game lost reads as nothing there. The same code runs in
//! the browser (`checks.run`) and in `cargo test` (`data_checks_pass`).
//!
//! Order: the stage is built (the level, then `player.at`, then each
//! `place` in order), the run starts, `set` applies, and the start values
//! are noted. Each step then runs tick by tick; `ever` expectations are
//! tested after the start and after every tick. Once the run ends (won or
//! lost) the remaining steps are skipped and the expectations read the
//! state on the tick it ended.

use bevy::prelude::*;

use super::model::{Btn, DataCheck, End, Expect, Level, Op, Pos, Side, Step, Want, Where, Y};
use crate::game::content::level::Level as LevelRes;
use crate::game::content::model::LevelFile;
use crate::game::rule_table::{COUNTERS, STAGE};
use crate::game::rules::model::RulesFile;
use crate::game::rules::{Outcome, Rules};
use crate::game::scenario::{Buttons, Scenario, StageDef};
use crate::game::tunables::{TunableIndex, Tunes, snapshot};

/// The seed every data check runs with, so `chance` gives the same result
/// every time.
pub const SEED: u64 = 7;

pub const NO_PLAYER: &str = "There is no player to place a thing beside.";

/// What a check runs under: the game's current tunables (path and value),
/// rules and saved level.
#[derive(Clone, Debug)]
pub struct CheckContext {
    pub tunes: Tunes,
    pub rules: RulesFile,
    pub level: LevelFile,
}

/// The context `world` runs now: its tunable values, its `Rules` (the
/// next run's) and its content `Level`.
pub fn context(world: &World) -> CheckContext {
    let idx = world.resource::<TunableIndex>();
    let tunes = snapshot(world)
        .into_iter()
        .filter_map(|(id, v)| idx.by_id(id).map(|e| (e.path.clone(), v)))
        .collect();
    CheckContext {
        tunes,
        rules: world
            .get_resource::<Rules>()
            .map(|r| r.0.clone())
            .unwrap_or_default(),
        level: world
            .get_resource::<LevelRes>()
            .map(|l| l.0.clone())
            .unwrap_or(LevelFile {
                v: 1,
                things: Vec::new(),
            }),
    }
}

/// Seconds as a check's words show them: whole, or with one decimal (at
/// least 0.1). 120 ticks are "2", 90 are "1.5".
pub fn seconds(ticks: u32) -> String {
    let tenths = (f64::from(ticks) / 6.0).round().max(1.0) as u32;
    if tenths.is_multiple_of(10) {
        format!("{}", tenths / 10)
    } else {
        format!("{}.{}", tenths / 10, tenths % 10)
    }
}

/// Where `at` is for a Thing `h` tall: `"floor"` rests it on the floor.
pub fn point(at: Pos, h: f32, stage: &StageDef) -> Vec2 {
    let y = match at.y {
        Y::At(y) => y as f32,
        Y::Floor => stage.floor_top + h / 2.0,
    };
    Vec2::new(at.x as f32, y)
}

/// Where `beside` puts a Thing of `size` next to the player at `player`:
/// `stage.beside` away, centre to centre. Left and right rest it on the
/// floor; above and below share the player's x.
pub fn beside_point(player: Vec2, side: Side, size: Vec2, stage: &StageDef) -> Vec2 {
    let rest = stage.floor_top + size.y / 2.0;
    match side {
        Side::Left => Vec2::new(player.x - stage.beside, rest),
        Side::Right => Vec2::new(player.x + stage.beside, rest),
        Side::Above => Vec2::new(player.x, player.y + stage.beside),
        Side::Below => Vec2::new(player.x, player.y - stage.beside),
    }
}

fn buttons(list: &[Btn]) -> Buttons {
    list.iter().fold(Buttons::NONE, |all, b| {
        all.union(match b {
            Btn::Left => Buttons::LEFT,
            Btn::Right => Buttons::RIGHT,
            Btn::Up => Buttons::UP,
            Btn::Down => Buttons::DOWN,
            Btn::A => Buttons::A,
            Btn::B => Buttons::B,
        })
    })
}

fn label(name: &str) -> &str {
    COUNTERS
        .iter()
        .find(|c| c.name == name)
        .map_or(name, |c| c.label)
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

/// "is at most": a comparison as a statement.
fn is_op(op: Op) -> &'static str {
    match op {
        Op::Lt => "is below",
        Op::Le => "is at most",
        Op::Eq => "is",
        Op::Ge => "is at least",
        Op::Gt => "is above",
    }
}

/// "be at most": a comparison after "expected it to".
fn be_op(op: Op) -> &'static str {
    match op {
        Op::Lt => "be below",
        Op::Le => "be at most",
        Op::Eq => "be",
        Op::Ge => "be at least",
        Op::Gt => "be above",
    }
}

/// "at most ": a comparison after "was never" (nothing for "=").
fn bare_op(op: Op) -> &'static str {
    match op {
        Op::Lt => "below ",
        Op::Le => "at most ",
        Op::Eq => "",
        Op::Ge => "at least ",
        Op::Gt => "above ",
    }
}

/// "a change of -1", "a change of at least 2".
fn a_change(op: Op, n: i64) -> String {
    match op {
        Op::Lt => format!("a change below {n}"),
        Op::Le => format!("a change of at most {n}"),
        Op::Eq => format!("a change of {n}"),
        Op::Ge => format!("a change of at least {n}"),
        Op::Gt => format!("a change above {n}"),
    }
}

/// What an `until` waits for, as a phrase: "lives going down by 1",
/// "winning", "the pickup sound".
pub fn until_words(w: &Want) -> String {
    match w {
        Want::Counter(c, op, n) => format!("{} {} {n}", label(c), is_op(*op)),
        Want::Change(c, Op::Eq, n) if *n < 0 => format!("{} going down by {}", label(c), -n),
        Want::Change(c, Op::Eq, n) if *n > 0 => format!("{} going up by {n}", label(c)),
        Want::Change(c, Op::Eq, _) => format!("{} staying the same", label(c)),
        Want::Change(c, op, n) => format!("the change in {} {} {n}", label(c), is_op(*op)),
        Want::Count(k, op, n) => format!("the {k} count {} {n}", is_op(*op)),
        Want::Outcome(End::Win) => "winning".into(),
        Want::Outcome(End::Lose) => "losing".into(),
        Want::Outcome(End::Playing) => "the run going on".into(),
        Want::Sound(s) => format!("the {s} sound"),
    }
}

/// Why the outcome is not `want`: "didn't win: the run is still going".
fn outcome_message(want: End, now: End) -> String {
    match (want, now) {
        (End::Win, End::Playing) => "didn't win: the run is still going".into(),
        (End::Win, End::Lose) => "didn't win: the run was lost".into(),
        (End::Lose, End::Playing) => "didn't lose: the run is still going".into(),
        (End::Lose, End::Win) => "didn't lose: the run was won".into(),
        (End::Playing, End::Win) => "the run ended: it was won".into(),
        (End::Playing, End::Lose) => "the run ended: it was lost".into(),
        _ => String::new(),
    }
}

struct Run<'a> {
    s: Scenario,
    /// Every counter's value once the run started and `set` applied.
    start: Vec<(String, i64)>,
    expect: &'a [Expect],
    /// Which `ever` expectations have held at some tick.
    seen: Vec<bool>,
}

impl Run<'_> {
    fn outcome(&self) -> End {
        match self.s.outcome() {
            Some(Outcome::Win) => End::Win,
            Some(Outcome::Lose) => End::Lose,
            None if self.s.run_over() => End::Lose,
            None => End::Playing,
        }
    }

    fn started(&self, c: &str) -> i64 {
        self.start
            .iter()
            .find(|(n, _)| n == c)
            .map_or(0, |(_, v)| *v)
    }

    fn holds(&mut self, w: &Want) -> bool {
        match w {
            Want::Counter(c, op, n) => self.s.counter(c).is_some_and(|v| op.holds(v, *n)),
            Want::Change(c, op, n) => {
                let a = self.started(c);
                self.s.counter(c).is_some_and(|b| op.holds(b - a, *n))
            }
            Want::Count(k, op, n) => op.holds(self.s.count(k) as i64, *n),
            Want::Outcome(e) => self.outcome() == *e,
            Want::Sound(s) => self.s.heard(s),
        }
    }

    /// Notes every `ever` expectation that holds now.
    fn observe(&mut self) {
        let expect = self.expect;
        for (i, e) in expect.iter().enumerate() {
            if e.ever && !self.seen[i] && self.holds(&e.want) {
                self.seen[i] = true;
            }
        }
    }

    /// `n` ticks holding `held`, stopping early if the run ends.
    fn ticks(&mut self, held: Buttons, n: u32) {
        for _ in 0..n {
            if self.s.run_over() {
                return;
            }
            self.s.step(held);
            self.observe();
        }
    }

    /// Ticks holding `held` until `e` holds, at most `within` ticks.
    fn until(&mut self, e: &Expect, within: u32, held: Buttons) -> Result<(), String> {
        let mut waited = 0;
        loop {
            if self.holds(&e.want) {
                return Ok(());
            }
            let what = capital(&until_words(&e.want));
            if self.s.run_over() {
                let how = if self.outcome() == End::Win {
                    "won"
                } else {
                    "lost"
                };
                return Err(format!("{what} didn't happen: the run was {how} first"));
            }
            if waited >= within {
                return Err(format!("{what} didn't happen within {} s", seconds(within)));
            }
            self.s.step(held);
            self.observe();
            waited += 1;
        }
    }

    /// Why `e` does not hold at the end, or `None` when it does.
    fn fails(&mut self, e: &Expect) -> Option<String> {
        if self.holds(&e.want) {
            return None;
        }
        Some(match &e.want {
            Want::Counter(c, op, n) => match self.s.counter(c) {
                Some(v) => format!("{} is {v}; expected it to {} {n}", label(c), be_op(*op)),
                None => super::model::no_counter(c),
            },
            Want::Change(c, op, n) => match self.s.counter(c) {
                Some(b) => format!(
                    "{} went from {} to {b}; expected {}",
                    label(c),
                    self.started(c),
                    a_change(*op, *n)
                ),
                None => super::model::no_counter(c),
            },
            Want::Count(k, op, n) => format!(
                "the {k} count is {}; expected it to {} {n}",
                self.s.count(k),
                be_op(*op)
            ),
            Want::Outcome(want) => outcome_message(*want, self.outcome()),
            Want::Sound(s) => format!("the {s} sound never played"),
        })
    }

    /// Why an `ever` expectation never held.
    fn never(&mut self, e: &Expect) -> String {
        match &e.want {
            Want::Counter(c, op, n) => match self.s.counter(c) {
                Some(v) => format!(
                    "{} was never {}{n} (it ended at {v})",
                    label(c),
                    bare_op(*op)
                ),
                None => super::model::no_counter(c),
            },
            Want::Change(c, op, n) => match self.s.counter(c) {
                Some(b) => format!(
                    "{} never had {} (it went from {} to {b})",
                    label(c),
                    a_change(*op, *n),
                    self.started(c)
                ),
                None => super::model::no_counter(c),
            },
            Want::Count(k, op, n) => format!(
                "the {k} count was never {}{n} (it ended at {})",
                bare_op(*op),
                self.s.count(k)
            ),
            Want::Outcome(want) => outcome_message(*want, self.outcome()),
            Want::Sound(s) => format!("the {s} sound never played"),
        }
    }
}

/// Runs `c` under `ctx`. `Ok(())` when every expectation holds; else the
/// first failing one's plain words.
pub fn run_check(c: &DataCheck, ctx: &CheckContext) -> Result<(), String> {
    let mut s = match c.given.level {
        Level::Empty => Scenario::new(),
        Level::Current => Scenario::with_level(&ctx.level),
    };
    s.seed(SEED).rules(ctx.rules.clone());
    for (path, v) in &ctx.tunes {
        s.tune(path, v.clone());
    }
    if let Some(at) = c.given.player {
        let h = s.size_of(STAGE.player).map_or(0.0, |z| z.y);
        let p = point(at, h, &STAGE);
        if !s.put(STAGE.player, p.x, p.y) {
            s.place(STAGE.player, p.x, p.y);
        }
    }
    let me = s.placed_pos(STAGE.player);
    for t in &c.given.place {
        let size = s.size_of(&t.kind).unwrap_or(Vec2::ZERO);
        let at = match t.at {
            Where::At(pos) => point(pos, size.y, &STAGE),
            Where::Beside(side) => {
                let Some(p) = me else {
                    return Err(NO_PLAYER.into());
                };
                beside_point(p, side, size, &STAGE)
            }
        };
        s.place(&t.kind, at.x, at.y);
    }
    s.start();
    for (name, n) in &c.given.set {
        s.set_counter(name, *n);
    }
    let start = COUNTERS
        .iter()
        .filter_map(|k| Some((k.name.to_string(), s.counter(k.name)?)))
        .collect();
    let mut run = Run {
        s,
        start,
        expect: &c.expect,
        seen: vec![false; c.expect.len()],
    };
    run.observe();
    for step in &c.steps {
        if run.s.run_over() {
            break;
        }
        match step {
            Step::Hold(b, n) => run.ticks(buttons(b), *n),
            Step::Press(b) => {
                run.s.press(buttons(&[*b]));
                run.observe();
            }
            Step::Wait(n) => run.ticks(Buttons::NONE, *n),
            Step::Until(e, within, held) => run.until(e, *within, buttons(held))?,
        }
    }
    for (i, e) in c.expect.iter().enumerate() {
        if e.ever {
            if !run.seen[i] {
                return Err(run.never(e));
            }
        } else if let Some(m) = run.fails(e) {
            return Err(m);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const STAGE_T: StageDef = StageDef {
        player: "P",
        floor: None,
        floor_top: -10.0,
        start: Vec2::new(0.0, 0.0),
        beside: 5.0,
    };

    #[test]
    fn seconds_are_whole_or_one_decimal() {
        assert_eq!(seconds(120), "2");
        assert_eq!(seconds(60), "1");
        assert_eq!(seconds(90), "1.5");
        assert_eq!(seconds(100), "1.7");
        assert_eq!(seconds(1), "0.1");
        assert_eq!(seconds(1800), "30");
    }

    #[test]
    fn beside_is_the_stage_distance_and_left_and_right_rest_on_the_floor() {
        let p = Vec2::new(3.0, 4.0);
        let size = Vec2::new(2.0, 6.0);
        assert_eq!(
            beside_point(p, Side::Right, size, &STAGE_T),
            Vec2::new(8.0, -7.0)
        );
        assert_eq!(
            beside_point(p, Side::Left, size, &STAGE_T),
            Vec2::new(-2.0, -7.0)
        );
        assert_eq!(
            beside_point(p, Side::Above, size, &STAGE_T),
            Vec2::new(3.0, 9.0)
        );
        assert_eq!(
            beside_point(p, Side::Below, size, &STAGE_T),
            Vec2::new(3.0, -1.0)
        );
    }

    #[test]
    fn a_floor_position_rests_on_the_floor() {
        let floor = Pos { x: 7, y: Y::Floor };
        assert_eq!(point(floor, 4.0, &STAGE_T), Vec2::new(7.0, -8.0));
        let at = Pos { x: -1, y: Y::At(2) };
        assert_eq!(point(at, 4.0, &STAGE_T), Vec2::new(-1.0, 2.0));
    }

    #[test]
    fn until_words_read_as_plain_phrases() {
        let c = COUNTERS[0].name.to_string();
        let l = COUNTERS[0].label;
        assert_eq!(
            until_words(&Want::Change(c.clone(), Op::Eq, -1)),
            format!("{l} going down by 1")
        );
        assert_eq!(
            until_words(&Want::Change(c.clone(), Op::Eq, 2)),
            format!("{l} going up by 2")
        );
        assert_eq!(
            until_words(&Want::Change(c.clone(), Op::Ge, 2)),
            format!("the change in {l} is at least 2")
        );
        assert_eq!(
            until_words(&Want::Counter(c, Op::Le, 0)),
            format!("{l} is at most 0")
        );
        assert_eq!(until_words(&Want::Outcome(End::Win)), "winning");
        assert_eq!(until_words(&Want::Sound("hit".into())), "the hit sound");
        assert_eq!(capital("winning"), "Winning");
    }

    #[test]
    fn outcome_messages_say_what_happened() {
        assert_eq!(
            outcome_message(End::Win, End::Playing),
            "didn't win: the run is still going"
        );
        assert_eq!(
            outcome_message(End::Lose, End::Win),
            "didn't lose: the run was won"
        );
        assert_eq!(
            outcome_message(End::Playing, End::Lose),
            "the run ended: it was lost"
        );
    }
}
