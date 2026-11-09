#!/usr/bin/env -S uv run --no-project --script
# /// script
# requires-python = ">=3.11"
# ///
"""Rebase conflicting PRs onto master, resolving conflicts with an LLM.

    uv run --no-project --script ci/ai_rebase.py 4438            # one PR
    uv run --no-project --script ci/ai_rebase.py --dirty         # every DIRTY PR, within backpressure headroom
    uv run --no-project --script ci/ai_rebase.py 4438 --dry-run  # resolve locally, don't push

Run inside a clean checkout of the repo. Needs OPENROUTER_API_KEY. The model
is `$AI_REBASE_MODEL`, default `~deepseek/deepseek-flash-latest`.

Most of our conflicts are shared append points (unity group includes,
`obs-internal.h` declarations, `test-graph.json`, `mod` lists, README rows):
both sides added lines next to each other and the answer is "keep both, in
order". The model gets one conflicted file at a time with the PR's own commit
message, and must return the whole resolved file. A resolution is refused,
and the PR is left alone with a comment, when it:
  - still contains conflict markers,
  - drops a line that was added on either side (both additions must survive),
  - fails `git diff --check`.
Lock files and generated outputs are never sent to the model. The push is
`--force-with-lease=<branch>:<old head>`, so a concurrent push by the author
wins. Dequeuing clears auto-merge, so the script re-arms it when it was armed.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import urllib.request
from pathlib import Path

REPO = "obs-rust/obs-studio"
MODEL = os.environ.get("AI_REBASE_MODEL", "~deepseek/deepseek-flash-latest")
MAX_FILE_BYTES = 400_000
NEVER_AI = re.compile(r"(Cargo\.lock|\.lock$|\.snap$|\.png$|\.bin$)")
MARKER = re.compile(r"^(<{7}|={7}|>{7}|\|{7})( |$)", re.M)


def run(*cmd: str, check: bool = True, input: str | None = None) -> str:
    p = subprocess.run(cmd, capture_output=True, text=True, input=input)
    if check and p.returncode:
        raise RuntimeError(f"{' '.join(cmd)}: {p.stderr.strip() or p.stdout.strip()}")
    return p.stdout


def gh_json(*args: str):
    return json.loads(run("gh", *args) or "null")


def ask_model(prompt: str) -> str:
    key = os.environ["OPENROUTER_API_KEY"]
    body = json.dumps({
        "model": MODEL,
        "temperature": 0,
        "messages": [
            {"role": "system", "content": (
                "You resolve git merge conflicts. Reply with the complete resolved file "
                "and nothing else: no code fences, no commentary. Keep every line that "
                "either side added unless the two sides edit the same line, in which "
                "case combine their intent. Keep sorted lists sorted.")},
            {"role": "user", "content": prompt},
        ],
    }).encode()
    req = urllib.request.Request(
        "https://openrouter.ai/api/v1/chat/completions", data=body,
        headers={"Authorization": f"Bearer {key}", "Content-Type": "application/json",
                 "X-Title": "obs-rust ai-rebase"})
    with urllib.request.urlopen(req, timeout=300) as r:
        text = json.load(r)["choices"][0]["message"]["content"]
    text = re.sub(r"^```[^\n]*\n|\n```\s*$", "", text)
    return text if text.endswith("\n") else text + "\n"


def stage(n: int, path: str) -> str:
    return run("git", "show", f":{n}:{path}", check=False)


def added_lines(base: str, side: str) -> set[str]:
    b = set(base.splitlines())
    return {l for l in side.splitlines() if l.strip() and l not in b}


def resolve(path: str, subject: str) -> str | None:
    if NEVER_AI.search(path):
        return f"{path}: not sent to the model (lock/binary)"
    conflicted = Path(path).read_text(errors="replace")
    if len(conflicted) > MAX_FILE_BYTES:
        return f"{path}: too large for an AI resolution"
    base, ours, theirs = stage(1, path), stage(2, path), stage(3, path)
    prompt = (f"File: {path}\nThe commit being replayed onto master: {subject}\n"
              "'ours' (HEAD) is current master; 'theirs' is the PR commit.\n\n"
              f"Conflicted file:\n{conflicted}")
    out = ask_model(prompt)
    if MARKER.search(out):
        return f"{path}: model left conflict markers"
    kept = set(out.splitlines())
    lost = (added_lines(base, ours) | added_lines(base, theirs)) - kept
    # Lines both sides changed may legitimately merge; refuse only when many vanish.
    if len(lost) > 2:
        return f"{path}: model dropped {len(lost)} added lines, e.g. {sorted(lost)[0]!r}"
    Path(path).write_text(out)
    run("git", "add", "--", path)
    return None


PROMPT_FILE = Path(__file__).with_name("ai_rebase_prompt.md")


def clud_cmd() -> list[str] | None:
    """The clud agent, run unattended on DeepSeek Flash through OpenRouter."""
    exe = shutil.which("clud")
    base = [exe] if exe else (["uvx", "clud"] if shutil.which("uvx") else None)
    if not base:
        return None
    return [*base, "--openrouter", "--model", MODEL, "--unattended"]


def resolve_step(files: list[str], subject: str) -> str | None:
    """Resolve one stopped rebase step, then check the result ourselves.

    The clud agent (prompt: ci/ai_rebase_prompt.md) can read neighbouring code
    and run the repo's fixers, which a single completion cannot. Without clud,
    each file goes to the model directly. Either way the checks below decide.
    """
    stages = {f: (stage(1, f), stage(2, f), stage(3, f)) for f in files}
    cmd = clud_cmd()
    if cmd:
        prompt = (PROMPT_FILE.read_text() + f"\nPR commit: {subject}\nConflicted files:\n"
                  + "\n".join(f"- {f}" for f in files) + "\n")
        p = subprocess.run([*cmd, "-p", prompt, "--", "--dangerously-skip-permissions"],
                           capture_output=True, text=True, timeout=1200)
        tail = (p.stdout or p.stderr).strip()[-300:]
        if p.returncode:
            return f"clud agent exited {p.returncode}: {tail}"
    else:
        for f in files:
            why = resolve(f, subject)
            if why:
                return why
    if run("git", "diff", "--name-only", "--diff-filter=U").split():
        return "agent left files unresolved"
    for f, (base, ours, theirs) in stages.items():
        if not Path(f).exists():
            continue
        text = Path(f).read_text(errors="replace")
        if MARKER.search(text):
            return f"{f}: conflict markers remain"
        lost = (added_lines(base, ours) | added_lines(base, theirs)) - set(text.splitlines())
        if len(lost) > 2:
            return f"{f}: {len(lost)} added lines dropped, e.g. {sorted(lost)[0]!r}"
    return None


def rebase_pr(n: int, dry_run: bool) -> str:
    pr = gh_json("pr", "view", str(n), "--repo", REPO, "--json",
                 "headRefName,headRefOid,isCrossRepository,autoMergeRequest,id,state,isDraft,body,"
                 "labels,statusCheckRollup,reviewDecision")
    if pr["state"] != "OPEN" or pr["isCrossRepository"]:
        return "skipped: not an open same-repo PR"
    br, old = pr["headRefName"], pr["headRefOid"]
    run("git", "fetch", "-q", "origin", "master", f"+refs/heads/{br}:refs/ai-rebase/{br}")
    run("git", "checkout", "-q", "--detach", f"refs/ai-rebase/{br}")
    env_ok = run("git", "rev-parse", "HEAD").strip() == old
    if not env_ok:
        return "skipped: head moved while fetching"
    p = subprocess.run(["git", "rebase", "origin/master"], capture_output=True, text=True)
    resolved = 0
    while p.returncode:
        files = run("git", "diff", "--name-only", "--diff-filter=U").split()
        if not files:
            run("git", "rebase", "--abort", check=False)
            return f"failed: rebase stopped without conflicts: {p.stderr.strip()[:300]}"
        subject = run("git", "log", "-1", "--format=%s", "REBASE_HEAD", check=False).strip()
        why = resolve_step(files, subject)
        if why:
            run("git", "rebase", "--abort", check=False)
            return f"refused: {why}"
        resolved += len(files)
        if subprocess.run(["git", "diff", "--cached", "--check"]).returncode:
            run("git", "rebase", "--abort", check=False)
            return "refused: resolution fails git diff --check"
        p = subprocess.run(["git", "-c", "core.editor=true", "rebase", "--continue"],
                           capture_output=True, text=True)
    new = run("git", "rev-parse", "HEAD").strip()
    if dry_run:
        return f"dry-run: {old[:9]} -> {new[:9]}, {resolved} file(s) resolved"
    run("git", "push", "-q", f"--force-with-lease=refs/heads/{br}:{old}", "origin", f"HEAD:refs/heads/{br}")
    go, why = should_enqueue(pr)
    if go:
        run("gh", "pr", "merge", str(n), "--repo", REPO, "--auto", "--squash",
            "--match-head-commit", new, check=False)
    verdict = "auto-merge armed" if go else f"left out of the queue: {why}"
    return f"pushed: {old[:9]} -> {new[:9]}, {resolved} file(s) AI-resolved; {verdict}"


HOLD = re.compile(r"do not (enqueue|merge)|don't (enqueue|merge)|\bhold\b|\bwip\b|stacked on #",
                  re.I)


def should_enqueue(pr: dict) -> tuple[bool, str]:
    """Put the rebased PR back in the merge queue unless something says not to.

    Biased towards yes (maintainer, 2026-11-09): a PR that conflicted was
    almost always on its way in. Only an explicit signal keeps it out: a draft,
    a hold in the body or labels, requested changes, or a check that failed
    for a reason other than the conflict on the old head. The queue itself
    applies back pressure, so admission is not re-decided here.
    """
    if pr.get("isDraft"):
        return False, "draft"
    labels = " ".join(l["name"] for l in pr.get("labels") or [])
    if HOLD.search(pr.get("body") or "") or HOLD.search(labels):
        return False, "hold in the PR body or labels"
    if pr.get("reviewDecision") == "CHANGES_REQUESTED":
        return False, "changes requested"
    failed = [c.get("name") or c.get("context") for c in pr.get("statusCheckRollup") or []
              if (c.get("conclusion") or c.get("state")) in ("FAILURE", "ERROR", "TIMED_OUT")]
    # A gate/aggregate failure follows from the real one; ignore it, judge the rest.
    real = [f for f in failed if f and "Gate" not in f and f != "claim"]
    if len(real) > 2:
        return False, f"{len(real)} checks failed on the old head ({', '.join(real[:3])})"
    return True, ""


def dirty_prs(limit: int) -> list[int]:
    prs = gh_json("pr", "list", "--repo", REPO, "--state", "open", "--limit", "300",
                  "--json", "number,isDraft,mergeStateStatus,createdAt")
    dirty = sorted((p for p in prs if p["mergeStateStatus"] == "DIRTY" and not p["isDraft"]),
                   key=lambda p: p["createdAt"])  # oldest first: FIFO fairness
    return [p["number"] for p in dirty[:limit]]


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("prs", nargs="*", type=int)
    ap.add_argument("--dirty", action="store_true")
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--comment", action="store_true", help="post the outcome on the PR")
    a = ap.parse_args()
    prs = list(a.prs)
    if a.dirty:
        sys.path.insert(0, str(Path(__file__).parent))
        import backpressure
        v = backpressure.measure(REPO)
        # A rebase restarts CI, so it spends queue capacity like a new PR does.
        prs += dirty_prs(max(1, v["headroom"]) if v["state"] == "open" else 0)
        print(f"backpressure {v['state']}, rebasing {prs}")
    start = run("git", "rev-parse", "HEAD").strip()
    rc = 0
    for n in prs:
        try:
            out = rebase_pr(n, a.dry_run)
        except Exception as e:  # report and move to the next PR
            run("git", "rebase", "--abort", check=False)
            out = f"failed: {e}"
        print(f"#{n} {out}")
        if a.comment and not out.startswith("skipped"):
            run("gh", "pr", "comment", str(n), "--repo", REPO, "--body",
                f"ai-rebase ({MODEL}): {out}", check=False)
        rc |= 0 if out.startswith(("pushed", "dry-run", "skipped")) else 1
        run("git", "checkout", "-q", "--detach", start, check=False)
    return rc


if __name__ == "__main__":
    sys.exit(main())
