#!/usr/bin/env -S uv run --no-project --script
# /// script
# requires-python = ">=3.11"
# ///
"""Recover proven hosted-runner acquisition failures on PRs and ejected groups.

    uv run --no-project --script ci/mq_acquire_retry.py [--run-id ID] [--dry-run]

Needs GH_TOKEN, actions: write, checks: read and contents: read. Queue recovery
also needs pull-requests: write and a workflow-triggering token (AI_REBASE_TOKEN).
Unknown evidence never permits a retry. CI Gate must still pass on the rerun.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import re
import subprocess
import uuid
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
    if type(attempt) is not int or attempt != 1:
        return "automatic retry already spent or unknown attempt"
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
    if len(gates) != 1:
        return "missing or duplicate CI Gate evidence"
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


def pr_snapshot(repo: str, number: int, *, fresh: bool = False) -> dict:
    """Read current state and a complete relevant timeline from trusted GraphQL."""
    owner, name = repo.split("/")
    query = """query($owner:String!,$name:String!,$number:Int!,$before:String){
      repository(owner:$owner,name:$name){pullRequest(number:$number){
        id number headRefOid state isDraft mergeable mergeStateStatus reviewDecision
        autoMergeRequest{enabledAt} body labels(first:100){nodes{name} pageInfo{hasNextPage}}
        timelineItems(last:100,before:$before,itemTypes:[REMOVED_FROM_MERGE_QUEUE_EVENT,
          ADDED_TO_MERGE_QUEUE_EVENT,AUTO_MERGE_DISABLED_EVENT,AUTO_MERGE_ENABLED_EVENT,
          PULL_REQUEST_COMMIT,HEAD_REF_FORCE_PUSHED_EVENT]){
          pageInfo{hasPreviousPage startCursor} nodes{__typename
            ...on RemovedFromMergeQueueEvent{id createdAt reason beforeCommit{oid}}
            ...on AddedToMergeQueueEvent{createdAt}
            ...on PullRequestCommit{commit{oid}}
            ...on HeadRefForcePushedEvent{afterCommit{oid}}
          }
        }
      }}
    }"""
    if fresh:
        # ghd keys arbitrary-query reads by the complete request body. A unique
        # operation name forces upstream evidence without disrupting the shared
        # daemon; GHD_FRESH_MS is only read when that daemon starts.
        query = query.replace("query(", f"query CapacityRecheck{uuid.uuid4().hex}(", 1)
    events, cursors = [], set()
    cursor = None
    snapshot = None
    while True:
        argv = ["uv", "run", "--no-project", "--script", str(Path(__file__).with_name("gh_graphql.py")),
                "-f", f"query={query}", "-f", f"owner={owner}", "-f", f"name={name}", "-F", f"number={number}"]
        if cursor:
            argv += ["-f", f"before={cursor}"]
        data = command(argv)
        if data.get("errors"):
            raise ValueError("PR timeline GraphQL errors")
        pr = data["data"]["repository"]["pullRequest"]
        timeline = pr.pop("timelineItems")
        labels = pr["labels"]
        if labels["pageInfo"]["hasNextPage"]:
            raise ValueError("incomplete PR labels")
        pr["labels"] = labels["nodes"]
        if snapshot is not None and pr != snapshot:
            raise ValueError("PR changed during timeline pagination")
        snapshot = pr
        events = timeline["nodes"] + events
        page = timeline["pageInfo"]
        if not page["hasPreviousPage"]:
            return dict(pr, events=events)
        cursor = page["startCursor"]
        if not cursor or cursor in cursors:
            raise ValueError("incomplete PR timeline")
        cursors.add(cursor)


def ejection(pr: dict, run: dict) -> dict | None:
    """A latest involuntary removal must bind the tested queue SHA and PR head."""
    labels = {label["name"] for label in pr["labels"]}
    hold = r"do not (enqueue|merge)|don't (enqueue|merge)|\bhold\b|\bwip\b|stacked on #"
    if (pr["state"] != "OPEN" or pr["isDraft"] or pr["mergeable"] != "MERGEABLE"
            or pr["mergeStateStatus"] in ("DIRTY", "UNKNOWN")
            or pr["reviewDecision"] == "CHANGES_REQUESTED"
            or labels.intersection({"queue-failed", "draft", "pool-active"})
            or re.search(hold, pr["body"] + " " + " ".join(labels), re.I)):
        return None
    events = list(reversed(pr["events"]))
    if not events or events[0]["__typename"] != "RemovedFromMergeQueueEvent":
        return None  # a manual disarm/requeue/push since removal wins
    removed = events[0]
    if (removed["reason"] not in ("failed_checks", "checks_timed_out")
            or (removed.get("beforeCommit") or {}).get("oid") != run["head_sha"]):
        return None
    # AddedToMergeQueue proves prior intent to merge; a manual disarm before
    # removal invalidates that intent even if GitHub later emits failed_checks.
    prior = events[1:]
    if not prior or prior[0]["__typename"] != "AddedToMergeQueueEvent":
        return None
    head = next(((event.get("commit") or event.get("afterCommit") or {}).get("oid")
                 for event in prior[1:] if event["__typename"] in ("PullRequestCommit", "HeadRefForcePushedEvent")), None)
    return removed if head == pr["headRefOid"] else None


def retry_marker(head: str) -> str:
    # Share the older infrastructure handler's per-head budget as well.
    return f"<!-- ai-rebase:requeue-ejected:{head} -->"


def recover_run(repo: str, run: dict, workflow_id: int, *, dry_run: bool, can_requeue: bool) -> None:
    label = f"capacity run {run['id']} attempt {run.get('run_attempt')}"
    if run.get("workflow_id") != workflow_id or run.get("event") not in ("pull_request", "merge_group"):
        print(f"{label}: ignored: unrelated workflow/event")
        return
    reason = classify(repo, run)
    if reason:
        print(f"{label}: skipped: {reason}")
        return
    fresh = command(["gh", "api", f"repos/{repo}/actions/runs/{run['id']}"])
    identity = ("id", "event", "workflow_id", "head_sha", "head_branch", "status", "conclusion", "run_attempt")
    if any(fresh.get(k) != run.get(k) for k in identity):
        print(f"{label}: skipped: run changed during inspection")
        return
    if run["event"] == "pull_request":
        if dry_run:
            print(f"{label}: dry-run: would rerun failed jobs")
        else:
            command(["gh", "api", "-X", "POST", f"repos/{repo}/actions/runs/{run['id']}/rerun-failed-jobs"])
            print(f"{label}: requested rerun of failed jobs")
        return
    recover_ejected(repo, run, label, dry_run=dry_run, can_requeue=can_requeue)


def recover_ejected(repo: str, run: dict, label: str, *, dry_run: bool, can_requeue: bool) -> None:
    match = re.fullmatch(r"gh-readonly-queue/master/pr-([1-9][0-9]*)-[0-9a-f]{40}", run.get("head_branch") or "")
    if not match:
        print(f"{label}: skipped: unknown merge-group branch")
        return
    number = int(match[1])
    pr = pr_snapshot(repo, number)
    removed = ejection(pr, run)
    if removed is None:
        print(f"{label}: skipped: no matching latest ejection on an unchanged, armed, mergeable head")
        return
    endpoint = f"repos/{repo}/issues/{number}/comments"
    marker = retry_marker(pr["headRefOid"])
    if any(marker in (c.get("body") or "") for c in items(endpoint + "?per_page=100")):
        print(f"{label}: skipped: automatic retry already spent for this PR head")
        return
    if not can_requeue:
        print(f"::warning::{label}: AI_REBASE_TOKEN is required to trigger fresh merge-group CI; no requeue")
        return
    if pr_snapshot(repo, number, fresh=True) != pr:
        print(f"{label}: skipped: PR changed during inspection")
        return
    if dry_run:
        print(f"{label}: dry-run: would re-arm #{number} at {pr['headRefOid']}")
        return
    # Serialized maintenance reserves the attempt before mutation, so even an
    # ambiguous enqueue API failure cannot create an endless recovery loop.
    body = (f"{marker}\nOne automatic capacity retry for `{pr['headRefOid']}` after "
            f"run {run['id']} attempt {run['run_attempt']} ({removed['id']}). CI Gate must pass on the new group.")
    command(["gh", "api", "-X", "POST", endpoint, "-f", f"body={body}"])
    command(["gh", "pr", "merge", str(number), "--repo", repo, "--auto", "--match-head-commit", pr["headRefOid"]])
    print(f"{label}: re-armed #{number} on the unchanged head")


def maintain(repo: str, *, run_id: int | None = None, dry_run: bool = False, can_requeue: bool = False) -> int:
    """Completed-event fast path, with a three-hour scheduled backstop."""
    rc = 0
    try:
        workflow = command(["gh", "api", f"repos/{repo}/actions/workflows/ci.yml"])
        if workflow["path"] != ".github/workflows/ci.yml":
            raise ValueError("unexpected CI workflow")
        runs = [command(["gh", "api", f"repos/{repo}/actions/runs/{run_id}"])] if run_id is not None else []
        since = (dt.datetime.now(dt.timezone.utc) - dt.timedelta(hours=3)).strftime("%Y-%m-%dT%H:%M:%SZ")
        recent = items(f"repos/{repo}/actions/workflows/{workflow['id']}/runs?status=completed&created=%3E%3D{since}&per_page=100",
                       "workflow_runs")
        runs += [run for run in recent if run["id"] != run_id]
        # Newest first; old failed attempts must never undo a newer transition.
        for run in runs:
            try:
                recover_run(repo, run, workflow["id"], dry_run=dry_run, can_requeue=can_requeue)
            except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
                print(f"capacity run {run['id']}: no retry authorized: API/evidence error: {error}")
                rc = 1
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
        print(f"capacity: no retry authorized: API/evidence error: {error}")
        return 1
    return rc


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--run-id", type=int)
    args = parser.parse_args()
    return maintain(os.environ.get("GITHUB_REPOSITORY", "obs-rust/obs-studio"), run_id=args.run_id,
                    dry_run=args.dry_run, can_requeue=os.environ.get("HAS_TOKEN") == "true")


if __name__ == "__main__":
    raise SystemExit(main())
