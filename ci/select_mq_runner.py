#!/usr/bin/env -S uv run --script
"""Probe opt-in queue runners; every unavailable/unknown state uses hosted CI."""

from __future__ import annotations

import json
import os
import random
import signal
import time
from contextlib import contextmanager
from datetime import datetime
from email.utils import parsedate_to_datetime
from pathlib import Path
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen

MAX_PAGES = 5
API_TIMEOUT = 10
SELECTION_TIMEOUT = 45
MAX_RETRIES = 2
MAX_RETRY_AFTER = 5
MAX_REQUESTS = 2 * MAX_PAGES


class SelectionDeadline(TimeoutError):
    """The shared selection deadline has expired; no scope can admit this run."""


@contextmanager
def wall_deadline():
    """Interrupt even DNS or trickling body reads on the Linux selector job."""
    if not hasattr(signal, "setitimer"):
        # The selector runs on Ubuntu; keep its helpers usable on other hosts.
        yield
        return

    timed_out = False

    def expired(signum, frame):
        nonlocal timed_out
        timed_out = True
        raise SelectionDeadline("selection deadline exhausted")

    previous_handler = signal.getsignal(signal.SIGALRM)
    previous_timer = signal.getitimer(signal.ITIMER_REAL)
    started = time.monotonic()
    signal.signal(signal.SIGALRM, expired)
    try:
        signal.setitimer(signal.ITIMER_REAL, SELECTION_TIMEOUT)
        yield
        if timed_out:
            raise SelectionDeadline("selection deadline exhausted")
    finally:
        signal.setitimer(signal.ITIMER_REAL, 0)
        signal.signal(signal.SIGALRM, previous_handler)
        if previous_timer[0]:
            remaining = max(0.001, previous_timer[0] - (time.monotonic() - started))
            signal.setitimer(signal.ITIMER_REAL, remaining, previous_timer[1])


class Probe:
    """Share the original ten-request budget between census and commitments."""

    def __init__(self, token: str, api_url: str, *, clock=None, sleep=None):
        self.token = token
        self.api_url = api_url.rstrip("/")
        self.remaining = MAX_REQUESTS
        self.clock = clock or time.monotonic
        self.sleep = sleep or time.sleep
        self.deadline = self.clock() + SELECTION_TIMEOUT

    def time_left(self) -> float:
        remaining = self.deadline - self.clock()
        if remaining <= 0:
            raise SelectionDeadline("selection deadline exhausted")
        return remaining

    def retry_delay(self, error: Exception, attempt: int) -> float:
        delay = random.uniform(0.25, 0.75) * 2**attempt
        if isinstance(error, HTTPError) and error.headers:
            value = error.headers.get("Retry-After")
            if value:
                try:
                    delay = float(value)
                except ValueError:
                    try:
                        delay = parsedate_to_datetime(value).timestamp() - time.time()
                    except (ValueError, TypeError, OverflowError):
                        pass
        return min(MAX_RETRY_AFTER, max(0, delay))

    def get(self, path: str, *, token: str | None = None) -> dict:
        request = Request(
            f"{self.api_url}/{path}",
            headers={
                "Authorization": f"Bearer {self.token if token is None else token}",
                "Accept": "application/vnd.github+json",
                "X-GitHub-Api-Version": "2022-11-28",
            },
        )
        for attempt in range(MAX_RETRIES + 1):
            timeout = min(API_TIMEOUT, self.time_left())
            if self.remaining <= 0:
                raise ValueError("request budget exhausted")
            self.remaining -= 1  # Retries consume the same ten-attempt budget.
            try:
                with urlopen(request, timeout=timeout) as response:
                    result = json.load(response)
                self.time_left()
                return result
            except SelectionDeadline:
                raise
            except (HTTPError, URLError, TimeoutError, ConnectionError) as error:
                if isinstance(error, HTTPError):
                    error.close()
                    if error.code != 429 and not 500 <= error.code < 600:
                        raise
                self.time_left()
                if attempt == MAX_RETRIES or self.remaining <= 0:
                    raise
                self.sleep(min(self.retry_delay(error, attempt), self.time_left()))

    def entries(self, path: str, key: str, token: str) -> list[dict]:
        result = []
        for page in range(1, MAX_PAGES + 1):
            data = self.get(f"{path}&per_page=100&page={page}", token=token)
            entries = data[key]
            total = data["total_count"]
            if not isinstance(entries, list) or type(total) is not int or total < 0:
                raise ValueError("invalid collection")
            result.extend(entries)
            if len(result) >= total:
                return result
            if len(entries) < 100:
                raise ValueError("incomplete collection")
        raise ValueError("collection exceeds probe limit")


