#!/usr/bin/env -S uv run --no-project --script
# /// script
# requires-python = ">=3.11"
# ///
"""Admission gate for new merge-queue work, computed from live GitHub state.

    uv run --no-project --script ci/backpressure.py            # JSON verdict, exit 0 open / 75 closed
    uv run --no-project --script ci/backpressure.py --check    # same, one line

Nothing here is configured. The 2026-11-09 queue collapse (114 open PRs, no
merge for 5 h) came from fixed knobs: every worker, chaperone and pool opened
and enqueued as fast as it could, while the real capacity, which is how fast
the merge queue actually lands PRs, had dropped to zero because hosted runners
stopped picking up jobs. A fixed cap is wrong both ways: too high when runners
starve, too low when they are fast. So the limit is measured, not set:

    capacity  = PRs merged in the last 2 h (the queue's observed throughput)
    limit     = max(1, capacity)            # Little's law with a ~2 h target wait
    in_flight = union of queue entries and non-draft armed PRs, minus parked red

An armed PR is parked only when unqueued, its complete CURRENT HEAD checks
are terminal red, and a complete branch workflow-run read has no active work.
Green, pending, missing or uncertain evidence still counts. Queue membership
always counts, including red PRs and PRs without auto-merge. Unknown API state
or incomplete pagination closes admission; this is not permission to retry,
requeue or bypass any caller's holds, reviews, drafts or admission leases.

The gate is OPEN when in_flight < limit and runners are being picked up, and
CLOSED otherwise. The starvation signal is the oldest queued job in the
current run attempt: after 20 minutes, adding work only deepens the hole.
Queued runs with no jobs cannot consume runners and do not signal starvation.
With no merges at all, the limit is 1: one PR probes the queue, so the system
recovers by itself when throughput returns (additive increase), and drops
back the moment throughput stalls (multiplicative decrease, since the window
empties).

Callers: `./push` warns when closed and refuses with PUSH_RESPECT_BACKPRESSURE=1,
pool managers stop opening PRs and chaperones stop enqueueing while it reads
closed, and ci/ai_rebase.py rebases at most `limit - in_flight` PRs per run.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import subprocess
import sys
from pathlib import Path
from urllib.parse import urlencode

REPO = "obs-rust/obs-studio"
WINDOW_H = 2
STARVED_MIN = 20
EXIT_CLOSED = 75


def gh_json(*args: str):
    command = (["uv", "run", "--no-project", "--script", str(Path(__file__).with_name("gh_graphql.py")),
                *args[1:]] if args[0] == "graphql" else ["gh", *args])
    out = subprocess.run(command, check=True, capture_output=True, text=True).stdout
    return json.loads(out or "null")


def now() -> dt.datetime:
    return dt.datetime.now(dt.timezone.utc)


def parse(ts: str) -> dt.datetime:
    return dt.datetime.fromisoformat(ts.replace("Z", "+00:00"))


def require(ok: bool, message: str) -> None:
    if not ok:
        raise ValueError(message)


def bounded_list(value, cap: int) -> list:
    # gh list hides pagination. At its cap we cannot prove completeness.
    require(isinstance(value, list) and len(value) < cap, "incomplete GitHub list")
    require(all(isinstance(item, dict) for item in value), "malformed GitHub list")
    return value


def repository(repo: str, fields: str) -> dict:
    owner, name = repo.split("/")
    query = f"{{repository(owner:{json.dumps(owner)},name:{json.dumps(name)}){{{fields}}}}}"
    result = gh_json("graphql", "-f", f"query={query}")
    require(isinstance(result, dict) and not result.get("errors"), "GraphQL errors")
    return result["data"]["repository"]


def connection_page(connection: dict, cursor, seen: set) -> tuple[list, str | None]:
    nodes = connection["nodes"]
    total = connection["totalCount"]
    info = connection["pageInfo"]
    require(isinstance(nodes, list) and type(total) is int and total >= 0,
            "malformed connection")
    require(type(info["hasNextPage"]) is bool, "missing pagination state")
    following = info["endCursor"] if info["hasNextPage"] else None
    if info["hasNextPage"]:
        require(bool(nodes) and isinstance(following, str) and following not in seen,
                "incomplete or repeated GitHub page")
        seen.add(following)
    return nodes, following


def queue_entries(repo: str) -> set[int]:
    numbers = set()
    cursor = None
    seen = set()
    expected = None
    while True:
        after = f",after:{json.dumps(cursor)}" if cursor else ""
        queue = repository(repo, f"mergeQueue{{entries(first:100{after}){{"
                           "totalCount nodes{pullRequest{number}} pageInfo{hasNextPage endCursor}}}")
        entries = queue["mergeQueue"]["entries"]
        nodes, cursor = connection_page(entries, cursor, seen)
        if expected is None:
            expected = entries["totalCount"]
        require(expected == entries["totalCount"], "queue changed during pagination")
        for node in nodes:
            number = node["pullRequest"]["number"]
            require(type(number) is int and number > 0 and number not in numbers,
                    "malformed or duplicate queue entry")
            numbers.add(number)
        if cursor is None:
            require(len(numbers) == expected, "incomplete merge queue")
            return numbers


def terminal_red(repo: str, pr: dict) -> bool:
    """Only complete checks attached to this exact head can prove it parked."""
    head = pr.get("headRefOid")
    if not isinstance(head, str) or len(head) != 40:
        return False
    cursor = None
    seen = set()
    contexts = []
    expected = None
    while True:
        after = f",after:{json.dumps(cursor)}" if cursor else ""
        fields = (f"pullRequest(number:{pr['number']}){{headRefOid commits(last:1){{nodes{{commit{{"
                  "oid statusCheckRollup{state contexts(first:100" + after + "){"
                  "totalCount pageInfo{hasNextPage endCursor} nodes{__typename "
                  "... on CheckRun{id status conclusion checkSuite{commit{oid}}} "
                  "... on StatusContext{id state commit{oid}}}}}}}}}")
        current = repository(repo, fields)["pullRequest"]
        commits = current["commits"]["nodes"]
        if current["headRefOid"] != head or len(commits) != 1 or commits[0]["commit"]["oid"] != head:
            return False
        rollup = commits[0]["commit"]["statusCheckRollup"]
        if rollup is None:
            return False
        page = rollup["contexts"]
        nodes, cursor = connection_page(page, cursor, seen)
        if expected is None:
            expected = page["totalCount"]
        require(expected == page["totalCount"], "checks changed during pagination")
        contexts.extend(nodes)
        if rollup["state"] != "FAILURE":
            return False
        if cursor is None:
            require(len(contexts) == expected and len({c["id"] for c in contexts}) == expected,
                    "incomplete or duplicate checks")
            break
    return terminal_red_contexts(contexts, head)


def terminal_red_contexts(contexts: list, head: str) -> bool:
    """Require every context to be terminal at head, with at least one red."""
    red = False
    for context in contexts:
        kind = context.get("__typename")
        if kind == "CheckRun":
            if context.get("checkSuite", {}).get("commit", {}).get("oid") != head:
                return False
            conclusion = context.get("conclusion")
            if context.get("status") != "COMPLETED" or conclusion not in {
                    "SUCCESS", "NEUTRAL", "SKIPPED", "FAILURE", "CANCELLED", "TIMED_OUT",
                    "ACTION_REQUIRED", "STARTUP_FAILURE"}:
                return False
            red |= conclusion in {"FAILURE", "CANCELLED", "TIMED_OUT", "ACTION_REQUIRED", "STARTUP_FAILURE"}
        elif kind == "StatusContext":
            if context.get("commit", {}).get("oid") != head or context.get("state") not in {"SUCCESS", "FAILURE", "ERROR"}:
                return False
            red |= context["state"] in {"FAILURE", "ERROR"}
        else:
            return False
    return red


def rest_items(endpoint: str, key: str) -> list:
    pages = gh_json("api", endpoint, "--paginate", "--slurp")
    require(isinstance(pages, list) and bool(pages), "missing API pages")
    total = pages[0]["total_count"]
    require(type(total) is int and total >= 0, "malformed API total")
    items = []
    for page in pages:
        require(page["total_count"] == total and isinstance(page[key], list),
                "inconsistent API pages")
        items.extend(page[key])
    require(all(isinstance(r, dict) and type(r.get("id")) is int and r["id"] > 0 for r in items),
            "malformed API item identity")
    require(len(items) == total and len({r["id"] for r in items}) == total,
            "incomplete or duplicate API items")
    return items


def active_runs(repo: str, candidates: list) -> list:
    # One status-independent inventory: partitioning by status can miss a run
    # that transitions between queries. A merge-group run cannot predate the
    # PR it includes. Bound by PR creation, not an arbitrary lookback/timeout;
    # branch runs and current-head suites below cover pre-PR head activity.
    since = min(parse(p["createdAt"]) for p in candidates).strftime("%Y-%m-%dT%H:%M:%SZ")
    endpoint = f"repos/{repo}/actions/runs?" + urlencode({"per_page": 100, "created": f">={since}"})
    return rest_items(endpoint, "workflow_runs")


def relevant_work(runs: list, pr: dict) -> bool:
    for run in runs:
        require(isinstance(run, dict) and isinstance(run.get("status"), str), "malformed run")
        if run["status"] == "completed":
            continue
        if run.get("head_sha") == pr["headRefOid"] or run.get("head_branch") == pr["headRefName"]:
            return True
        if run.get("event") == "merge_group":
            return True  # A group can contain more than the PR named in its ref.
        associated = run.get("pull_requests")
        # Missing association (common for merge_group) is uncertainty, not
        # evidence that a synthetic-ref run belongs to somebody else.
        if not isinstance(associated, list) or not associated:
            return True
        require(all(isinstance(p, dict) and type(p.get("number")) is int and p["number"] > 0
                    for p in associated), "malformed run PR association")
        if any(p["number"] == pr["number"] for p in associated):
            return True
    return False


def inactive_suites(repo: str, pr: dict) -> bool:
    suites = rest_items(f"repos/{repo}/commits/{pr['headRefOid']}/check-suites?per_page=100", "check_suites")
    # Suites can be requested before their provider publishes any CheckRun.
    return all(s.get("status") == "completed" and s.get("head_sha") == pr["headRefOid"] for s in suites)


def same_head(repo: str, pr: dict) -> bool:
    current = repository(repo, f"pullRequest(number:{pr['number']}){{headRefOid}}")
    return current["pullRequest"]["headRefOid"] == pr["headRefOid"]


def inactive_runs(repo: str, pr: dict) -> bool:
    # Include all runs on the PR branch, not just head_sha: a retry may not
    # have created its new CheckRun yet, and PR test-merge SHAs can differ.
    branch = pr.get("headRefName")
    if not isinstance(branch, str) or not branch:
        return False
    endpoint = f"repos/{repo}/actions/runs?" + urlencode({"branch": branch, "per_page": 100})
    runs = rest_items(endpoint, "workflow_runs")
    return all(r.get("status") == "completed" for r in runs)


def queued_job_start(jobs: list, attempt_start: dt.datetime) -> dt.datetime | None:
    """A rerun's original job timestamps cannot predate its current attempt."""
    require(all(isinstance(j.get("status"), str) for j in jobs), "malformed job status")
    return min((max(parse(j["created_at"]), attempt_start) for j in jobs if j["status"] == "queued"),
               default=None)


