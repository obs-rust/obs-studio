#!/usr/bin/env -S uv run --no-project --script
# /// script
# requires-python = ">=3.11"
# ///
"""Retry only proven hosted-runner acquisition cancellations in the live queue.

    uv run --no-project --script ci/mq_acquire_retry.py [--dry-run]

Needs GH_TOKEN (github.token), actions: write, checks: read and contents: read.
Unknown evidence never permits a retry. CI Gate must still pass on the rerun.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
from pathlib import Path

ACQUISITION = "The job was not acquired by Runner of type hosted even after multiple attempts"
CAPACITY = ": runner capacity condition: job did not acquire a runner; validation did not run"
RESULT_STEPS = {"Check Run Jobs ✅", "Check Job Results ✅"}


def command(argv: list[str]):
    result = subprocess.run(argv, check=True, capture_output=True, text=True, timeout=60)
    return json.loads(result.stdout or "null")


def items(endpoint: str, key: str | None = None) -> list[dict]:
    """Read all pages; incomplete inventories cannot authorize a mutation."""
    pages = command(["gh", "api", "--paginate", "--slurp", endpoint])
    values = [item for page in pages for item in (page[key] if key else page)]
    if key and (not pages or any(page["total_count"] != len(values) for page in pages)):
        raise ValueError(f"incomplete inventory: {endpoint}")
    return values


def queue_page(repo: str, cursor: str | None) -> dict:
    owner, name = repo.split("/")
    after = f",after:{json.dumps(cursor)}" if cursor else ""
    query = (f"{{repository(owner:{json.dumps(owner)},name:{json.dumps(name)}){{mergeQueue{{"
             f"entries(first:100{after}){{totalCount nodes{{headCommit{{oid}}}}"
             "pageInfo{hasNextPage endCursor}}}}}")
    result = command(["uv", "run", "--no-project", "--script",
                      str(Path(__file__).with_name("gh_graphql.py")), "-f", f"query={query}"])
    if result.get("errors"):
        raise ValueError("merge queue GraphQL errors")
    return result["data"]["repository"]["mergeQueue"]["entries"]


def queue_heads(repo: str) -> set[str]:
    heads: set[str] = set()
    cursor = None
    seen = set()
    total = None
    count = 0
    while True:
        entries = queue_page(repo, cursor)
        if total is None:
            total = entries["totalCount"]
        if entries["totalCount"] != total:
            raise ValueError("merge queue changed during pagination")
        count += len(entries["nodes"])
        for node in entries["nodes"]:
            sha = (node.get("headCommit") or {}).get("oid")
            # Entries awaiting a build may have no synthetic head yet.
            if sha is None:
                continue
            if not re.fullmatch(r"[0-9a-f]{40}", sha):
                raise ValueError("invalid merge queue head")
            heads.add(sha)
        seen.add(cursor)
        info = entries["pageInfo"]
        if not info["hasNextPage"]:
            if count != total:
                raise ValueError("incomplete merge queue inventory")
            return heads
        cursor = info["endCursor"]
        if not cursor or cursor in seen:
            raise ValueError("incomplete merge queue pagination")


def failures(repo: str, job: dict) -> list[str]:
    # Never send the token to an API URL supplied by a job response.
    expected = f"https://api.github.com/repos/{repo}/check-runs/"
    url = job.get("check_run_url") or ""
    if not url.startswith(expected) or not url[len(expected):].isdigit():
        raise ValueError("missing or unexpected check-run URL")
    annotations = items(f"repos/{repo}/check-runs/{url[len(expected):]}/annotations?per_page=100")
    return [a["message"] for a in annotations if a["annotation_level"] == "failure"]


def classify(repo: str, run: dict) -> str | None:
    """Return a refusal reason, or None for a proven acquisition-only failure."""
    attempt = run.get("run_attempt")
    if type(attempt) is not int or not 1 <= attempt < 3:
        return "attempt limit (maximum 3) or unknown attempt"
    if run.get("status") != "completed":
        return "run still active; defer until all jobs finish"
    if run.get("conclusion") not in ("failure", "cancelled"):
        return "run has no failed/cancelled result"
    # latest includes successful jobs retained from prior attempts. An attempt's
    # own /jobs response can omit them after rerun-failed-jobs.
    jobs = items(f"repos/{repo}/actions/runs/{run['id']}/jobs?filter=latest&per_page=100", "jobs")
    if not jobs or any(j.get("status") != "completed" for j in jobs):
        return "missing or unfinished jobs"
    acquisition = []
    gates = []
    for job in jobs:
        conclusion = job.get("conclusion")
        if conclusion in ("success", "skipped"):
            continue
        if job["name"] == "CI Gate ✅" and conclusion == "failure":
            gates.append(job)
        elif (conclusion == "cancelled" and job.get("runner_name") == ""
              and job.get("runner_id") == 0 and job.get("steps") == []
              and failures(repo, job) == [ACQUISITION]):
            acquisition.append(job["name"])
            print(f"queue run {run['id']}: acquisition cancellation: {job['name']}")
        else:
            return f"unproven failure: {job['name']} ({conclusion})"
    if not acquisition:
        return "no proven acquisition cancellation"
    return aggregate_reason(repo, gates, acquisition)


def aggregate_reason(repo: str, gates: list[dict], acquisition: list[str]) -> str | None:
    """Only the known CI Gate result-checking steps and diagnostics are safe."""
    expected = {name + CAPACITY for name in acquisition}
    for gate in gates:
        steps = gate.get("steps") or []
        failed = [s for s in steps if s.get("conclusion") not in ("success", "skipped")]
        if not failed or any(s.get("conclusion") != "failure" or s["name"] not in RESULT_STEPS for s in failed):
            return "CI Gate failed outside result aggregation"
        messages = set(failures(repo, gate))
        # Generic exit annotations are only safe alongside the full, exact set
        # of capacity errors, with no scope/API/missing-job/test diagnostics.
        if not expected <= messages or messages - expected - {"Process completed with exit code 1."}:
            return "CI Gate has independent or incomplete failure evidence"
    return None


def retry_run(repo: str, head: str, run: dict, *, dry_run: bool) -> None:
    label = f"queue run {run['id']} attempt {run.get('run_attempt')}"
    if run.get("event") != "merge_group" or run.get("head_sha") != head:
        print(f"{label}: ignored: event/head mismatch")
        return
    if run.get("status") not in ("in_progress", "completed"):
        print(f"{label}: deferred: {run.get('status')}")
        return
    reason = classify(repo, run)
    if reason:
        print(f"{label}: skipped: {reason}")
        return
    fresh = command(["gh", "api", f"repos/{repo}/actions/runs/{run['id']}"])
    identity = ("id", "event", "head_sha", "status", "conclusion", "run_attempt")
    if any(fresh.get(k) != run.get(k) for k in identity):
        print(f"{label}: skipped: run changed during inspection")
        return
    if head not in queue_heads(repo):
        print(f"{label}: skipped: head left the merge queue")
        return
    if dry_run:
        print(f"{label}: dry-run: would rerun failed jobs")
    else:
        command(["gh", "api", "-X", "POST", f"repos/{repo}/actions/runs/{run['id']}/rerun-failed-jobs"])
        print(f"{label}: requested rerun of failed jobs")


def maintain(repo: str, *, dry_run: bool = False) -> int:
    """Inspect live merge-group runs and rerun only acquisition failures."""
    rc = 0
    try:
        heads = queue_heads(repo)
        print(f"queue: {len(heads)} current merge-group head(s)")
        for head in sorted(heads):
            runs = items(f"repos/{repo}/actions/runs?event=merge_group&head_sha={head}&per_page=100", "workflow_runs")
            for run in runs:
                try:
                    retry_run(repo, head, run, dry_run=dry_run)
                except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
                    print(f"queue run {run['id']}: skipped: API/evidence error: {error}")
                    rc = 1
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
        print(f"queue: no retry authorized: API/evidence error: {error}")
        return 1
    return rc


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()
    return maintain(os.environ.get("GITHUB_REPOSITORY", "obs-rust/obs-studio"), dry_run=args.dry_run)


if __name__ == "__main__":
    raise SystemExit(main())
