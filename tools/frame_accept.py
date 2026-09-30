#!/usr/bin/env python3
"""Acceptance checks for the cabinet frame, behind tools/frame-accept.sh.

    frame_accept.py --check-copies <game-dir>
    frame_accept.py --shots <dir> --window WxH --ratio <4:3|1:1|3:4> [--frame on|off]

--check-copies diffs the files a game must carry verbatim against this
template and passes only the differences the fleet allows: the two size
constants in src/display.rs, the crate name in tests/display_shape.rs, and
the test module of src/frame/driver.rs. Anything else prints as file + hunk
and exits 1.

--shots reads 02-title.png, 05-mid-play.png and 08-pause.png from <dir>
(1:1 pixel captures of the whole window) and checks that the game sits where
`display::letterbox` puts it, that the gap around it is black, that the
bezel beyond it is not, and the brightness relations of
docs/conventions.md, "Checking the dimming". Pillow only.
"""

import argparse
import difflib
import re
import sys
from pathlib import Path

TEMPLATE = Path(__file__).resolve().parent.parent

RATIOS = {"4:3": (960, 720), "1:1": (720, 720), "3:4": (720, 960)}

# A pixel counts as lit when its brightest channel is above this.
BLACK_LEVEL = 12
# Share of a region that may be lit and still read as black (anti-aliased
# edges of the game's own picture, compression noise in a capture).
BLACK_SLACK = 0.005
# Plain bars (frame off) are the clear colour: flat, and no brighter than this.
BAR_CEILING = 64
# Bezel at idle may be at most this share of full scale (lesson L9).
IDLE_CEILING = 0.35 * 255
# Paused must equal play within this share.
PAUSE_TOLERANCE = 0.10


# ---------------------------------------------------------------- geometry


def gap_for(window):
    """src/frame/layout.rs gap_for: 8 px at a 1080 short side, rounded."""
    short = min(window)
    return (8 * short + 540) // 1080


