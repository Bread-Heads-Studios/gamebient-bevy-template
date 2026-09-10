#!/usr/bin/env python3
"""Build the catalog sizzle reel from each game's recording.

Run from the workspace root (or pass --root):
  reel.py --init        write docs/reel.json from docs/video-notes-index.md
  reel.py               build build/reel/reel-16x9.mp4 and reel-9x16.mp4
  reel.py --aspect 9x16 build one aspect only

docs/reel.json: {"cta": [line1, line2], "card_secs": 1.5, "end_secs": 3.0,
"games": [{"folder", "clip", "start", "length"}]}. `clip` is a beat name
(`06-delivery-popup`) or an alias (`signature`, `game-over`). Each game gets
a title card (its assets/cartridge.png + title), then its clip trimmed to
start/length (16:9 from build/record/clips/, 9:16 from clips/vertical/);
the reel ends on a CTA card. Needs ffmpeg, ffprobe, rsvg-convert. Stdlib only.

Segments (cards and clips) are intermediate Matroska files with lossless
PCM audio; loudness normalization and near-silent clip windows are handled
per segment, then the final concat pass copies video and encodes audio to
AAC exactly once, so segment boundaries stay in sync.
"""

import argparse
import json
import pathlib
import re
import shutil
import subprocess
import sys
import xml.sax.saxutils

HERE = pathlib.Path(__file__).resolve().parent
DEFAULTS = {"cta": ["Play the demo at", "colecovisiongx.com"], "card_secs": 1.5, "end_secs": 3.0}
GAME_DEFAULTS = {"start": 1.0, "length": 4.5}
ASPECTS = {"16x9": ((1920, 1080), "clips"), "9x16": ((1080, 1920), "clips/vertical")}
FADE = 0.15
LOUDNORM = "loudnorm=I=-16:TP=-1.5:LRA=11"
SILENCE = "anullsrc=r=48000:cl=stereo"
SILENCE_THRESHOLD_DB = -50.0
# Per-segment encode: lossless PCM audio so silence detection / normalization
# choices made per clip don't get re-encoded (and potentially blow up) again
# at concat time. Video is a normal x264 intermediate.
ENCODE = ["-c:v", "libx264", "-crf", "20", "-pix_fmt", "yuv420p",
          "-c:a", "pcm_s16le", "-ar", "48000", "-ac", "2"]
# Final concat: copy video untouched, encode audio to AAC exactly once so
# every segment boundary shares one continuous audio stream (no per-segment
# AAC priming samples to throw off start_time).
CONCAT_ARGS = ["-c:v", "copy", "-c:a", "aac", "-b:a", "192k", "-ar", "48000", "-movflags", "+faststart"]


def fit_size(text, width, cap):
    return min(cap, int(width / (0.8 * max(1, len(text)))))


def parse_index(markdown):
    rows, header = [], None
    for line in markdown.splitlines():
        if not line.startswith("|"):
            continue
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if header is None:
            header = cells
            continue
        if set("".join(cells)) <= set("-: "):
            continue
        folder = cells[header.index("Folder")]
        m = re.search(r"clips/([A-Za-z0-9_-]+)\.mp4", cells[header.index("Lead clip")])
        if folder and m:
            rows.append({"folder": folder, "clip": m.group(1)})
    return rows


def init_config(markdown):
    return {**DEFAULTS, "games": [{**g, **GAME_DEFAULTS} for g in parse_index(markdown)]}


def load_config(text):
    cfg = {**DEFAULTS, **json.loads(text)}
    cfg["games"] = [{**GAME_DEFAULTS, **g} for g in cfg.get("games", [])]
    return cfg


def resolve_clip(clip, beat_names):
    if clip == "signature":
        found = [b for b in beat_names if b.startswith("06-")]
    elif clip == "game-over":
        found = [b for b in beat_names if b == "09-game-over"]
    else:
        found = [b for b in beat_names if b == clip]
    if not found:
        raise ValueError(f"clip {clip!r} not among beats {beat_names}")
    return found[0]


def fill(template, **values):
    out = template
    for key, value in values.items():
        out = out.replace("{{" + key + "}}", xml.sax.saxutils.escape(str(value)))
    return out


def card_args(png, secs, size, out):
    w, h = size
    return ["-loop", "1", "-t", f"{secs:.3f}", "-i", str(png),
            "-f", "lavfi", "-t", f"{secs:.3f}", "-i", SILENCE,
            "-vf", f"scale={w}:{h},fps=60,format=yuv420p",
            "-map", "0:v", "-map", "1:a", *ENCODE, "-shortest", str(out)]


def clip_args(src, start, length, size, has_audio, out, normalize=True):
    w, h = size
    args = ["-ss", f"{start:.3f}", "-t", f"{length:.3f}", "-i", str(src)]
    if not has_audio:
        args += ["-f", "lavfi", "-t", f"{length:.3f}", "-i", SILENCE]
    af = f"afade=t=in:d={FADE},afade=t=out:st={length - FADE:.3f}:d={FADE}"
    if normalize:
        af += f",{LOUDNORM}"
    return args + ["-vf", f"scale={w}:{h},fps=60,format=yuv420p", "-af", af,
                   "-map", "0:v", "-map", "0:a" if has_audio else "1:a", *ENCODE, str(out)]


def concat_list(paths):
    return "".join("file '" + str(p).replace("'", "'\\''") + "'\n" for p in paths)


