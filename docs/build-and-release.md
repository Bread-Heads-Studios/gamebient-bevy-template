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
- **`store-assets.sh [--released-at YYYY-MM-DD] [--set-developer NAME] [--set-developer-url URL] [--set-tags "a, b"] [--dry-run]`**
  — the step between `record.sh` and publishing: turns a `tools/record.sh`
  capture into the store metadata the website renders. Inputs:
  `build/record/shots/{04,05,06,07}-*.png` and `build/record/clips/signature.mp4`
  (falling back to `05-mid-play.mp4`, then `04-early-play.mp4`). Outputs:
  `assets/screenshots/01.png…04.png` and `assets/trailer.mp4` (H.264/AAC,
  `+faststart`, ≤20s, refuses to write one over 8 MB), both at the game's own
  size from `tools/game-size.sh` (960x720, 720x720 or 720x960). Footage whose
  ratio is not the game's is refused; re-record it. It writes
  `properties.aspect` from the game's size, and `properties.marquee` /
  `properties.bezel` when `assets/marquee.png` / `assets/bezel.png` exist
  (removing the key when the file does not). It also rewrites
  `assets/info.json`'s `properties.screenshots`/`trailer_url`/`version` plus
  `released_at`/`Developer`/`Developer URL`/`Tags` (only via the matching flag
  or when still a `PLACEHOLDER:` value; otherwise reports what's still missing).
  The website's `scripts/game-release-date.mjs` supplies the exact
  `--released-at` date from the game's on-chain release transaction.
  `tools/rollout-store-assets.sh <game-dir>` copies it into a game checkout.
- **`tools/game-size.sh [--scale FACTOR] [path/to/display.rs]`** — prints the
  game's size as `<width>x<height>`, parsed from `GAME_WIDTH` / `GAME_HEIGHT`
  in `src/display.rs`. `--scale 1.5` prints the capture size. Every media tool
  takes its size from here. A game that has not been converted yet and has no
  `src/display.rs` sets `GAME_SIZE=1280x720` in the environment instead.
- **`tools/frame-art.sh`** — renders `tools/marquee.svg` to
  `assets/marquee.png` (1080x360) and `tools/bezel.svg` to `assets/bezel.png`
  (1920x1920) with `rsvg-convert`. It refuses a bezel with detail in its
  central 1080x1080 or with high contrast, and writes to `assets/` only when
  every check passes. `tools/rollout-frame-art.sh <game-dir>` copies it into
  a game checkout.
- **`fetch-cartridge.sh`** — downloads the flat `gamebient-game.tar.gz` from the
  latest GitHub release into `dist/assets/`. Non-fatal and private-repo-safe
  (resolves the asset through the GitHub API with `GH_TOKEN`).
- **`Cross.toml`** — installs each target's dev libraries (ALSA/udev/wayland/xkb) in
  the cross image, and forwards `CARGO_PROFILE_RELEASE_*` so CI can relax the Pi
  build's LTO (see below).

## Aspect ratio and cabinet art in the metadata

A game is 4:3 (960x720), 1:1 (720x720) or 3:4 (720x960). The website and the
cabinets must know which before the game loads, and the cabinets draw a
marquee and a bezel around it. Three optional fields under `properties` in
`assets/info.json` carry this:

| Field | Value | Written by | When missing |
|---|---|---|---|
| `properties.aspect` | `"4:3"`, `"1:1"`, `"3:4"` or `"16:9"` | `tools/store-assets.sh`, from `src/display.rs` | the game is treated as `16:9` |
| `properties.marquee` | `https://<host>/assets/marquee.png`, a 1080x360 PNG | `tools/store-assets.sh`, when `assets/marquee.png` exists | the shared ColecoVision GX marquee is shown |
| `properties.bezel` | `https://<host>/assets/bezel.png`, a 1920x1920 PNG | `tools/store-assets.sh`, when `assets/bezel.png` exists | the shared ColecoVision GX bezel is shown |

`<host>` is `properties.game_url`. The template ships `"aspect": "4:3"` only.
It ships no `assets/marquee.png` or `assets/bezel.png` and no `marquee` or
`bezel` URL, so an unfinished game never shows placeholder art on a cabinet:
it gets the shared art. A game author designs `tools/marquee.svg` and
`tools/bezel.svg`, runs `tools/frame-art.sh` to write the PNGs, and then
`tools/store-assets.sh` adds the two URLs. Never publish the placeholder art.
Native cabinets read `assets/marquee.png` and `assets/bezel.png` from the
cartridge itself, so the PNGs ship in `assets/` like the cover.

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
