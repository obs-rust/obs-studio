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


def online_match(path: str, wanted: set[str], token: str, api_url: str) -> bool:
    """Check one runner list, with the same request and page limits for each scope."""
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
        if any(
            runner["status"] == "online"
            and wanted.issubset(label["name"].casefold() for label in runner["labels"])
            for runner in runners
        ):
            return True
        if len(runners) < 100 or page * 100 >= data["total_count"]:
            break
    return False


def select_labels(
    event: str,
    labels: str,
    token: str,
    repo: str,
    api_url: str = "https://api.github.com",
) -> str:
    """Return compact label JSON or an empty Actions output (hosted fallback).

    Busy runners count as online: this is an availability probe, not a capacity
    reservation. Bound pagination and each HTTP request so the probe stays cheap.
    """
    if event != "merge_group" or not labels:
        return ""
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
        if not token:
            warning("MQ_RUNNER_STATUS_TOKEN is missing")
            return ""
        wanted = {label.casefold() for label in requested}
        matched = False
        owner = repo.split("/", 1)[0]
        for scope, path in (
            ("repository", f"repos/{repo}"),
            ("organization", f"orgs/{owner}"),
        ):
            try:
                if online_match(path, wanted, token, api_url):
                    matched = True
            except Exception as error:
                # An unavailable scope must not discard a match from the other.
                # Log only the error class/status, never server text or credentials.
                status = f" {error.code}" if isinstance(error, HTTPError) else ""
                warning(
                    f"{scope} availability probe failed ({type(error).__name__}{status})",
                    hosted=False,
                )
        if matched:
            return json.dumps(requested, separators=(",", ":"))
        warning("no online runner matches all labels within the probe limit")
    except Exception as error:
        # Exception messages can contain credentials or server-controlled text.
        warning(f"availability probe failed ({type(error).__name__})")
    return ""


def main() -> int:
    labels = select_labels(
        os.environ.get("GITHUB_EVENT_NAME", ""),
        os.environ.get("MQ_RUNNER_LABELS", ""),
        os.environ.get("MQ_RUNNER_STATUS_TOKEN", ""),
        os.environ.get("GITHUB_REPOSITORY", ""),
        os.environ.get("GITHUB_API_URL", "https://api.github.com"),
    )
    try:
        with Path(os.environ["GITHUB_OUTPUT"]).open("a", encoding="utf-8") as output:
            output.write(f"labels={labels}\n")
    except Exception as error:
        warning(f"could not publish selection ({type(error).__name__})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
