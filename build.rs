use std::process::Command;

fn main() {
    let sha = Command::new("git")
        .args(["rev-parse", "--short=7", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            std::env::var("VERCEL_GIT_COMMIT_SHA")
                .ok()
                .map(|s| s.chars().take(7).collect())
                .filter(|s: &String| !s.is_empty())
        })
        .unwrap_or_else(|| "unknown".into());
    println!(
        "cargo:rustc-env=GX_BUILD_ID={}+{sha}",
        std::env::var("CARGO_PKG_VERSION").unwrap()
    );
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs");
    println!("cargo:rerun-if-env-changed=VERCEL_GIT_COMMIT_SHA");
    embed_content();
}

/// Writes `$OUT_DIR/content_files.rs`: every file under `content/` except
/// `content/schema/`, as `(path relative to content/, bytes)` sorted by path.
fn embed_content() {
    use std::path::{Path, PathBuf};

    fn walk(dir: &Path, rel: &str, out: &mut Vec<(String, PathBuf)>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let path = e.path();
            let rel_path = if rel.is_empty() {
                name.clone()
            } else {
                format!("{rel}/{name}")
            };
            if path.is_dir() {
                if rel.is_empty() && name == "schema" {
                    continue;
                }
                walk(&path, &rel_path, out);
            } else {
                out.push((rel_path, path));
            }
        }
    }

    let root = Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("content");
    let mut files = Vec::new();
    walk(&root, "", &mut files);
    files.sort_by(|a, b| a.0.cmp(&b.0));
    let mut code = String::from("pub static EMBEDDED: &[(&str, &[u8])] = &[\n");
    for (rel, abs) in &files {
        code.push_str(&format!(
            "    ({rel:?}, include_bytes!({:?})),\n",
            abs.to_string_lossy()
        ));
    }
    code.push_str("];\n");
    let out = Path::new(&std::env::var("OUT_DIR").unwrap()).join("content_files.rs");
    std::fs::write(out, code).unwrap();
    println!("cargo:rerun-if-changed=content");
}
