#!/usr/bin/env -S uv run --no-project --script
# /// script
# requires-python = ">=3.11"
# ///
"""Rebase conflicting PRs onto master, resolving conflicts with an LLM.

    uv run --no-project --script ci/ai_rebase.py 4438            # one PR
    uv run --no-project --script ci/ai_rebase.py --dirty         # every DIRTY PR, within backpressure headroom
    uv run --no-project --script ci/ai_rebase.py --requeue-ejected # retry infrastructure ejections, no push token
    uv run --no-project --script ci/ai_rebase.py 4438 --dry-run  # resolve locally, don't push

Run inside a clean checkout of the repo. Needs OPENROUTER_API_KEY. The model
is `$AI_REBASE_MODEL`, default `~deepseek/deepseek-flash-latest`.

Most of our conflicts are shared append points (unity group includes,
`obs-internal.h` declarations, `test-graph.d`, `mod` lists, README rows):
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
import datetime as dt
import functools
import importlib.util
import json
import os
import re
import shutil
import subprocess
import sys
import urllib.request
import urllib.parse
from pathlib import Path

REPO = "obs-rust/obs-studio"
MODEL = os.environ.get("AI_REBASE_MODEL", "~deepseek/deepseek-flash-latest")
# Load before checking out older PR commits, just like the agent prompt below.
_generated_spec = importlib.util.spec_from_file_location(
    "native_expectations", Path(__file__).with_name("native_expectations.py"))
native_expectations = importlib.util.module_from_spec(_generated_spec)
_generated_spec.loader.exec_module(native_expectations)
MAX_FILE_BYTES = 400_000
NEVER_AI = re.compile(r"(Cargo\.lock|\.lock$|\.snap$|\.png$|\.bin$)")
MARKER = re.compile(r"^(<{7}|={7}|>{7}|\|{7})( |$)", re.M)


def plain_env() -> dict[str, str]:
    """The environment with color forcing removed. setup-soldr exports
    CLICOLOR_FORCE=1 and FORCE_COLOR=1, and gh then colors --json output even
    into a pipe; CLICOLOR_FORCE=0 is what turns it back off."""
    env = {k: v for k, v in os.environ.items() if k not in ("FORCE_COLOR", "GH_FORCE_TTY")}
    env.update(CLICOLOR_FORCE="0", NO_COLOR="1")
    return env


def run(*cmd: str, check: bool = True, input: str | None = None) -> str:
    p = subprocess.run(cmd, capture_output=True, text=True, input=input, env=plain_env())
    if check and p.returncode:
        raise RuntimeError(f"{' '.join(cmd)}: {p.stderr.strip() or p.stdout.strip()}")
    return p.stdout


def gh_json(*args: str):
    if args[:2] == ("api", "graphql"):
        return json.loads(run("uv", "run", "--no-project", "--script",
                              str(Path(__file__).with_name("gh_graphql.py")), *args[2:]))
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
    generated = [f for f in files if f in native_expectations.CASES]
    sources = [f for f in files if f not in native_expectations.CASES]
    stages = {f: (stage(1, f), stage(2, f), stage(3, f)) for f in sources}
    cmd = clud_cmd() if sources else None
    if cmd:
        prompt = (PROMPT + f"\nPR commit: {subject}\nConflicted files:\n"
                  + "\n".join(f"- {f}" for f in sources)
                  + "\nGenerated snapshots are regenerated by the driver; leave them alone.\n")
        p = subprocess.run([*cmd, "-p", prompt, "--", "--dangerously-skip-permissions"],
                           capture_output=True, text=True, timeout=1200)
        tail = (p.stdout or p.stderr).strip()[-300:]
        if p.returncode:
            return f"clud agent exited {p.returncode}: {tail}"
    else:
        for f in sources:
            why = resolve(f, subject)
            if why:
                return why
    unresolved = run("git", "diff", "--name-only", "--diff-filter=U").split()
    if any(f not in generated for f in unresolved):
        return "agent left files unresolved"
    for f, sides in stages.items():
        why = check_resolution(f, *sides)
        if why:
            return why
    if generated:
        try:
            native_expectations.regenerate(Path.cwd(), generated)
        except (OSError, ValueError, KeyError, TypeError) as exc:
            return f"generated snapshot regeneration failed: {exc}"
        run("git", "add", "--", *generated)
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


# Checks run on the rebased head before arming. The unity build-group check is
# always on: it needs no toolchain, takes seconds, and catches the resolver
# filing a merged sibling's file into the wrong group (#4588 after #4585, which
# broke every Tier 2 job). AI_REBASE_PREFLIGHT adds a heavier local check, e.g.
# "uv run --no-project ci/windows_link_check.py": branch-only work published
# after the 2026-11-09 backlog failed the Windows COFF link about one time in
# three, and checking here keeps those PRs out of the queue instead of spending
# a hosted run to find out.
PREFLIGHT = " && ".join(filter(None, ["uv run --no-project ci/test_build_groups.py",
                                      os.environ.get("AI_REBASE_PREFLIGHT", "")]))
PREFLIGHT_ERROR = re.compile(
    r"undefined symbol|unresolved external|error(\[E\d+\])?: |error LNK"
    # soldr refuses while another build (or a stale lock) holds the target dir.
    r"|refused to modify .* while build lock"
    # ci/test_build_groups.py names the offending file in its assertion.
    r"|must include exactly its group|First extra element \d+:"
)


def preflight() -> tuple[bool, str]:
    """Run PREFLIGHT, retrying once; name the failing lines, not a traceback.

    A failure with no compiler or linker error line is retried once: on
    2026-11-10 two preflights sharing one soldr cache failed 4 of 8 PRs that
    each passed alone (#4495, #4503).
    """
    for _ in range(2):
        p = subprocess.run(PREFLIGHT, shell=True, capture_output=True, text=True, timeout=3600)
        if not p.returncode:
            return True, ""
        lines = (p.stdout + p.stderr).splitlines()
        errors = sorted({l.strip()[-160:] for l in lines if PREFLIGHT_ERROR.search(l)})
        if errors:
            return False, f"preflight failed: {' / '.join(errors[:4])}"
    return False, f"preflight `{PREFLIGHT}` failed twice with no compiler error line"

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
        try:
            run("gh", "pr", "merge", str(n), "--repo", REPO, "--auto",
                "--match-head-commit", new)
        except RuntimeError as e:
            return f"failed to arm auto-merge: {e}"
    if why == "failed in the merge queue":
        return f"left out: {why}"
    return "auto-merge armed" if go else f"left out of the queue: {why}"


def rebase_pr(n: int, dry_run: bool) -> str:
    pr = gh_json("pr", "view", str(n), "--repo", REPO, "--json",
                 "headRefName,headRefOid,isCrossRepository,autoMergeRequest,id,state,isDraft,body,"
                 "number,labels,statusCheckRollup,reviewDecision,updatedAt")
    if pr["state"] != "OPEN" or pr["isCrossRepository"]:
        return "skipped: not an open same-repo PR"
    if worker_active(pr):
        return "skipped: worker active"
    # Snapshot the old head's queue outcome before the rebase changes it.
    pr["failedInQueue"] = failed_in_queue(pr)
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


def worker_active(pr: dict, *, now: dt.datetime | None = None) -> bool:
    """Cheap, conservative ownership signal shared by bulk and explicit PRs.

    GitHub's commit pushedDate is often null. PR updatedAt advances on pushes,
    including an old authored commit, and may also grant extra quiet time after
    a comment/label edit. Prefer that false positive to interrupting a worker.
    """
    if any(l["name"] == "pool-active" for l in pr.get("labels") or []):
        return True
    updated = pr.get("updatedAt")
    if not updated:
        return True  # unavailable activity data must not expose a live worker
    stamp = dt.datetime.fromisoformat(updated.replace("Z", "+00:00"))
    return (now or dt.datetime.now(dt.timezone.utc)) - stamp < dt.timedelta(minutes=30)


def queue_events(pr: dict):
    """Read the head's relevant timeline backwards through the shared ghd wrapper."""
    query = """query($number:Int!,$before:String){
      repository(owner:"obs-rust",name:"obs-studio"){
        pullRequest(number:$number){headRefOid
          timelineItems(last:100,before:$before,itemTypes:[REMOVED_FROM_MERGE_QUEUE_EVENT,
              ADDED_TO_MERGE_QUEUE_EVENT,AUTO_MERGE_DISABLED_EVENT,AUTO_MERGE_ENABLED_EVENT,
              PULL_REQUEST_COMMIT,HEAD_REF_FORCE_PUSHED_EVENT]){
            pageInfo{hasPreviousPage startCursor}
            nodes{__typename
              ...on RemovedFromMergeQueueEvent{id createdAt reason beforeCommit{oid}}
              ...on PullRequestCommit{commit{oid}}
              ...on HeadRefForcePushedEvent{afterCommit{oid}}
            }
          }
        }
      }
    }"""
    cursor = None
    while True:
        args = ["-f", f"query={query}", "-F", f"number={pr['number']}"]
        if cursor:
            args += ["-f", f"before={cursor}"]
        data = gh_json("api", "graphql", *args)
        if data.get("errors"):
            raise RuntimeError(f"cannot read queue removal: {data['errors']}")
        current = data["data"]["repository"]["pullRequest"]
        if current["headRefOid"] != pr["headRefOid"]:
            raise RuntimeError("head moved while reading queue removal")
        timeline = current["timelineItems"]
        yield from reversed(timeline["nodes"])
        page = timeline["pageInfo"]
        if not page["hasPreviousPage"]:
            return
        next_cursor = page["startCursor"]
        if not next_cursor or next_cursor == cursor:
            raise RuntimeError("incomplete queue removal timeline")
        cursor = next_cursor


def failed_in_queue(pr: dict) -> bool:
    """Bind the latest failed ejection to the PR head that preceded it.

    beforeCommit is the synthetic queue commit, NOT the PR head. Ignore newer
    head arrivals until the latest removal, then compare its preceding head to
    the current one. Restoring an identical failed head must preserve failure.
    """
    if any(l["name"] == "queue-failed" for l in pr.get("labels") or []):
        return True
    if not pr.get("number"):
        return False
    failed = False
    for event in queue_events(pr):
        kind = event["__typename"]
        if kind == "RemovedFromMergeQueueEvent" and not failed:
            if event["reason"] != "failed_checks":
                return False
            failed = True
        elif failed and kind in ("PullRequestCommit", "HeadRefForcePushedEvent"):
            head = (event.get("commit") or event.get("afterCommit") or {}).get("oid")
            if not head:
                raise RuntimeError("cannot establish the head of the failed queue removal")
            return head == pr["headRefOid"]
    if failed:
        raise RuntimeError("cannot establish the head of the failed queue removal")
    return False


def should_enqueue(pr: dict, merged_parent: int | None = None) -> tuple[bool, str]:
    """Put the rebased PR back in the merge queue unless something says not to.

    Biased towards yes (maintainer, 2026-11-09): a PR that conflicted was
    almost always on its way in. Only an explicit signal keeps it out: a draft,
    a hold in the body or labels, requested changes, or a check that failed
    for a reason other than the conflict on the old head. The queue itself
    applies back pressure, so admission is not re-decided here.
    """
    if (pr["failedInQueue"] if "failedInQueue" in pr else failed_in_queue(pr)):
        return False, "failed in the merge queue"
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
                  "--json", "number,isDraft,mergeStateStatus,createdAt,body,labels,updatedAt")
    # Drafts are someone's hold, except a stacked draft, which waits only for
    # its parent; rebase_pr skips it while the parent is unmerged.
    dirty = []
    for p in prs:
        if p["mergeStateStatus"] != "DIRTY" or (p["isDraft"] and not STACKED.search(p.get("body") or "")):
            continue
        if worker_active(p):
            print(f"#{p['number']} skipped: worker active")
            continue
        dirty.append(p)
    dirty.sort(key=lambda p: p["createdAt"])  # oldest first: FIFO fairness
    return [p["number"] for p in dirty[:limit]]


