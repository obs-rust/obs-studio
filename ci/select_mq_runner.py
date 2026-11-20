#!/usr/bin/env -S uv run --script
"""Prefer bounded queueing on the opt-in Dell over slower hosted completion.

MQ_RUNNER_MAX_RUNS defaults to 3 concurrent runs on six runners. Reports 22/23
(2026-11-19) measured Dell/hosted medians of 27.8/46.3 minutes and Dell job waits
of p90 349 s, max 615 s: even the observed maximum wait leaves Dell ahead by
8.25 minutes. Keep the idle-capacity path, then allow busy-Dell admission below
that configurable run cap; online capacity must still cover one run's demand.
Older undecided runs reserve slots to bound simultaneous selector decisions.
"""

from __future__ import annotations

import json
import os
import random
import signal
import time
from contextlib import contextmanager
from datetime import datetime, timezone
from email.utils import parsedate_to_datetime
from itertools import count
from pathlib import Path
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen

MAX_PAGES = 5
API_TIMEOUT = 10
SELECTION_TIMEOUT = 45
MAX_RETRIES = 2
MAX_RETRY_AFTER = 5
# The master ruleset builds five entries; a regroup can leave both waves visible.
MAX_ACTIVE_RUNS = 5 * 2
# Two runner scopes, two run statuses, and a jobs collection for each active run.
# Include pagination and retries, while retaining the shared wall-clock deadline.
MAX_REQUESTS = (4 + MAX_ACTIVE_RUNS) * MAX_PAGES * (MAX_RETRIES + 1)
# Hard admission floor: all expanded PR-routable jobs, including both Tier 2 phases.
# ci/test_member_pr_routing.py checks this bound against the workflow inventory.
PR_RUNNER_DEMAND = 13


class RequestBudgetExhausted(ValueError):
    """An active run must reserve capacity when its jobs cannot be probed."""

    def __init__(self, message: str, *, entries=()):
        super().__init__(message)
        self.entries = list(entries)


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
    """Share a bounded regroup-sized budget between census and commitments."""

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

    def get(self, path: str, *, token: str | None = None, retry: bool = True) -> dict:
        request = Request(
            f"{self.api_url}/{path}",
            headers={
                "Authorization": f"Bearer {self.token if token is None else token}",
                "Accept": "application/vnd.github+json",
                "X-GitHub-Api-Version": "2022-11-28",
            },
        )
        retries = MAX_RETRIES if retry else 0
        for attempt in range(retries + 1):
            timeout = min(API_TIMEOUT, self.time_left())
            if self.remaining <= 0:
                raise RequestBudgetExhausted("request budget exhausted")
            self.remaining -= 1  # Retries consume the same shared budget.
            try:
                with urlopen(request, timeout=timeout) as response:
                    result = response_data(path, response)
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
                if self.remaining <= 0:
                    raise RequestBudgetExhausted("request budget exhausted") from error
                if attempt == retries:
                    raise
                self.sleep(min(self.retry_delay(error, attempt), self.time_left()))

    def page(self, path: str, key: str, token: str) -> tuple[list[dict], int]:
        """Read and validate one REST collection page."""
        data = self.get(path, token=token)
        entries = data[key]
        total = data["total_count"]
        if not isinstance(entries, list) or type(total) is not int or total < 0:
            raise ValueError("invalid collection")
        return entries, total

    def entries(self, path: str, key: str, token: str) -> list[dict]:
        # Active runs change status while GitHub counts and pages them. A short
        # page ends a traversal; total_count alone cannot end a full page. Retry
        # a mismatch from page one, accepting either a consistent count or two
        # consecutive traversals with the same IDs despite a stale count.
        mutable = key == "workflow_runs"
        previous = None
        for attempt in range(MAX_RETRIES + 1):
            result = []
            totals = set()
            for page in range(1, MAX_PAGES + 1):
                try:
                    entries, total = self.page(
                        f"{path}&per_page=100&page={page}", key, token
                    )
                except RequestBudgetExhausted as error:
                    raise RequestBudgetExhausted(str(error), entries=result) from error
                result.extend(entries)
                totals.add(total)
                if not mutable:
                    if len(result) >= total:
                        return result
                    if len(entries) < 100:
                        raise ValueError("incomplete collection")
                elif len(entries) < 100:
                    # A missing identity still occupies a conservative run
                    # slot; active_runs normalizes it without a jobs lookup.
                    ids = frozenset(
                        entry.get("id", ("missing", index))
                        for index, entry in enumerate(result)
                    )
                    if (
                        totals == {len(ids)} and len(ids) == len(result)
                    ) or ids == previous:
                        # Retain duplicate observations: active_runs makes any
                        # terminal observation win over a stale active one.
                        return result
                    previous = ids
                    break
            else:
                raise ValueError("collection exceeds probe limit")
            if attempt < MAX_RETRIES:
                warning(
                    f"runs probe: collection changed (entries={len(ids)}, "
                    f"total_count={total}); retrying snapshot {attempt + 1}/{MAX_RETRIES}",
                    hosted=False,
                )
                self.sleep(
                    min(self.retry_delay(ValueError(), attempt), self.time_left())
                )
        raise ValueError("run collection did not stabilize")


