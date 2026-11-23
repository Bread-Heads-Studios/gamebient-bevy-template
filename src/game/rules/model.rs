//! The rules file (`content/rules.json`): its typed form and the one
//! checker every side runs. The game (`rules::set_rules`), the editor's
//! model (`src/rules/model.ts`) and the editor's server
//! (`crates/editor-server/src/rules/model.rs`, a verbatim copy of this
//! file) accept and refuse exactly the same files with the same
//! sentences; `cases.json`, copied beside every copy, is the proof.
//!
//! This file uses nothing but `std` and `serde_json`, so it can be copied
//! as is. Checks run in a fixed order and the first failure is the answer;
//! the TypeScript model runs them in the same order.

use std::collections::BTreeSet;

use serde_json::{Map, Value};

pub const MAX_BYTES: usize = 32 * 1024;
pub const MAX_RULES: usize = 64;
pub const MAX_IF: usize = 8;
pub const MAX_DO: usize = 8;
pub const MAX_SOURCE: usize = 200;
pub const MAX_TICKS: f64 = 36000.0;
pub const MAX_NUMBER: f64 = 1_000_000.0;
pub const MAX_MOVE: f64 = 1000.0;
pub const MAX_PLACE: f64 = 100_000.0;
pub const MAX_TIMERS: usize = 16;
pub const MAX_TIMER_NAMES: usize = 8;
/// Deeper JSON than this is refused as unreadable on every side (serde's
/// own limit is deeper still, so both sides refuse the same files).
pub const MAX_DEPTH: usize = 32;

pub const TRIGGERS: &[&str] = &["start", "every", "button", "touch", "counter", "timer"];
pub const CONDITIONS: &[&str] = &["compare", "chance"];
pub const ACTIONS: &[&str] = &[
    "add", "set", "remove", "spawn", "respawn", "jump", "move", "sound", "timer", "end_run",
];
/// A rule's fields, in the order `canonical_text` writes them.
pub const RULE_KEYS: &[&str] = &["id", "when", "if", "do", "source", "off"];

pub const TOO_BIG: &str = "The rules file is larger than 32 KB.";
pub const UNREADABLE: &str = "The rules file could not be read.";
pub const FILE_SHAPE: &str = "The rules file must be an object with \"v\" and \"rules\".";
pub const FILE_V: &str = "The rules file must have \"v\" set to 1.";
pub const FILE_LIST: &str = "The rules file needs a \"rules\" list.";
pub const FILE_MANY: &str = "A rules file holds at most 64 rules.";
pub const FILE_TIMERS: &str = "A rules file can start at most 16 timers.";
pub const FILE_TIMER_NAMES: &str = "A rules file can use at most 8 timer names.";
pub const RULE_SHAPE: &str = "A rule must be an object.";
pub const RULE_ID: &str = "A rule's id must be 1 to 32 characters of a-z, 0-9 or -.";
pub const RULE_SOURCE: &str = "A rule's sentence must be text of at most 200 characters.";
pub const RULE_OFF: &str = "A rule's \"off\" must be true or false.";
pub const RULE_WHEN: &str = "A rule needs a \"when\".";
pub const RULE_IF_LIST: &str = "A rule's \"if\" must be a list.";
pub const RULE_IF_MANY: &str = "A rule can have at most 8 conditions.";
pub const RULE_DO_LIST: &str = "A rule needs a \"do\" list.";
pub const RULE_DO_EMPTY: &str = "A rule needs at least one action.";
pub const RULE_DO_MANY: &str = "A rule can have at most 8 actions.";
pub const WHEN_ONE: &str = "A \"when\" must be exactly one trigger.";
pub const START: &str = "\"start\" must be true.";
pub const EVERY: &str = "\"every\" must be a whole number of ticks from 1 to 36000.";
pub const BUTTON: &str = "\"button\" must be [\"A\" or \"B\", \"pressed\" or \"held\"].";
pub const TOUCH: &str = "\"touch\" must name two kinds.";
pub const COUNTER: &str = "\"counter\" must be [counter, comparison, number].";
pub const TIMER_NAME: &str = "A timer's name must be 1 to 32 characters of a-z, 0-9 or -.";
pub const IF_ONE: &str = "A condition must be exactly one test.";
pub const COMPARE: &str = "\"compare\" must be [counter, comparison, number].";
pub const CHANCE: &str = "\"chance\" must be a whole number from 1 to 100.";
pub const DO_ONE: &str = "An action must be exactly one step.";
pub const ADD: &str = "\"add\" must be [counter, number].";
pub const SET: &str = "\"set\" must be [counter, number].";
pub const SPAWN: &str = "\"spawn\" must be [kind, place].";
pub const MOVE: &str = "\"move\" must be [target, dx, dy].";
pub const MOVE_RANGE: &str = "A move must be from -1000 to 1000 in each direction.";
pub const SOUND: &str = "\"sound\" must name a sound.";
pub const TIMER: &str = "\"timer\" must be [name, ticks].";
pub const TICKS: &str = "A timer must run from 1 to 36000 ticks.";
pub const END_RUN: &str = "\"end_run\" must be \"win\" or \"lose\".";
pub const TARGET: &str = "A target must be self, other or a kind.";
pub const SELF_TOUCH: &str = "\"self\" and \"other\" work only in a rule that starts with a touch.";
pub const PLACE: &str = "A place must be start, random, self or {\"x\", \"y\"}.";
pub const PLACE_RANGE: &str = "A place must be from -100000 to 100000 in each direction.";
pub const NUMBER: &str = "A number in a rule must be a whole number from -1000000 to 1000000.";

