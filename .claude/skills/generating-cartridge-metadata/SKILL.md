---
name: generating-cartridge-metadata
description: Use when filling in, fixing, or reviewing a Gamebient game's assets/info.json before it is minted or published on the ColecoVision GX marketplace, or when a game's metadata shows a placeholder description, a dead or wrong Vercel host, a 1x1 cartridge.png, or a binary_url that 404s
---

# Generating Cartridge Metadata

## Overview

`assets/info.json` is what the marketplace mints and what the website reads to
launch the game. It must be true on the live site the day it is minted. The
file has a fixed shape; the work is filling three things well (description,
genre, hosts) and verifying every URL.

## The contract

`info.json` IS this, and only this, 4-space indented:

```json
{
    "name": "Grand Theft Otto",
    "description": "A top-down GTA parody in which you steal nothing. You are Otto Kleinschmidt, tax auditor: hunt petty crimes, write them in your notebook, and file the report before your own jaywalking earns five wanted stars.",
    "image": "https://grand-theft-otto-five.vercel.app/assets/cartridge.png",
    "external_url": "https://ColecoVisionGX.com",
    "attributes": [
        { "trait_type": "Genre", "value": "Bureaucratic Crime Parody" },
        { "trait_type": "Platform", "value": "Web" },
        { "trait_type": "Players", "value": "1" }
    ],
    "properties": {
        "files": [
            { "uri": "https://grand-theft-otto-five.vercel.app/assets/cartridge.png", "type": "image/png" }
        ],
        "category": "game",
        "game_url": "https://grand-theft-otto-five.vercel.app",
        "demo_url": "https://grand-theft-otto-five.vercel.app",
        "verify_url": "https://grand-theft-otto-five.vercel.app/verify.zip",
        "binary_url": "https://grand-theft-otto-five.vercel.app/assets/grand-theft-otto.tar.gz",
        "binary_type": "bevy-tar"
    }
}
```

| Field | Rule |
|-------|------|
| `description` | One or two sentences, 20–45 words: the hook, then the core loop, in the game's own voice. Facts come from README, design spec, and in-game copy. |
| `Genre` | A recognizable genre phrase of one to three words, specific to this game ("Rail Shooter", "Rhythm Fighter", "Arcade Golf", "Arena Eater"), never bare "Arcade" when something truer exists. |
| `Players` | "1" or "2" as the game actually supports. |
| extra attributes | Only a gameplay fact a player cares about (`Missions: 5`, `Holes: 9`). Nothing else. |
| `image`, `files[0].uri` | Identical; both `https://<host>/assets/cartridge.png`; the file must be a real 768x1024 PNG, not the 1x1 placeholder. |
| `game_url`, `demo_url` | `https://<host>` — the host that actually serves the game. |
| `verify_url` | `<host>/verify.zip` — `build_web.sh` publishes it alongside the game bundle; same host as `game_url`/`demo_url`. |
| `binary_url` | `https://<host>/assets/<tarball>` where `<tarball>` is the flat cartridge name in `release.yml`'s Pi job and `fetch-cartridge.sh`'s `ASSET=`. All three must agree. |

## Process

1. Read README, `docs/` specs, and the title/how-to-play copy in `src/ui/`.
2. Find the live host. Vercel adds suffixes (`gulper-kappa`, `pack-the-ripper-ochre`); the slug in the repo is a guess until confirmed:
   ```bash
   curl -s -o /dev/null -w '%{http_code}\n' https://<host>/assets/info.json
   ```
3. Read the tarball name from `release.yml` and `fetch-cartridge.sh`; use exactly that.
4. Write the file to the contract above. Validate: `python3 -m json.tool assets/info.json`.
5. Check the cover: `file assets/cartridge.png` reports 768 x 1024 **and** the
   image, when Read, carries no "PLACEHOLDER" notice. Otherwise the cover is
   not done; use `designing-cartridge-covers` before calling the metadata ready.
6. Report which URLs resolve and which still 404 (`binary_url` 404s until a
   `v*` release exists and Vercel has `GH_TOKEN`; that is expected, say so).
   The marketplace mints what the *deployed* host serves, so also curl
   `https://<host>/assets/cartridge.png` and note whether the live cover is
   the real one or still a stub awaiting a deploy.

## Common mistakes

| Mistake | Fix |
|---------|-----|
| Three-sentence description listing every unit, mode, and upgrade | Hook + loop, ≤45 words. The store page has the README for the rest. |
| Adding `Engine`, `Controls`, `Input`, `Developer`, `Lives`, `Mode` traits | Delete. The fixed trio plus at most one gameplay fact. "Built with Bevy" is never a selling point. |
| Genre left as the template's "Arcade" | Name the actual genre. |
| Host taken from the folder name | Curl it. Three of seventeen games had a suffixed host. |
| `image` pointing at a marquee PNG or a title screenshot | Point at `cartridge.png`, the 768x1024 cover. |
| Reformatting to 2-space or compact JSON | Keep the fleet's 4-space style so diffs stay small. |
