//! Which pack a launch asks for:
//! - native: `--look <id|dir>`, else `GX_LOOK=<id|dir>`;
//! - web: `?look=<id>`, built-in packs only.
//!
//! A value with a `/`, a `\` or a leading `.` is a folder on disk (native
//! only, live-reloaded). `default` or nothing means the game's own look.

use bevy::prelude::*;

#[derive(Resource, Debug, Clone, PartialEq, Eq, Default)]
pub enum Selection {
    #[default]
    Default,
    /// A pack built into this game (`assets/looks/<id>/`).
    Embedded(String),
    /// A pack folder on disk (native development; hot-reloaded).
    Dir(String),
}

fn classify(value: Option<String>, allow_dir: bool) -> Selection {
    match value {
        None => Selection::Default,
        Some(v) if v.is_empty() || v == "default" => Selection::Default,
        Some(v) if v.contains('/') || v.contains('\\') || v.starts_with('.') || v.contains(':') => {
            if allow_dir {
                Selection::Dir(v)
            } else {
                Selection::Default
            }
        }
        Some(v) => Selection::Embedded(v),
    }
}

pub fn from_args_env(args: &[String], env: Option<&str>) -> Selection {
    let flag = args
        .windows(2)
        .find(|w| w[0] == "--look")
        .map(|w| w[1].clone());
    classify(flag.or_else(|| env.map(str::to_string)), true)
}

pub fn from_query(search: &str) -> Selection {
    let value = search
        .trim_start_matches('?')
        .split('&')
        .find_map(|pair| pair.strip_prefix("look=").map(str::to_string));
    classify(value, false)
}

/// The selection this process was launched with.
pub fn from_launch() -> Selection {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let args: Vec<String> = std::env::args().collect();
        from_args_env(&args, std::env::var("GX_LOOK").ok().as_deref())
    }
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window()
            .and_then(|w| w.location().search().ok())
            .map_or(Selection::Default, |q| from_query(&q))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn native_prefers_the_flag_over_the_env() {
        assert_eq!(from_args_env(&args(&["game"]), None), Selection::Default);
        assert_eq!(
            from_args_env(&args(&["game"]), Some("night-shift")),
            Selection::Embedded("night-shift".into())
        );
        assert_eq!(
            from_args_env(
                &args(&["game", "--look", "./packs/fae"]),
                Some("night-shift")
            ),
            Selection::Dir("./packs/fae".into())
        );
        assert_eq!(
            from_args_env(&args(&["game", "--look", "default"]), None),
            Selection::Default
        );
        assert_eq!(
            from_args_env(&args(&["game"]), Some("")),
            Selection::Default
        );
    }

    #[test]
    fn web_takes_built_in_ids_only() {
        assert_eq!(
            from_query("?mute=1&look=night-shift"),
            Selection::Embedded("night-shift".into())
        );
        assert_eq!(from_query("?look=../../etc"), Selection::Default);
        assert_eq!(
            from_query("?look=https://evil.example/pack"),
            Selection::Default
        );
        assert_eq!(from_query("?mute=1"), Selection::Default);
    }
}