def warning(reason: str, *, hosted: bool = True) -> None:
    fallback = "; using hosted runners" if hosted else ""
    print(f"::warning::Merge-queue runner: {reason}{fallback}.")


def probe_error(error: Exception) -> str:
    """Include validation reasons, escaping Actions annotation control characters."""
    detail = ""
    if isinstance(error, (ValueError, KeyError)):
        detail = ": " + str(error).replace("%", "%25").replace("\r", "%0D").replace(
            "\n", "%0A"
        )
    elif isinstance(error, HTTPError):
        detail = f" {error.code}"
    return f"{type(error).__name__}{detail}"


def decision(reason: str, labels: str = "") -> str:
    """Publish only fixed diagnostics and counts, never API text or credentials."""
    if labels:
        print(f"Merge-queue runner: {reason}.")
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


def selector_finished(jobs: list[dict]) -> bool:
    """Only an unsuccessful selector proves no unseen Dell work can follow."""
    for job in jobs:
        if job["name"] != "Select Merge Queue Runner":
            continue
        if job.get("conclusion") not in (None, "success"):
            # Successful outputs are internal to Actions. Heavy-job labels can
            # appear later, so success (or a missing conclusion) cannot release
            # an older run's provisional reservation. Retain its rank until
            # routing is visible; brief under-use is safer than overcommit.
            return True
    return False


def job_commitments(jobs: list[dict], wanted: set[str]) -> tuple[int, bool, bool]:
    """Return queued Dell jobs, live Dell work and resolved reservations."""
    queued = 0
    dell = decided = False
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
            decided = True
            if job.get("conclusion") is None and job["status"] != "completed":
                dell = True
            if job.get("conclusion") is None and job["status"] in (
                "queued",
                "waiting",
                "pending",
                "requested",
            ):
                queued += 1
        elif labels and any(
            name in job["name"]
            for name in (
                "Rust Cross Lint 🦀 (",
                "Rust Tier 2 Linux 🐧 (",
                "macOS Rust Artifacts 🐧",
            )
        ):
            decided = True
    if not decided:
        decided = selector_finished(jobs)
    return queued, dell, decided


# Unknown timestamps sort before every real run, conservatively holding slots.
UNKNOWN_CREATED = datetime.min.replace(tzinfo=timezone.utc)


def run_snapshot(run: dict, status: str, missing_id: int, event: str) -> dict:
    """Normalize missing routing fields conservatively within a filtered list."""
    if "id" in run and (type(run["id"]) is not int or run["id"] < 1):
        raise ValueError("invalid active run ID")
    defaults = {
        "id": missing_id,
        "event": event,
        "status": status,
        "created_at": UNKNOWN_CREATED.isoformat(),
    }
    for key, value in defaults.items():
        if key not in run:
            warning(f"runs probe: missing key '{key}'; reserving run", hosted=False)
            run = {key: value, **run}
    return run


def active_runs(repo: str, probe: Probe, token: str, event: str = "merge_group") -> dict:
    """Keep only active run snapshots; terminal snapshots release reservations."""
    runs = {}
    finished = set()
    missing_ids = count(-1, -1)
    for status in ("queued", "in_progress"):
        try:
            entries = probe.entries(
                f"repos/{repo}/actions/workflows/ci.yml/runs?event={event}&status={status}",
                "workflow_runs",
                token,
            )
        except ValueError as error:
            raise ValueError(f"runs probe ({status}): {error}") from error
        except SelectionDeadline:
            raise
        except Exception as error:
            raise ValueError(f"runs probe ({status}): {probe_error(error)}") from error
        for run in entries:
            run = run_snapshot(run, status, next(missing_ids), event)
            run_id = run["id"]
            if (
                run.get("status") in ("completed", "cancelled")
                or run.get("conclusion") is not None
            ):
                runs.pop(run_id, None)
                finished.add(run_id)
                continue
            if run_id in finished:
                continue
            if (
                type(run_id) is not int
                or run["event"] != event
                or run["status"] not in ("queued", "in_progress")
            ):
                raise ValueError(f"runs probe ({status}): invalid active run")
            created = datetime.fromisoformat(run["created_at"].replace("Z", "+00:00"))
            if created.tzinfo is None:
                raise ValueError(f"runs probe ({status}): missing run timezone")
            runs[run_id] = ((created, run_id), run["status"])
    return runs