def warning(reason: str, *, hosted: bool = True) -> None:
    fallback = "; using hosted runners" if hosted else ""
    print(f"::warning::Merge-queue runner: {reason}{fallback}.")


def decision(reason: str, labels: str = "") -> str:
    """Publish only fixed diagnostics and counts, never API text or credentials."""
    summary = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary:
        try:
            with Path(summary).open("a", encoding="utf-8") as output:
                selected = "self-hosted" if labels else "hosted"
                output.write(
                    f"### Merge-queue runner selection\n\nSelected **{selected}**: {reason}.\n"
                )
        except Exception as error:
            warning(f"could not publish summary ({type(error).__name__})", hosted=False)
    return labels


def online_runners(path: str, wanted: set[str], probe: Probe) -> dict[int, bool]:
    """Collect matching online runner IDs and busy states within bounded pages."""
    matched = {}
    for page in range(1, MAX_PAGES + 1):
        data = probe.get(f"{path}/actions/runners?per_page=100&page={page}")
        runners = data["runners"]
        for runner in runners:
            if runner["status"] != "online" or not wanted.issubset(
                label["name"].casefold() for label in runner["labels"]
            ):
                continue
            runner_id = runner["id"]
            busy = runner["busy"]
            if type(runner_id) is not int or type(busy) is not bool:
                raise ValueError("invalid runner capacity data")
            matched[runner_id] = matched.get(runner_id, False) or busy
        if len(runners) < 100 or page * 100 >= data["total_count"]:
            break
    return matched


def job_commitments(jobs: list[dict], wanted: set[str]) -> tuple[int, bool, bool]:
    """Read the common heavy-pool choice from labels, including queued work."""
    queued = 0
    dell = hosted = False
    for job in jobs:
        raw_labels = job["labels"]
        if (
            not isinstance(raw_labels, list)
            or any(not isinstance(label, str) for label in raw_labels)
            or not isinstance(job["name"], str)
            or job["status"]
            not in (
                "queued",
                "waiting",
                "pending",
                "requested",
                "in_progress",
                "completed",
            )
        ):
            raise ValueError("invalid job routing data")
        labels = {label.casefold() for label in raw_labels}
        if wanted.issubset(labels):
            dell = True
            if job["status"] in ("queued", "waiting", "pending", "requested"):
                queued += 1
        elif labels and any(
            name in job["name"]
            for name in ("Rust Cross Lint 🦀 (", "Rust Tier 2 Linux 🐧 (")
        ):
            hosted = True
    return queued, dell, hosted


def inflight_commitments(repo: str, wanted: set[str], probe: Probe) -> tuple[int, int]:
    """Count queued Dell jobs and active Dell/older undecided Pull runs.

    Outputs aren't exposed by the jobs API. A routed heavy job's labels prove
    the choice; before those jobs appear, older runs reserve a run slot. This
    gives identical empty-pool snapshots the same deterministic winners.
    """
    current_id = int(os.environ["GITHUB_RUN_ID"])
    token = os.environ["GITHUB_TOKEN"]
    if current_id < 1 or not token:
        raise ValueError("missing Actions probe identity")
    runs = {}
    for status in ("queued", "in_progress"):
        for run in probe.entries(
            f"repos/{repo}/actions/workflows/ci.yml/runs?event=merge_group&status={status}",
            "workflow_runs",
            token,
        ):
            run_id = run["id"]
            if (
                type(run_id) is not int
                or run_id < 1
                or run["event"] != "merge_group"
                or run["status"] not in ("queued", "in_progress")
            ):
                raise ValueError("invalid active run")
            created = datetime.fromisoformat(run["created_at"].replace("Z", "+00:00"))
            if created.tzinfo is None:
                raise ValueError("missing run timezone")
            runs[run_id] = (created, run_id)
    current = runs[current_id]  # An incomplete/eventually consistent list is unsafe.
    queued = committed = 0
    for run_id, priority in sorted(runs.items(), key=lambda item: item[1]):
        if run_id == current_id:
            continue
        jobs = probe.entries(
            f"repos/{repo}/actions/runs/{run_id}/jobs?filter=latest", "jobs", token
        )
        pending, dell, hosted = job_commitments(jobs, wanted)
        queued += pending
        # Count known Dell runs regardless of age. Unknown older runs also
        # reserve slots; an observed hosted choice releases their reservation.
        committed += dell or (not hosted and priority < current)
    return queued, committed


def admission_available(
    repo: str, wanted: set[str], probe: Probe, online: int, idle: int, required: int
) -> tuple[bool, str]:
    """The idle floor and in-flight run cap must both admit this run."""
    if idle < required:
        return False, ""
    queued, committed = inflight_commitments(repo, wanted, probe)
    limit = online // required
    counts = f"; queued={queued}, committed={committed}, run_limit={limit}"
    return idle - queued >= required and committed < limit, counts


