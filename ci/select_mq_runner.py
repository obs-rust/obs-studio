#!/usr/bin/env -S uv run --script
"""Probe opt-in queue runners; every unavailable/unknown state uses hosted CI."""

from __future__ import annotations

import json
import os
from pathlib import Path
from urllib.error import HTTPError
from urllib.request import Request, urlopen

MAX_PAGES = 5
API_TIMEOUT = 3


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


def online_runners(
    path: str, wanted: set[str], token: str, api_url: str
) -> dict[int, bool]:
    """Collect matching online runner IDs and busy states within bounded pages."""
    matched = {}
    for page in range(1, MAX_PAGES + 1):
        request = Request(
            f"{api_url.rstrip('/')}/{path}/actions/runners?per_page=100&page={page}",
            headers={
                "Authorization": f"Bearer {token}",
                "Accept": "application/vnd.github+json",
                "X-GitHub-Api-Version": "2022-11-28",
            },
        )
        with urlopen(request, timeout=API_TIMEOUT) as response:
            data = json.load(response)
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


def required_capacity(min_idle: str, demand: str) -> int:
    """Empty Actions variables use defaults; malformed thresholds fail to hosted."""
    minimum = int(min_idle or "2")
    per_run = int(demand or "1")
    if minimum < 1 or per_run < 1:
        raise ValueError("capacity thresholds must be positive")
    return max(minimum, per_run)


def select_labels(
    event: str,
    labels: str,
    token: str,
    repo: str,
    api_url: str = "https://api.github.com",
    *,
    min_idle: str = "2",
    demand: str = "1",
) -> str:
    """Return compact label JSON or an empty Actions output (hosted fallback).

    Require spare capacity, without reserving runners. Pagination and each HTTP
    request stay bounded; runners visible in both scopes count only once.
    """
    if event != "merge_group" or not labels:
        return decision("opt-in merge-group runner selection is disabled")
    try:
        requested = json.loads(labels)
        if (
            not isinstance(requested, list)
            or not requested
            or any(
                not isinstance(label, str) or not label.strip() for label in requested
            )
        ):
            raise ValueError("labels must be a nonempty string array")
        required = required_capacity(min_idle, demand)
        if not token:
            warning("MQ_RUNNER_STATUS_TOKEN is missing")
            return decision("runner probe token is missing")
        wanted = {label.casefold() for label in requested}
        matched = {}
        unavailable = []
        owner = repo.split("/", 1)[0]
        for scope, path in (
            ("repository", f"repos/{repo}"),
            ("organization", f"orgs/{owner}"),
        ):
            try:
                runners = online_runners(path, wanted, token, api_url)
                for runner_id, busy in runners.items():
                    # A later busy observation wins over an earlier idle snapshot.
                    matched[runner_id] = matched.get(runner_id, False) or busy
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
        if idle >= required:
            return decision(
                f"sufficient idle capacity ({counts})",
                json.dumps(requested, separators=(",", ":")),
            )
        reason = f"insufficient idle capacity within the probe limit ({counts})"
        warning(reason)
        return decision(reason)
    except Exception as error:
        # Exception messages can contain credentials or server-controlled text.
        reason = f"availability probe failed ({type(error).__name__})"
        warning(reason)
        return decision(reason)


def main() -> int:
    labels = select_labels(
        os.environ.get("GITHUB_EVENT_NAME", ""),
        os.environ.get("MQ_RUNNER_LABELS", ""),
        os.environ.get("MQ_RUNNER_STATUS_TOKEN", ""),
        os.environ.get("GITHUB_REPOSITORY", ""),
        os.environ.get("GITHUB_API_URL", "https://api.github.com"),
        min_idle=os.environ.get("MQ_RUNNER_MIN_IDLE", ""),
        demand=os.environ.get("MQ_RUNNER_DEMAND", ""),
    )
    try:
        with Path(os.environ["GITHUB_OUTPUT"]).open("a", encoding="utf-8") as output:
            output.write(f"labels={labels}\n")
    except Exception as error:
        warning(f"could not publish selection ({type(error).__name__})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
