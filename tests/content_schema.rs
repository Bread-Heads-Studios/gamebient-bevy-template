//! The JSON schemas in `content/schema/` and the Rust parsers in
//! `game::content::model` must agree on every sample. Rules a schema cannot
//! state (unique ids, byte limits) are tested in the model's unit tests.

use gamebient_game::game::content::model::{
    LevelFile, SfxFile, SongsFile, SpritesFile, TunablesFile,
};

fn schema(name: &str) -> jsonschema::Validator {
    let path = format!(
        "{}/content/schema/{name}.schema.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(path).unwrap();
    jsonschema::validator_for(&serde_json::from_str(&text).unwrap()).unwrap()
}

fn agree<T, E>(name: &str, parse: fn(&[u8]) -> Result<T, E>, valid: &[&str], invalid: &[&str]) {
    let v = schema(name);
    for (samples, want) in [(valid, true), (invalid, false)] {
        for s in samples {
            let json: serde_json::Value = serde_json::from_str(s).unwrap();
            assert_eq!(v.is_valid(&json), want, "{name} schema on {s}");
            assert_eq!(parse(s.as_bytes()).is_ok(), want, "{name} parser on {s}");
        }
    }
}

#[test]
fn schema_files_are_valid_schemas() {
    for n in ["level", "tunables", "sprites", "songs", "sfx"] {
        schema(n);
    }
}

#[test]
fn level() {
    agree(
        "level",
        LevelFile::parse,
        &[
            r#"{"v":1,"things":[]}"#,
            r#"{"v":1,"things":[{"id":"p1","kind":"Player","at":[1,2]}]}"#,
            r#"{"v":1,"things":[{"id":"a_b-9","kind":"Platform","at":[-3.5,0],"size":[10,2]}]}"#,
        ],
        &[
            r#"{"v":2,"things":[]}"#,
            r#"{"things":[]}"#,
            r#"{"v":1,"things":[],"extra":1}"#,
            r#"{"v":1,"things":[{"id":"P","kind":"K","at":[0,0]}]}"#,
            r#"{"v":1,"things":[{"id":"","kind":"K","at":[0,0]}]}"#,
            r#"{"v":1,"things":[{"id":"a","kind":"1K","at":[0,0]}]}"#,
            r#"{"v":1,"things":[{"id":"a","kind":"K","at":[0]}]}"#,
            r#"{"v":1,"things":[{"id":"a","kind":"K","at":[0,0],"size":[0,1]}]}"#,
            r#"{"v":1,"things":[{"id":"a","kind":"K","at":[0,0],"size":[1,-1]}]}"#,
            r#"{"v":1,"things":[{"id":"a","kind":"K","at":[0,0],"z":1}]}"#,
            r#"{"v":1,"things":[{"id":"a","at":[0,0]}]}"#,
        ],
    );
    // The 2000-thing cap agrees too.
    let things = |n: usize| {
        let t: Vec<String> = (0..n)
            .map(|i| format!(r#"{{"id":"t{i}","kind":"K","at":[0,0]}}"#))
            .collect();
        format!(r#"{{"v":1,"things":[{}]}}"#, t.join(","))
    };
    let (ok, bad) = (things(2000), things(2001));
    agree("level", LevelFile::parse, &[&ok], &[&bad]);
}

#[test]
fn tunables() {
    agree(
        "tunables",
        TunablesFile::parse,
        &[
            r#"{"v":1,"values":{}}"#,
            r#"{"v":1,"values":{"player.speed":4.5,"flag":true,"name":"x","tint":[1,0.5,0]}}"#,
        ],
        &[
            r#"{"v":1}"#,
            r#"{"v":3,"values":{}}"#,
            r#"{"v":1,"values":{"a":null}}"#,
            r#"{"v":1,"values":{"a":{"b":1}}}"#,
            r#"{"v":1,"values":{"a":[1,"x"]}}"#,
            r#"{"v":1,"values":{"a":[[1]]}}"#,
            r#"{"v":1,"values":{"":1}}"#,
        ],
    );
}

#[test]
fn sprites() {
    agree(
        "sprites",
        SpritesFile::parse,
        &[
            r#"{"v":1,"sprites":{}}"#,
            r#"{"v":1,"sprites":{"Player":{}}}"#,
            r#"{"v":1,"sprites":{"Coin":{"frames":4,"fps":12,"scale":2.5,"anchor":[0.5,1]}}}"#,
        ],
        &[
            r#"{"v":1,"sprites":{"Coin":{"frames":0}}}"#,
            r#"{"v":1,"sprites":{"Coin":{"frames":1.5}}}"#,
            r#"{"v":1,"sprites":{"Coin":{"scale":0}}}"#,
            r#"{"v":1,"sprites":{"Coin":{"scale":-2}}}"#,
            r#"{"v":1,"sprites":{"Coin":{"fps":0}}}"#,
            r#"{"v":1,"sprites":{"Coin":{"anchor":[2,0]}}}"#,
            r#"{"v":1,"sprites":{"Coin":{"colour":1}}}"#,
            r#"{"v":1,"sprites":{"9bad":{}}}"#,
            r#"{"v":2,"sprites":{}}"#,
        ],
    );
}

const SONG: &str = r#"{"bpm":120,"beats_per_bar":4,"steps_per_beat":4,"bars":4,"voices":["a","b","c"],"instruments":[{"env":[15,8,0]}]}"#;

#[test]
fn songs() {
    let song = |from: &str, to: &str| {
        format!(r#"{{"v":1,"songs":{{"Menu":{}}}}}"#, SONG.replace(from, to))
    };
    let valid = [
        r#"{"v":1,"songs":{}}"#.to_string(),
        song("", ""),
        song(r#""bpm":120"#, r#""bpm":30"#),
        song(r#""bars":4"#, r#""bars":64"#),
        song(
            r#"{"env":[15,8,0]}"#,
            r#"{"env":[15],"release":3,"arp":[0,4,7],"vibrato":[5,0.5],"noise":["periodic","div2048"]}"#,
        ),
        song(
            r#""bars":4"#,
            r#""bars":4,"noise":"x","noise_instruments":[{"env":[1]}]"#,
        ),
    ];
    let invalid = [
        song(r#""bpm":120"#, r#""bpm":29"#),
        song(r#""bpm":120"#, r#""bpm":301"#),
        song(r#""bars":4"#, r#""bars":0"#),
        song(r#""bars":4"#, r#""bars":65"#),
        song(r#"["a","b","c"]"#, r#"["a","b"]"#),
        song(r#"["a","b","c"]"#, r#"["a","b","c","d"]"#),
        song(r#"[15,8,0]"#, r#"[16]"#),
        song(r#"[15,8,0]"#, r#"[]"#),
        song(r#""bars":4"#, r#""bars":4,"mood":1"#),
        song(r#"{"env":[15,8,0]}"#, r#"{"env":[1],"release":256}"#),
        song(r#"{"env":[15,8,0]}"#, r#"{"env":[1],"arp":[200]}"#),
        song(r#"{"env":[15,8,0]}"#, r#"{"env":[1],"vibrato":[1]}"#),
        song(r#"{"env":[15,8,0]}"#, r#"{"env":[1],"noise":["white"]}"#),
        song(
            r#"{"env":[15,8,0]}"#,
            r#"{"env":[1],"noise":["white","low"]}"#,
        ),
        song(
            r#"{"env":[15,8,0]}"#,
            r#"{"env":[1],"noise":["loud","div512"]}"#,
        ),
        r#"{"v":1,"songs":{"1x":{}}}"#.to_string(),
    ];
    let valid: Vec<&str> = valid.iter().map(String::as_str).collect();
    let invalid: Vec<&str> = invalid.iter().map(String::as_str).collect();
    agree("songs", SongsFile::parse, &valid, &invalid);
}

#[test]
fn sfx() {
    let wrap = |op: &str| format!(r#"{{"v":1,"sfx":{{"Jump":[[{op}]]}}}}"#);
    let valid = [
        r#"{"v":1,"sfx":{}}"#.to_string(),
        wrap(r#"{"op":"tone","voice":0,"from":"C4","to":440.5,"a0":15,"a1":0,"frames":6}"#),
        wrap(r#"{"op":"hit","voice":2,"note":880,"frames":600}"#),
        wrap(r#"{"op":"slide","voice":1,"from":"C4","to":"C5","frames":1}"#),
        wrap(r#"{"op":"noise","mode":"white","rate":"div512","a0":8,"a1":0,"frames":3}"#),
        wrap(r#"{"op":"noise","mode":"periodic","rate":"tone3","a0":8,"a1":0,"frames":3}"#),
        wrap(r#"{"op":"rest","frames":2}"#),
        r#"{"v":1,"sfx":{"Win":[[{"op":"rest","frames":1}],[{"op":"rest","frames":2},{"op":"rest","frames":3}]]}}"#
            .to_string(),
    ];
    let invalid = [
        wrap(r#"{"op":"hit","voice":3,"note":"C4","frames":1}"#),
        wrap(r#"{"op":"hit","voice":0,"note":"C4","frames":0}"#),
        wrap(r#"{"op":"hit","voice":0,"note":"C4","frames":601}"#),
        wrap(r#"{"op":"tone","voice":0,"from":1,"to":2,"a0":16,"a1":0,"frames":1}"#),
        wrap(r#"{"op":"tone","voice":0,"from":1,"to":2,"a0":1,"frames":1}"#),
        wrap(r#"{"op":"rest"}"#),
        wrap(r#"{"op":"rest","frames":1,"voice":0}"#),
        wrap(r#"{"op":"boom","frames":1}"#),
        wrap(r#"{"op":"slide","voice":0,"from":0,"to":2,"frames":1}"#),
        wrap(r#"{"op":"noise","mode":"","rate":"div512","a0":1,"a1":0,"frames":1}"#),
        wrap(r#"{"op":"noise","mode":"white","rate":"low","a0":1,"a1":0,"frames":1}"#),
        wrap(r#"{"op":"noise","mode":"pink","rate":"div512","a0":1,"a1":0,"frames":1}"#),
        r#"{"v":1,"sfx":{"X":[]}}"#.to_string(),
        r#"{"v":1,"sfx":{"X":[[]]}}"#.to_string(),
        r#"{"v":1,"sfx":{"1x":[[{"op":"rest","frames":1}]]}}"#.to_string(),
    ];
    let valid: Vec<&str> = valid.iter().map(String::as_str).collect();
    let invalid: Vec<&str> = invalid.iter().map(String::as_str).collect();
    agree("sfx", SfxFile::parse, &valid, &invalid);
}

#[test]
fn built_in_songs_written_as_data_fit_the_songs_schema() {
    use gamebient_game::game::audio::songs;
    use gamebient_game::game::content::audio::data_from_song;
    let mut map = serde_json::Map::new();
    for (name, s) in [
        ("menu", &songs::MENU),
        ("play", &songs::PLAY),
        ("play_layer", &songs::PLAY_LAYER),
        ("game_over", &songs::GAME_OVER),
        ("demo_end", &songs::DEMO_END),
    ] {
        map.insert(
            name.into(),
            serde_json::to_value(data_from_song(s)).unwrap(),
        );
    }
    let file = serde_json::json!({ "v": 1, "songs": map });
    let v = schema("songs");
    assert!(
        v.is_valid(&file),
        "{:?}",
        v.iter_errors(&file)
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
    );
}
