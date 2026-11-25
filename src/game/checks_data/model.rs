//! The checks file (`content/checks.json`): plain-English checks as data,
//! their typed form and the one checker every side runs. The game
//! (`checks_data::set_checks`), the editor's model (`src/checks/model.ts`)
//! and the editor's server (`crates/editor-server/src/checks/model.rs`, a
//! verbatim copy of this file) accept and refuse exactly the same files
//! with the same sentences; `cases.json`, copied beside every copy, is the
//! proof.
//!
//! This file uses nothing but `std` and `serde_json`, so it can be copied
//! as is. Checks run in a fixed order and the first failure is the answer;
//! the TypeScript model runs them in the same order. Text is counted as in
//! the rules model: the file's size in UTF-8 bytes, a sentence's length and
//! a name's 40-character cut in code points, unknown fields in code point
//! order. Every number is an integer of at most 7 digits (`scan_numbers`
//! reads the raw text before any JSON parser does). No verdict depends on
//! the order of an object's keys.

use std::collections::BTreeSet;

use serde_json::{Map, Value};

pub const MAX_BYTES: usize = 32 * 1024;
pub const MAX_CHECKS: usize = 64;
pub const MAX_ID: usize = 48;
pub const MAX_SOURCE: usize = 200;
pub const MAX_PLACE: usize = 16;
pub const MAX_SET: usize = 16;
pub const MAX_STEPS: usize = 16;
pub const MAX_EXPECT: usize = 8;
/// Ticks of moves in one check: 30 s at 60 Hz.
pub const MAX_TICKS: u32 = 1800;
pub const MAX_POS: f64 = 100_000.0;
pub const MAX_NUMBER: f64 = 1_000_000.0;
/// Deeper JSON than this is unreadable.
pub const MAX_DEPTH: usize = 32;
pub const BUTTONS: &[&str] = &["left", "right", "up", "down", "A", "B"];
pub const EXPECTS: &[&str] = &["counter", "change", "count", "outcome", "sound"];
/// A check's keys, in the order `canonical_text` writes them.
pub const CHECK_KEYS: &[&str] = &["id", "source", "off", "given", "do", "expect"];
pub const GIVEN_KEYS: &[&str] = &["level", "player", "place", "set"];

pub const TOO_BIG: &str = "The checks file is larger than 32 KB.";
pub const UNREADABLE: &str = "The checks file could not be read.";
pub const NUMBER_DIGITS: &str = "Numbers in a checks file are whole numbers of up to 7 digits.";
pub const FILE_SHAPE: &str = "The checks file must be an object with \"checks\".";
pub const FILE_LIST: &str = "The checks file needs a \"checks\" list.";
pub const FILE_MANY: &str = "A checks file holds at most 64 checks.";
pub const CHECK_SHAPE: &str = "A check must be an object.";
pub const CHECK_ID: &str = "A check's id must be 1 to 48 characters of a-z, 0-9 or -.";
pub const CHECK_SOURCE: &str = "A check's sentence must be text of 1 to 200 characters.";
pub const CHECK_OFF: &str = "A check's \"off\" must be true or false.";
pub const GIVEN_SHAPE: &str = "A check's \"given\" must be an object.";
pub const LEVEL: &str = "\"level\" must be \"empty\" or \"current\".";
pub const PLAYER: &str = "\"player\" must be an object with an optional \"at\".";
pub const AT: &str = "A position must be [x, y] of whole numbers, and y may be \"floor\".";
pub const AT_RANGE: &str = "A position must be from -100000 to 100000 in each direction.";
pub const PLACE_LIST: &str = "\"place\" must be a list.";
pub const PLACE_MANY: &str = "A check can place at most 16 things.";
pub const PLACE_SHAPE: &str = "A placed thing must be {\"kind\", and \"at\" or \"beside\"}.";
pub const PLACE_WHERE: &str = "A placed thing needs exactly one of \"at\" and \"beside\".";
pub const BESIDE: &str = "\"beside\" must be \"left\", \"right\", \"above\" or \"below\".";
pub const SET_LIST: &str = "\"set\" must be a list.";
pub const SET_MANY: &str = "A check can set at most 16 starting values.";
pub const SET_SHAPE: &str = "A starting value must be [counter, number].";
pub const NUMBER: &str = "A number in a check must be a whole number from -1000000 to 1000000.";
pub const DO_LIST: &str = "A check needs a \"do\" list.";
pub const DO_MANY: &str = "A check can have at most 16 steps.";
pub const STEP_SHAPE: &str = "A step must be hold, press, wait or until.";
pub const HOLD: &str = "A \"hold\" step must be {\"hold\": [buttons], \"ticks\": 1 to 1800}.";
pub const HOLD_BUTTONS: &str = "\"hold\" must list 1 to 6 buttons.";
pub const PRESS: &str = "A \"press\" step must name one button.";
pub const WAIT: &str = "A \"wait\" step must be a whole number of ticks from 1 to 1800.";
pub const UNTIL: &str = "An \"until\" step must be {\"until\": expectation, \"within\": 1 to 1800 ticks}, with an optional \"hold\".";
pub const UNTIL_EVER: &str =
    "An \"until\" waits for something that holds now, so it can't use \"ever\".";