REQUEUE_FIELDS = ("number,headRefOid,state,isDraft,autoMergeRequest,body,labels,"
                  "mergeStateStatus,reviewDecision,updatedAt")
BAD_CONCLUSIONS = {"failure", "timed_out", "startup_failure", "action_required", "stale"}
SETUP_STEP = re.compile(r"^(?:Run actions/checkout@|(?:Post )?Set[ -]?up[ -]+(?:soldr|job|runner)\b|"
                        r"(?:Run )?[^ ]*/setup-soldr@|Restore\b.*cache|Cache\b.*restore)", re.I)
RUNNER_LOST = re.compile(r"runner (?:has )?lost communication|runner.*(?:disconnected|shutdown)|"
                         r"runner.*stopped communicating", re.I)
NETWORK_ERROR = re.compile(r"could not resolve host|temporary failure in name resolution|"
                           r"failed to connect to|connection reset by peer|network is unreachable|"
                           r"(?:HTTP|status code)[: /]*(?:502|503|504)\b", re.I)
CODE_ERROR = re.compile(r"assertion.*failed|test result: FAILED|\bFAIL(?:ED|URE)[: ]+.*test|"
                        r"error\[E\d+\]|error C\d+|undefined reference|undefined symbol|"
                        r":\d+(?::\d+)?: (?:fatal )?error:|"
                        r"fatal error:.*(?:No such file|file not found|cannot open)", re.I)


