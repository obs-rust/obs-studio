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
    in_flight = PRs in the merge queue + open non-draft PRs with auto-merge armed

The gate is OPEN when in_flight < limit and runners are being picked up, and
CLOSED otherwise. The starvation signal is the oldest `queued` workflow run:
if nothing has started for 20 minutes, adding work only deepens the hole.
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

REPO = "obs-rust/obs-studio"
WINDOW_H = 2
STARVED_MIN = 20
EXIT_CLOSED = 75


def gh_json(*args: str):
    out = subprocess.run(["gh", *args], check=True, capture_output=True, text=True).stdout
    return json.loads(out or "null")


def now() -> dt.datetime:
    return dt.datetime.now(dt.timezone.utc)


def parse(ts: str) -> dt.datetime:
    return dt.datetime.fromisoformat(ts.replace("Z", "+00:00"))


def measure(repo: str = REPO) -> dict:
    since = (now() - dt.timedelta(hours=WINDOW_H)).strftime("%Y-%m-%dT%H:%M:%SZ")
    merged = gh_json("pr", "list", "--repo", repo, "--state", "merged", "--search",
                     f"merged:>={since}", "--limit", "200", "--json", "number")
    open_prs = gh_json("pr", "list", "--repo", repo, "--state", "open", "--limit", "300",
                       "--json", "number,isDraft,autoMergeRequest,mergeStateStatus")
    queued_runs = gh_json("run", "list", "--repo", repo, "--status", "queued", "--limit", "200",
                          "--json", "databaseId,createdAt,event")
    armed = [p["number"] for p in open_prs if not p["isDraft"] and p.get("autoMergeRequest")]
    oldest = min((parse(r["createdAt"]) for r in queued_runs), default=None)
    starved_min = round((now() - oldest).total_seconds() / 60) if oldest else 0
    capacity = len(merged)
    limit = max(1, capacity)
    in_flight = len(armed)
    starved = starved_min >= STARVED_MIN
    reasons = []
    if in_flight >= limit:
        reasons.append(f"{in_flight} PRs in flight >= {limit} merged in the last {WINDOW_H} h")
    if starved:
        reasons.append(f"oldest queued run has waited {starved_min} min for a runner")
    return {
        "state": "closed" if reasons else "open",
        "reasons": reasons,
        "limit": limit,
        "in_flight": in_flight,
        "headroom": max(0, limit - in_flight) if not starved else 0,
        "merged_window": capacity,
        "queued_runs": len(queued_runs),
        "oldest_queued_min": starved_min,
        "open_prs": len(open_prs),
    }


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--repo", default=REPO)
    ap.add_argument("--check", action="store_true", help="print one line instead of JSON")
    a = ap.parse_args()
    try:
        v = measure(a.repo)
    except (subprocess.CalledProcessError, json.JSONDecodeError) as e:
        # GitHub could not be read: an unknown queue admits nothing new, and
        # callers get a verdict instead of a traceback.
        err = (getattr(e, "stderr", "") or str(e)).strip().splitlines()[-1:] or [""]
        v = {"state": "closed", "reasons": [f"GitHub state unavailable: {err[0][:200]}"],
             "limit": 0, "in_flight": 0, "headroom": 0}
    if a.check:
        why = "; ".join(v["reasons"]) or f"headroom {v['headroom']}"
        print(f"backpressure {v['state']}: {why}")
    else:
        print(json.dumps(v, indent=2))
    return EXIT_CLOSED if v["state"] == "closed" else 0


if __name__ == "__main__":
    sys.exit(main())
