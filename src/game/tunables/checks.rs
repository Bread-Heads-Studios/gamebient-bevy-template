//! Scenario checks a card can ask the editor to re-run: named, titled in
//! plain words, and **never panicking** (they run inside the wasm game,
//! where a panic aborts everything). A check builds its own headless stage,
//! applies the tunable values it is handed, and answers `Ok(())` or a
//! plain-words `Err`. `cargo test` runs every registered check at the
//! defaults through [`tests::every_registered_check_passes_at_defaults`].
//!
//! ```ignore
//! fn coin_is_collected(t: &Tunes) -> Result<(), String> {
//!     let mut s = stage(t);
//!     s.place("Coin", 40.0, -290.0).start().hold(Buttons::RIGHT, 30);
//!     ensure!(s.coins() == 1, "the coin was not picked up");
//!     Ok(())
//! }
//! ```

use bevy::prelude::*;

use super::value::Value;

/// The tunable values a check runs under: `(path, value)`, every tunable.
pub type Tunes = Vec<(String, Value)>;

/// One check. `run` must not panic.
pub struct Check {
    pub name: &'static str,
    pub title: &'static str,
    pub run: fn(&Tunes) -> Result<(), String>,
}

/// The game's checks, inserted by its plugin (an empty list in the template).
#[derive(Resource, Default)]
pub struct CheckList(pub &'static [Check]);

/// Early return with a plain-words message when `cond` is false.
#[macro_export]
macro_rules! ensure {
    ($cond:expr, $($arg:tt)*) => {
        if !$cond {
            return Err(format!($($arg)*));
        }
    };
}

/// Runs the named checks in order. An unknown name answers
/// `Err("no check named …")` rather than being skipped, so a card that
/// names a check the game lost says so.
pub fn run_all(
    checks: &[Check],
    names: &[&str],
    tunes: &Tunes,
) -> Vec<(String, Result<(), String>)> {
    names
        .iter()
        .map(|n| {
            let r = match checks.iter().find(|c| c.name == *n) {
                Some(c) => (c.run)(tunes),
                None => Err(format!("no check named {n}")),
            };
            (n.to_string(), r)
        })
        .collect()
}

/// The value a check was handed for `path`, if any.
pub fn tune_value<'a>(tunes: &'a Tunes, path: &str) -> Option<&'a Value> {
    tunes.iter().find(|(p, _)| p == path).map(|(_, v)| v)
}

#[cfg(test)]
pub mod tests {
    use super::*;

    fn passes(_: &Tunes) -> Result<(), String> {
        Ok(())
    }
    fn fails_when_zero(t: &Tunes) -> Result<(), String> {
        ensure!(
            tune_value(t, "ammo.start") != Some(&Value::Int(0)),
            "B never fires with 0 ammo"
        );
        Ok(())
    }
    const CHECKS: &[Check] = &[
        Check {
            name: "a",
            title: "A passes",
            run: passes,
        },
        Check {
            name: "b",
            title: "B fires",
            run: fails_when_zero,
        },
    ];

    #[test]
    fn run_all_reports_each_named_check_and_unknown_names() {
        let tunes = vec![("ammo.start".to_string(), Value::Int(0))];
        let out = run_all(CHECKS, &["a", "b", "zzz"], &tunes);
        assert_eq!(out[0], ("a".into(), Ok(())));
        assert_eq!(
            out[1],
            ("b".into(), Err("B never fires with 0 ammo".into()))
        );
        assert_eq!(out[2].1, Err("no check named zzz".into()));
    }

    /// Every check the game registers passes under its default values.
    /// Inherited by every game: a check that fails at the defaults is a
    /// rule the game no longer follows, or a check that no longer fits.
    #[test]
    fn every_registered_check_passes_at_defaults() {
        use crate::game::replay::build_headless_app;
        use crate::game::tunables::{TunableIndex, snapshot};
        let app = build_headless_app();
        let idx = app.world().resource::<TunableIndex>().clone();
        let tunes: Tunes = snapshot(app.world())
            .into_iter()
            .filter_map(|(id, v)| idx.by_id(id).map(|e| (e.path.clone(), v)))
            .collect();
        let list = app.world().resource::<CheckList>().0;
        let names: Vec<&str> = list.iter().map(|c| c.name).collect();
        let failed: Vec<_> = run_all(list, &names, &tunes)
            .into_iter()
            .filter_map(|(n, r)| r.err().map(|e| format!("{n}: {e}")))
            .collect();
        assert!(
            failed.is_empty(),
            "checks failing at the defaults:\n{}",
            failed.join("\n")
        );
    }
}
