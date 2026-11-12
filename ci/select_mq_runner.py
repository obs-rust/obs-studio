#!/usr/bin/env -S uv run --script
"""Probe opt-in queue runners; every unavailable/unknown state uses hosted CI."""

from __future__ import annotations

import json
import os
from pathlib import Path
from urllib.request import Request, urlopen

MAX_PAGES = 5
API_TIMEOUT = 3


def warning(reason: str) -> None:
    print(f"::warning::Merge-queue runner: {reason}; using hosted runners.")


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
        for page in range(1, MAX_PAGES + 1):
            request = Request(
                f"{api_url.rstrip('/')}/repos/{repo}/actions/runners?per_page=100&page={page}",
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
                if runner["status"] == "online" and wanted.issubset(
                    label["name"].casefold() for label in runner["labels"]
                ):
                    return json.dumps(requested, separators=(",", ":"))
            if len(runners) < 100 or page * 100 >= data["total_count"]:
                break
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
