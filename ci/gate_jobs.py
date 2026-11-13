#!/usr/bin/env -S uv run --script
# pyright: basic, reportImplicitRelativeImport=false
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
from pathlib import Path

from ci_scope import Scope, queue_changed_files, read_text, repo_cmocka_source_paths, scope

UNSUCCESSFUL = {
    "failure",
    "cancelled",
    "timed_out",
    "startup_failure",
    "action_required",
}
# Maintenance outside the gate (ci/test_merge_queue.py), and the gate itself.
EXEMPT_PREFIXES = ("CI Pre 🧹", "Cache Janitor 🧹", "CI Gate ✅")
# The jobs listing is one API call; three attempts cover a 5xx blip or a
# momentary rate-limit without holding the gate's 30 s budget.
ATTEMPTS = 3
BACKOFF_SECONDS = 2.0
SCOPE_JOB = "Build 🧱 / Event Data 🔎"


class JobListError(RuntimeError):
    """The run's job list could not be read: every attempt failed."""


def runnerless(job: dict) -> bool:
    """Explicit runner-capacity evidence, rather than missing API fields."""
    return job.get("runner_name") == "" and job.get("runner_id") == 0 and not job.get("steps")


def blocking(
    jobs: list[dict], *, require_scope: bool = False, expected: set[str] | None = None
) -> list[str]:
    """Names of the finished jobs that should fail the gate."""
    bad = [
        j["name"]
        for j in jobs
        if j.get("status") == "completed"
        and j.get("conclusion") in UNSUCCESSFUL
        and not j["name"].startswith(EXEMPT_PREFIXES)
    ]
    required = set(expected or ())
    if require_scope:
        required.add(SCOPE_JOB)
    # A successful parent/reusable call is not proof that its matrix children ran.
    passed = {
        j["name"] for j in jobs
        if j.get("status") == "completed" and j.get("conclusion") == "success"
        and not (j["name"] in required and runnerless(j))
    }
    bad.extend(name for name in sorted(required - passed) if name not in bad)
    return bad


def jobs_argv() -> list[str]:
    """The `gh api` call that lists every job of this run attempt."""
    path = (
        f"repos/{os.environ['GITHUB_REPOSITORY']}/actions/runs/{os.environ['GITHUB_RUN_ID']}"
        f"/attempts/{os.environ.get('GITHUB_RUN_ATTEMPT', '1')}/jobs?per_page=100"
    )
    return ["gh", "api", "--paginate", path, "--jq", ".jobs[]"]


def run_jobs(
    runner: Callable | None = None,
    sleep: Callable[[float], None] | None = None,
    attempts: int = ATTEMPTS,
) -> list[dict]:
    """Every job of this run attempt, retrying the API call on a failure."""
    argv = jobs_argv()
    run = runner or subprocess.run
    wait = sleep or time.sleep
    stderr = ""
    for attempt in range(1, attempts + 1):
        finished = run(argv, capture_output=True, text=True, check=False)
        if finished.returncode == 0:
            return [
                json.loads(line)
                for line in finished.stdout.splitlines()
                if line.strip()
            ]
        stderr = (finished.stderr or "").strip()
        if attempt < attempts:
            wait(BACKOFF_SECONDS * attempt)
    raise JobListError(
        f"`{' '.join(argv)}` failed {attempts} times; gh said: {stderr or '<no stderr>'}"
    )


def expected_jobs(selected: Scope, files: list[str] | tuple[str, ...] = ()) -> set[str]:
    """The queue execution manifest: exact job names, never proof credit.

    Keep names identical to build-project.yaml, including matrix mode/part names.
    The extra cross lane executes all three lint-only targets in one job; the two
    main targets retain independent runners and test linking.
    """
    expected = {SCOPE_JOB, "Build 🧱 / CI Mode 🛂", "Format 🔍 / checks"}
    if not selected.docs_only:
        expected.add("Build 🧱 / Fast Gate ⚡")
    if selected.windows_link:
        expected.add("Build 🧱 / Windows Link Check 🔗 (x86_64-pc-windows-msvc)")
    if selected.cross_lint:
        expected.update({
            "Build 🧱 / Rust Cross Lint 🦀 (x86_64-pc-windows-msvc)",
            "Build 🧱 / Rust Cross Lint 🦀 (aarch64-apple-darwin)",
            "Build 🧱 / Rust Cross Lint 🦀 (release targets)",
        })
    if selected.cross_targets:
        expected.add(f"Build 🧱 / Cross Compile 🧰 ({selected.cross_targets})")
    if selected.tier2:
        expected.add("Build 🧱 / Rust Tier 2 Linux 🐧 (build)")
        expected.update(f"Build 🧱 / Rust Tier 2 Linux 🐧 (tests {mode}, all)" for mode in ("OFF", "ON"))
    if selected.qt:
        expected.add("Build 🧱 / Qt Frontend Linux 🐧")
    for enabled, runner in ((selected.macos, "macos-26"), (selected.windows, "windows-2025-vs2026")):
        if enabled:
            expected.add(f"Build 🧱 / Rust Tests 🦀 ({runner})")
            expected.add(f"Build 🧱 / Rust Tier 2 ({runner})")
    if any(path.endswith(".swift") for path in files):
        expected.add("Format 🔍 / swift-format")
    return expected


def queue_coverage() -> set[str]:
    """Recompute full scope with the same checkout inputs as Event Data.

    No reusable outputs or attestations are used: nested cancellations can lose
    those outputs. CI Gate checks out the full tree so linked crates, cmocka
    sources and cross/Qt CMake rules cannot disappear in a sparse checkout.
    """
    files = queue_changed_files()
    selected = scope(
        files, lambda path: Path(path).is_file(), read_text,
        cmocka_paths=repo_cmocka_source_paths(Path(".")), event="merge_group",
    )
    return expected_jobs(selected, files)


def main() -> int:
    queue = os.environ.get("GITHUB_EVENT_NAME") == "merge_group"
    try:
        jobs = run_jobs()
    except JobListError as error:
        print(f"::error::CI Gate could not read this run's jobs: {error}")
        return 1
    scope_failed = False
    expected = set()
    if queue:
        try:
            expected = queue_coverage()
            print("Merge-group required execution manifest: " + json.dumps(sorted(expected)))
        except (OSError, ValueError, subprocess.CalledProcessError) as error:
            print(
                f"::error::CI Gate could not resolve merge-group scope from GITHUB_EVENT_PATH and git diff: {error}"
            )
            scope_failed = True
    bad = blocking(jobs, require_scope=queue, expected=expected)
    for name in bad:
        record = next((j for j in jobs if j["name"] == name), {})
        if runnerless(record):
            print(
                f"::error::{name}: runner capacity condition: job did not acquire a runner; validation did not run"
            )
        else:
            print(
                f"::error::{name} did not succeed (conclusion={record.get('conclusion', 'missing')})"
            )
    if not bad and not scope_failed:
        print("Every required job succeeded; only out-of-scope jobs may skip.")
    return 1 if bad or scope_failed else 0


if __name__ == "__main__":
    sys.exit(main())