/// A name as a sentence shows it: at most 40 characters.
fn shown(s: &str) -> String {
    s.chars().take(40).collect()
}
pub fn no_kind(k: &str) -> String {
    format!("This game has no kind called {}.", shown(k))
}
pub fn no_counter(c: &str) -> String {
    format!("This game has no counter called {}.", shown(c))
}
pub fn no_sound(s: &str) -> String {
    format!("This game has no sound called {}.", shown(s))
}
pub fn file_field(k: &str) -> String {
    format!("The rules file has an unknown field \"{}\".", shown(k))
}
pub fn rule_field(k: &str) -> String {
    format!("A rule has an unknown field \"{}\".", shown(k))
}
pub fn no_trigger(k: &str) -> String {
    format!("There is no trigger called \"{}\".", shown(k))
}
pub fn game_trigger(k: &str) -> String {
    format!("This game has no \"{}\" trigger.", shown(k))
}
pub fn no_condition(k: &str) -> String {
    format!("There is no condition called \"{}\".", shown(k))
}
pub fn game_condition(k: &str) -> String {
    format!("This game has no \"{}\" condition.", shown(k))
}
pub fn no_action(k: &str) -> String {
    format!("There is no action called \"{}\".", shown(k))
}
pub fn game_action(k: &str) -> String {
    format!("This game has no \"{}\" action.", shown(k))
}
pub fn bad_op(op: &str) -> String {
    format!(
        "\"{}\" is not a comparison: use <, <=, =, >= or >.",
        shown(op)
    )
}
pub fn dup_id(id: &str) -> String {
    format!("The id \"{}\" is used by more than one rule.", shown(id))
}
/// A rule's sentence inside a file's answer: `Rule 3: …` (1-based).
pub fn in_rule(n: usize, s: &str) -> String {
    format!("Rule {n}: {s}")
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
    fn parse(s: &str) -> Option<Op> {
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
    A,
    B,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    Pressed,
    Held,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Trigger {
    Start,
    Every(u32),
    Button(Btn, Edge),
    Touch(String, String),
    Counter(String, Op, i64),
    Timer(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Cond {
    Compare(String, Op, i64),
    Chance(u32),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    Me,
    Other,
    Kind(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Place {
    Start,
    Random,
    Me,
    At(f32, f32),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Act {
    Add(String, i64),
    Set(String, i64),
    Remove(Target),
    Spawn(String, Place),
    Respawn(Target),
    Jump(Target),
    Move(Target, f32, f32),
    Sound(String),
    Timer(String, u32),
    /// `true` is a win.
    EndRun(bool),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Rule {
    pub id: String,
    pub when: Trigger,
    pub conds: Vec<Cond>,
    pub acts: Vec<Act>,
    pub source: Option<String>,
    pub off: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RulesFile {
    pub rules: Vec<Rule>,
}

/// The names a game supports: what the catalog's `rules` key lists.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Vocab {
    pub triggers: Vec<String>,
    pub conditions: Vec<String>,
    pub actions: Vec<String>,
    pub counters: Vec<String>,
    pub kinds: Vec<String>,
    pub sounds: Vec<String>,
}

impl Vocab {
    /// Reads the catalog's `rules` object (counters by `name`). Anything
    /// missing reads as an empty list.
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
        Vocab {
            triggers: names("triggers"),
            conditions: names("conditions"),
            actions: names("actions"),
            counters: v["counters"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|c| c["name"].as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default(),
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

/// Any number from `lo` to `hi`.
fn number(v: &Value, lo: f64, hi: f64) -> Option<f32> {
    let f = v.as_f64()?;
    (f.is_finite() && f >= lo && f <= hi).then_some(f as f32)
}

/// `^[a-z0-9-]{1,32}$`: a rule id or a timer name.
fn valid_name(s: &str) -> bool {
    (1..=32).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// The one key and value of a one-key object.
fn single(v: &Value) -> Option<(&str, &Value)> {
    let o = v.as_object()?;
    if o.len() != 1 {
        return None;
    }
    o.iter().next().map(|(k, x)| (k.as_str(), x))
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

fn depth(v: &Value) -> usize {
    match v {
        Value::Array(a) => 1 + a.iter().map(depth).max().unwrap_or(0),
        Value::Object(o) => 1 + o.values().map(depth).max().unwrap_or(0),
        _ => 0,
    }
}

/// Checks a whole rules file. The first failure is the answer.
pub fn check_file(bytes: &[u8], vocab: &Vocab) -> Result<RulesFile, String> {
    if bytes.len() > MAX_BYTES {
        return Err(TOO_BIG.into());
    }
    let v: Value = serde_json::from_slice(bytes).map_err(|_| UNREADABLE.to_string())?;
    if depth(&v) > MAX_DEPTH {
        return Err(UNREADABLE.into());
    }
    let Some(o) = v.as_object() else {
        return Err(FILE_SHAPE.into());
    };
    if let Some(k) = first_unknown(o, &["v", "rules"]) {
        return Err(file_field(k));
    }
    if o.get("v").and_then(|x| whole(x, 1.0, 1.0)).is_none() {
        return Err(FILE_V.into());
    }
    let Some(list) = o.get("rules").and_then(Value::as_array) else {
        return Err(FILE_LIST.into());
    };
    if list.len() > MAX_RULES {
        return Err(FILE_MANY.into());
    }
    let mut rules = Vec::with_capacity(list.len());
    let mut ids = BTreeSet::new();
    for (i, r) in list.iter().enumerate() {
        let rule = check_rule(r, vocab).map_err(|e| in_rule(i + 1, &e))?;
        if !ids.insert(rule.id.clone()) {
            return Err(in_rule(i + 1, &dup_id(&rule.id)));
        }
        rules.push(rule);
    }
    let mut timers = 0usize;
    let mut names = BTreeSet::new();
    for r in &rules {
        if let Trigger::Timer(n) = &r.when {
            names.insert(n.clone());
        }
        for a in &r.acts {
            if let Act::Timer(n, _) = a {
                timers += 1;
                names.insert(n.clone());
            }
        }
    }
    if timers > MAX_TIMERS {
        return Err(FILE_TIMERS.into());
    }
    if names.len() > MAX_TIMER_NAMES {
        return Err(FILE_TIMER_NAMES.into());
    }
    Ok(RulesFile { rules })
}

/// Checks one rule against the game's names. The sentence has no
/// `Rule N:` prefix (`check_file` adds it).
pub fn check_rule(v: &Value, vocab: &Vocab) -> Result<Rule, String> {
    let Some(o) = v.as_object() else {
        return Err(RULE_SHAPE.into());
    };
    if let Some(k) = first_unknown(o, RULE_KEYS) {
        return Err(rule_field(k));
    }
    let id = match o.get("id").and_then(Value::as_str) {
        Some(s) if valid_name(s) => s.to_string(),
        _ => return Err(RULE_ID.into()),
    };
    let source = match o.get("source") {
        None => None,
        Some(Value::String(s)) if s.chars().count() <= MAX_SOURCE => Some(s.clone()),
        Some(_) => return Err(RULE_SOURCE.into()),
    };
    let off = match o.get("off") {
        None => false,
        Some(Value::Bool(b)) => *b,
        Some(_) => return Err(RULE_OFF.into()),
    };
    let Some(when) = o.get("when") else {
        return Err(RULE_WHEN.into());
    };
    let when = check_trigger(when, vocab)?;
    let touch = matches!(when, Trigger::Touch(..));
    let conds = match o.get("if") {
        None => Vec::new(),
        Some(Value::Array(a)) => {
            if a.len() > MAX_IF {
                return Err(RULE_IF_MANY.into());
            }
            a.iter()
                .map(|c| check_cond(c, vocab))
                .collect::<Result<Vec<_>, _>>()?
        }
        Some(_) => return Err(RULE_IF_LIST.into()),
    };
    let acts = match o.get("do") {
        Some(Value::Array(a)) => {
            if a.is_empty() {
                return Err(RULE_DO_EMPTY.into());
            }
            if a.len() > MAX_DO {
                return Err(RULE_DO_MANY.into());
            }
            a.iter()
                .map(|x| check_act(x, vocab, touch))
                .collect::<Result<Vec<_>, _>>()?
        }
        _ => return Err(RULE_DO_LIST.into()),
    };
    Ok(Rule {
        id,
        when,
        conds,
        acts,
        source,
        off,
    })
}

fn kind(k: &str, vocab: &Vocab) -> Result<(), String> {
    if listed(&vocab.kinds, k) {
        Ok(())
    } else {
        Err(no_kind(k))
    }
}

fn counter(c: &str, vocab: &Vocab) -> Result<(), String> {
    if listed(&vocab.counters, c) {
        Ok(())
    } else {
        Err(no_counter(c))
    }
}

/// `[counter, comparison, number]`, with `shape` as the sentence for the
/// wrong shape.
fn comparison(x: &Value, vocab: &Vocab, shape: &str) -> Result<(String, Op, i64), String> {
    let a = x
        .as_array()
        .filter(|a| a.len() == 3 && a[0].is_string() && a[1].is_string() && a[2].is_number())
        .ok_or(shape)?;
    let c = a[0].as_str().unwrap_or_default();
    counter(c, vocab)?;
    let op_text = a[1].as_str().unwrap_or_default();
    let op = Op::parse(op_text).ok_or_else(|| bad_op(op_text))?;
    let n = whole(&a[2], -MAX_NUMBER, MAX_NUMBER).ok_or(NUMBER)?;
    Ok((c.to_string(), op, n))
}

fn check_trigger(v: &Value, vocab: &Vocab) -> Result<Trigger, String> {
    let Some((k, x)) = single(v) else {
        return Err(WHEN_ONE.into());
    };
    if !TRIGGERS.contains(&k) {
        return Err(no_trigger(k));
    }
    if !listed(&vocab.triggers, k) {
        return Err(game_trigger(k));
    }
    Ok(match k {
        "start" => {
            if *x != Value::Bool(true) {
                return Err(START.into());
            }
            Trigger::Start
        }
        "every" => Trigger::Every(whole(x, 1.0, MAX_TICKS).ok_or(EVERY)? as u32),
        "button" => {
            let a = x.as_array().filter(|a| a.len() == 2).ok_or(BUTTON)?;
            let b = match a[0].as_str() {
                Some("A") => Btn::A,
                Some("B") => Btn::B,
                _ => return Err(BUTTON.into()),
            };
            let e = match a[1].as_str() {
                Some("pressed") => Edge::Pressed,
                Some("held") => Edge::Held,
                _ => return Err(BUTTON.into()),
            };
            Trigger::Button(b, e)
        }
        "touch" => {
            let a = x.as_array().filter(|a| a.len() == 2).ok_or(TOUCH)?;
            let (Some(p), Some(q)) = (a[0].as_str(), a[1].as_str()) else {
                return Err(TOUCH.into());
            };
            kind(p, vocab)?;
            kind(q, vocab)?;
            Trigger::Touch(p.to_string(), q.to_string())
        }
        "counter" => {
            let (c, op, n) = comparison(x, vocab, COUNTER)?;
            Trigger::Counter(c, op, n)
        }
        "timer" => {
            let n = x.as_str().filter(|s| valid_name(s)).ok_or(TIMER_NAME)?;
            Trigger::Timer(n.to_string())
        }
        _ => return Err(no_trigger(k)),
    })
}

fn check_cond(v: &Value, vocab: &Vocab) -> Result<Cond, String> {
    let Some((k, x)) = single(v) else {
        return Err(IF_ONE.into());
    };
    if !CONDITIONS.contains(&k) {
        return Err(no_condition(k));
    }
    if !listed(&vocab.conditions, k) {
        return Err(game_condition(k));
    }
    Ok(match k {
        "compare" => {
            let (c, op, n) = comparison(x, vocab, COMPARE)?;
            Cond::Compare(c, op, n)
        }
        "chance" => Cond::Chance(whole(x, 1.0, 100.0).ok_or(CHANCE)? as u32),
        _ => return Err(no_condition(k)),
    })
}

fn target(x: &Value, vocab: &Vocab, touch: bool) -> Result<Target, String> {
    let s = x.as_str().ok_or(TARGET)?;
    match s {
        "self" | "other" if !touch => Err(SELF_TOUCH.into()),
        "self" => Ok(Target::Me),
        "other" => Ok(Target::Other),
        k => {
            kind(k, vocab)?;
            Ok(Target::Kind(k.to_string()))
        }
    }
}

fn place(x: &Value, touch: bool) -> Result<Place, String> {
    match x {
        Value::String(s) => match s.as_str() {
            "start" => Ok(Place::Start),
            "random" => Ok(Place::Random),
            "self" if touch => Ok(Place::Me),
            "self" => Err(SELF_TOUCH.into()),
            _ => Err(PLACE.into()),
        },
        Value::Object(o) => {
            if o.len() != 2 {
                return Err(PLACE.into());
            }
            let (Some(px), Some(py)) = (o.get("x"), o.get("y")) else {
                return Err(PLACE.into());
            };
            if !px.is_number() || !py.is_number() {
                return Err(PLACE.into());
            }
            let px = number(px, -MAX_PLACE, MAX_PLACE).ok_or(PLACE_RANGE)?;
            let py = number(py, -MAX_PLACE, MAX_PLACE).ok_or(PLACE_RANGE)?;
            Ok(Place::At(px, py))
        }
        _ => Err(PLACE.into()),
    }
}

fn check_act(v: &Value, vocab: &Vocab, touch: bool) -> Result<Act, String> {
    let Some((k, x)) = single(v) else {
        return Err(DO_ONE.into());
    };
    if !ACTIONS.contains(&k) {
        return Err(no_action(k));
    }
    if !listed(&vocab.actions, k) {
        return Err(game_action(k));
    }
    Ok(match k {
        "add" | "set" => {
            let shape = if k == "add" { ADD } else { SET };
            let a = x
                .as_array()
                .filter(|a| a.len() == 2 && a[0].is_string() && a[1].is_number())
                .ok_or(shape)?;
            let c = a[0].as_str().unwrap_or_default();
            counter(c, vocab)?;
            let n = whole(&a[1], -MAX_NUMBER, MAX_NUMBER).ok_or(NUMBER)?;
            if k == "add" {
                Act::Add(c.to_string(), n)
            } else {
                Act::Set(c.to_string(), n)
            }
        }
        "remove" => Act::Remove(target(x, vocab, touch)?),
        "respawn" => Act::Respawn(target(x, vocab, touch)?),
        "jump" => Act::Jump(target(x, vocab, touch)?),
        "spawn" => {
            let a = x
                .as_array()
                .filter(|a| a.len() == 2 && a[0].is_string())
                .ok_or(SPAWN)?;
            let k = a[0].as_str().unwrap_or_default();
            kind(k, vocab)?;
            Act::Spawn(k.to_string(), place(&a[1], touch)?)
        }
        "move" => {
            let a = x
                .as_array()
                .filter(|a| a.len() == 3 && a[1].is_number() && a[2].is_number())
                .ok_or(MOVE)?;
            let t = target(&a[0], vocab, touch)?;
            let dx = number(&a[1], -MAX_MOVE, MAX_MOVE).ok_or(MOVE_RANGE)?;
            let dy = number(&a[2], -MAX_MOVE, MAX_MOVE).ok_or(MOVE_RANGE)?;
            Act::Move(t, dx, dy)
        }
        "sound" => {
            let s = x.as_str().ok_or(SOUND)?;
            if !listed(&vocab.sounds, s) {
                return Err(no_sound(s));
            }
            Act::Sound(s.to_string())
        }
        "timer" => {
            let a = x
                .as_array()
                .filter(|a| a.len() == 2 && a[0].is_string() && a[1].is_number())
                .ok_or(TIMER)?;
            let n = a[0].as_str().unwrap_or_default();
            if !valid_name(n) {
                return Err(TIMER_NAME.into());
            }
            let t = whole(&a[1], 1.0, MAX_TICKS).ok_or(TICKS)?;
            Act::Timer(n.to_string(), t as u32)
        }
        "end_run" => match x.as_str() {
            Some("win") => Act::EndRun(true),
            Some("lose") => Act::EndRun(false),
            _ => return Err(END_RUN.into()),
        },
        _ => return Err(no_action(k)),
    })
}

/// The editor's way of writing a rules file, byte for byte (`writeRules`
/// in the editor's `src/rules/model.ts`): one rule per line; each rule's
/// keys in `RULE_KEYS` order; every object inside a rule with its keys
/// sorted; whole numbers without a fraction.
pub fn canonical_text(rules: &[Value]) -> String {
    if rules.is_empty() {
        return "{\n  \"v\": 1,\n  \"rules\": []\n}\n".into();
    }
    let lines: Vec<String> = rules.iter().map(canonical_rule).collect();
    format!(
        "{{\n  \"v\": 1,\n  \"rules\": [\n    {}\n  ]\n}}\n",
        lines.join(",\n    ")
    )
}

fn canonical_rule(r: &Value) -> String {
    let Some(o) = r.as_object() else {
        return compact(r);
    };
    let parts: Vec<String> = RULE_KEYS
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

    /// The text a case names: its `text`, padded to `pad_to` bytes, or
    /// `many` generated rules.
    pub(crate) fn case_text(case: &Value) -> String {
        if let Some(n) = case["many"].as_u64() {
            let rules: Vec<String> = (1..=n)
                .map(|i| {
                    format!(
                        r#"{{"id":"r{i}","when":{{"start":true}},"do":[{{"add":["score",1]}}]}}"#
                    )
                })
                .collect();
            return format!(r#"{{"v":1,"rules":[{}]}}"#, rules.join(","));
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
        assert!(cases.len() > 80);
        for case in cases {
            let text = case_text(case);
            let got = check_file(text.as_bytes(), &vocab).err();
            let want = case["error"].as_str().map(str::to_string);
            assert_eq!(got, want, "{}", case["name"]);
        }
    }

    #[test]
    fn canonical_text_matches_the_shared_cases() {
        let all: Value = serde_json::from_str(CASES).unwrap();
        for c in all["canonical"].as_array().unwrap() {
            let rules = c["rules"].as_array().unwrap();
            assert_eq!(canonical_text(rules), c["text"].as_str().unwrap());
        }
    }

    #[test]
    fn a_checked_file_comes_back_typed() {
        let all: Value = serde_json::from_str(CASES).unwrap();
        let vocab = Vocab::from_json(&all["vocab"]);
        let text = br#"{"v":1,"rules":[{"id":"spike-hurts","when":{"touch":["Player","Spike"]},"if":[{"chance":50}],"do":[{"add":["lives",-1]},{"respawn":"Player"},{"move":["self",2.5,0]},{"spawn":["Coin",{"x":1,"y":-2}]},{"end_run":"win"}],"source":"x","off":true}]}"#;
        let file = check_file(text, &vocab).unwrap();
        assert_eq!(
            file.rules[0],
            Rule {
                id: "spike-hurts".into(),
                when: Trigger::Touch("Player".into(), "Spike".into()),
                conds: vec![Cond::Chance(50)],
                acts: vec![
                    Act::Add("lives".into(), -1),
                    Act::Respawn(Target::Kind("Player".into())),
                    Act::Move(Target::Me, 2.5, 0.0),
                    Act::Spawn("Coin".into(), Place::At(1.0, -2.0)),
                    Act::EndRun(true),
                ],
                source: Some("x".into()),
                off: true,
            }
        );
    }

    #[test]
    fn comparisons_hold_as_written() {
        assert!(Op::Lt.holds(1, 2) && !Op::Lt.holds(2, 2));
        assert!(Op::Le.holds(2, 2) && Op::Eq.holds(3, 3) && Op::Ge.holds(3, 3));
        assert!(Op::Gt.holds(4, 3) && !Op::Gt.holds(3, 3));
    }
}