def timestamp(value: str) -> dt.datetime:
    return dt.datetime.fromisoformat(value.replace("Z", "+00:00"))


def requeue_api_items(endpoint: str, key: str | None = None) -> list[dict]:
    """Public Actions reads and PR comments; never silently truncate a page."""
    pages = gh_json("api", "--paginate", "--slurp", endpoint)
    items = [item for page in pages for item in (page[key] if key else page)]
    if key and pages and pages[0].get("total_count", len(items)) > len(items):
        raise RuntimeError("incomplete Actions evidence inventory")
    return items


@functools.lru_cache(maxsize=256)
def run_jobs(run_id: int) -> list[dict]:
    return requeue_api_items(f"repos/{REPO}/actions/runs/{run_id}/jobs?filter=latest&per_page=100", "jobs")


def passed_elsewhere(workflow: dict, job: dict) -> bool:
    """A control is the same workflow and matrix job, on a different runner, ±1h."""
    if "self-hosted" not in job.get("labels", []) or not job.get("runner_name"):
        return False
    start = timestamp(job["started_at"])
    # Run creation can precede job start by hours of runner queueing. Search
    # a full day of this workflow; qualify controls by their actual job times.
    lo, hi = start - dt.timedelta(hours=24), start + dt.timedelta(hours=1)
    window = urllib.parse.quote(f"{lo.isoformat()}..{hi.isoformat()}")
    runs = requeue_api_items(f"repos/{REPO}/actions/workflows/{workflow['workflow_id']}/runs"
                            f"?event=merge_group&created={window}&per_page=100",
                            "workflow_runs")
    for other in runs:
        if other["id"] == workflow["id"] or other["workflow_id"] != workflow["workflow_id"]:
            continue
        for control in run_jobs(other["id"]):
            if (control["name"] == job["name"] and control["conclusion"] == "success"
                    and control.get("runner_name") and control["runner_name"] != job["runner_name"]
                    and control.get("started_at")
                    and abs(timestamp(control["started_at"]) - start) <= dt.timedelta(hours=1)):
                return True
    return False