def queued_starvation(repo: str, runs: list) -> tuple[int, list[int]]:
    """Only inspect jobs when the attempt is old enough to starve a runner."""
    clock = now()
    starts = []
    ignored = []
    for run in runs:
        run_id = run["databaseId"]
        require(type(run_id) is int and run_id > 0, "malformed queued run identity")
        attempt_start = parse(run.get("startedAt") or run["createdAt"])
        if clock - attempt_start < dt.timedelta(minutes=STARVED_MIN):
            starts.append(attempt_start)  # An upper bound below the threshold; no REST call needed.
            continue
        jobs = rest_items(f"repos/{repo}/actions/runs/{run_id}/jobs?filter=latest&per_page=100", "jobs")
        if not jobs:
            ignored.append(run_id)
        start = queued_job_start(jobs, attempt_start)
        if start is not None:
            starts.append(start)
    oldest = min(starts, default=clock)
    return max(0, int((clock - oldest).total_seconds() // 60)), ignored


def measure(repo: str = REPO) -> dict:
    since = (now() - dt.timedelta(hours=WINDOW_H)).strftime("%Y-%m-%dT%H:%M:%SZ")
    merged = gh_json("pr", "list", "--repo", repo, "--state", "merged", "--search",
                     f"merged:>={since}", "--limit", "200", "--json", "number")
    open_prs = gh_json("pr", "list", "--repo", repo, "--state", "open", "--limit", "300",
                       "--json", "number,isDraft,autoMergeRequest,mergeStateStatus,headRefOid,headRefName,createdAt")
    queued_runs = gh_json("run", "list", "--repo", repo, "--status", "queued", "--limit", "200",
                          "--json", "databaseId,createdAt,startedAt,event")
    merged = bounded_list(merged, 200)
    open_prs = bounded_list(open_prs, 300)
    queued_runs = bounded_list(queued_runs, 200)
    for items in (merged, open_prs):
        require(all(type(p["number"]) is int and p["number"] > 0 for p in items)
                and len({p["number"] for p in items}) == len(items), "malformed or duplicate PRs")
    for pr in open_prs:
        require(type(pr["number"]) is int and type(pr["isDraft"]) is bool
                and "autoMergeRequest" in pr, "malformed open PR")
    queued = queue_entries(repo)
    armed = {p["number"] for p in open_prs if not p["isDraft"] and p["autoMergeRequest"]}
    parked = set()
    candidates = [p for p in open_prs if p["number"] in armed - queued and terminal_red(repo, p)]
    runs = active_runs(repo, candidates) if candidates else []
    for pr in candidates:
        if (not relevant_work(runs, pr) and inactive_runs(repo, pr)
                and inactive_suites(repo, pr) and same_head(repo, pr)):
            parked.add(pr["number"])
    # A queue entry always wins, including a PR queued while we read checks.
    queued |= queue_entries(repo)
    parked -= queued
    counted = queued | (armed - parked)
    starved_min, ignored = queued_starvation(repo, queued_runs)
    capacity = len(merged)
    limit = max(1, capacity)
    in_flight = len(counted)
    starved = starved_min >= STARVED_MIN
    reasons = []
    if in_flight >= limit:
        reasons.append(f"{in_flight} PRs in flight >= {limit} merged in the last {WINDOW_H} h")
    if starved:
        reasons.append(f"oldest queued job has waited {starved_min} min for a runner")
    return {
        "state": "closed" if reasons else "open",
        "reasons": reasons,
        "limit": limit,
        "in_flight": in_flight,
        "counted": in_flight,
        "parked_red": len(parked),
        "headroom": max(0, limit - in_flight) if not starved else 0,
        "merged_window": capacity,
        "queued_runs": len(queued_runs),
        "oldest_queued_min": starved_min,
        "ignored_queued_run_ids": ignored,
        "open_prs": len(open_prs),
    }


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--repo", default=REPO)
    ap.add_argument("--check", action="store_true", help="print one line instead of JSON")
    ap.add_argument("--verbose", "-v", action="store_true", help="log ignored job-less queued runs")
    a = ap.parse_args()
    try:
        v = measure(a.repo)
    except (subprocess.CalledProcessError, OSError, ValueError, KeyError, TypeError, IndexError, AttributeError) as e:
        # GitHub could not be read: an unknown queue admits nothing new, and
        # callers get a verdict instead of a traceback.
        err = (getattr(e, "stderr", "") or str(e)).strip().splitlines()[-1:] or [""]
        v = {"state": "closed", "reasons": [f"GitHub state unavailable: {err[0][:200]}"],
             "limit": 0, "in_flight": 0, "headroom": 0}
    if a.verbose and v.get("ignored_queued_run_ids"):
        print(f"ignored queued runs with zero jobs: {v['ignored_queued_run_ids']}", file=sys.stderr)
    if a.check:
        why = "; ".join(v["reasons"]) or f"headroom {v['headroom']}"
        counts = (f"counted={v['counted']} parked_red={v['parked_red']}"
                  if "counted" in v else "counted=unknown parked_red=unknown")
        print(f"backpressure {v['state']}: {counts}; {why}")
    else:
        print(json.dumps(v, indent=2))
    return EXIT_CLOSED if v["state"] == "closed" else 0


if __name__ == "__main__":
    sys.exit(main())
