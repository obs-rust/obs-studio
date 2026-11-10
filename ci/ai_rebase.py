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
import importlib.util
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


def removed_lines(base: str, side: str) -> set[str]:
    s = set(side.splitlines())
    return {l for l in base.splitlines() if l.strip() and l not in s}


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


# Read once at start: the PR checkout may predate this file.
PROMPT = Path(__file__).with_name("ai_rebase_prompt.md").read_text()


def clud_cmd() -> list[str] | None:
    """The clud agent, run unattended on DeepSeek Flash through OpenRouter."""
    exe = shutil.which("clud")
    base = [exe] if exe else (["uvx", "clud"] if shutil.which("uvx") else None)
    if not base:
        return None
    return [*base, "--openrouter", "--model", MODEL, "--unattended"]


def check_resolution(f: str, base: str, ours: str, theirs: str) -> str | None:
    """Judge one resolved file against its three merge stages."""
    if not Path(f).exists():
        return None
    text = Path(f).read_text(errors="replace")
    if MARKER.search(text):
        return f"{f}: conflict markers remain"
    kept = set(text.splitlines())
    # A port moves a C file out of a core unity group by deleting its
    # include; keeping that line compiles the C twice (#4214's first
    # rebase, and #4387's forced merge before it).
    revived = removed_lines(base, theirs) & kept - added_lines(base, ours)
    if revived:
        return f"{f}: the PR deleted {sorted(revived)[0]!r} but the resolution keeps it"
    lost = (added_lines(base, ours) | added_lines(base, theirs)) - kept
    if len(lost) > 2:
        return f"{f}: {len(lost)} added lines dropped, e.g. {sorted(lost)[0]!r}"
    return None


