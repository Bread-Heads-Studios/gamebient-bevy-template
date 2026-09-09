#!/usr/bin/env python3
"""Cut beat clips and stills from a recording and render chapters.md.

Usage: cut_clips.py <record_dir>   (the RECORD_DIR that record.sh produced)

Reads manifest.json + events.jsonl, writes:
  clips/<beat>.mp4     2 s before to 4 s after every logged beat
  clips/signature.mp4  copy of the 06-* beat clip
  clips/game-over.mp4  copy of the 09-game-over clip
  shots/<beat>.png     the exact beat frame (the autopilot's screenshot contract)
  chapters.md          timestamped table of states, beats, pause, score milestones
Stdlib only; needs ffmpeg on PATH.
"""

import json
import pathlib
import shutil
import subprocess
import sys

PRE_S = 2.0
POST_S = 4.0


def clip_window(beat_t, duration, pre=PRE_S, post=POST_S):
    """(start, length) in seconds, clamped to [0, duration]."""
    start = max(0.0, beat_t - pre)
    end = min(duration, beat_t + post)
    return (start, max(0.0, end - start))


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


def main(record_dir):
    root = pathlib.Path(record_dir)
    manifest = json.loads((root / "manifest.json").read_text(encoding="utf-8"))
    events = load_events(root / "events.jsonl")
    tour = root / "tour.mp4"
    clips_dir = root / "clips"
    shots_dir = root / "shots"
    clips_dir.mkdir(exist_ok=True)
    shots_dir.mkdir(exist_ok=True)
    duration = manifest["duration_s"]
    fps = manifest["fps"]
    written = []
    for beat in manifest["beats"]:
        name, t = beat["name"], beat["frame"] / fps
        start, length = clip_window(t, duration)
        out = clips_dir / f"{name}.mp4"
        ffmpeg("-ss", f"{start:.3f}", "-i", str(tour), "-t", f"{length:.3f}",
               "-c:v", "libx264", "-crf", "18", "-pix_fmt", "yuv420p", "-an", str(out))
        written.append(f"clips/{name}.mp4")
        ffmpeg("-ss", f"{t:.3f}", "-i", str(tour), "-frames:v", "1", str(shots_dir / f"{name}.png"))
        if name.startswith("06-"):
            shutil.copyfile(out, clips_dir / "signature.mp4")
            written.append("clips/signature.mp4")
        if name == "09-game-over":
            shutil.copyfile(out, clips_dir / "game-over.mp4")
            written.append("clips/game-over.mp4")
    (root / "chapters.md").write_text(render_chapters(manifest, events, written), encoding="utf-8")
    print(f"cut_clips: {len(manifest['beats'])} beats -> {clips_dir}")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    main(sys.argv[1])
