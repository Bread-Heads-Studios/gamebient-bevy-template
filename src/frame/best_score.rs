//! The native score to beat. The template keeps no score between launches
//! and a native build has no host to ask, so the frame stores the best
//! score in one small text file: a decimal integer and a newline.
//!
//! Location, first match wins:
//! 1. `$GX_DATA_DIR/best-score`
//! 2. `$XDG_DATA_HOME/gamebient/<package name>/best-score`
//! 3. `$HOME/.local/share/gamebient/<package name>/best-score`
//!
//! With none of those set the best score lasts for the session only.
//!
//! The stored score serves both orders: `game::scoring::HIGHER_SCORE_IS_BETTER`
//! picks the comparison (see `driver::is_record`); 0 means no best yet.

use std::path::{Path, PathBuf};

const FILE_NAME: &str = "best-score";

fn set(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|v| !v.is_empty())
}

/// Where the best score lives, from the three environment values.
pub fn best_score_path(
    gx_data_dir: Option<&str>,
    xdg_data_home: Option<&str>,
    home: Option<&str>,
    game: &str,
) -> Option<PathBuf> {
    if let Some(dir) = set(gx_data_dir) {
        return Some(PathBuf::from(dir).join(FILE_NAME));
    }
    if let Some(dir) = set(xdg_data_home) {
        return Some(
            PathBuf::from(dir)
                .join("gamebient")
                .join(game)
                .join(FILE_NAME),
        );
    }
    set(home).map(|dir| {
        PathBuf::from(dir)
            .join(".local")
            .join("share")
            .join("gamebient")
            .join(game)
            .join(FILE_NAME)
    })
}

/// [`best_score_path`] for this process and this game.
pub fn best_score_path_from_env() -> Option<PathBuf> {
    let var = |name: &str| std::env::var(name).ok();
    best_score_path(
        var("GX_DATA_DIR").as_deref(),
        var("XDG_DATA_HOME").as_deref(),
        var("HOME").as_deref(),
        env!("CARGO_PKG_NAME"),
    )
}

/// A missing, empty or damaged file is a best score of 0.
pub fn parse_best(text: &str) -> u64 {
    text.trim().parse().unwrap_or(0)
}

pub fn load_best(path: &Path) -> u64 {
    std::fs::read_to_string(path)
        .map(|text| parse_best(&text))
        .unwrap_or(0)
}

/// Writes through a temporary file and a rename, so a power cut mid-write
/// leaves the old score rather than an empty file.
pub fn save_best(path: &Path, best: u64) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, format!("{best}\n"))?;
    std::fs::rename(&temporary, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gx_data_dir_wins() {
        assert_eq!(
            best_score_path(
                Some("/data/gx"),
                Some("/xdg"),
                Some("/home/pi"),
                "voidrunner"
            ),
            Some(PathBuf::from("/data/gx/best-score"))
        );
    }

    #[test]
    fn xdg_data_home_is_second() {
        assert_eq!(
            best_score_path(None, Some("/xdg"), Some("/home/pi"), "voidrunner"),
            Some(PathBuf::from("/xdg/gamebient/voidrunner/best-score"))
        );
    }

    #[test]
    fn home_is_last() {
        assert_eq!(
            best_score_path(None, None, Some("/home/pi"), "voidrunner"),
            Some(PathBuf::from(
                "/home/pi/.local/share/gamebient/voidrunner/best-score"
            ))
        );
    }

    #[test]
    fn empty_variables_count_as_unset() {
        assert_eq!(
            best_score_path(Some(""), Some("  "), Some("/home/pi"), "voidrunner"),
            Some(PathBuf::from(
                "/home/pi/.local/share/gamebient/voidrunner/best-score"
            ))
        );
        assert_eq!(best_score_path(None, None, None, "voidrunner"), None);
    }

    #[test]
    fn parse_accepts_an_integer_and_nothing_else() {
        assert_eq!(parse_best("12340\n"), 12340);
        assert_eq!(parse_best("  7  "), 7);
        assert_eq!(parse_best(""), 0);
        assert_eq!(parse_best("-5"), 0);
        assert_eq!(parse_best("12.5"), 0);
        assert_eq!(parse_best("abc"), 0);
    }

    #[test]
    fn save_then_load_round_trips_and_creates_the_directory() {
        let dir = std::env::temp_dir().join(format!("gx-best-score-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("nested").join("best-score");
        assert_eq!(load_best(&path), 0);
        save_best(&path, 12340).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "12340\n");
        assert_eq!(load_best(&path), 12340);
        save_best(&path, 99).unwrap();
        assert_eq!(load_best(&path), 99);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
