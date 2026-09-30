#!/usr/bin/env python3
"""Cut beat clips and stills from a recording and render chapters.md.

Usage: cut_clips.py <record_dir>   (the RECORD_DIR that record.sh produced)

Reads manifest.json + events.jsonl, writes:
  clips/<beat>.mp4          2 s before to 4 s after every logged beat, carrying
                            the tour's audio (AAC) when it has any
  clips/signature.mp4       copy of the 06-* beat clip
  clips/game-over.mp4       copy of the 09-game-over clip
  shots/<beat>.png          the exact beat frame (the autopilot's screenshot contract)
  chapters.md               timestamped table of states, beats, pause, score milestones
  banner.png                9:16 title/CTA banner rendered from vertical-banner.svg,
                            its plates placed above and below the gameplay for
                            the recording's aspect (see banner_layout)
  clips/vertical/<beat>.mp4 each beat clip reframed to 1080x1920
  tour-vertical.mp4         the full tour reframed to 1080x1920
The vertical outputs need `rsvg-convert` on PATH; without it they are skipped.
Exits with a friendly message (no traceback) if manifest.json, events.jsonl,
or tour.mp4 is missing from <record_dir>, or ffmpeg is not on PATH.
Stdlib only; needs ffmpeg on PATH.
"""

import json
import pathlib
import shutil
import subprocess
import sys
import xml.sax.saxutils

PRE_S = 2.0
POST_S = 4.0


def clip_window(beat_t, duration, pre=PRE_S, post=POST_S):
    """(start, length) in seconds, clamped to [0, duration]."""
    start = max(0.0, beat_t - pre)
    end = min(duration, beat_t + post)
    return (start, max(0.0, end - start))


def still_time(t, duration, fps):
    """Clamp a still's timestamp so ffmpeg can always seek to a real frame.

    A beat logged on the tour's last frame (or past it, in a partial log)
    would otherwise seek at or beyond `duration`, where there is no frame
    to grab.
    """
    return min(t, max(0.0, duration - 1.0 / fps))


def fmt_time(t):
    total = round(t * 100)
    minutes, cs = divmod(total, 6000)
    return f"{minutes:02d}:{cs // 100:02d}.{cs % 100:02d}"


def milestones(scores):
    """First non-zero score, then every crossing of a power of ten."""
    out = []
    threshold = None
    for t, value in scores:
        value = int(value)
        if value <= 0:
            continue
        if threshold is None:
            out.append((t, value))
            threshold = 10 ** len(str(value))
            continue
        if value >= threshold:
            out.append((t, value))
            while value >= threshold:
                threshold *= 10
    return out


def load_events(path):
    with open(path, encoding="utf-8") as f:
        return [json.loads(line) for line in f if line.strip()]


def render_chapters(manifest, events, clips):
    rows = []
    scores = []
    for e in events:
        kind, data, t = e["kind"], e["data"], e["t"]
        if kind == "state":
            rows.append((t, "state", data["name"]))
        elif kind == "beat":
            rows.append((t, "beat", data["name"]))
        elif kind == "pause":
            rows.append((t, "pause", "on" if data["value"] else "off"))
        elif kind == "score":
            scores.append((t, data["value"]))
    for t, value in milestones(scores):
        rows.append((t, "score", str(value)))
    rows.sort(key=lambda r: r[0])
    lines = [f"# {manifest['name']} — chapters", ""]
    lines.append(
        f"Tour length: {fmt_time(manifest['duration_s'])} "
        f"({manifest['frames']} frames @ {manifest['fps']} fps)"
    )
    lines += ["", "| time | kind | detail |", "|---|---|---|"]
    lines += [f"| {fmt_time(t)} | {kind} | {detail} |" for t, kind, detail in rows]
    lines += ["", "## Clips", ""]
    lines += [f"- {c}" for c in clips]
    lines.append("")
    return "\n".join(lines)


def ffmpeg(*args):
    subprocess.run(["ffmpeg", "-y", "-loglevel", "error", *args], check=True)


