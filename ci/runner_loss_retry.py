#!/usr/bin/env -S uv run --no-project --script
# /// script
# requires-python = ">=3.11"
# ///
"""Rerun a finished Pull run whose failed jobs died only of runner or network loss.

    uv run --no-project --script ci/runner_loss_retry.py --run-id ID [--dry-run]

Started by ai-rebase.yaml's `runner-loss-retry` job on a completed `Pull`
workflow_run, so recovery takes seconds, not the 30 minutes of the cron.
Needs GH_TOKEN with actions: write, checks: read, contents: read; re-arming an
ejected merge group also needs the obs-rust-ai-rebase App token (HAS_TOKEN).

Rules (ci/test_runner_loss_retry.py):
- at most MAX_AUTOMATIC_RERUNS automatic reruns per run (run_attempt 1 and 2
  may be rerun, attempt 3 never);
- every failed job must be proven infrastructure: runner shutdown/lost
  communication or network loss (a failed setup step alone is not proof). One real test or
  compiler failure, or one job with no evidence, refuses the whole run;
- pull_request: `rerun --failed` while the PR head is still the run's head;
- merge_group: `rerun --failed` while the group is still queued at that exact
  SHA, else a fresh queue execution (mq_acquire_retry.recover_group) at once,
  after a short wait for GitHub to record the ejection.
"""

from __future__ import annotations

import argparse
import importlib.util
import os
import re
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
MAX_AUTOMATIC_RERUNS = 2
GATE = "CI Gate ✅"
FAILED = {"failure", "timed_out", "startup_failure"}
EJECTION_POLLS = 4
EJECTION_WAIT = 30