def parse_max_volume(stderr):
    """Extract the dB value from ffmpeg volumedetect's 'max_volume: -91.0 dB' line."""
    m = re.search(r"max_volume:\s*(-?\d+(?:\.\d+)?)\s*dB", stderr)
    return float(m.group(1)) if m else None


def is_near_silent(src, start, length):
    """True when the [start, start+length) window of src has no meaningful audio.

    A window with no audio at all (volumedetect finds nothing to report) or
    whose peak is below SILENCE_THRESHOLD_DB is "near silent": loudnorm's
    single-pass gain computation blows up to NaN/Inf on it and aborts the
    encoder, so callers should skip normalization for these windows.
    """
    result = subprocess.run(
        ["ffmpeg", "-hide_banner", "-ss", f"{start:.3f}", "-t", f"{length:.3f}",
         "-i", str(src), "-af", "volumedetect", "-f", "null", "-"],
        capture_output=True, text=True)
    max_volume = parse_max_volume(result.stderr)
    return max_volume is None or max_volume < SILENCE_THRESHOLD_DB


def run(label, args):
    result = subprocess.run(["ffmpeg", "-y", "-loglevel", "error", *args],
                            capture_output=True, text=True)
    if result.returncode != 0:
        lines = [l for l in result.stderr.splitlines() if l.strip()]
        last = lines[-1] if lines else (result.stderr.strip() or "unknown ffmpeg error")
        sys.exit(f"reel: ffmpeg failed on {label}: {last}")


def has_audio(path):
    out = subprocess.run(["ffprobe", "-v", "error", "-select_streams", "a",
                          "-show_entries", "stream=index", "-of", "csv=p=0", str(path)],
                         capture_output=True, text=True, check=True).stdout
    return bool(out.strip())


def render_svg(template_name, work, png_name, **values):
    svg = work / (png_name[:-4] + ".svg")
    svg.write_text(fill((HERE / template_name).read_text(encoding="utf-8"), **values), encoding="utf-8")
    png = work / png_name
    subprocess.run(["rsvg-convert", str(svg), "-o", str(png)], check=True)
    return png


def build(root, cfg, aspect):
    size, clip_dir = ASPECTS[aspect]
    out_dir = root / "build" / "reel"
    work = out_dir / "work" / aspect
    work.mkdir(parents=True, exist_ok=True)
    segments = []
    for i, g in enumerate(cfg["games"]):
        rec = root / "games" / g["folder"] / "build" / "record"
        manifest_path = rec / "manifest.json"
        if not manifest_path.exists():
            sys.exit(f"reel: no recording for {g['folder']}; run tools/record.sh there")
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        beat = resolve_clip(g["clip"], [b["name"] for b in manifest["beats"]])
        title = manifest["name"].upper()
        gwork = work / f"{i:02d}-{g['folder']}"
        gwork.mkdir(exist_ok=True)
        shutil.copyfile(root / "games" / g["folder"] / "assets" / "cartridge.png", gwork / "cover.png")
        card = render_svg(f"card-{aspect}.svg", gwork, "card.png", TITLE=title,
                          TITLE_SIZE=fit_size(title, 820 if aspect == "16x9" else 960, 110))
        seg = gwork / "card.mkv"
        run(f"{g['folder']} card", card_args(card, cfg["card_secs"], size, seg))
        segments.append(seg)
        src = rec / clip_dir / f"{beat}.mp4"
        if not src.exists():
            sys.exit(f"reel: {src} missing; re-run tools/record.sh in {g['folder']}")
        seg = gwork / "clip.mkv"
        normalize = not is_near_silent(src, g["start"], g["length"])
        run(f"{g['folder']} clip ({src})",
            clip_args(src, g["start"], g["length"], size, has_audio(src), seg, normalize=normalize))
        segments.append(seg)
    end = render_svg(f"end-{aspect}.svg", work, "end.png", COUNT=f"{len(cfg['games'])} GAMES",
                     CTA_1=cfg["cta"][0], CTA_2=cfg["cta"][1])
    seg = work / "end.mkv"
    run("end card", card_args(end, cfg["end_secs"], size, seg))
    segments.append(seg)
    listing = work / "segments.txt"
    listing.write_text(concat_list(segments), encoding="utf-8")
    out = out_dir / f"reel-{aspect}.mp4"
    run(f"concat ({aspect})",
        ["-f", "concat", "-safe", "0", "-i", str(listing), *CONCAT_ARGS, str(out)])
    print(f"reel: {out}")
    return out


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--root", default=".")
    p.add_argument("--init", action="store_true")
    p.add_argument("--aspect", choices=["16x9", "9x16", "both"], default="both")
    args = p.parse_args(argv)
    root = pathlib.Path(args.root).resolve()
    config_path = root / "docs" / "reel.json"
    if args.init:
        index = root / "docs" / "video-notes-index.md"
        if not index.exists():
            sys.exit(f"reel: {index} not found")
        config_path.write_text(json.dumps(init_config(index.read_text(encoding="utf-8")), indent=2) + "\n",
                               encoding="utf-8")
        print(f"reel: wrote {config_path}; reorder or trim it, then run reel.py")
        return
    if not config_path.exists():
        sys.exit(f"reel: {config_path} not found; run reel.py --init first")
    for tool in ("ffmpeg", "ffprobe", "rsvg-convert"):
        if shutil.which(tool) is None:
            sys.exit(f"reel: {tool} not found on PATH")
    cfg = load_config(config_path.read_text(encoding="utf-8"))
    for aspect in (["16x9", "9x16"] if args.aspect == "both" else [args.aspect]):
        build(root, cfg, aspect)


if __name__ == "__main__":
    main()