pub const TICKS_MANY: &str = "A check's moves can run at most 1800 ticks (30 s).";
pub const EXPECT_LIST: &str = "A check needs an \"expect\" list.";
pub const EXPECT_EMPTY: &str = "A check needs at least one expectation.";
pub const EXPECT_MANY: &str = "A check can have at most 8 expectations.";
pub const EXPECT_SHAPE: &str =
    "An expectation must be exactly one of counter, change, count, outcome or sound.";
pub const COUNTER: &str = "\"counter\" must be [counter, comparison, number].";
pub const CHANGE: &str = "\"change\" must be [counter, comparison, number].";
pub const COUNT: &str = "\"count\" must be [kind, comparison, number].";
pub const OUTCOME: &str = "\"outcome\" must be \"win\", \"lose\" or \"playing\".";
pub const SOUND: &str = "\"sound\" must name a sound.";
pub const EVER: &str = "\"ever\" must be true or false, on counter, change, count or outcome only.";

/// A name as a sentence shows it: at most 40 code points.
fn shown(s: &str) -> String {
    s.chars().take(40).collect()
}
pub fn no_kind(k: &str) -> String {
    format!("This game has no kind called {}.", shown(k))
}
pub fn cant_place(k: &str) -> String {
    format!("A check can't place a {}.", shown(k))
}
pub fn no_counter(c: &str) -> String {
    format!("This game has no counter called {}.", shown(c))
}
pub fn no_sound(s: &str) -> String {
    format!("This game has no sound called {}.", shown(s))
}
pub fn set_range(c: &str, min: i64, max: i64) -> String {
    format!("{} must start from {min} to {max}.", shown(c))
}
pub fn no_button(b: &str) -> String {
    format!(
        "There is no button called \"{}\": use left, right, up, down, A or B.",
        shown(b)
    )
}
pub fn no_expect(k: &str) -> String {
    format!("There is no expectation called \"{}\".", shown(k))
}
pub fn bad_op(op: &str) -> String {
    format!(
        "\"{}\" is not a comparison: use <, <=, =, >= or >.",
        shown(op)
    )
}
pub fn file_field(k: &str) -> String {
    format!("The checks file has an unknown field \"{}\".", shown(k))
}
pub fn check_field(k: &str) -> String {
    format!("A check has an unknown field \"{}\".", shown(k))
}
pub fn given_field(k: &str) -> String {
    format!("\"given\" has an unknown field \"{}\".", shown(k))
}
pub fn dup_id(id: &str) -> String {
    format!("The id \"{}\" is used by more than one check.", shown(id))
}
/// A check's sentence inside a file's answer: `Check 3: …` (1-based).
pub fn in_check(n: usize, s: &str) -> String {
    format!("Check {n}: {s}")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Lt,
    Le,
    Eq,
    Ge,
    Gt,
}

impl Op {
    pub fn parse(s: &str) -> Option<Op> {
        Some(match s {
            "<" => Op::Lt,
            "<=" => Op::Le,
            "=" => Op::Eq,
            ">=" => Op::Ge,
            ">" => Op::Gt,
            _ => return None,
        })
    }