CTA = ("Play the demo at", "colecovisiongx.com")
BANNER_TEMPLATE = pathlib.Path(__file__).with_name("vertical-banner.svg")
CANVAS = (1080, 1920)
# Plate geometry in canvas rows. "Regular" is the banner as it was drawn for
# 16:9; "compact" is for a gameplay band that leaves under 280 rows on each
# side (3:4 leaves 240). *_base is a text baseline measured from its plate's top.
REGULAR = {"title_h": 170, "title_gap": 46, "title_base": 115, "title_cap": 96,
           "cta_h": 200, "cta_gap": 56, "cta1_base": 82, "cta1_size": 46,
           "cta2_base": 158, "cta2_size": 58}
COMPACT = {"title_h": 130, "title_base": 90, "title_cap": 72,
           "cta_h": 150, "cta1_base": 58, "cta1_size": 34,
           "cta2_base": 116, "cta2_size": 46}
MIN_EDGE = 24


def title_size(title, width=900, cap=96):
    """Font size that fits `title` (Arial Black, ~0.8 em per glyph) in `width` px."""
    return min(cap, int(width / (0.8 * max(1, len(title)))))


def gameplay_band(src_w, src_h):
    """(top, bottom) rows the gameplay covers on the 1080x1920 canvas.

    Mirrors vertical_filter(): the source is scaled to 1080 wide with an even
    height (ffmpeg's scale=1080:-2) and centred vertically.
    """
    height = min(CANVAS[1], 2 * round(CANVAS[0] * src_h / src_w / 2))
    top = (CANVAS[1] - height) // 2
    return (top, top + height)


def banner_layout(src_w, src_h):
    """Where the title and CTA plates go for a source of this size.

    The plates use only the rows the gameplay leaves free, so they never
    cover it. With 280 free rows or more on each side they keep their full
    size and sit against the band; with 198 to 279 they shrink and are
    centred in their strip; with fewer they are hidden. Upper-case keys are
    the slots of vertical-banner.svg.
    """
    top, bottom = gameplay_band(src_w, src_h)
    free = top
    if free >= REGULAR["cta_gap"] + REGULAR["cta_h"] + MIN_EDGE:
        mode, g = "regular", REGULAR
        title_y = top - g["title_gap"] - g["title_h"]
        cta_y = bottom + g["cta_gap"]
    elif free >= COMPACT["cta_h"] + 2 * MIN_EDGE:
        mode, g = "compact", COMPACT
        title_y = (free - g["title_h"]) // 2
        cta_y = bottom + (free - g["cta_h"]) // 2
    else:
        mode, g = "none", COMPACT
        title_y, cta_y = 0, CANVAS[1] - g["cta_h"]
    return {
        "mode": mode,
        "band": (top, bottom),
        "DISPLAY": "none" if mode == "none" else "inline",
        "TITLE_PLATE_Y": title_y,
        "TITLE_PLATE_H": g["title_h"],
        "TITLE_Y": title_y + g["title_base"],
        "TITLE_CAP": g["title_cap"],
        "CTA_PLATE_Y": cta_y,
        "CTA_PLATE_H": g["cta_h"],
        "CTA_1_Y": cta_y + g["cta1_base"],
        "CTA_1_SIZE": g["cta1_size"],
        "CTA_2_Y": cta_y + g["cta2_base"],
        "CTA_2_SIZE": g["cta2_size"],
    }


def banner_svg(template, title, cta, layout):
    title = title.upper()
    esc = xml.sax.saxutils.escape
    out = (template.replace("{{TITLE}}", esc(title))
           .replace("{{TITLE_SIZE}}", str(title_size(title, cap=layout["TITLE_CAP"])))
           .replace("{{CTA_1}}", esc(cta[0]))
           .replace("{{CTA_2}}", esc(cta[1])))
    for key, value in layout.items():
        if key.isupper():
            out = out.replace("{{" + key + "}}", str(value))
    return out


def vertical_filter():
    return ("[0:v]split=2[bg][fg];"
            "[bg]scale=1080:1920:force_original_aspect_ratio=increase,crop=1080:1920,"
            "scale=270:480,gblur=sigma=8,scale=1080:1920,eq=brightness=-0.12[bgb];"
            "[fg]scale=1080:-2[fgs];"
            "[bgb][fgs]overlay=(W-w)/2:(H-h)/2[base];"
            "[base][1:v]overlay=0:0[v]")