def failed_job_kind(workflow: dict, job: dict) -> str:
    failed = [s["name"] for s in job.get("steps", []) if s.get("conclusion") in BAD_CONCLUSIONS]
    if failed and all(SETUP_STEP.search(name) for name in failed):
        return "infra"
    if passed_elsewhere(workflow, job):
        return "infra"
    try:
        log = run("gh", "run", "view", str(workflow["id"]), "--repo", REPO,
                  "--job", str(job["id"]), "--log-failed")
    except RuntimeError:
        # A lost runner may never upload logs. Check-run annotations carry
        # GitHub's runner-loss diagnostic even for a job with no steps.
        check_url = job.get("check_run_url")
        if not check_url or not check_url.startswith(f"https://api.github.com/repos/{REPO}/check-runs/"):
            return "unknown"
        annotations = requeue_api_items(check_url + "/annotations?per_page=100")
        log = "\n".join(a.get("message") or "" for a in annotations)
    if len(log) > 2_000_000:
        return "unknown"
    # Explicit compiler/assertion failures take precedence over network text in
    # a test name or diagnostic. A matching healthy runner is independent proof.
    if CODE_ERROR.search(log):
        return "code"
    if RUNNER_LOST.search(log) or NETWORK_ERROR.search(log):
        return "infra"
    if failed and any(re.search(r"\btest|\bbuild|\bcompile|\blint|\blink", name, re.I) for name in failed):
        return "code"
    return "unknown"