def required_capacity(min_idle: str, demand: str, mode: str = "") -> int:
    """Both modes honor the idle floor and the heavy jobs' per-run demand."""
    if mode not in ("", "prefer"):
        raise ValueError("unknown runner selection mode")
    minimum = int(min_idle or "1")
    per_run = int(demand or "1")
    if minimum < 1 or per_run < 1:
        raise ValueError("capacity thresholds must be positive")
    return max(minimum, per_run)


def requested_labels(labels: str) -> list[str]:
    requested = json.loads(labels)
    if (
        not isinstance(requested, list)
        or not requested
        or any(not isinstance(label, str) or not label.strip() for label in requested)
    ):
        raise ValueError("labels must be a nonempty string array")
    return requested


def select_labels(
    event: str,
    labels: str,
    token: str,
    repo: str,
    api_url: str = "https://api.github.com",
    *,
    min_idle: str = "1",
    demand: str = "1",
    mode: str = "",
) -> str:
    """Return compact label JSON or an empty Actions output (hosted fallback).

    Require spare capacity in both threshold and prefer modes. Pagination and
    each HTTP request stay bounded; runners visible in both scopes count only
    once. In-flight commitments and older undecided runs reserve capacity.
    """
    if event != "merge_group" or not labels:
        return decision("opt-in merge-group runner selection is disabled")
    try:
        requested = requested_labels(labels)
        required = required_capacity(min_idle, demand, mode)
        if not token:
            warning("MQ_RUNNER_STATUS_TOKEN is missing")
            return decision("runner probe token is missing")
        wanted = {label.casefold() for label in requested}
        probe = Probe(token, api_url)
        matched = {}
        unavailable = []
        owner = repo.split("/", 1)[0]
        for scope, path in (
            ("repository", f"repos/{repo}"),
            ("organization", f"orgs/{owner}"),
        ):
            try:
                runners = online_runners(path, wanted, probe)
                for runner_id, busy in runners.items():
                    # A later busy observation wins over an earlier idle snapshot.
                    matched[runner_id] = matched.get(runner_id, False) or busy
            except SelectionDeadline:
                raise
            except Exception as error:
                # An unavailable scope must not discard a match from the other.
                # Log only the error class/status, never server text or credentials.
                unavailable.append(scope)
                status = f" {error.code}" if isinstance(error, HTTPError) else ""
                warning(
                    f"{scope} availability probe failed ({type(error).__name__}{status})",
                    hosted=False,
                )
        busy = sum(matched.values())
        idle = len(matched) - busy
        counts = f"online={len(matched)}, busy={busy}, idle={idle}, required={required}"
        if unavailable:
            counts += f"; unavailable scopes: {', '.join(unavailable)}"
        admitted, commitments = admission_available(
            repo, wanted, probe, len(matched), idle, required
        )
        probe.time_left()
        counts += commitments
        prefix = {"prefer": "prefer mode: "}.get(mode, "")
        if admitted:
            reason = f"{prefix}sufficient idle capacity"
            return decision(
                f"{reason} ({counts})",
                json.dumps(requested, separators=(",", ":")),
            )
        reason = f"{prefix}insufficient idle capacity"
        reason = (
            f"{reason} after in-flight commitments within the probe limit ({counts})"
        )
        warning(reason)
        return decision(reason)
    except Exception as error:
        # Exception messages can contain credentials or server-controlled text.
        reason = f"availability probe failed ({type(error).__name__})"
        warning(reason)
        return decision(reason)


def main() -> int:
    try:
        with wall_deadline():
            labels = select_labels(
                os.environ.get("GITHUB_EVENT_NAME", ""),
                os.environ.get("MQ_RUNNER_LABELS", ""),
                os.environ.get("MQ_RUNNER_STATUS_TOKEN", ""),
                os.environ.get("GITHUB_REPOSITORY", ""),
                os.environ.get("GITHUB_API_URL", "https://api.github.com"),
                min_idle=os.environ.get("MQ_RUNNER_MIN_IDLE", ""),
                demand=os.environ.get("MQ_RUNNER_DEMAND", ""),
                mode=os.environ.get("MQ_RUNNER_MODE", ""),
            )
    except SelectionDeadline:
        warning("selection deadline exhausted")
        labels = decision("selection deadline exhausted")
    try:
        with Path(os.environ["GITHUB_OUTPUT"]).open("a", encoding="utf-8") as output:
            output.write(f"heavy-labels={labels}\n")
    except Exception as error:
        warning(f"could not publish selection ({type(error).__name__})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