def vertical_args(src, banner, out):
    return ["-i", str(src), "-i", str(banner), "-filter_complex", vertical_filter(),
            "-map", "[v]", "-map", "0:a?", "-c:v", "libx264", "-crf", "20",
            "-pix_fmt", "yuv420p", "-c:a", "copy", str(out)]


def clip_args(tour, start, length, out):
    return ["-ss", f"{start:.3f}", "-i", str(tour), "-t", f"{length:.3f}",
            "-map", "0:v", "-map", "0:a?", "-c:v", "libx264", "-crf", "18",
            "-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "192k", str(out)]


def main(record_dir):
    root = pathlib.Path(record_dir)
    manifest_path = root / "manifest.json"
    events_path = root / "events.jsonl"
    tour = root / "tour.mp4"
    for what, path in (
        ("manifest.json", manifest_path),
        ("events.jsonl", events_path),
        ("tour.mp4", tour),
    ):
        if not path.exists():
            sys.exit(f"cut_clips: {what} not found in {root}")
    if shutil.which("ffmpeg") is None:
        sys.exit("cut_clips: ffmpeg not found on PATH")

    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    events = load_events(events_path)
    clips_dir = root / "clips"
    shots_dir = root / "shots"
    clips_dir.mkdir(exist_ok=True)
    shots_dir.mkdir(exist_ok=True)
    duration = manifest["duration_s"]
    fps = manifest["fps"]

    # Computed from the manifest's beats alone, so chapters.md can be
    # written before any clip is cut — a run that dies partway through
    # cutting still leaves a complete chapters table naming every clip.
    have_vertical = shutil.which("rsvg-convert") and BANNER_TEMPLATE.exists()
    written = []
    for beat in manifest["beats"]:
        name = beat["name"]
        written.append(f"clips/{name}.mp4")
        if name.startswith("06-"):
            written.append("clips/signature.mp4")
        if name == "09-game-over":
            written.append("clips/game-over.mp4")
    if have_vertical:
        for beat in manifest["beats"]:
            written.append(f"clips/vertical/{beat['name']}.mp4")
        written.append("tour-vertical.mp4")
    (root / "chapters.md").write_text(render_chapters(manifest, events, written), encoding="utf-8")

    for beat in manifest["beats"]:
        name, t = beat["name"], beat["frame"] / fps
        start, length = clip_window(t, duration)
        out = clips_dir / f"{name}.mp4"
        ffmpeg(*clip_args(tour, start, length, out))
        t_still = still_time(t, duration, fps)
        ffmpeg("-ss", f"{t_still:.3f}", "-i", str(tour), "-frames:v", "1", str(shots_dir / f"{name}.png"))
        if name.startswith("06-"):
            shutil.copyfile(out, clips_dir / "signature.mp4")
        if name == "09-game-over":
            shutil.copyfile(out, clips_dir / "game-over.mp4")
    print(f"cut_clips: {len(manifest['beats'])} beats -> {clips_dir}")

    if shutil.which("rsvg-convert") and BANNER_TEMPLATE.exists():
        layout = banner_layout(manifest["width"], manifest["height"])
        svg = root / "banner.svg"
        svg.write_text(banner_svg(BANNER_TEMPLATE.read_text(encoding="utf-8"), manifest["name"], CTA, layout),
                       encoding="utf-8")
        banner = root / "banner.png"
        subprocess.run(["rsvg-convert", str(svg), "-o", str(banner)], check=True)
        vdir = clips_dir / "vertical"
        vdir.mkdir(exist_ok=True)
        for beat in manifest["beats"]:
            name = beat["name"]
            ffmpeg(*vertical_args(clips_dir / f"{name}.mp4", banner, vdir / f"{name}.mp4"))
        ffmpeg(*vertical_args(tour, banner, root / "tour-vertical.mp4"))
        top, bottom = layout["band"]
        print(f"cut_clips: vertical 9:16 (gameplay at y {top}-{bottom}, {layout['mode']} plates) "
              f"-> {vdir} and tour-vertical.mp4")
    else:
        print("cut_clips: rsvg-convert or vertical-banner.svg missing; skipping vertical outputs")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    main(sys.argv[1])