def load(name: str):
    spec = importlib.util.spec_from_file_location(name, HERE / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    sys.modules.setdefault(name, module)
    spec.loader.exec_module(module)
    return module


bot = load("ai_rebase")  # the one classifier: RUNNER_LOST, NETWORK_ERROR, CODE_ERROR
retry = load("mq_acquire_retry")  # API helpers, group membership, fresh-queue re-arm

# GitHub's own wording for a host that went down under a job.
SHUTDOWN = re.compile(r"runner has received a shutdown signal|"
                      r"lost communication with the server|"
                      r"runner.*(?:was|has been) (?:shut ?down|terminated)", re.I)


def job_kind(job: dict, text: str) -> str:
    """infra, code or unknown for one failed job, from its annotations and log."""
    if bot.CODE_ERROR.search(text):
        return "code"  # a real failure wins over network text inside it
    if bot.RUNNER_LOST.search(text) or SHUTDOWN.search(text) or bot.NETWORK_ERROR.search(text):
        return "infra"
    # A failed setup step alone is not proof: a PR that breaks an action pin or
    # setup-soldr inputs fails there too. Only runner/network text counts.
    return "unknown"


def run_refusal(run: dict, jobs: list[dict]) -> str | None:
    """Why this run may not be retried at all, from the run record alone."""
    attempt = run.get("run_attempt")
    if type(attempt) is not int or attempt < 1:
        return "unknown attempt"
    if attempt > MAX_AUTOMATIC_RERUNS:
        return f"automatic reruns spent (attempt {attempt})"
    if run.get("status") != "completed" or run.get("conclusion") != "failure":
        return "run is not a completed failure"
    if run.get("event") not in ("pull_request", "merge_group"):
        return "unrelated event"
    if not jobs or any(j.get("status") != "completed" for j in jobs):
        return "missing or unfinished jobs"
    return None


def decide(run: dict, jobs: list[dict], texts: dict[int, str]) -> tuple[bool, str]:
    """(rerun?, reason). Pure: all evidence is passed in."""
    refusal = run_refusal(run, jobs)
    if refusal:
        return False, refusal
    infra = []
    for job in jobs:
        conclusion = job.get("conclusion")
        # The gate only reports the others; a cancelled job is collateral of a
        # failed sibling and counts only next to a proven infra job below.
        if conclusion in ("success", "skipped", "neutral", "cancelled") or job["name"].split(" / ")[-1] == GATE:
            continue
        if conclusion not in FAILED:
            return False, f"unproven job: {job['name']} ({conclusion})"
        kind = job_kind(job, texts.get(job["id"], ""))
        if kind != "infra":
            return False, f"{kind} failure: {job['name']}"
        infra.append(job["name"])
    if not infra:
        return False, "no job failed of runner or network loss"
    return True, "runner/network loss: " + ", ".join(infra)


def job_text(repo: str, run_id: int, job: dict) -> str:
    """Annotations (a lost runner may upload no log) plus the failed-step log."""
    parts = []
    url = job.get("check_run_url") or ""
    if url.startswith(f"https://api.github.com/repos/{repo}/check-runs/"):
        parts += [a.get("message") or "" for a in retry.items(url + "/annotations?per_page=100")]
    try:
        out = subprocess.run(["gh", "run", "view", str(run_id), "--repo", repo, "--job", str(job["id"]),
                              "--log-failed"], capture_output=True, text=True, timeout=120,
                             env=retry.plain_env())
        if out.returncode == 0 and len(out.stdout) <= 2_000_000:
            parts.append(out.stdout)
    except (OSError, subprocess.SubprocessError):
        pass
    return "\n".join(parts)


def pr_head_matches(repo: str, run: dict) -> bool:
    """A rerun of a run for a superseded head only wastes runners."""
    prs = run.get("pull_requests") or []
    if len(prs) != 1:
        return False
    pr = retry.command(["gh", "api", f"repos/{repo}/pulls/{prs[0]['number']}"])
    return pr["state"] == "open" and pr["head"]["sha"] == run["head_sha"]


def recover_group(repo: str, run: dict, label: str, *, dry_run: bool, can_requeue: bool,
                  sleep=time.sleep) -> str:
    """Rerun the live group, or re-arm the ejected one, as soon as GitHub shows which."""
    match = re.fullmatch(r"gh-readonly-queue/master/pr-([1-9][0-9]*)-[0-9a-f]{40}", run.get("head_branch") or "")
    if not match:
        return "skipped: unknown merge-group branch"
    number = int(match[1])
    for poll in range(EJECTION_POLLS):
        pr = retry.pr_snapshot(repo, number, fresh=poll > 0)
        if retry.live_group(pr, run) or retry.ejection(pr, run):
            retry.recover_group(repo, run, label, dry_run=dry_run, can_requeue=can_requeue)
            return "recovered"
        if retry.held(pr):
            return "skipped: PR is held, conflicting or closed"
        sleep(EJECTION_WAIT)  # the removal event can trail the failed run by seconds
    return "skipped: no live group or latest ejection for this exact group"


def maintain(repo: str, run_id: int, *, dry_run: bool = False, can_requeue: bool = False,
             sleep=time.sleep) -> int:
    label = f"run {run_id}"
    try:
        run = retry.command(["gh", "api", f"repos/{repo}/actions/runs/{run_id}"])
        label = f"run {run_id} attempt {run.get('run_attempt')}"
        workflow = retry.command(["gh", "api", f"repos/{repo}/actions/workflows/ci.yml"])
        if run.get("workflow_id") != workflow["id"] or workflow["path"] != ".github/workflows/ci.yml":
            print(f"{label}: ignored: not the CI workflow")
            return 0
        jobs = retry.items(f"repos/{repo}/actions/runs/{run_id}/jobs?filter=latest&per_page=100", "jobs")
        # Cheap refusals first: only look at logs for a run that could be retried.
        early, reason = decide(run, jobs, {j["id"]: "runner has received a shutdown signal" for j in jobs})
        if not early:
            print(f"{label}: skipped: {reason}")
            return 0
        texts = {j["id"]: job_text(repo, run_id, j) for j in jobs
                 if j.get("conclusion") in FAILED and j["name"].split(" / ")[-1] != GATE}
        ok, reason = decide(run, jobs, texts)
        if not ok:
            print(f"{label}: skipped: {reason}")
            return 0
        print(f"{label}: {reason}")
        if not retry.run_unchanged(repo, run):
            print(f"{label}: skipped: run changed during inspection")
            return 0
        if run["event"] == "pull_request":
            if not pr_head_matches(repo, run):
                print(f"{label}: skipped: PR head moved on, closed, or unknown")
                return 0
            retry.rerun_failed(repo, run, label, dry_run=dry_run)
        else:
            print(f"{label}: {recover_group(repo, run, label, dry_run=dry_run, can_requeue=can_requeue, sleep=sleep)}")
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
        print(f"{label}: no retry authorized: API/evidence error: {error}")
        return 1
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run-id", type=int, required=True)
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()
    return maintain(os.environ.get("GITHUB_REPOSITORY", "obs-rust/obs-studio"), args.run_id,
                    dry_run=args.dry_run, can_requeue=os.environ.get("HAS_TOKEN") == "true")


if __name__ == "__main__":
    raise SystemExit(main())
