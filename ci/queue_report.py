#!/usr/bin/env -S uv run --script
"""Report a merge-queue run's wall time and rebuilds in its job summary (#930 E1).

    uv run ci/queue_report.py

Run in CI Gate on merge_group runs (GITHUB_RUN_ID, GITHUB_REPOSITORY and,
for the summary, GITHUB_STEP_SUMMARY set; gh authenticated with a token
that has actions: read). Writes the run's wall time (from its creation),
the runner time its finished jobs took, the slowest jobs, and which queue
attempt this is for the PR: attempt 2 or later means master moved under the
PR (a merge ahead of it failed, or a bypass merged outside the queue) and
it was rebuilt. Informational only: the step never fails CI Gate, and it no
longer needs a workflow `continue-on-error` (WF-003) -- a failing `gh` call is
reported in the summary and exits 0. Standard library only.
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

from cancel_superseded import queue_pr
from cache_metrics import creation_to_start_seconds


def _t(stamp: str) -> datetime:
    return datetime.fromisoformat(stamp.replace("Z", "+00:00"))


def attempt(runs: list[dict], current: dict) -> tuple[int, list[int]]:
    """(attempt number, ids of earlier queue runs) for current's PR."""
    pr = queue_pr(current["head_branch"])
    earlier = sorted(
        (r for r in runs
         if r["id"] != current["id"]
         and queue_pr(r.get("head_branch") or "") == pr
         and r["created_at"] < current["created_at"]),
        key=lambda r: r["created_at"],
    )
    return len(earlier) + 1, [r["id"] for r in earlier]


def _minutes(job: dict) -> float | None:
    if not job.get("started_at") or not job.get("completed_at"):
        return None
    return (_t(job["completed_at"]) - _t(job["started_at"])).total_seconds() / 60


def summary(current: dict, jobs: list[dict], att: tuple[int, list[int]], now: str) -> str:
    wall = (_t(now) - _t(current["created_at"])).total_seconds() / 60
    timed = [(m, j["name"]) for j in jobs
             if j.get("conclusion") != "skipped" and (m := _minutes(j)) is not None]
    n, earlier = att
    lines = [
        f"### Merge queue: PR #{queue_pr(current['head_branch'])}, attempt {n}",
        "",
        f"- Wall time: {wall:.1f} min (run created to CI Gate)",
        f"- Runner time: {sum(m for m, _ in timed):.1f} min across {len(timed)} finished jobs",
    ]
    if n > 1:
        lines.append(f"- rebuilt: master moved under this PR; earlier queue runs: {', '.join(map(str, earlier))}")
    lines += ["", "| Job | min |", "|---|---|"]
    lines += [f"| {name} | {m:.1f} |" for m, name in sorted(timed, reverse=True)[:8]]
    dispatch = [(duration, job["name"]) for job in jobs
                if (duration := creation_to_start_seconds(job)) is not None]
    if dispatch:
        lines += ["", "### Job creation-to-start elapsed time", "",
                  "Elapsed job creation to runner start; this may include dependency waiting "
                  "and cannot isolate runner dispatch latency. Missing timestamps remain unknown.", "",
                  "| Job | seconds |", "|---|---:|"]
        lines += [f"| {name} | {duration:.1f} |"
                  for duration, name in sorted(dispatch, reverse=True)[:8]]
    setup_steps = []
    for job in jobs:
        for step in job.get("steps", []):
            name = step["name"]
            if name.lower().startswith("post") or not any(word in name.lower() for word in ("cache", "set up soldr")):
                continue
            duration = _minutes(step)
            if duration is not None and step.get("conclusion") != "skipped":
                setup_steps.append((duration, job["name"], name))
    if setup_steps:
        lines += ["", "### Setup and cache steps", "",
                  "Named step durations; cache restores do not prove compiler reuse.", "",
                  "| Job | Step | seconds |", "|---|---|---:|"]
        lines += [f"| {job} | {step} | {minutes * 60:.1f} |"
                  for minutes, job, step in sorted(setup_steps, reverse=True)]
    return "\n".join(lines) + "\n"


def gh(*args: str) -> str:
    return subprocess.run(["gh", *args], check=True, capture_output=True, text=True).stdout


def _unavailable(reason: str) -> str:
    """The summary section written when the report cannot be collected."""
    return (
        "### Merge queue: queue report unavailable\n"
        "\n"
        f"- {reason}\n"
        "\n"
        "The report is informational; CI Gate is not affected.\n"
    )


def _write_summary(md: str) -> None:
    if path := os.environ.get("GITHUB_STEP_SUMMARY"):
        with open(path, "a", encoding="utf-8") as f:
            f.write(md)


def main() -> int:
    # WF-003: the workflow step carries no `continue-on-error`, so a failing
    # `gh` call (rate limit, transient 5xx, a token without `actions: read`)
    # must not fail CI Gate. Say why in the summary and exit 0 instead.
    try:
        repo = os.environ["GITHUB_REPOSITORY"]
        run_id = os.environ["GITHUB_RUN_ID"]
        current = json.loads(gh("api", f"repos/{repo}/actions/runs/{run_id}"))
        runs = json.loads(gh("api", f"repos/{repo}/actions/workflows/{current['workflow_id']}/runs?event=merge_group&per_page=100"))["workflow_runs"]
        jobs = json.loads(gh("api", f"repos/{repo}/actions/runs/{run_id}/jobs?per_page=100"))["jobs"]
        now = datetime.now(timezone.utc).isoformat()
        md = summary(current, jobs, attempt(runs, current), now)
    except Exception as exc:  # any failure here is informational only
        reason = str(exc).strip().splitlines()[0] if str(exc).strip() else type(exc).__name__
        detail = getattr(exc, "stderr", "") or ""
        detail = detail.strip().splitlines()[0] if detail.strip() else ""
        md = _unavailable(f"queue report unavailable: {reason}" + (f" ({detail})" if detail else ""))
        print(md, file=sys.stderr)
        _write_summary(md)
        return 0
    print(md)
    _write_summary(md)
    return 0


if __name__ == "__main__":
    sys.exit(main())