def letterbox(window, game, reserve_top, gap):
    """src/display.rs letterbox: integer floor, whole window if nothing is left."""
    ww, wh = window
    gw, gh = game
    sides = gap * 2
    avail_w = max(ww - sides, 0)
    avail_h = max(wh - reserve_top - sides, 0)
    if avail_w == 0 or avail_h == 0 or gw == 0 or gh == 0:
        return (0, 0, ww, wh)
    if avail_w * gh <= avail_h * gw:
        w, h = avail_w, max(avail_w * gh // gw, 1)
    else:
        w, h = max(avail_h * gw // gh, 1), avail_h
    return (gap + (avail_w - w) // 2, reserve_top + gap + (avail_h - h) // 2, w, h)


def expected_layout(window, game, frame_on):
    """(game rect, marquee height, gap) for one window."""
    if not frame_on:
        return letterbox(window, game, 0, 0), 0, 0
    ww, wh = window
    marquee = ww // 3 if wh > ww else 0
    gap = gap_for(window)
    return letterbox(window, game, marquee, gap), marquee, gap


# ------------------------------------------------------------------ pixels


def load_luma_and_peak(path):
    from PIL import Image

    img = Image.open(path).convert("RGB")
    return img, img.convert("L")


def region_stats(rgb, luma, rects):
    """(mean luma 0..255, share of pixels whose brightest channel is lit) over
    the union of disjoint `rects` given as (x, y, w, h)."""
    from PIL import ImageStat

    total = 0
    luma_sum = 0.0
    lit = 0
    for x, y, w, h in rects:
        if w <= 0 or h <= 0:
            continue
        box = (x, y, x + w, y + h)
        n = w * h
        total += n
        luma_sum += ImageStat.Stat(luma.crop(box)).mean[0] * n
        # Brightest channel per pixel = pixelwise max of the three bands.
        r, g, b = rgb.crop(box).split()
        from PIL import ImageChops

        peak = ImageChops.lighter(ImageChops.lighter(r, g), b)
        hist = peak.histogram()
        lit += sum(hist[BLACK_LEVEL + 1 :])
    if total == 0:
        return None, None
    return luma_sum / total, lit / total


def flat_share(rgb, rects):
    """(share of the most common colour, that colour) over the union of rects."""
    from collections import Counter

    counts = Counter()
    for x, y, w, h in rects:
        if w > 0 and h > 0:
            for n, colour in rgb.crop((x, y, x + w, y + h)).getcolors(maxcolors=w * h):
                counts[colour] += n
    total = sum(counts.values())
    if not total:
        return None, None
    colour, n = counts.most_common(1)[0]
    return n / total, colour


def ring_rects(outer, inner):
    """The pixels of `outer` not in `inner`, as up to four rects."""
    ox, oy, ow, oh = outer
    ix, iy, iw, ih = inner
    return [
        (ox, oy, ow, iy - oy),
        (ox, iy + ih, ow, oy + oh - (iy + ih)),
        (ox, iy, ix - ox, ih),
        (ix + iw, iy, ox + ow - (ix + iw), ih),
    ]


def shots_check(args):
    try:
        window = tuple(int(v) for v in args.window.lower().split("x"))
        assert len(window) == 2 and min(window) > 0
    except (ValueError, AssertionError):
        print(f"frame-accept: --window must look like 1920x1080, got {args.window!r}")
        return 2
    if args.ratio not in RATIOS:
        print(f"frame-accept: --ratio must be one of {', '.join(RATIOS)}")
        return 2
    game = RATIOS[args.ratio]
    frame_on = args.frame == "on"
    shots = Path(args.shots)
    names = {"idle": "02-title.png", "play": "05-mid-play.png", "paused": "08-pause.png"}
    try:
        import PIL  # noqa: F401
    except ImportError:
        print("frame-accept: Pillow is required (python3 -m pip install pillow)")
        return 2

    rect, marquee_h, gap = expected_layout(window, game, frame_on)
    gx, gy, gw, gh = rect
    backing = (gx - gap, gy - gap, gw + 2 * gap, gh + 2 * gap)
    portrait = window[1] > window[0]
    print(
        f"window {window[0]}x{window[1]}  ratio {args.ratio}  frame {args.frame}  "
        f"game rect x={gx} y={gy} w={gw} h={gh}  gap {gap}  marquee rows {marquee_h}"
    )

    failures = []
    bars = None
    rows = []
    means = {}
    for label, fname in names.items():
        path = shots / fname
        if not path.is_file():
            failures.append(f"{fname}: missing in {shots}")
            continue
        rgb, luma = load_luma_and_peak(path)
        if rgb.size != window:
            failures.append(f"{fname}: is {rgb.size[0]}x{rgb.size[1]}, window is {window[0]}x{window[1]}")
            continue
        full = (0, 0, window[0], window[1])
        # What lies outside the backing rect, minus the marquee band.
        outside = [
            r for r in ring_rects(full, backing) if r[2] > 0 and r[3] > 0
        ]
        if marquee_h:
            outside = [
                (x, max(y, marquee_h), w, h - max(0, marquee_h - y) if y < marquee_h else h)
                for x, y, w, h in outside
                if y + h > marquee_h
            ]
        out_mean, out_lit = region_stats(rgb, luma, outside)
        game_mean, _ = region_stats(rgb, luma, [rect])
        _, gap_lit = (
            region_stats(rgb, luma, ring_rects(backing, rect)) if gap else (None, None)
        )
        mq_mean = None
        if marquee_h:
            mq_mean, _ = region_stats(rgb, luma, [(0, 0, window[0], marquee_h)])
        means[label] = {"outside": out_mean, "marquee": mq_mean}

        verdicts = []
        if gap_lit is not None and gap_lit > BLACK_SLACK:
            verdicts.append(f"gap not black ({gap_lit:.1%} lit)")
        if game_mean is not None and game_mean < 0.5:
            verdicts.append("game rect is black")
        if frame_on:
            if out_mean is not None and out_mean < 1.0:
                verdicts.append("no bezel beyond the gap")
        else:
            # Frame off: plain bars, which are the game's clear colour (dim
            # and flat), not black and not art.
            share, colour = flat_share(rgb, outside)
            if share is not None and share < 1 - BLACK_SLACK:
                verdicts.append(f"bars are not one flat colour ({share:.1%} match)")
            elif colour is not None and max(colour) > BAR_CEILING:
                verdicts.append(f"bars are bright {colour}")
            if colour is not None:
                bars = f"bars {colour}"
        for v in verdicts:
            failures.append(f"{fname}: {v}")
        rows.append(
            (
                fname,
                "-" if gap_lit is None else f"{gap_lit:.2%}",
                "-" if out_mean is None else f"{out_mean:.1f}",
                "-" if mq_mean is None else f"{mq_mean:.1f}",
                f"{game_mean:.1f}",
                "ok" if not verdicts else "FAIL",
            )
        )

    header = ("shot", "gap lit", "outside mean", "marquee mean", "game mean", "rects")
    widths = [max(len(header[i]), *(len(r[i]) for r in rows)) if rows else len(header[i]) for i in range(6)]
    print("  ".join(h.ljust(w) for h, w in zip(header, widths)))
    for r in rows:
        print("  ".join(c.ljust(w) for c, w in zip(r, widths)))

    if frame_on and len(means) == 3:
        key = "marquee" if portrait else "outside"
        what = "marquee" if portrait else "bezel"
        idle, play, paused = (means[k][key] for k in ("idle", "play", "paused"))
        print(f"{what}: idle {idle:.1f}  play {play:.1f}  paused {paused:.1f}")
        if not portrait and idle > IDLE_CEILING:
            failures.append(f"bezel idle {idle:.1f} is above 0.35 x 255 = {IDLE_CEILING:.1f}")
        if not play < idle:
            failures.append(f"{what} play {play:.1f} is not below idle {idle:.1f}")
        if abs(paused - play) > PAUSE_TOLERANCE * max(play, 1e-9):
            failures.append(
                f"{what} paused {paused:.1f} is not within 10% of play {play:.1f}"
            )
        if portrait:
            print(
                "bezel (not gated): idle {:.1f}  play {:.1f}  paused {:.1f}".format(
                    *(means[k]["outside"] for k in ("idle", "play", "paused"))
                )
            )
    elif not frame_on:
        print(f"frame off: brightness relations do not apply; {bars or 'no bars measured'}")

    if failures:
        print("FAIL")
        for f in failures:
            print(f"  {f}")
        return 1
    print("PASS")
    return 0


# ------------------------------------------------------------ check-copies


def lib_name(game):
    """[lib] name of a game's Cargo.toml, else the package name with - as _."""
    text = (game / "Cargo.toml").read_text()
    section = None
    lib = pkg = None
    for line in text.splitlines():
        s = line.strip()
        if s.startswith("["):
            section = s
            continue
        m = re.match(r'name\s*=\s*"([^"]+)"', s)
        if m and section == "[lib]":
            lib = m.group(1)
        elif m and section == "[package]":
            pkg = m.group(1)
    return lib or (pkg or "").replace("-", "_")


def read_text(path):
    return path.read_text(encoding="utf-8").splitlines(keepends=True)


def hunk(label, want, got):
    return "".join(
        difflib.unified_diff(want, got, fromfile=f"template/{label}", tofile=f"game/{label}", n=1)
    )


def tree_files(root):
    return sorted(p.relative_to(root) for p in root.rglob("*") if p.is_file())


def check_copies(game_arg):
    game = Path(game_arg).resolve()
    if not (game / "Cargo.toml").is_file():
        print(f"frame-accept: {game} has no Cargo.toml")
        return 2
    problems = []
    flat = (game / "src/sim.rs").is_file() and not (game / "src/game/mod.rs").is_file()

    def exact(rel_t, rel_g=None):
        t, g = TEMPLATE / rel_t, game / (rel_g or rel_t)
        label = rel_g or rel_t
        if not g.is_file():
            problems.append(f"{label}: missing\n")
        elif t.read_bytes() != g.read_bytes():
            try:
                problems.append(f"{label}: differs\n{hunk(label, read_text(t), read_text(g))}")
            except UnicodeDecodeError:
                problems.append(f"{label}: differs (binary)\n")

    def exact_tree(rel_t, rel_g=None):
        troot, groot = TEMPLATE / rel_t, game / (rel_g or rel_t)
        label = rel_g or rel_t
        if not groot.is_dir():
            problems.append(f"{label}/: missing\n")
            return
        want, got = set(tree_files(troot)), set(tree_files(groot))
        for f in sorted(want - got):
            problems.append(f"{label}/{f}: missing\n")
        for f in sorted(got - want):
            problems.append(f"{label}/{f}: not in the template\n")
        for f in sorted(want & got):
            if (troot / f).read_bytes() != (groot / f).read_bytes():
                try:
                    problems.append(
                        f"{label}/{f}: differs\n"
                        + hunk(f"{label}/{f}", read_text(troot / f), read_text(groot / f))
                    )
                except UnicodeDecodeError:
                    problems.append(f"{label}/{f}: differs (binary)\n")

    # src/display.rs: only the two constants.
    const = re.compile(r"^pub const (GAME_WIDTH|GAME_HEIGHT): u32 = \d+;$")

    def norm_display(lines):
        return [
            f"pub const {m.group(1)}: u32 = N;\n" if (m := const.match(l.rstrip("\n"))) else l
            for l in lines
        ]

    g = game / "src/display.rs"
    if not g.is_file():
        problems.append("src/display.rs: missing\n")
    else:
        want, got = norm_display(read_text(TEMPLATE / "src/display.rs")), norm_display(read_text(g))
        if want != got:
            problems.append(f"src/display.rs: differs beyond GAME_WIDTH/GAME_HEIGHT\n{hunk('src/display.rs', want, got)}")
        if sum(1 for l in read_text(g) if const.match(l.rstrip("\n"))) != 2:
            problems.append("src/display.rs: GAME_WIDTH and GAME_HEIGHT are not both plain `pub const X: u32 = N;` lines\n")

    # src/frame/: everything, except driver.rs from its test module on.
    troot, groot = TEMPLATE / "src/frame", game / "src/frame"
    if not groot.is_dir():
        problems.append("src/frame/: missing\n")
    else:
        want, got = set(tree_files(troot)), set(tree_files(groot))
        for f in sorted(want - got):
            problems.append(f"src/frame/{f}: missing\n")
        for f in sorted(got - want):
            problems.append(f"src/frame/{f}: not in the template\n")
        for f in sorted(want & got):
            label = f"src/frame/{f}"
            if str(f) == "driver.rs":

                def head(path):
                    lines = read_text(path)
                    at = next((i for i, l in enumerate(lines) if l.startswith("#[cfg(test)]")), len(lines))
                    return lines[:at]

                a, b = head(troot / f), head(groot / f)
                if a != b:
                    problems.append(f"{label}: differs above `#[cfg(test)]`\n{hunk(label, a, b)}")
            elif (troot / f).read_bytes() != (groot / f).read_bytes():
                try:
                    problems.append(f"{label}: differs\n{hunk(label, read_text(troot / f), read_text(groot / f))}")
                except UnicodeDecodeError:
                    problems.append(f"{label}: differs (binary)\n")

    exact("src/ui/fit.rs", "src/fit.rs" if flat else "src/ui/fit.rs")
    exact_tree("src/game/record", "src/record" if flat else "src/game/record")

    # tests/display_shape.rs: the crate name only.
    name = lib_name(game)
    g = game / "tests/display_shape.rs"
    if not g.is_file():
        problems.append("tests/display_shape.rs: missing\n")
    else:
        want = read_text(TEMPLATE / "tests/display_shape.rs")
        got = [l.replace(f"{name}::", "gamebient_game::") for l in read_text(g)]
        if want != got:
            problems.append(
                f"tests/display_shape.rs: differs beyond the crate name ({name})\n"
                + hunk("tests/display_shape.rs", want, got)
            )

    for f in (
        "tools/frame-art.sh",
        "tools/game-size.sh",
        "tools/record.sh",
        "tools/store-assets.sh",
        "tools/cut_clips.py",
        "tools/test_cut_clips.py",
        "tools/vertical-banner.svg",
    ):
        exact(f)
    for skill in ("designing-cartridge-covers", "generating-cartridge-metadata"):
        exact_tree(f".claude/skills/{skill}")

    # A game never carries the template's own tests or rollout scripts. A
    # template copy (it has rollout-aspect.sh) is exempt.
    if not (game / "tools/rollout-aspect.sh").is_file():
        tools = game / "tools"
        stray = [
            p.name
            for p in sorted(tools.glob("*"))
            if p.is_file()
            and (
                (p.name.startswith("test_") and p.name != "test_cut_clips.py")
                or p.name.startswith("rollout-")
            )
        ]
        for n in stray:
            problems.append(f"tools/{n}: a game does not carry this\n")

    if problems:
        print(f"frame-accept --check-copies: FAIL for {game}")
        for p in problems:
            print(p.rstrip("\n"))
        return 1
    print(f"frame-accept --check-copies: PASS for {game}")
    return 0


def main(argv):
    ap = argparse.ArgumentParser(prog="frame-accept.sh", description=__doc__.split("\n\n")[0])
    ap.add_argument("--check-copies", metavar="GAME_DIR")
    ap.add_argument("--shots", metavar="DIR")
    ap.add_argument("--window", metavar="WxH")
    ap.add_argument("--ratio", choices=list(RATIOS))
    ap.add_argument("--frame", choices=["on", "off"], default="on")
    args = ap.parse_args(argv)
    if args.check_copies and not args.shots:
        return check_copies(args.check_copies)
    if args.shots and args.window and args.ratio and not args.check_copies:
        return shots_check(args)
    ap.print_usage(sys.stderr)
    print(
        "frame-accept: use --check-copies <game-dir>, or --shots <dir> --window WxH --ratio <r> [--frame on|off]",
        file=sys.stderr,
    )
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
