#!/usr/bin/env python3
"""Vibe-coding arm of the natural-language rules eval.

For each case in the eval set, starting from the Coin Run fixture (the
template at a pinned commit plus eval/vibe/fixture/), a headless Claude Code
agent gets the beginner's sentence and edits the Rust game directly. Then the
runner checks what a person would see:

  * what the agent replied: a change with a READBACK, a QUESTION, or CANNOT
  * how long the agent took, and how long one more build takes
  * how much code changed
  * the repo's own CI gates: fmt, clippy -D warnings, cargo test (which
    includes the replay determinism tests)
  * the case's hidden behaviour test (eval/vibe/hidden/<id>.rs), copied in
    only after the agent has finished, so it can't be coded to

Results go to <work>/results/<id>.json plus summary.md. See
eval/vibe/README.md.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import statistics
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent
EVAL_SET = REPO / "docs/superpowers/specs/2026-11-01-natural-language-rules-eval.jsonl"
BASE = (HERE / "BASE").read_text().split()[0]
FIXTURE = HERE / "fixture"
SETUP = HERE / "setup"
HIDDEN = HERE / "hidden"
PROMPT = (HERE / "prompt.md").read_text()

# g-cases start from a rule already in the game (the rules arm's scene_extra).
SETUPS = {"g01": "coin_counts", "g03": "hurt", "g05": "coin_scores", "g06": "hurt"}

# Cases that can be applied one after another to the same game without
# contradicting each other. Chain mode applies them in this order and re-runs
# every earlier hidden test after each step, to see whether old behaviour
# survives new requests.
CHAIN = [
    "a03", "a04", "a05", "a01", "a09", "a02", "a07", "a06", "a10",
    "a13", "b02", "d01", "c02", "d03", "c03", "b03", "a11",
]

GATES = [
    ("fmt", ["cargo", "fmt", "--all", "--", "--check"]),
    ("clippy", ["cargo", "clippy", "--all-targets", "--all-features", "--", "-D", "warnings"]),
    ("suite", ["cargo", "test", "--all-targets", "--all-features"]),
]


def sh(cmd, cwd, env=None, timeout=None, check=False):
    p = subprocess.run(
        cmd, cwd=cwd, env=env, timeout=timeout, capture_output=True, text=True
    )
    if check and p.returncode != 0:
        sys.exit(f"{' '.join(cmd)} failed in {cwd}:\n{p.stdout}\n{p.stderr}")
    return p


def git(args, cwd, check=True):
    return sh(["git", "-c", "user.name=vibe-eval", "-c", "user.email=vibe-eval@localhost",
               "-c", "commit.gpgsign=false", *args], cwd, check=check)


def load_cases():
    return {c["id"]: c for c in map(json.loads, EVAL_SET.read_text().splitlines())}


def cargo_env(work: Path):
    env = dict(os.environ)
    env["CARGO_TARGET_DIR"] = str(work / "target")
    # Without debug info: with it, each test binary is ~1.5 GB and one
    # case's build fills a disk.
    env["CARGO_PROFILE_DEV_DEBUG"] = "false"
    env["GX_MUTE"] = "1"
    return env


def prepare_fixture(work: Path) -> str:
    """Worktree at BASE plus the overlay, committed. Returns its sha."""
    fx = work / "fixture"
    stamp = work / "fixture.sha"
    if fx.exists() and stamp.exists():
        return stamp.read_text().strip()
    if fx.exists():
        git(["worktree", "remove", "--force", str(fx)], REPO, check=False)
        shutil.rmtree(fx, ignore_errors=True)
    git(["worktree", "add", "--detach", str(fx), BASE], REPO)
    shutil.copytree(FIXTURE, fx, dirs_exist_ok=True)
    git(["add", "-A"], fx)
    git(["commit", "-q", "-m", "Coin Run fixture"], fx)
    sha = git(["rev-parse", "HEAD"], fx).stdout.strip()
    print("building the fixture and running its CI gates (first run takes ~15 min)...", flush=True)
    for name, cmd in GATES:
        p = sh(cmd, fx, env=cargo_env(work))
        if p.returncode != 0:
            sys.exit(f"the fixture itself fails {name}; fix eval/vibe/fixture first:\n"
                     f"{p.stdout[-3000:]}\n{p.stderr[-3000:]}")
    stamp.write_text(sha)
    return sha


def new_worktree(work: Path, sha: str) -> Path:
    """The one directory every case is built in. Cargo keys a path
    dependency's artifacts by its path, so a directory per case would leave
    a full set of game and test binaries behind for each one."""
    d = work / "case"
    if d.exists():
        git(["worktree", "remove", "--force", str(d)], REPO, check=False)
        shutil.rmtree(d, ignore_errors=True)
    git(["worktree", "add", "--detach", str(d), sha], REPO)
    return d


def apply_setup(d: Path, case_id: str):
    name = SETUPS.get(case_id)
    if not name:
        return None
    git(["apply", str(SETUP / f"{name}.patch")], d)
    git(["add", "-A"], d)
    git(["commit", "-q", "-m", f"setup: {name}"], d)
    return name


def run_agent(d: Path, text: str, args, work: Path) -> dict:
    prompt = PROMPT.replace("{request}", text)
    cmd = ["claude", "-p", prompt, "--output-format", "json", "--dangerously-skip-permissions"]
    if args.model:
        cmd += ["--model", args.model]
    env = cargo_env(work)
    env["IS_SANDBOX"] = "1"  # allow skipping permissions when running as root
    t0 = time.monotonic()
    try:
        p = sh(cmd, d, env=env, timeout=args.agent_timeout)
        out, err, rc = p.stdout, p.stderr, p.returncode
    except subprocess.TimeoutExpired as e:
        partial = e.stdout or ""
        out = partial.decode(errors="replace") if isinstance(partial, bytes) else partial
        err, rc = f"agent timed out after {args.agent_timeout} s", -1
    secs = time.monotonic() - t0
    try:
        j = json.loads(out)
    except json.JSONDecodeError:
        j = {"result": out, "is_error": True}
    reply = j.get("result") or ""
    return {
        "agent_secs": round(secs, 1),
        "agent_rc": rc,
        "agent_error": bool(j.get("is_error")) or rc != 0,
        "agent_stderr": err[-2000:] if rc != 0 else "",
        "turns": j.get("num_turns"),
        "cost_usd": j.get("total_cost_usd"),
        "reply": reply,
        **parse_reply(reply),
    }


def parse_reply(reply: str) -> dict:
    def grab(tag):
        m = re.search(rf"^\s*\**{tag}:?\**:?\s*(.+)", reply, re.M | re.S)
        return m.group(1).strip().split("\n\n")[0] if m else None

    q, c, r = grab("QUESTION"), grab("CANNOT"), grab("READBACK")
    kind = "question" if q else "cannot" if c else "changes" if r else "unclear"
    return {"reply_kind": kind, "question": q, "cannot": c, "readback": r}


def diff_stats(d: Path, results: Path, name: str) -> dict:
    git(["add", "-A"], d)
    stat = git(["diff", "--cached", "--shortstat", "HEAD"], d).stdout
    files = git(["diff", "--cached", "--name-only", "HEAD"], d).stdout.split()
    (results / f"{name}.diff").write_text(git(["diff", "--cached", "--binary", "HEAD"], d).stdout)
    num = lambda pat: int(m.group(1)) if (m := re.search(pat, stat)) else 0
    return {
        "files_changed": files,
        "insertions": num(r"(\d+) insertion"),
        "deletions": num(r"(\d+) deletion"),
        "regenerated_fixture": "tests/fixtures/selftest.gxr" in files,
    }


def failures(output: str) -> list[str]:
    return sorted(set(re.findall(r"^---- (\S+) stdout ----", output, re.M)))[:20]


def run_gates(d: Path, work: Path) -> dict:
    env = cargo_env(work)
    out = {}
    t0 = time.monotonic()
    p = sh(["cargo", "build"], d, env=env)
    out["build_ok"] = p.returncode == 0
    out["build_secs"] = round(time.monotonic() - t0, 1)
    if not out["build_ok"]:
        out["build_error"] = p.stderr[-3000:]
    for name, cmd in GATES:
        p = sh(cmd, d, env=env)
        out[f"{name}_ok"] = p.returncode == 0
        if p.returncode != 0:
            out[f"{name}_failures"] = failures(p.stdout) or [p.stderr[-1500:]]
    return out


def run_hidden(d: Path, work: Path, case_id: str) -> dict:
    src = HIDDEN / f"{case_id}.rs"
    if not src.exists():
        return {"hidden": "none"}
    name = f"zz_hidden_{case_id}"
    shutil.copy(src, d / "tests" / f"{name}.rs")
    p = sh(["cargo", "test", "--all-features", "--test", name], d, env=cargo_env(work))
    (d / "tests" / f"{name}.rs").unlink()
    if p.returncode == 0:
        return {"hidden": "pass"}
    if "error[E" in p.stderr or "could not compile" in p.stderr:
        return {"hidden": "compile_error", "hidden_detail": p.stderr[-2000:]}
    return {"hidden": "fail", "hidden_failures": failures(p.stdout),
            "hidden_detail": p.stdout[-2000:]}


def run_case(case: dict, sha: str, work: Path, results: Path, args, tag: str = "") -> dict:
    name = case["id"] + tag
    out_file = results / f"{name}.json"
    if out_file.exists() and not args.force:
        print(f"{name}: already done, skipping (--force to redo)")
        return json.loads(out_file.read_text())
    t0 = time.monotonic()
    d = new_worktree(work, sha)
    r = {"id": case["id"], "run": name, "category": case["category"], "text": case["text"],
         "expect": case["expect"], "setup": apply_setup(d, case["id"])}
    print(f"{name}: agent working on {case['text']!r}", flush=True)
    r |= run_agent(d, case["text"], args, work)
    r |= diff_stats(d, results, name)
    print(f"{name}: replied {r['reply_kind']} after {r['agent_secs']} s; checking", flush=True)
    r |= run_gates(d, work)
    r |= run_hidden(d, work, case["id"])
    r["wall_secs"] = round(time.monotonic() - t0, 1)
    out_file.write_text(json.dumps(r, indent=2))
    print(f"{name}: hidden={r['hidden']} suite_ok={r.get('suite_ok')} "
          f"clippy_ok={r.get('clippy_ok')} (+{r['insertions']}/-{r['deletions']})", flush=True)
    if not args.keep:
        git(["worktree", "remove", "--force", str(d)], REPO, check=False)
    return r


def run_chain(cases: dict, sha: str, work: Path, results: Path, args):
    d = new_worktree(work, sha)
    log = []
    for i, cid in enumerate(CHAIN):
        case = cases[cid]
        git(["add", "-A"], d)
        git(["commit", "-q", "--allow-empty", "-m", f"before {cid}"], d)
        print(f"chain {i + 1}/{len(CHAIN)} {cid}: {case['text']!r}", flush=True)
        step = {"step": i + 1, "id": cid, "text": case["text"]}
        step |= run_agent(d, case["text"], args, work)
        step |= diff_stats(d, results, f"chain-{i + 1:02d}-{cid}")
        step |= run_gates(d, work)
        step["hidden"] = {prev: run_hidden(d, work, prev)["hidden"] for prev in CHAIN[: i + 1]}
        log.append(step)
        (results / "chain.json").write_text(json.dumps(log, indent=2))
        broken = [k for k, v in step["hidden"].items() if v != "pass"]
        print(f"  suite_ok={step.get('suite_ok')} failing hidden tests: {broken or 'none'}", flush=True)
    write_chain_summary(log, results)


# Hidden tests that should already pass on the untouched fixture: a17 asks
# for something the game already does, and a12 only checks that nothing but
# a screen shake changed.
PASS_ON_FIXTURE = {"a12", "a17"}


def run_baseline(work: Path):
    """Every hidden test on the untouched fixture. All but PASS_ON_FIXTURE
    must fail, or the test can't tell a change from no change."""
    d = work / "fixture"
    bad = []
    for src in sorted(HIDDEN.glob("*.rs")):
        cid = src.stem
        got = run_hidden(d, work, cid)["hidden"]
        want = "pass" if cid in PASS_ON_FIXTURE else "fail"
        flag = "" if got == want else "   <-- expected " + want
        print(f"{cid}: {got}{flag}", flush=True)
        if got != want:
            bad.append(cid)
    if bad:
        sys.exit(f"baseline mismatch: {bad}")
    print("baseline ok: every hidden test distinguishes the change from no change")


