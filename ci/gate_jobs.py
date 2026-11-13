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
import tempfile
import re
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
    if any(path.endswith(".json") and path.startswith((
        "plugins/win-capture/data/", "plugins/rtmp-services/data/"
    )) for path in files):
        expected.add("Validate Data 🕵️")
    return expected


def pr_expected_jobs(selected: Scope, files: list[str], plan: dict) -> set[str]:
    """Require real selected PR work unless the base verifier proves its credit.

    This consumes only independently recomputed BASE output, never caller/head
    workflow outputs. Old Local-Gate/verifier output is compatible via real CI.
    """
    from dataclasses import replace

    selected = replace(selected, cross_lint=not selected.docs_only)
    expected = expected_jobs(selected, files)
    data = any(path.endswith(".json") and path.startswith((
        "plugins/win-capture/data/", "plugins/rtmp-services/data/"
    )) for path in files)
    if data:
        expected.add("Validate Data 🕵️")
    valid = (plan.get("admission_version") == 1 and plan.get("queue_policy") == "heavy-v1"
             and isinstance(plan.get("gates"), dict) and isinstance(plan.get("jobs"), dict))
    if not valid:
        return expected
    names = {
        "check-format.yaml:checks": {"Format 🔍 / checks"},
        "check-format.yaml:swift-format": {"Format 🔍 / swift-format"},
        "ci.yml:validate-data": {"Validate Data 🕵️"},
        "build-project.yaml:rust-tests-linux": {"Build 🧱 / Fast Gate ⚡"},
        "build-project.yaml:rust-cross-lint": {
            "Build 🧱 / Rust Cross Lint 🦀 (x86_64-pc-windows-msvc)",
            "Build 🧱 / Rust Cross Lint 🦀 (aarch64-apple-darwin)",
            "Build 🧱 / Rust Cross Lint 🦀 (release targets)",
        },
    }
    heavy = expected - {SCOPE_JOB, "Build 🧱 / CI Mode 🛂"} - set().union(*names.values())
    gates = plan["gates"]
    complete = (plan.get("trusted") is True and plan.get("fast_path") is True
                and bool(plan.get("required_client_gates"))
                and all(gates.get(gate) == "valid" for gate in plan["required_client_gates"]))
    if complete:
        expected -= heavy
    for key, job_names in names.items():
        status = plan["jobs"].get(key, {})
        needed = status.get("required_gates", [])
        if (status.get("skip") is True and plan.get("trusted") is True and needed
                and all(gates.get(gate) == "valid" for gate in needed)):
            # Fast Gate owns native touched-target compilation as well as tests.
            if key not in ("build-project.yaml:rust-tests-linux", "build-project.yaml:rust-cross-lint") or complete:
                expected -= job_names
    return expected


def pr_coverage() -> set[str]:
    """Re-evaluate the immutable base verifier; API/errors grant no skip credit."""
    event = json.loads(Path(os.environ["GITHUB_EVENT_PATH"]).read_text())
    pr = event["pull_request"]
    base, head, number = pr["base"]["sha"], pr["head"]["sha"], event["number"]
    if (not isinstance(base, str) or not isinstance(head, str)
            or not re.fullmatch(r"[0-9a-f]{40}", base)
            or not re.fullmatch(r"[0-9a-f]{40}", head)
            or type(number) is not int or number <= 0):
        raise ValueError("pull_request requires immutable base/head SHAs and number")
    files = subprocess.run(
        ["git", "diff", "--no-ext-diff", "--no-renames", "--name-only", "-z", base, head, "--"],
        check=True, capture_output=True, text=True,
    ).stdout.strip("\0").split("\0")
    files = [path for path in files if path]
    selected = scope(files, lambda path: Path(path).is_file(), read_text,
                     cmocka_paths=repo_cmocka_source_paths(Path(".")), event="pull_request")
    plan = {}
    try:
        source = subprocess.run(["git", "show", f"{base}:ci/pr_attestation.py"],
                                check=True, capture_output=True, text=True).stdout
        with tempfile.TemporaryDirectory(prefix="base-pr-admission-") as temp:
            script = Path(temp) / "pr_attestation.py"
            script.write_text(source, encoding="utf-8")
            # Do not let the subprocess append conflicting step outputs.
            env = dict(os.environ)
            env.pop("GITHUB_OUTPUT", None)
            result = subprocess.run(
                ["uv", "run", "--no-project", "--script", str(script), "--base", base,
                 "--head", head, "--number", str(number)],
                env=env, check=True, capture_output=True, text=True, timeout=240,
            )
            plan = json.loads(result.stdout)
            if not isinstance(plan, dict):
                raise ValueError("verifier output is not an object")
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"Base verifier unavailable; requiring real PR checks: {error}")
        plan = {}
    return pr_expected_jobs(selected, files, plan)


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
    pr = os.environ.get("GITHUB_EVENT_NAME") == "pull_request"
    if queue or pr:
        try:
            expected = queue_coverage() if queue else pr_coverage()
            print("Merge-group required execution manifest: " + json.dumps(sorted(expected)))
        except (OSError, ValueError, KeyError, TypeError, AttributeError, subprocess.SubprocessError) as error:
            print(
                f"::error::CI Gate could not resolve merge-group scope from GITHUB_EVENT_PATH and git diff: {error}"
            )
            scope_failed = True
    bad = blocking(jobs, require_scope=queue or pr, expected=expected)
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