def queue_failure_kind(event: dict) -> str:
    sha = (event.get("beforeCommit") or {}).get("oid")
    if not sha:
        return "unknown"
    runs = requeue_api_items(f"repos/{REPO}/actions/runs?event=merge_group&head_sha={sha}&per_page=100",
                            "workflow_runs")
    kinds = []
    for workflow in runs:
        if workflow.get("event") != "merge_group" or workflow.get("head_sha") != sha:
            continue
        # A cancelled workflow can contain a real failure that caused the
        # cancellation. Inspect every failed job before treating it as infra.
        jobs = run_jobs(workflow["id"])
        # Runnerless cancellations belong exclusively to mq_acquire_retry's
        # exact annotation + CI Gate classifier. A generic "cancelled = infra"
        # fallback must not bypass its refusal or per-head recovery budget.
        if any(j.get("conclusion") == "cancelled" and not j.get("runner_name")
               and not j.get("steps") for j in jobs):
            kinds.append("unknown")
            continue
        failed = [j for j in jobs if j.get("conclusion") in BAD_CONCLUSIONS]
        substantive = [j for j in failed if j["name"].split(" / ")[-1] != "CI Gate ✅"]
        kinds.extend(failed_job_kind(workflow, j) for j in substantive)
        cancelled = any(j.get("conclusion") == "cancelled" for j in jobs) or workflow.get("conclusion") == "cancelled"
        if failed and not substantive and not cancelled:
            kinds.append("unknown")  # a lone gate failure is not proven infra
        if cancelled:
            kinds.append("infra")
        elif workflow.get("conclusion") in BAD_CONCLUSIONS and not failed:
            kinds.append("unknown")
        if workflow.get("conclusion") is None:
            kinds.append("unknown")
    return "code" if "code" in kinds else "unknown" if not kinds or "unknown" in kinds else "infra"


def requeue_marker(head: str) -> str:
    return f"<!-- ai-rebase:requeue-ejected:{head} -->"


def requeue_comments(number: int) -> list[dict]:
    return requeue_api_items(f"repos/{REPO}/issues/{number}/comments?per_page=100")


def requeue_hold(pr: dict, now: dt.datetime) -> bool:
    labels = [l["name"] for l in pr.get("labels") or []]
    return (pr["state"] != "OPEN" or pr.get("isDraft") or pr.get("autoMergeRequest") is not None
            or pr.get("mergeStateStatus") in ("DIRTY", "UNKNOWN")
            or bool({"queue-failed", "draft"}.intersection(labels)) or worker_active(pr, now=now)
            or live_holds(pr.get("body") or "", None) or bool(HOLD.search(" ".join(labels)))
            or pr.get("reviewDecision") == "CHANGES_REQUESTED")


def retry_ejection(pr: dict, now: dt.datetime) -> dict | None:
    """Bind a recent, latest removal to the same PR head, never the queue SHA."""
    events = iter(queue_events(pr))
    event = next(events, None)
    if not event or event["__typename"] != "RemovedFromMergeQueueEvent":
        return None
    if event.get("reason") not in ("failed_checks", "checks_timed_out"):
        return None
    age = now - timestamp(event["createdAt"])
    if not dt.timedelta() <= age <= dt.timedelta(hours=3):
        return None
    head = next(((p.get("commit") or p.get("afterCommit") or {}).get("oid") for p in events
                 if p["__typename"] in ("PullRequestCommit", "HeadRefForcePushedEvent")), None)
    return event if head == pr["headRefOid"] else None