    pub fn holds(self, a: i64, b: i64) -> bool {
        match self {
            Op::Lt => a < b,
            Op::Le => a <= b,
            Op::Eq => a == b,
            Op::Ge => a >= b,
            Op::Gt => a > b,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Btn {
    Left,
    Right,
    Up,
    Down,
    A,
    B,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Level {
    #[default]
    Empty,
    Current,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Y {
    At(i64),
    /// Resting on the stage's floor (`StageDef::floor_top`).
    Floor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pos {
    pub x: i64,
    pub y: Y,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
    Above,
    Below,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Where {
    At(Pos),
    Beside(Side),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placed {
    pub kind: String,
    pub at: Where,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Given {
    pub level: Level,
    pub player: Option<Pos>,
    pub place: Vec<Placed>,
    pub set: Vec<(String, i64)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    Win,
    Lose,
    Playing,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Want {
    Counter(String, Op, i64),
    Change(String, Op, i64),
    Count(String, Op, i64),
    Outcome(End),
    Sound(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Expect {
    pub want: Want,
    /// True: it only has to hold at some tick during the run.
    pub ever: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    Hold(Vec<Btn>, u32),
    Press(Btn),
    Wait(u32),
    /// The expectation, the most ticks to wait, the buttons held meanwhile.
    Until(Expect, u32, Vec<Btn>),
}

impl Step {
    /// The ticks this step can run.
    pub fn ticks(&self) -> u32 {
        match self {
            Step::Hold(_, n) | Step::Wait(n) | Step::Until(_, n, _) => *n,
            Step::Press(_) => 1,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataCheck {
    pub id: String,
    pub source: String,
    pub off: bool,
    pub given: Given,
    pub steps: Vec<Step>,
    pub expect: Vec<Expect>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChecksFile {
    pub checks: Vec<DataCheck>,
}

/// One counter a check may name, and the range `set` must stay in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CounterRange {
    pub name: String,
    pub min: i64,
    pub max: i64,
}

/// The names a game's checks may use: what the catalog's `checks_data`
/// key lists.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Vocab {
    pub counters: Vec<CounterRange>,
    /// Kinds a check may `place` (never the player's kind).
    pub placeable: Vec<String>,
    /// Kinds a check may `count`: every kind.
    pub kinds: Vec<String>,
    pub sounds: Vec<String>,
}

impl Vocab {
    /// Reads the catalog's `checks_data` object. Anything missing reads as
    /// an empty list; a counter with no range reads as ±1000000.
    pub fn from_json(v: &Value) -> Vocab {
        let names = |k: &str| -> Vec<String> {
            v[k].as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default()
        };
        let counters = v["counters"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|c| {
                        Some(CounterRange {
                            name: c["name"].as_str()?.to_string(),
                            min: c["min"].as_i64().unwrap_or(-1_000_000),
                            max: c["max"].as_i64().unwrap_or(1_000_000),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        Vocab {
            counters,
            placeable: names("placeable"),
            kinds: names("kinds"),
            sounds: names("sounds"),
        }
    }
}

fn listed(list: &[String], s: &str) -> bool {
    list.iter().any(|x| x == s)
}

/// A whole number from `lo` to `hi`.
fn whole(v: &Value, lo: f64, hi: f64) -> Option<i64> {
    let f = v.as_f64()?;
    (f.fract() == 0.0 && f >= lo && f <= hi).then_some(f as i64)
}

/// `^[a-z0-9-]{1,48}$`.
fn valid_id(s: &str) -> bool {
    (1..=MAX_ID).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// The first key (in code point order) that is not `known`.
fn first_unknown<'a>(o: &'a Map<String, Value>, known: &[&str]) -> Option<&'a str> {
    let mut ks: Vec<&str> = o
        .keys()
        .map(String::as_str)
        .filter(|k| !known.contains(k))
        .collect();
    ks.sort_unstable();
    ks.first().copied()
}

/// Every number token outside a string is `-?(0|[1-9][0-9]{0,6})`, as in
/// the rules model: a token is a run of `0-9 + - . e E` that starts with
/// `-` or a digit; string contents (with their `\` escapes) are skipped.
pub fn scan_numbers(bytes: &[u8]) -> Result<(), String> {
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'"' {
            i += 1;
            while i < bytes.len() && bytes[i] != b'"' {
                i += if bytes[i] == b'\\' { 2 } else { 1 };
            }
            i += 1;
        } else if b == b'-' || b.is_ascii_digit() {
            let start = i;
            while i < bytes.len()
                && matches!(bytes[i], b'0'..=b'9' | b'+' | b'-' | b'.' | b'e' | b'E')
            {
                i += 1;
            }
            if !integer_token(&bytes[start..i]) {
                return Err(NUMBER_DIGITS.into());
            }
        } else {
            i += 1;
        }
    }
    Ok(())
}

/// `-?(0|[1-9][0-9]{0,6})`.
fn integer_token(t: &[u8]) -> bool {
    let d = t.strip_prefix(b"-").unwrap_or(t);
    match d {
        [b'0'] => true,
        [b'1'..=b'9', rest @ ..] => rest.len() <= 6 && rest.iter().all(u8::is_ascii_digit),
        _ => false,
    }
}

fn depth(v: &Value) -> usize {
    match v {
        Value::Array(a) => 1 + a.iter().map(depth).max().unwrap_or(0),
        Value::Object(o) => 1 + o.values().map(depth).max().unwrap_or(0),
        _ => 0,
    }
}

/// Checks a whole checks file. The first failure is the answer.
pub fn check_file(bytes: &[u8], vocab: &Vocab) -> Result<ChecksFile, String> {
    if bytes.len() > MAX_BYTES {
        return Err(TOO_BIG.into());
    }
    scan_numbers(bytes)?;
    let v: Value = serde_json::from_slice(bytes).map_err(|_| UNREADABLE.to_string())?;
    if depth(&v) > MAX_DEPTH {
        return Err(UNREADABLE.into());
    }
    let Some(o) = v.as_object() else {
        return Err(FILE_SHAPE.into());
    };
    if let Some(k) = first_unknown(o, &["checks"]) {
        return Err(file_field(k));
    }
    let Some(list) = o.get("checks").and_then(Value::as_array) else {
        return Err(FILE_LIST.into());
    };
    if list.len() > MAX_CHECKS {
        return Err(FILE_MANY.into());
    }
    let mut checks = Vec::with_capacity(list.len());
    let mut ids = BTreeSet::new();
    for (i, c) in list.iter().enumerate() {
        let check = check_one(c, vocab).map_err(|e| in_check(i + 1, &e))?;
        if !ids.insert(check.id.clone()) {
            return Err(in_check(i + 1, &dup_id(&check.id)));
        }
        checks.push(check);
    }
    Ok(ChecksFile { checks })
}

/// Checks one check against the game's names. The sentence has no
/// `Check N:` prefix (`check_file` adds it).
pub fn check_one(v: &Value, vocab: &Vocab) -> Result<DataCheck, String> {
    let Some(o) = v.as_object() else {
        return Err(CHECK_SHAPE.into());
    };
    if let Some(k) = first_unknown(o, CHECK_KEYS) {
        return Err(check_field(k));
    }
    let id = match o.get("id").and_then(Value::as_str) {
        Some(s) if valid_id(s) => s.to_string(),
        _ => return Err(CHECK_ID.into()),
    };
    let source = match o.get("source") {
        Some(Value::String(s)) if (1..=MAX_SOURCE).contains(&s.chars().count()) => s.clone(),
        _ => return Err(CHECK_SOURCE.into()),
    };
    let off = match o.get("off") {
        None => false,
        Some(Value::Bool(b)) => *b,
        Some(_) => return Err(CHECK_OFF.into()),
    };
    let given = match o.get("given") {
        None => Given::default(),
        Some(g) => check_given(g, vocab)?,
    };
    let steps = match o.get("do") {
        Some(Value::Array(a)) => {
            if a.len() > MAX_STEPS {
                return Err(DO_MANY.into());
            }
            a.iter()
                .map(|s| check_step(s, vocab))
                .collect::<Result<Vec<_>, _>>()?
        }
        _ => return Err(DO_LIST.into()),
    };
    if steps.iter().map(Step::ticks).sum::<u32>() > MAX_TICKS {
        return Err(TICKS_MANY.into());
    }
    let expect = match o.get("expect") {
        Some(Value::Array(a)) => {
            if a.is_empty() {
                return Err(EXPECT_EMPTY.into());
            }
            if a.len() > MAX_EXPECT {
                return Err(EXPECT_MANY.into());
            }
            a.iter()
                .map(|e| check_expect(e, vocab, true))
                .collect::<Result<Vec<_>, _>>()?
        }
        _ => return Err(EXPECT_LIST.into()),
    };
    Ok(DataCheck {
        id,
        source,
        off,
        given,
        steps,
        expect,
    })
}

fn check_given(v: &Value, vocab: &Vocab) -> Result<Given, String> {
    let Some(o) = v.as_object() else {
        return Err(GIVEN_SHAPE.into());
    };
    if let Some(k) = first_unknown(o, GIVEN_KEYS) {
        return Err(given_field(k));
    }
    let level = match o.get("level") {
        None => Level::Empty,
        Some(x) => match x.as_str() {
            Some("empty") => Level::Empty,
            Some("current") => Level::Current,
            _ => return Err(LEVEL.into()),
        },
    };
    let player = match o.get("player") {
        None => None,
        Some(Value::Object(p)) => {
            if first_unknown(p, &["at"]).is_some() {
                return Err(PLAYER.into());
            }
            match p.get("at") {
                None => None,
                Some(a) => Some(check_pos(a)?),
            }
        }
        Some(_) => return Err(PLAYER.into()),
    };
    let place = match o.get("place") {
        None => Vec::new(),
        Some(Value::Array(a)) => {
            if a.len() > MAX_PLACE {
                return Err(PLACE_MANY.into());
            }
            a.iter()
                .map(|t| check_placed(t, vocab))
                .collect::<Result<Vec<_>, _>>()?
        }
        Some(_) => return Err(PLACE_LIST.into()),
    };
    let set = match o.get("set") {
        None => Vec::new(),
        Some(Value::Array(a)) => {
            if a.len() > MAX_SET {
                return Err(SET_MANY.into());
            }
            a.iter()
                .map(|x| check_set(x, vocab))
                .collect::<Result<Vec<_>, _>>()?
        }
        Some(_) => return Err(SET_LIST.into()),
    };
    Ok(Given {
        level,
        player,
        place,
        set,
    })
}

fn check_pos(v: &Value) -> Result<Pos, String> {
    let a = v
        .as_array()
        .filter(|a| a.len() == 2 && a[0].is_number() && (a[1].is_number() || a[1] == "floor"))
        .ok_or(AT)?;
    let x = whole(&a[0], -MAX_POS, MAX_POS).ok_or(AT_RANGE)?;
    let y = if a[1].is_number() {
        Y::At(whole(&a[1], -MAX_POS, MAX_POS).ok_or(AT_RANGE)?)
    } else {
        Y::Floor
    };
    Ok(Pos { x, y })
}

fn check_placed(v: &Value, vocab: &Vocab) -> Result<Placed, String> {
    let Some(o) = v.as_object() else {
        return Err(PLACE_SHAPE.into());
    };
    if first_unknown(o, &["kind", "at", "beside"]).is_some() {
        return Err(PLACE_SHAPE.into());
    }
    let Some(kind) = o.get("kind").and_then(Value::as_str) else {
        return Err(PLACE_SHAPE.into());
    };
    if !listed(&vocab.kinds, kind) {
        return Err(no_kind(kind));
    }
    if !listed(&vocab.placeable, kind) {
        return Err(cant_place(kind));
    }
    let at = match (o.get("at"), o.get("beside")) {
        (Some(a), None) => Where::At(check_pos(a)?),
        (None, Some(b)) => Where::Beside(match b.as_str() {
            Some("left") => Side::Left,
            Some("right") => Side::Right,
            Some("above") => Side::Above,
            Some("below") => Side::Below,
            _ => return Err(BESIDE.into()),
        }),
        _ => return Err(PLACE_WHERE.into()),
    };
    Ok(Placed {
        kind: kind.to_string(),
        at,
    })
}

fn check_set(v: &Value, vocab: &Vocab) -> Result<(String, i64), String> {
    let a = v
        .as_array()
        .filter(|a| a.len() == 2 && a[0].is_string() && a[1].is_number())
        .ok_or(SET_SHAPE)?;
    let c = a[0].as_str().unwrap_or_default();
    let range = vocab
        .counters
        .iter()
        .find(|r| r.name == c)
        .ok_or_else(|| no_counter(c))?;
    let n = whole(&a[1], -MAX_NUMBER, MAX_NUMBER).ok_or(NUMBER)?;
    if n < range.min || n > range.max {
        return Err(set_range(c, range.min, range.max));
    }
    Ok((c.to_string(), n))
}

fn button(s: &str) -> Result<Btn, String> {
    Ok(match s {
        "left" => Btn::Left,
        "right" => Btn::Right,
        "up" => Btn::Up,
        "down" => Btn::Down,
        "A" => Btn::A,
        "B" => Btn::B,
        _ => return Err(no_button(s)),
    })
}

fn buttons(v: &Value) -> Result<Vec<Btn>, String> {
    let a = v
        .as_array()
        .filter(|a| !a.is_empty() && a.len() <= BUTTONS.len())
        .ok_or(HOLD_BUTTONS)?;
    a.iter()
        .map(|b| {
            b.as_str()
                .ok_or_else(|| HOLD_BUTTONS.to_string())
                .and_then(button)
        })
        .collect()
}

fn ticks(v: Option<&Value>) -> Option<u32> {
    v.and_then(|x| whole(x, 1.0, f64::from(MAX_TICKS)))
        .map(|n| n as u32)
}

fn check_step(v: &Value, vocab: &Vocab) -> Result<Step, String> {
    let Some(o) = v.as_object() else {
        return Err(STEP_SHAPE.into());
    };
    if let Some(e) = o.get("until") {
        if first_unknown(o, &["until", "within", "hold"]).is_some() {
            return Err(UNTIL.into());
        }
        let e = check_expect(e, vocab, false)?;
        let within = ticks(o.get("within")).ok_or(UNTIL)?;
        let held = match o.get("hold") {
            None => Vec::new(),
            Some(h) => buttons(h)?,
        };
        return Ok(Step::Until(e, within, held));
    }
    if let Some(h) = o.get("hold") {
        if first_unknown(o, &["hold", "ticks"]).is_some() {
            return Err(HOLD.into());
        }
        let held = buttons(h)?;
        let n = ticks(o.get("ticks")).ok_or(HOLD)?;
        return Ok(Step::Hold(held, n));
    }
    if let Some(p) = o.get("press") {
        if o.len() != 1 {
            return Err(PRESS.into());
        }
        return Ok(Step::Press(button(p.as_str().ok_or(PRESS)?)?));
    }
    if o.contains_key("wait") {
        if o.len() != 1 {
            return Err(WAIT.into());
        }
        return Ok(Step::Wait(ticks(o.get("wait")).ok_or(WAIT)?));
    }
    Err(STEP_SHAPE.into())
}

/// `[name, comparison, number]`, `shape` the sentence for the wrong shape.
fn comparison(x: &Value, shape: &str) -> Result<(String, Op, i64), String> {
    let a = x
        .as_array()
        .filter(|a| a.len() == 3 && a[0].is_string() && a[1].is_string() && a[2].is_number())
        .ok_or(shape)?;
    Ok((
        a[0].as_str().unwrap_or_default().to_string(),
        Op::parse(a[1].as_str().unwrap_or_default())
            .ok_or_else(|| bad_op(a[1].as_str().unwrap_or_default()))?,
        whole(&a[2], -MAX_NUMBER, MAX_NUMBER).ok_or(NUMBER)?,
    ))
}

/// `[name, comparison, number]` checked in the rules' order: the shape,
/// the name, the comparison, the number.
fn named_comparison(
    x: &Value,
    shape: &str,
    known: impl Fn(&str) -> Result<(), String>,
) -> Result<(String, Op, i64), String> {
    let a = x
        .as_array()
        .filter(|a| a.len() == 3 && a[0].is_string() && a[1].is_string() && a[2].is_number())
        .ok_or(shape)?;
    known(a[0].as_str().unwrap_or_default())?;
    comparison(x, shape)
}

fn check_expect(v: &Value, vocab: &Vocab, allow_ever: bool) -> Result<Expect, String> {
    let Some(o) = v.as_object() else {
        return Err(EXPECT_SHAPE.into());
    };
    let ever = match o.get("ever") {
        None => false,
        Some(Value::Bool(b)) => *b,
        Some(_) => return Err(EVER.into()),
    };
    let keys: Vec<&str> = o
        .keys()
        .map(String::as_str)
        .filter(|k| *k != "ever")
        .collect();
    let [k] = keys.as_slice() else {
        return Err(EXPECT_SHAPE.into());
    };
    let k = *k;
    if !EXPECTS.contains(&k) {
        return Err(no_expect(k));
    }
    if k == "sound" && o.contains_key("ever") {
        return Err(EVER.into());
    }
    if ever && !allow_ever {
        return Err(UNTIL_EVER.into());
    }
    let x = &o[k];
    let counter = |c: &str| {
        if vocab.counters.iter().any(|r| r.name == c) {
            Ok(())
        } else {
            Err(no_counter(c))
        }
    };
    let want = match k {
        "counter" => {
            let (c, op, n) = named_comparison(x, COUNTER, counter)?;
            Want::Counter(c, op, n)
        }
        "change" => {
            let (c, op, n) = named_comparison(x, CHANGE, counter)?;
            Want::Change(c, op, n)
        }
        "count" => {
            let (c, op, n) = named_comparison(x, COUNT, |k| {
                if listed(&vocab.kinds, k) {
                    Ok(())
                } else {
                    Err(no_kind(k))
                }
            })?;
            Want::Count(c, op, n)
        }
        "outcome" => Want::Outcome(match x.as_str() {
            Some("win") => End::Win,
            Some("lose") => End::Lose,
            Some("playing") => End::Playing,
            _ => return Err(OUTCOME.into()),
        }),
        _ => {
            let s = x.as_str().ok_or(SOUND)?;
            if !listed(&vocab.sounds, s) {
                return Err(no_sound(s));
            }
            Want::Sound(s.to_string())
        }
    };
    Ok(Expect { want, ever })
}

/// The editor's way of writing a checks file, byte for byte (`writeChecks`
/// in the editor's `src/checks/model.ts`): one check per line; each
/// check's keys in `CHECK_KEYS` order; every object inside a check with
/// its keys sorted; whole numbers without a fraction.
pub fn canonical_text(checks: &[Value]) -> String {
    if checks.is_empty() {
        return "{\n  \"checks\": []\n}\n".into();
    }
    let lines: Vec<String> = checks.iter().map(canonical_check).collect();
    format!(
        "{{\n  \"checks\": [\n    {}\n  ]\n}}\n",
        lines.join(",\n    ")
    )
}

fn canonical_check(c: &Value) -> String {
    let Some(o) = c.as_object() else {
        return compact(c);
    };
    let parts: Vec<String> = CHECK_KEYS
        .iter()
        .filter_map(|k| {
            o.get(*k)
                .map(|v| format!("{}:{}", Value::String((*k).to_string()), compact(v)))
        })
        .collect();
    format!("{{{}}}", parts.join(","))
}

fn compact(v: &Value) -> String {
    match v {
        Value::Number(n) => match n.as_f64() {
            Some(f) if f.fract() == 0.0 && f.abs() < 1e15 => format!("{}", f as i64),
            _ => n.to_string(),
        },
        Value::Array(a) => format!("[{}]", a.iter().map(compact).collect::<Vec<_>>().join(",")),
        Value::Object(o) => {
            let mut keys: Vec<&String> = o.keys().collect();
            keys.sort_unstable();
            let parts: Vec<String> = keys
                .iter()
                .map(|k| {
                    format!(
                        "{}:{}",
                        Value::String((*k).clone()),
                        compact(&o[k.as_str()])
                    )
                })
                .collect();
            format!("{{{}}}", parts.join(","))
        }
        _ => v.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CASES: &str = include_str!("cases.json");

    /// The text a case names: its `file` written out, its `text` (padded
    /// to `pad_to` bytes before its last character), or `many` generated
    /// checks.
    pub(crate) fn case_text(case: &Value) -> String {
        if let Some(n) = case["many"].as_u64() {
            let checks: Vec<String> = (1..=n)
                .map(|i| {
                    format!(
                        r#"{{"id":"c{i}","source":"s","do":[],"expect":[{{"outcome":"playing"}}]}}"#
                    )
                })
                .collect();
            return format!(r#"{{"checks":[{}]}}"#, checks.join(","));
        }
        if !case["file"].is_null() {
            return serde_json::to_string(&case["file"]).unwrap();
        }
        let text = case["text"].as_str().unwrap_or_default().to_string();
        match case["pad_to"].as_u64() {
            Some(n) => {
                let (head, tail) = text.split_at(text.len() - 1);
                format!("{head}{}{tail}", " ".repeat(n as usize - text.len()))
            }
            None => text,
        }
    }

    #[test]
    fn every_shared_case_is_accepted_or_refused_with_its_sentence() {
        let all: Value = serde_json::from_str(CASES).unwrap();
        let vocab = Vocab::from_json(&all["vocab"]);
        let cases = all["cases"].as_array().unwrap();
        assert!(cases.len() > 90);
        for case in cases {
            let got = check_file(case_text(case).as_bytes(), &vocab).err();
            let want = case["error"].as_str().map(str::to_string);
            assert_eq!(got, want, "{}", case["name"]);
        }
    }

    #[test]
    fn canonical_text_matches_the_shared_cases() {
        let all: Value = serde_json::from_str(CASES).unwrap();
        for c in all["canonical"].as_array().unwrap() {
            let checks = c["checks"].as_array().unwrap();
            assert_eq!(canonical_text(checks), c["text"].as_str().unwrap());
        }
    }

    #[test]
    fn a_checked_file_comes_back_typed() {
        let all: Value = serde_json::from_str(CASES).unwrap();
        let vocab = Vocab::from_json(&all["vocab"]);
        let text = br#"{"checks":[{"id":"spike","source":"Touching a spike costs a life","off":true,"given":{"level":"current","player":{"at":[-200,"floor"]},"place":[{"kind":"Spike","beside":"right"},{"kind":"Coin","at":[5,-6]}],"set":[["lives",3]]},"do":[{"hold":["right","A"],"ticks":60},{"press":"B"},{"wait":2},{"until":{"outcome":"win"},"within":30,"hold":["left"]}],"expect":[{"change":["lives","=",-1]},{"count":["Coin",">=",0],"ever":true},{"sound":"hit"}]}]}"#;
        let file = check_file(text, &vocab).unwrap();
        assert_eq!(
            file.checks[0],
            DataCheck {
                id: "spike".into(),
                source: "Touching a spike costs a life".into(),
                off: true,
                given: Given {
                    level: Level::Current,
                    player: Some(Pos {
                        x: -200,
                        y: Y::Floor
                    }),
                    place: vec![
                        Placed {
                            kind: "Spike".into(),
                            at: Where::Beside(Side::Right)
                        },
                        Placed {
                            kind: "Coin".into(),
                            at: Where::At(Pos { x: 5, y: Y::At(-6) })
                        },
                    ],
                    set: vec![("lives".into(), 3)],
                },
                steps: vec![
                    Step::Hold(vec![Btn::Right, Btn::A], 60),
                    Step::Press(Btn::B),
                    Step::Wait(2),
                    Step::Until(
                        Expect {
                            want: Want::Outcome(End::Win),
                            ever: false
                        },
                        30,
                        vec![Btn::Left]
                    ),
                ],
                expect: vec![
                    Expect {
                        want: Want::Change("lives".into(), Op::Eq, -1),
                        ever: false
                    },
                    Expect {
                        want: Want::Count("Coin".into(), Op::Ge, 0),
                        ever: true
                    },
                    Expect {
                        want: Want::Sound("hit".into()),
                        ever: false
                    },
                ],
            }
        );
        assert_eq!(
            file.checks[0].steps.iter().map(Step::ticks).sum::<u32>(),
            93
        );
    }

    #[test]
    fn comparisons_hold_as_written() {
        assert!(Op::Lt.holds(1, 2) && !Op::Lt.holds(2, 2));
        assert!(Op::Le.holds(2, 2) && Op::Eq.holds(3, 3) && Op::Ge.holds(3, 3));
        assert!(Op::Gt.holds(4, 3) && !Op::Gt.holds(3, 3));
        assert_eq!(Op::parse("=="), None);
    }
}
