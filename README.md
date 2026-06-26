# Gamebient Game

A Bevy game for the Gamebient ColecoVision GX platform, built from the
[Gamebient Bevy template](#about-this-template). Runs natively and in the browser
(WebGL2), and ships as a Pi cartridge binary.

## Quick start

```bash
./init-game.sh "My Game" OWNER/REPO   # rename the template to your game, then self-deletes
cargo run                             # native debug build — a window opens
```

Controls: **WASD / arrow keys** move, **ENTER** starts (and restarts after game over).

## Building

### All targets

```bash
./build.sh web     # wasm32 bundle -> dist/ (the path Vercel serves)
./build.sh x86     # x86_64-unknown-linux-gnu (native on Linux, cross elsewhere)
./build.sh pi      # aarch64-unknown-linux-gnu (Raspberry Pi), via cross
```

Bundle a compiled Linux target (binary + assets + launcher) into a tarball:

```bash
./package.sh x86   # -> build/gamebient-game-x86.tar.gz
./package.sh pi    # -> build/gamebient-game-pi.tar.gz
```

**Requirements:**
- Web: `rustup target add wasm32-unknown-unknown`, `cargo install wasm-bindgen-cli@0.2.108 --locked`, and `wasm-opt` (binaryen).
- Pi / cross builds: `cargo install cross --locked` and a running Docker daemon.

> The `wasm-bindgen-cli` version **must** match the `wasm-bindgen` library version in `Cargo.lock`. If you bump the dependency, bump the CLI (in `install.sh` and the workflows) in lockstep, or the bundle fails to load at runtime.

### Desktop (quick run)

```bash
cargo run                    # debug, native host
cargo run --release          # release, native host
```

## Deploying (web)

**Vercel** runs `build_web.sh` to build `dist/` and serves it as a static site.

> **Required env var:** set `GH_TOKEN` in the Vercel project — a GitHub
> **fine-grained PAT** with **Contents: Read** on this repo. The build uses it to
> fetch the Pi cartridge binary (`gamebient-game.tar.gz`) from the latest GitHub
> release into `dist/assets/`, so `assets/info.json`'s `binary_url` resolves
> without keeping the ~45 MB blob in git. Without the token the web game still
> deploys, but `binary_url` returns 404. See [docs/build-and-release.md](docs/build-and-release.md).

## Releases

Push a `v*` tag (e.g. `v0.0.1`) to trigger `.github/workflows/release.yml`, which
builds all three targets and publishes a GitHub Release with:
`gamebient-game-pi.tar.gz`, `gamebient-game-x86.tar.gz`, `gamebient-game-web.zip`,
and the flat cartridge `gamebient-game.tar.gz`.

## Project structure

```
src/
  main.rs            App + curated DefaultPlugins, window, UI-scale helper
  game/
    mod.rs           GamePlugin: state machine, resources, gameplay systems, cleanup
    states.rs        GameState (Menu / Playing / GameOver)
    player.rs        WASD-movable player (the "add a system" example)
    scoring.rs       GameData + add_score(), with unit tests
  assets/mod.rs      AssetsPlugin (procedural; add asset loading here)
  ui/
    mod.rs / menu.rs / hud.rs   Title + game-over screens, score/lives HUD
```

Geometry is procedural — no external model files. See
[docs/conventions.md](docs/conventions.md) for the patterns to follow and
[AGENTS.md](AGENTS.md) for agent-oriented guidance.

## About this template

This repository was generated from the Gamebient Bevy template: a minimal runnable
skeleton plus a full production stack (multi-target build, CI, tag-driven releases,
and the Gamebient cartridge pipeline). Run `./init-game.sh` once to make it yours.

## License

Copyright © 2026 Bread Heads Studios. All rights reserved. Proprietary software —
see [LICENSE](LICENSE). Built on open-source libraries (including
[Bevy](https://bevyengine.org/), MIT/Apache-2.0) under their own terms.
