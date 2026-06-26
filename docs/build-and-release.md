# Build & Release

How this template builds for three targets and ships releases. See
[../AGENTS.md](../AGENTS.md) for the short version and the gotchas.

## Targets

| Name  | Rust target                  | How it builds                                   |
|-------|------------------------------|-------------------------------------------------|
| `web` | `wasm32-unknown-unknown`     | `build_web.sh`: cargo → `wasm-bindgen` → `wasm-opt -Oz` → brotli |
| `x86` | `x86_64-unknown-linux-gnu`   | native `cargo build` on Linux; `cross` off-Linux |
| `pi`  | `aarch64-unknown-linux-gnu`  | `cross build` (Docker)                          |

## Scripts

- **`build.sh <pi|x86|web>`** — compiles exactly one target. `web` execs
  `build_web.sh` so Vercel's `buildCommand` stays a single entry point.
- **`build_web.sh`** — builds the wasm bundle into `dist/`, finds the compiled
  `.wasm` by glob and runs `wasm-bindgen --out-name gamebient-game` for stable
  output names, optimizes with `wasm-opt` (with the required feature flags),
  brotli-compresses via Node, copies `assets/`, and invokes `fetch-cartridge.sh`.
- **`package.sh <pi|x86>`** — bundles `target/<triple>/release/gamebient-game` +
  `assets/` + a generated `run.sh` launcher into `build/gamebient-game-<target>.tar.gz`.
  The Pi launcher sets `WINIT_UNIX_BACKEND=x11`; x86 lets winit auto-select.
- **`fetch-cartridge.sh`** — downloads the flat `gamebient-game.tar.gz` from the
  latest GitHub release into `dist/assets/`. Non-fatal and private-repo-safe
  (resolves the asset through the GitHub API with `GH_TOKEN`).
- **`Cross.toml`** — installs each target's dev libraries (ALSA/udev/wayland/xkb) in
  the cross image, and forwards `CARGO_PROFILE_RELEASE_*` so CI can relax the Pi
  build's LTO (see below).

## CI

`.github/workflows/ci.yml`, on PRs and pushes:

- **`check`** — `cargo fmt --check`, `cargo clippy --all-targets -D warnings`,
  `cargo test`, with the Linux build deps and a cargo cache.
- **`build-web`** — reproduces the Vercel build (wasm bundle) and uploads it.

Heavy cross-compiles are intentionally **not** run per-PR — a Pi/x86-only break is
caught at release time.

## Release

`.github/workflows/release.yml`, on `v*` tags only:

1. A matrix builds `web`, `x86`, and `pi`.
   - The Pi job relaxes to **thin LTO + `codegen-units=16`** (via
     `CARGO_PROFILE_RELEASE_*` env forwarded by `Cross.toml`) and adds swap, because
     fat LTO + `codegen-units=1` OOM-kills `rustc` for aarch64. x86/web keep fat LTO.
2. It packages the per-target tarballs and a **flat** `gamebient-game.tar.gz`
   cartridge (layout `./gamebient-game` + `./assets/`).
3. A `release` job publishes a GitHub Release with all artifacts.

## Cartridge binary flow

The ~45 MB Pi cartridge binary is **not** committed to git. Instead:

```
release.yml (on tag)            Vercel build (on deploy)
  build pi  ──► gamebient-game.tar.gz ──► GitHub Release asset
                                              │
                                              ▼  fetch-cartridge.sh (needs GH_TOKEN)
                                         dist/assets/gamebient-game.tar.gz
                                              │
                                              ▼  served by Vercel
                          assets/info.json  binary_url  ──► /assets/gamebient-game.tar.gz
```

### Setting `GH_TOKEN` in Vercel

1. Create a GitHub **fine-grained personal access token**:
   - Resource owner: your org; Repository access: only this repo.
   - Repository permissions: **Contents → Read-only** (this covers release-asset
     downloads). Everything else: No access.
2. Add it to the Vercel project as `GH_TOKEN` (Production, and Preview if you want
   previews to serve the binary).
3. Fine-grained tokens expire — rotate before expiry, or `binary_url` silently 404s.
