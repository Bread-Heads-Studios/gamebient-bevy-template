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
}
