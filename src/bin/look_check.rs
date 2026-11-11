//! `cargo run --bin look-check -- <pack folder>...`
//! Checks Look Packs against this game's contract (`ui::theme::LOOK`) and
//! the pack limits, prints every problem and each pack's content hash.
//!
//! `build_web.sh` and the editor and verify builds compile every bin for
//! wasm32, where there is no filesystem, so the CLI is native-only and the
//! wasm build gets an empty `main`.

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::process::ExitCode {
    use std::path::Path;
    use std::process::ExitCode;

    use gamebient_game::look::pack;
    use gamebient_game::look::validate::{Severity, validate};
    use gamebient_game::ui::theme;

    let dirs: Vec<String> = std::env::args().skip(1).collect();
    if dirs.is_empty() {
        eprintln!("usage: look-check <pack folder>...");
        return ExitCode::from(2);
    }
    let mut failed = false;
    for d in &dirs {
        let path = Path::new(d.trim_end_matches('/'));
        let id = path.file_name().and_then(|n| n.to_str());
        let files = match pack::read_dir(path) {
            Ok(f) => f,
            Err(e) => {
                println!("{d}: error: pack: {e}");
                failed = true;
                continue;
            }
        };
        let report = validate(&files, &theme::LOOK, id);
        for p in &report.problems {
            let tag = match p.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
            };
            println!("{d}: {tag}: {}: {}", p.at, p.message);
        }
        if report.ok() {
            println!("{d}: ok, content_hash {}", report.content_hash);
        } else {
            failed = true;
        }
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