def write_chain_summary(log, results: Path):
    lines = ["# Chain run", "",
             "Each row is one request applied on top of all the earlier ones. "
             "'Kept' counts how many earlier requests still pass their hidden test.", "",
             "| Step | Case | Reply | Own test | Earlier kept | Suite | Lines +/- | Agent s |",
             "|---|---|---|---|---|---|---|---|"]
    for s in log:
        earlier = [v for k, v in s["hidden"].items() if k != s["id"]]
        kept = sum(v == "pass" for v in earlier)
        lines.append(
            f"| {s['step']} | {s['id']} | {s['reply_kind']} | {s['hidden'][s['id']]} | "
            f"{kept}/{len(earlier)} | {'ok' if s.get('suite_ok') else 'FAIL'} | "
            f"+{s['insertions']}/-{s['deletions']} | {s['agent_secs']} |")
    (results / "chain.md").write_text("\n".join(lines) + "\n")


def passed(r: dict) -> bool:
    gates = all(r.get(k) for k in ("build_ok", "fmt_ok", "clippy_ok", "suite_ok"))
    return r["reply_kind"] == "changes" and gates and r["hidden"] in ("pass", "none")


def write_summary(rs: list[dict], results: Path):
    def pct(n, d):
        return f"{n}/{d} ({100 * n // d}%)" if d else "n/a"

    clear = [r for r in rs if r["expect"] == "changes"]
    ask = [r for r in rs if r["expect"] == "clarify"]
    refuse = [r for r in rs if r["expect"] == "cannot"]
    done = [r for r in rs if r["reply_kind"] == "changes"]
    secs = [r["agent_secs"] + r.get("build_secs", 0) for r in done]
    lines = [
        "# Vibe-coding arm: results", "",
        "Automatic numbers only. Readbacks, questions and refusals still need a person "
        "(see eval/vibe/README.md, 'Grading').", "",
        "| Metric | Result |", "|---|---|",
        f"| Clear requests done, gates green, hidden test passes | {pct(sum(passed(r) for r in clear), len(clear))} |",
        f"| Ambiguous requests answered with a question | {pct(sum(r['reply_kind'] == 'question' for r in ask), len(ask))} |",
        f"| Clear requests answered with a question | {pct(sum(r['reply_kind'] == 'question' for r in clear), len(clear))} |",
        f"| 'Refusal' cases attempted anyway | {pct(sum(r['reply_kind'] == 'changes' for r in refuse), len(refuse))} |",
        f"| ...of those, hidden test passes (where one exists) | {pct(sum(r['hidden'] == 'pass' for r in refuse), sum(r['hidden'] != 'none' for r in refuse))} |",
        f"| Changes that left CI red (fmt, clippy or tests) | {pct(sum(not all(r.get(k) for k in ('fmt_ok', 'clippy_ok', 'suite_ok')) for r in done), len(done))} |",
        f"| Sim changes without a regenerated replay fixture | {pct(sum(not r['regenerated_fixture'] and not r.get('suite_ok') for r in done), len(done))} |",
    ]
    if secs:
        lines.append(f"| Seconds from request to a fresh build (median, max) | {statistics.median(secs):.0f}, {max(secs):.0f} |")
    costs = [r["cost_usd"] for r in rs if r.get("cost_usd")]
    if costs:
        lines.append(f"| Agent cost, USD (total, median per case) | {sum(costs):.2f}, {statistics.median(costs):.2f} |")
    lines += ["", "| Case | Expect | Reply | Hidden | fmt | clippy | suite | Lines +/- | Agent s | Readback / question / reason |",
              "|---|---|---|---|---|---|---|---|---|---|"]
    ok = lambda v: "ok" if v else "FAIL"
    for r in sorted(rs, key=lambda r: r["run"]):
        said = (r.get("readback") or r.get("question") or r.get("cannot") or "").replace("|", "/").replace("\n", " ")
        lines.append(
            f"| {r['run']} | {r['expect']} | {r['reply_kind']} | {r['hidden']} | {ok(r.get('fmt_ok'))} | "
            f"{ok(r.get('clippy_ok'))} | {ok(r.get('suite_ok'))} | +{r['insertions']}/-{r['deletions']} | "
            f"{r['agent_secs']} | {said[:200]} |")
    (results / "summary.md").write_text("\n".join(lines) + "\n")
    print(f"wrote {results / 'summary.md'}")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    pick = ap.add_mutually_exclusive_group(required=True)
    pick.add_argument("--cases", help="comma-separated ids, e.g. a01,b03")
    pick.add_argument("--all", action="store_true", help="every case in the eval set")
    pick.add_argument("--chain", action="store_true", help="the cumulative chain (see CHAIN)")
    pick.add_argument("--fixture-only", action="store_true", help="build the fixture and stop")
    pick.add_argument("--baseline", action="store_true",
                      help="run every hidden test on the untouched fixture (sanity check)")
    ap.add_argument("--work", type=Path, default=Path.home() / ".cache/gamebient-vibe-eval",
                    help="worktrees, build cache and results (default: %(default)s)")
    ap.add_argument("--model", help="model for the agent (default: Claude Code's default)")
    ap.add_argument("--repeat", type=int, default=1, help="runs per case (the spec asks for 5)")
    ap.add_argument("--agent-timeout", type=int, default=1800, help="seconds per agent run")
    ap.add_argument("--keep", action="store_true", help="keep the last case's worktree")
    ap.add_argument("--force", action="store_true", help="redo cases that already have results")
    args = ap.parse_args()

    if not shutil.which("claude") and not (args.fixture_only or args.baseline):
        sys.exit("needs the Claude Code CLI (`claude`) on PATH, signed in")
    args.work.mkdir(parents=True, exist_ok=True)
    results = args.work / "results"
    results.mkdir(exist_ok=True)
    cases = load_cases()
    sha = prepare_fixture(args.work)
    print(f"fixture {sha[:9]} in {args.work / 'fixture'}")
    if args.fixture_only:
        return
    if args.baseline:
        run_baseline(args.work)
        return
    if args.chain:
        run_chain(cases, sha, args.work, results, args)
        return
    ids = list(cases) if args.all else args.cases.split(",")
    unknown = [i for i in ids if i not in cases]
    if unknown:
        sys.exit(f"unknown case ids: {unknown}")
    rs = []
    for cid in ids:
        for k in range(args.repeat):
            tag = f".r{k + 1}" if args.repeat > 1 else ""
            rs.append(run_case(cases[cid], sha, args.work, results, args, tag))
    done = [json.loads(p.read_text()) for p in sorted(results.glob("*.json")) if p.name != "chain.json"]
    write_summary(done, results)


if __name__ == "__main__":
    main()
