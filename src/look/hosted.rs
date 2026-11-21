//! Hosted looks: a pack published on the ColecoVision GX site, picked by the
//! host page and swapped in mid-run. The host sends `gx:custom` `look.set`
//! `{id, base, hash, files}` (or `look.clear`); this module parses it under
//! the trust rule below. `mod.rs` wires the system that fetches the files
//! (`fetch::start_hosted`, web only) and the `look.applied` answer
//! (`fetch::arrive`).
//!
//! **Trust:** anyone can send the command, so a pack is fetched only from
//! `HOSTED_ORIGIN` (compiled in; only a deployed website commit can put
//! files there) and applied only if its content hash matches `hash` and it
//! passes the validator. Presentation only: the sim never sees a look.

use gamebient_input::HostEvent;
use serde::Deserialize;

use super::names;
use super::validate::limits;

/// Where hosted packs live. `GX_LOOK_ORIGIN` at build time overrides it
/// (staging, or `http://localhost:3000/looks/` for a local website).
pub const HOSTED_ORIGIN: &str = match option_env!("GX_LOOK_ORIGIN") {
    Some(origin) => origin,
    None => "https://colecovisiongx.com/looks/",
};

pub const SET: &str = "look.set";
pub const CLEAR: &str = "look.clear";
pub const APPLIED: &str = "look.applied";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostedLook {
    pub id: String,
    /// Ends in `/`; under the hosted origin.
    pub base: String,
    pub hash: String,
    pub files: Vec<String>,
}