def observed_job_commitments(
    jobs: list[dict], wanted: set[str], demand: int, run_id: int
) -> tuple[int, bool, bool]:
    """An incomplete job snapshot holds one run rather than aborting admission."""
    try:
        return job_commitments(jobs, wanted)
    except KeyError as error:
        warning(
            f"jobs probe (run {run_id}): {probe_error(error)}; reserving run",
            hosted=False,
        )
        return demand, True, True


def current_priority(runs: dict, current_id: int):
    """Unknown current ordering must never outrank an undecided peer."""
    current = runs.get(current_id)
    if current is None:
        # A stable retried page can omit this run. Without its timestamp every
        # undecided peer must rank ahead of us; do not guess its priority.
        warning(
            f"runs probe: missing key {current_id} (current run); "
            "reserving all undecided peers", hosted=False,
        )
    current = current[0] if current and UNKNOWN_CREATED not in current[0] else None
    return current


def commitment_runs(repo: str, probe: Probe, token: str, mixed: bool) -> dict:
    """Queue priorities precede PRs, retaining distinct unknown-identity slots."""
    runs = active_runs(repo, probe, token)
    if mixed:
        # Queue runs sort before every PR, even a PR created earlier. Inspect
        # live PR jobs for both events so the queue sees their queued demand.
        runs = {key: ((0, *value[0]), value[1]) for key, value in runs.items()}
        prs = active_runs(repo, probe, token, "pull_request")
        offset = min(0, min(runs, default=0))
        runs.update({key if key > 0 else key + offset: ((1, *value[0]), value[1])
                     for key, value in prs.items()})
    return runs


def inflight_commitments(
    repo: str, wanted: set[str], probe: Probe, demand: int = 1,
    *, mq_required: int = 0, pr_required: int = 0,
) -> tuple[int, int]:
    """Count queued Dell jobs and active Dell/older undecided runs.

    Outputs aren't exposed by the jobs API. Completed heavy-job labels still
    prove the route, but only live Dell jobs commit capacity. Inspect queued
    workflows too: their selector may already have finished. Older genuinely
    undecided runs reserve a slot, giving identical snapshots the same winners.
    """
    current_id = int(os.environ["GITHUB_RUN_ID"])
    token = os.environ["GITHUB_TOKEN"]
    if current_id < 1 or not token:
        raise ValueError("missing Actions probe identity")
    mixed = os.environ.get("DELL_PR_ROUTING") == "on"
    runs = commitment_runs(repo, probe, token, mixed)
    current = current_priority(runs, current_id)
    queued = committed = unprobed = 0
    for run_id, (priority, _) in sorted(runs.items(), key=lambda item: item[1][0]):
        if run_id == current_id:
            continue
        weight = (pr_required if priority[0] == 1 else mq_required) if mixed else 1
        peer_demand = weight if mixed else demand
        if run_id < 0:  # Missing identity: cannot safely probe or release this peer.
            queued += peer_demand
            committed += weight
            continue
        try:
            jobs = probe.entries(
                f"repos/{repo}/actions/runs/{run_id}/jobs?filter=latest", "jobs", token
            )
            pending, dell, decided = observed_job_commitments(jobs, wanted, peer_demand, run_id)
        except RequestBudgetExhausted as error:
            unprobed += 1
            observed, _, _ = observed_job_commitments(error.entries, wanted, peer_demand, run_id)
            queued += max(observed, peer_demand)
            committed += weight
            continue
        except ValueError as error:
            raise ValueError(f"jobs probe (run {run_id}): {error}") from error
        except SelectionDeadline:
            raise
        except Exception as error:
            raise ValueError(
                f"jobs probe (run {run_id}): {probe_error(error)}"
            ) from error
        queued += pending
        # Count known Dell runs regardless of age. Unknown older runs also
        # reserve slots; hosted or finished Dell work releases their reservation.
        committed += weight * (dell or (not decided and (current is None or priority < current)))
    if unprobed:
        reservation = (
            "reserving each unprobed active run's required capacity"
            if mixed else f"reserving at least {demand} runners per unprobed active run"
        )
        warning(
            f"jobs probe: request budget exhausted; unprobed={unprobed}; "
            f"{reservation}",
            hosted=False,
        )
    return queued, committed


