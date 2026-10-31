#!/usr/bin/env -S uv run --script
"""Fail CI Gate when any job of this run did not succeed (#1793).

    uv run ci/gate_jobs.py

`needs.*.result` only sees the gate's direct needs. A job inside the
reusable build-project call can end cancelled (for example, "The job was
not acquired by Runner of type hosted even after multiple attempts") while
the call itself reports success, so the gate lists every job of the run
attempt instead. Run in CI Gate with GITHUB_RUN_ID, GITHUB_RUN_ATTEMPT and
GITHUB_REPOSITORY set and gh authenticated (actions: read).

The job list is a network call, so it is retried: a rate-limited or 5xx
response used to fail a run whose every job had passed, with nothing in the
log saying why (#3124). When every attempt fails, gh's own stderr is
printed.
Standard library only.
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import time
from collections.abc import Callable

UNSUCCESSFUL = {"failure", "cancelled", "timed_out", "startup_failure", "action_required"}
# Maintenance outside the gate (ci/test_merge_queue.py), and the gate itself.
EXEMPT_PREFIXES = ("CI Pre 🧹", "Cache Janitor 🧹", "CI Gate ✅")
# The jobs listing is one API call; three attempts cover a 5xx blip or a
# momentary rate-limit without holding the gate's 30 s budget.
ATTEMPTS = 3
BACKOFF_SECONDS = 2.0


class JobListError(RuntimeError):
    """The run's job list could not be read: every attempt failed."""


def blocking(jobs: list[dict]) -> list[str]:
    """Names of the finished jobs that should fail the gate."""
    return [
        j["name"] for j in jobs
        if j.get("status") == "completed"
        and j.get("conclusion") in UNSUCCESSFUL
        and not j["name"].startswith(EXEMPT_PREFIXES)
    ]


def jobs_argv() -> list[str]:
    """The `gh api` call that lists every job of this run attempt."""
    path = (f"repos/{os.environ['GITHUB_REPOSITORY']}/actions/runs/{os.environ['GITHUB_RUN_ID']}"
            f"/attempts/{os.environ.get('GITHUB_RUN_ATTEMPT', '1')}/jobs?per_page=100")
    return ["gh", "api", "--paginate", path, "--jq", ".jobs[]"]


def run_jobs(runner: Callable | None = None, sleep: Callable[[float], None] | None = None,
             attempts: int = ATTEMPTS) -> list[dict]:
    """Every job of this run attempt, retrying the API call on a failure."""
    argv = jobs_argv()
    run = runner or subprocess.run
    wait = sleep or time.sleep
    stderr = ""
    for attempt in range(1, attempts + 1):
        finished = run(argv, capture_output=True, text=True, check=False)
        if finished.returncode == 0:
            return [json.loads(line) for line in finished.stdout.splitlines() if line.strip()]
        stderr = (finished.stderr or "").strip()
        if attempt < attempts:
            wait(BACKOFF_SECONDS * attempt)
    raise JobListError(f"`{' '.join(argv)}` failed {attempts} times; gh said: {stderr or '<no stderr>'}")


def main() -> int:
    try:
        jobs = run_jobs()
    except JobListError as error:
        print(f"::error::CI Gate could not read this run's jobs: {error}")
        return 1
    bad = blocking(jobs)
    for name in bad:
        print(f"::error::{name} did not succeed")
    if not bad:
        print("Every finished job of this run succeeded or was skipped.")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