def requeue_pr(pr: dict, dry_run: bool, *, now: dt.datetime | None = None) -> str:
    now = now or dt.datetime.now(dt.timezone.utc)
    if requeue_hold(pr, now):
        return "skipped: held, active, armed or conflicting"
    event = retry_ejection(pr, now)
    if event is None:
        return "skipped: no recent latest ejection on the unchanged head"
    head = pr["headRefOid"]
    marker = requeue_marker(head)
    kind = "infra" if event["reason"] == "checks_timed_out" else queue_failure_kind(event)
    if kind == "unknown":
        return "skipped: incomplete or unrecognized merge-group evidence"
    if kind == "infra" and any(marker in (c.get("body") or "") for c in requeue_comments(pr["number"])):
        return "skipped: automatic retry already spent for this head"
    if dry_run:
        return f"dry-run: {'re-arm' if kind == 'infra' else 'label queue-failed'} {head}"
    fresh = gh_json("pr", "view", str(pr["number"]), "--repo", REPO, "--json", REQUEUE_FIELDS)
    if fresh["headRefOid"] != head or requeue_hold(fresh, now):
        return "skipped: PR changed during classification"
    if next(iter(queue_events(fresh)), None) != event:
        return "skipped: queue transition changed during classification"
    if kind == "code":
        run("gh", "pr", "edit", str(pr["number"]), "--repo", REPO, "--add-label", "queue-failed")
        return "left out: real test/build failure; labelled queue-failed"
    if any(marker in (c.get("body") or "") for c in requeue_comments(pr["number"])):
        return "skipped: automatic retry already spent for this head"
    # Reserve the attempt BEFORE arming. If the API fails, prefer consuming an
    # attempt to an endless retry loop. Workflow concurrency serializes runs.
    run("gh", "pr", "comment", str(pr["number"]), "--repo", REPO, "--body",
        f"{marker}\nai-rebase: one automatic infrastructure retry for `{head}` "
        f"after {event['reason']} at {event['createdAt']}.")
    run("gh", "pr", "merge", str(pr["number"]), "--repo", REPO, "--auto", "--match-head-commit", head)
    return f"re-armed: infrastructure ejection on {head}"


def requeue_ejected(prs: list[int], dry_run: bool, limit: int) -> int:
    candidates = [gh_json("pr", "view", str(n), "--repo", REPO, "--json", REQUEUE_FIELDS) for n in prs] if prs else gh_json(
        "pr", "list", "--repo", REPO, "--state", "open", "--limit", "1000", "--json", REQUEUE_FIELDS)
    if len(candidates) >= 1000:
        raise RuntimeError("incomplete open PR inventory")
    mutations, rc = 0, 0
    for pr in candidates:
        if mutations >= limit:
            break
        try:
            out = requeue_pr(pr, dry_run)
            mutations += not out.startswith("skipped")
        except Exception as e:
            out, rc = f"failed: {e}", 1
            mutations += 1  # a partially completed attempt also spends headroom
        print(f"#{pr['number']} {out}")
    return rc


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("prs", nargs="*", type=int)
    ap.add_argument("--dirty", action="store_true")
    ap.add_argument("--requeue-ejected", action="store_true", help="retry infrastructure ejections without rebasing")
    ap.add_argument("--limit", type=int, default=10, help="maximum requeue actions per run")
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--comment", action="store_true", help="post the outcome on the PR")
    a = ap.parse_args()
    if a.requeue_ejected:
        if a.dirty or a.limit < 1:
            ap.error("--requeue-ejected needs a positive --limit and cannot be combined with --dirty")
        return requeue_ejected(a.prs, a.dry_run, a.limit)
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
        rc |= 0 if out.startswith(("pushed", "dry-run", "skipped")) and "failed to arm" not in out else 1
        run("git", "checkout", "-q", "--detach", start, check=False)
    return rc


if __name__ == "__main__":
    sys.exit(main())