impl HostedLook {
    /// `(file name, URL)` for every file, in the command's order.
    pub fn urls(&self) -> Vec<(String, String)> {
        self.files
            .iter()
            .map(|f| (f.clone(), format!("{}{f}", self.base)))
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostedCommand {
    Set(HostedLook),
    Clear,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// `base` is not under the hosted origin.
    Origin,
    /// Too many files.
    Caps,
    /// Anything else malformed.
    Invalid(String),
}

impl Refusal {
    /// The `reason` field of `look.applied`.
    pub fn reason(&self) -> String {
        match self {
            Refusal::Origin => "origin".into(),
            Refusal::Caps => "caps".into(),
            Refusal::Invalid(why) => format!("invalid: {why}"),
        }
    }
}

#[derive(Deserialize)]
struct Wire {
    id: String,
    base: String,
    hash: String,
    files: Vec<String>,
}

/// `None` unless `name` is `look.set` or `look.clear`. On refusal, the id
/// the command carried (or `""`) so the answer can name it.
pub fn parse(
    name: &str,
    data: &str,
    origin: &str,
) -> Option<Result<HostedCommand, (String, Refusal)>> {
    match name {
        CLEAR => Some(Ok(HostedCommand::Clear)),
        SET => Some(parse_set(data, origin)),
        _ => None,
    }
}

fn parse_set(data: &str, origin: &str) -> Result<HostedCommand, (String, Refusal)> {
    let wire: Wire = serde_json::from_str(data)
        .map_err(|e| (String::new(), Refusal::Invalid(format!("data: {e}"))))?;
    let id = wire.id.clone();
    let fail = |r: Refusal| (id.clone(), r);
    if !names::is_id(&wire.id) {
        return Err(fail(Refusal::Invalid("id".into())));
    }
    if !base_ok(&wire.base, origin) {
        return Err(fail(Refusal::Origin));
    }
    if wire.hash.len() != 64
        || !wire
            .hash
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err(fail(Refusal::Invalid("hash".into())));
    }
    if wire.files.len() > limits::FILES_MAX {
        return Err(fail(Refusal::Caps));
    }
    if !wire.files.iter().any(|f| f == names::MANIFEST_FILE) {
        return Err(fail(Refusal::Invalid("files: no look.json".into())));
    }
    let mut seen = std::collections::BTreeSet::new();
    for f in &wire.files {
        if !(f == names::MANIFEST_FILE || names::is_file_name(f)) || !seen.insert(f) {
            return Err(fail(Refusal::Invalid(format!("files: {f}"))));
        }
    }
    Ok(HostedCommand::Set(HostedLook {
        id: wire.id,
        base: wire.base,
        hash: wire.hash,
        files: wire.files,
    }))
}

/// Under `origin`, ending in `/`, and the rest is plain path characters
/// with no `..` (so no query, fragment, backslash or escape out of it).
fn base_ok(base: &str, origin: &str) -> bool {
    let Some(rest) = base.strip_prefix(origin) else {
        return false;
    };
    base.ends_with('/')
        && !rest.contains("..")
        && rest
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.' | b'/'))
}

/// The `look.applied` answer to the host.
pub fn applied(id: &str, outcome: Result<(), String>) -> HostEvent {
    let data = match outcome {
        Ok(()) => serde_json::json!({ "id": id, "ok": true }),
        Err(reason) => serde_json::json!({ "id": id, "ok": false, "reason": reason }),
    };
    HostEvent::Custom {
        name: APPLIED.into(),
        data: data.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const O: &str = "https://colecovisiongx.com/looks/";
    const H: &str = "fde29d4708776830c953b2183ec382c02e22eceb45cbe37d63fbf6c82e09002b";

    fn set(id: &str, base: &str, hash: &str, files: &[&str]) -> String {
        serde_json::json!({ "id": id, "base": base, "hash": hash, "files": files }).to_string()
    }
    fn good() -> String {
        set(
            "fizzix",
            &format!("{O}voidrunner/{H}/"),
            H,
            &["look.json", "logo.png"],
        )
    }

    #[test]
    fn other_names_are_not_ours() {
        assert!(parse("tunables.set", "{}", O).is_none());
        assert!(parse("look.other", "{}", O).is_none());
    }

    #[test]
    fn a_good_set_parses() {
        let Some(Ok(HostedCommand::Set(look))) = parse(SET, &good(), O) else {
            panic!("expected a set");
        };
        assert_eq!(look.id, "fizzix");
        assert_eq!(look.hash, H);
        assert_eq!(
            look.urls()[1],
            (
                "logo.png".to_string(),
                format!("{O}voidrunner/{H}/logo.png")
            )
        );
    }

    #[test]
    fn clear_ignores_its_data() {
        assert_eq!(parse(CLEAR, "null", O), Some(Ok(HostedCommand::Clear)));
        assert_eq!(parse(CLEAR, "{\"x\":1}", O), Some(Ok(HostedCommand::Clear)));
    }

    fn refusal(data: &str) -> (String, Refusal) {
        match parse(SET, data, O) {
            Some(Err(e)) => e,
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    #[test]
    fn bases_off_the_origin_are_refused() {
        for base in [
            format!("https://evil.example/looks/voidrunner/{H}/"),
            format!("https://colecovisiongx.com.evil.example/looks/{H}/"),
            format!("http://colecovisiongx.com/looks/voidrunner/{H}/"),
            format!("{O}voidrunner/../../{H}/"),
            format!("{O}voidrunner/{H}/?x=1/"),
            format!("{O}voidrunner/{H}/#/"),
            format!("{O}voidrunner/{H}"),
            format!("{O}voidrunner\\{H}/"),
        ] {
            let (id, r) = refusal(&set("fizzix", &base, H, &["look.json"]));
            assert_eq!(r, Refusal::Origin, "{base}");
            assert_eq!(id, "fizzix");
        }
    }

    #[test]
    fn too_many_files_is_caps() {
        let names: Vec<String> = (0..=limits::FILES_MAX)
            .map(|i| format!("f{i}.png"))
            .collect();
        let mut files: Vec<&str> = vec!["look.json"];
        files.extend(names.iter().map(String::as_str));
        let (_, r) = refusal(&set("fizzix", &format!("{O}v/{H}/"), H, &files));
        assert_eq!(r, Refusal::Caps);
    }

    #[test]
    fn bad_fields_are_invalid() {
        let base = format!("{O}v/{H}/");
        for data in [
            "not json".to_string(),
            set("Fizzix", &base, H, &["look.json"]),
            set("", &base, H, &["look.json"]),
            set("fizzix", &base, &H.to_uppercase(), &["look.json"]),
            set("fizzix", &base, "abc", &["look.json"]),
            set("fizzix", &base, H, &[]),
            set("fizzix", &base, H, &["logo.png"]),
            set("fizzix", &base, H, &["look.json", "look.json"]),
            set("fizzix", &base, H, &["look.json", "../x.png"]),
            set("fizzix", &base, H, &["look.json", "font.ttf"]),
        ] {
            let (_, r) = refusal(&data);
            assert!(matches!(r, Refusal::Invalid(_)), "{data}: {r:?}");
        }
    }

    #[test]
    fn reasons_and_events_match_the_protocol() {
        assert_eq!(Refusal::Origin.reason(), "origin");
        assert_eq!(Refusal::Caps.reason(), "caps");
        assert_eq!(
            Refusal::Invalid("bad hash".into()).reason(),
            "invalid: bad hash"
        );
        let gamebient_input::HostEvent::Custom { name, data } = applied("fizzix", Ok(())) else {
            panic!()
        };
        assert_eq!(name, APPLIED);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&data).unwrap(),
            serde_json::json!({ "id": "fizzix", "ok": true })
        );
        let gamebient_input::HostEvent::Custom { data, .. } = applied("x", Err("hash".into()))
        else {
            panic!()
        };
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&data).unwrap(),
            serde_json::json!({ "id": "x", "ok": false, "reason": "hash" })
        );
    }

    #[test]
    fn the_production_origin_is_the_default() {
        if option_env!("GX_LOOK_ORIGIN").is_none() {
            assert_eq!(HOSTED_ORIGIN, "https://colecovisiongx.com/looks/");
        }
    }
}