def resolve_step(files: list[str], subject: str) -> str | None:
    """Resolve one stopped rebase step, then check the result ourselves.

    The clud agent (prompt: ci/ai_rebase_prompt.md) can read neighbouring code
    and run the repo's fixers, which a single completion cannot. Without clud,
    each file goes to the model directly. Either way the checks below decide.
    """
    stages = {f: (stage(1, f), stage(2, f), stage(3, f)) for f in files}
    cmd = clud_cmd()
    if cmd:
        prompt = (PROMPT + f"\nPR commit: {subject}\nConflicted files:\n"
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
    for f, sides in stages.items():
        why = check_resolution(f, *sides)
        if why:
            return why
    return None


def has_commit(sha: str) -> bool:
    return not subprocess.run(["git", "cat-file", "-e", f"{sha}^{{commit}}"],
                              capture_output=True).returncode


def rebase_target(stack: tuple[int, str, str] | None) -> list[str] | str:
    """`git rebase` arguments, or a skip/failure message."""
    if not stack:
        return ["origin/master"]
    parent, base, state = stack
    if state != "MERGED":
        return f"skipped: stacked on #{parent}, which is not merged yet"
    if not has_commit(base):
        # The parent's branch is usually deleted after merge; GitHub keeps its
        # head under refs/pull/<P>/head.
        run("git", "fetch", "-q", "origin", f"+refs/pull/{parent}/head:refs/ai-rebase/pr-{parent}",
            check=False)
        if not has_commit(base):
            return f"failed: parent head {base[:9]} of #{parent} is not fetchable"
    return ["--onto", "origin/master", base]


def replay(target: list[str]) -> int | str:
    """Run the rebase, resolving each stopped step; files resolved, or a message."""
    p = subprocess.run(["git", "rebase", *target], capture_output=True, text=True)
    resolved = 0
    while p.returncode:
        files = run("git", "diff", "--name-only", "--diff-filter=U").split()
        subject = run("git", "log", "-1", "--format=%s", "REBASE_HEAD", check=False).strip()
        why = (resolve_step(files, subject) if files
               else f"rebase stopped without conflicts: {p.stderr.strip()[:300]}")
        if not why and subprocess.run(["git", "diff", "--cached", "--check"]).returncode:
            why = "resolution fails git diff --check"
        if why:
            run("git", "rebase", "--abort", check=False)
            return f"refused: {why}"
        resolved += len(files)
        p = subprocess.run(["git", "-c", "core.editor=true", "rebase", "--continue"],
                           capture_output=True, text=True)
    return resolved


# Optional local check run on the rebased head before arming, e.g.
# "uv run --no-project ci/windows_link_check.py". Branch-only work published
# after the 2026-11-09 backlog failed the Windows COFF link about one time in
# three; checking here keeps those PRs out of the queue instead of spending a
# hosted run to find out.
PREFLIGHT = os.environ.get("AI_REBASE_PREFLIGHT", "")


def preflight() -> tuple[bool, str]:
    p = subprocess.run(PREFLIGHT, shell=True, capture_output=True, text=True, timeout=3600)
    if p.returncode:
        tail = (p.stdout + p.stderr).strip().splitlines()[-3:]
        return False, f"preflight `{PREFLIGHT}` failed: {' / '.join(tail)[:300]}"
    return True, ""


def publish(n: int, pr: dict, stack: tuple[int, str, str] | None, new: str) -> str:
    """Push the rebased head, settle a landed stack, then decide on the queue."""
    br, old = pr["headRefName"], pr["headRefOid"]
    run("git", "push", "-q", f"--force-with-lease=refs/heads/{br}:{old}", "origin", f"HEAD:refs/heads/{br}")
    if stack:
        # The parent landed, so this is now an ordinary PR: replace the
        # stacking note (it would read as a hold) and take it out of draft.
        pr["body"] = STACKED.sub(f"Parent #{stack[0]} has merged; ai-rebase replayed "
                                 "only this PR's own commits onto master.", pr.get("body") or "", 1)
        run("gh", "pr", "edit", str(n), "--repo", REPO, "--body", pr["body"], check=False)
        if pr.get("isDraft"):
            run("gh", "pr", "ready", str(n), "--repo", REPO, check=False)
            pr["isDraft"] = False
    go, why = should_enqueue(pr, stack[0] if stack else None)
    if go and PREFLIGHT:
        go, why = preflight()
    if go:
        run("gh", "pr", "merge", str(n), "--repo", REPO, "--auto", "--squash",
            "--match-head-commit", new, check=False)
    return "auto-merge armed" if go else f"left out of the queue: {why}"


def rebase_pr(n: int, dry_run: bool) -> str:
    pr = gh_json("pr", "view", str(n), "--repo", REPO, "--json",
                 "headRefName,headRefOid,isCrossRepository,autoMergeRequest,id,state,isDraft,body,"
                 "labels,statusCheckRollup,reviewDecision")
    if pr["state"] != "OPEN" or pr["isCrossRepository"]:
        return "skipped: not an open same-repo PR"
    br, old = pr["headRefName"], pr["headRefOid"]
    run("git", "fetch", "-q", "origin", "master", f"+refs/heads/{br}:refs/ai-rebase/{br}")
    run("git", "checkout", "-q", "--detach", f"refs/ai-rebase/{br}")
    if run("git", "rev-parse", "HEAD").strip() != old:
        return "skipped: head moved while fetching"
    stack = stacked_parent(pr)
    target = rebase_target(stack)
    if isinstance(target, str):
        return target
    resolved = replay(target)
    if isinstance(resolved, str):
        return resolved
    new = run("git", "rev-parse", "HEAD").strip()
    if dry_run:
        return f"dry-run: {old[:9]} -> {new[:9]}, {resolved} file(s) resolved"
    verdict = publish(n, pr, stack, new)
    return f"pushed: {old[:9]} -> {new[:9]}, {resolved} file(s) AI-resolved; {verdict}"

STACKED = re.compile(r"Stacked on #(\d+)[^\n]*?(?:after\s+`?([0-9a-f]{7,40})`?)?[^\n]*", re.I)


def stacked_parent(pr: dict) -> tuple[int, str, str] | None:
    """(parent PR, last parent commit, parent state) for a stacked PR.

    Stacked drafts say "Stacked on #P: review only the commits after <sha>"
    (im-stacked-template). Once P squash-merges, replaying from <sha> onto
    master carries only this PR's own commits; a plain rebase would replay
    P's commits too and conflict with their squashed copy.
    """
    m = STACKED.search(pr.get("body") or "")
    if not m:
        return None
    parent = gh_json("pr", "view", m.group(1), "--repo", REPO, "--json", "headRefOid,state")
    return int(m.group(1)), m.group(2) or parent["headRefOid"], parent["state"]


HOLD = re.compile(r"do not (enqueue|merge)|don't (enqueue|merge)|\bhold\b|\bwip\b|stacked on #",
                  re.I)


def live_holds(text: str, merged_parent: int | None) -> bool:
    """A hold line that waits on the parent ("until #P merges") ends when P has."""
    lines = [l for l in text.splitlines() if HOLD.search(l)]
    if merged_parent is not None:
        lines = [l for l in lines if f"#{merged_parent}" not in l]
    return bool(lines)


def should_enqueue(pr: dict, merged_parent: int | None = None) -> tuple[bool, str]:
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
    if live_holds(pr.get("body") or "", merged_parent) or HOLD.search(labels):
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
                  "--json", "number,isDraft,mergeStateStatus,createdAt,body")
    # Drafts are someone's hold, except a stacked draft, which waits only for
    # its parent; rebase_pr skips it while the parent is unmerged.
    dirty = sorted((p for p in prs if p["mergeStateStatus"] == "DIRTY"
                    and (not p["isDraft"] or STACKED.search(p.get("body") or ""))),
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
        spec = importlib.util.spec_from_file_location(
            "backpressure", Path(__file__).with_name("backpressure.py"))
        backpressure = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(backpressure)
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