def queue_admission(
    repo: str, wanted: set[str], probe: Probe, online: int, idle: int,
    required: int, options: dict,
) -> tuple[bool, str]:
    """Allow busy MQ work; mixed traffic retains PR capacity weights."""
    max_runs = int(os.environ.get("MQ_RUNNER_MAX_RUNS") or "3")
    if max_runs < 1:
        raise ValueError("MQ_RUNNER_MAX_RUNS must be positive")
    if online < required:
        return False, f"; run_limit={max_runs}"
    queued, committed = inflight_commitments(repo, wanted, probe, required, **options)
    idle_slots = max(0, (idle - queued) // required)
    # With PR routing on, commitments retain each event's full runner demand:
    # the MQ budget is max_runs * required, so a large PR is never one small MQ.
    weight = required if options else 1
    limit = max_runs * weight
    counts = (
        f"; queued={queued}, committed={committed}, run_limit={limit}"
        f", idle_run_slots={idle_slots}"
    )
    if options:
        counts += f"; commitments measured in runner capacity, max_runs={max_runs}"
    if committed + weight > limit:
        return False, counts
    if idle_slots >= 1:
        return True, counts + "; admission=idle"
    return True, counts + "; admission=busy queue (expected faster than hosted)"


def admission_available(
    repo: str, wanted: set[str], probe: Probe, online: int, idle: int, required: int,
    reserve: int = 0, mq_required: int = 0, pr_required: int = 0, queue: bool = True,
) -> tuple[bool, str]:
    """Queue overcommit is MQ-only; PRs retain their idle floor and MQ reserve."""
    mixed = os.environ.get("DELL_PR_ROUTING") == "on"
    options = {"mq_required": mq_required, "pr_required": pr_required} if mixed else {}
    if queue:
        return queue_admission(repo, wanted, probe, online, idle, required, options)
    if idle - reserve < required:
        return False, ""
    queued, committed = inflight_commitments(repo, wanted, probe, required, **options)
    idle_slots = (idle - queued - reserve) // required
    limit = online - reserve if mixed else (online - reserve) // required
    counts = (
        f"; queued={queued}, committed={committed}, run_limit={limit}"
        f", idle_run_slots={max(0, idle_slots)}"
    )
    if mixed:
        counts += "; commitments measured in runner capacity"
        return idle_slots >= 1 and committed + required <= limit, counts
    return idle_slots >= 1 and committed < limit, counts


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


def eligible_pr(repo: str, probe: Probe) -> bool:
    """Read identity only from the Actions payload and authenticated membership.

    The workflow extracts this module and pr_attestation from immutable BASE.
    Public membership's authenticated 204 and private membership's active state
    follow the admission verifier's contract; failures never grant access.
    """
    from pr_attestation import LOGIN, member

    payload = json.loads(Path(os.environ["GITHUB_EVENT_PATH"]).read_text())
    pr = payload["pull_request"]
    base = pr["base"]["repo"]
    head = pr["head"]["repo"]
    repository = payload["repository"]
    if repo.split("/", 1)[0] != "obs-rust" or any(
        value["full_name"] != repo for value in (base, head, repository)
    ):
        return False
    if type(base["id"]) is not int or base["id"] < 1 or not (
        base["id"] == head["id"] == repository["id"]
    ):
        return False
    login = pr["user"]["login"]
    if not isinstance(login, str) or not LOGIN.fullmatch(login):
        return False
    return member(login, lambda path: membership_read(probe, path)) is None


def response_data(path: str, response) -> dict:
    """The authenticated public-membership endpoint returns no JSON on 204."""
    if "/public_members/" in path and getattr(response, "status", None) == 204:
        return {"public_member": True, "user": {"login": path.rsplit("/", 1)[1]}}
    return json.load(response)


def membership_read(probe: Probe, path: str) -> dict:
    """A public 404 needs the private active-state check; other errors deny."""
    try:
        return probe.get(path, retry=False)
    except HTTPError as error:
        if error.code == 404 and "/public_members/" in path:
            return {"public_member": False}
        raise


def selection_enabled(event: str, labels: str) -> bool:
    return bool(labels) and (
        event == "merge_group"
        or (event == "pull_request" and os.environ.get("DELL_PR_ROUTING") == "on")
    )


def routing_capacity(event: str, min_idle: str, demand: str, mode: str) -> dict[str, int]:
    """Only MQs may overcommit; enabled PRs retain their own demand and reserve."""
    required = required_capacity(min_idle, demand, mode)
    capacity = {"required": required, "queue": event == "merge_group"}
    if os.environ.get("DELL_PR_ROUTING") != "on":
        return capacity
    requested = required_capacity("1", os.environ.get("MQ_PR_DEMAND") or str(PR_RUNNER_DEMAND))
    # Selection labels reach the entire PR routing graph; a variable cannot
    # shrink that graph. Keep both admission and peer weights above its count.
    pr_required = max(required, PR_RUNNER_DEMAND, requested)
    if event == "pull_request" and requested < PR_RUNNER_DEMAND:
        warning(
            f"MQ_PR_DEMAND={requested} is below the PR-routed job count "
            f"{PR_RUNNER_DEMAND}; reserving the full inventory", hosted=False,
        )
    capacity.update(mq_required=required, pr_required=pr_required)
    if event == "pull_request":
        reserve = int(os.environ.get("MQ_PR_RESERVE") or demand or "1")
        if reserve < 0:
            raise ValueError("PR reserve must be nonnegative")
        capacity.update(required=pr_required, reserve=reserve)
    return capacity


def capacity_summary(event: str, capacity: dict[str, int]) -> str:
    return f", pr_reserve={capacity['reserve']}" if event == "pull_request" else ""


def routing_eligible(event: str, repo: str, probe: Probe) -> bool:
    return event != "pull_request" or eligible_pr(repo, probe)


def availability_summary(online: int, busy: int, idle: int, required: int, unavailable: list[str]) -> str:
    counts = f"online={online}, busy={busy}, idle={idle}, required={required}"
    if unavailable:
        counts += f"; unavailable scopes: {', '.join(unavailable)}"
    return counts


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

    Both threshold and prefer modes allow bounded busy-Dell queueing. Pagination and
    each HTTP request stay bounded; runners visible in both scopes count only
    once. In-flight commitments and older undecided runs reserve capacity.
    """
    if not selection_enabled(event, labels):
        return decision("opt-in merge-group runner selection is disabled")
    step = "configuration"
    try:
        requested = requested_labels(labels)
        capacity = routing_capacity(event, min_idle, demand, mode)
        required = capacity["required"]
        if not token:
            warning("MQ_RUNNER_STATUS_TOKEN is missing")
            return decision("runner probe token is missing")
        wanted = {label.casefold() for label in requested}
        probe = Probe(token, api_url)
        step = "PR eligibility"
        if not routing_eligible(event, repo, probe):
            return decision("PR author is not a verified same-repository active member")
        matched = {}
        unavailable = []
        owner = repo.split("/", 1)[0]
        for scope, path in (
            ("repository", f"repos/{repo}"),
            ("organization", f"orgs/{owner}"),
        ):
            step = f"{scope} availability probe"
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
                warning(
                    f"{step} failed ({probe_error(error)})",
                    hosted=False,
                )
        busy = sum(matched.values())
        idle = len(matched) - busy
        counts = availability_summary(len(matched), busy, idle, required, unavailable)
        counts += capacity_summary(event, capacity)
        step = "in-flight commitments probe"
        admitted, commitments = admission_available(
            repo, wanted, probe, len(matched), idle, **capacity
        )
        probe.time_left()
        counts += commitments
        prefix = {"prefer": "prefer mode: "}.get(mode, "")
        if admitted:
            reason = f"{prefix}Dell capacity available"
            return decision(
                f"{reason} ({counts})",
                json.dumps(requested, separators=(",", ":")),
            )
        reason = f"{prefix}Dell capacity unavailable"
        reason = (
            f"{reason} after in-flight commitments within the probe limit ({counts})"
        )
        reason = f"capacity-driven hosted routing: {reason}"
        warning(reason)
        return decision(reason)
    except Exception as error:
        # Exception messages can contain credentials or server-controlled text.
        reason = f"{step} failed ({probe_error(error)})"
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
            # Empty means each workflow consumer chooses its hosted fallback:
            # Tier 2 uses ubuntu-latest; host-package jobs retain ubuntu-26.04.
            output.write(f"heavy-labels={labels}\n")
    except Exception as error:
        warning(f"could not publish selection ({type(error).__name__})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
