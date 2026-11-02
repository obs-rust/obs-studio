#!/usr/bin/env -S uv run --script
"""Does this master push have to refresh the asan compiler archive? (#3501)

    GH_TOKEN=... GITHUB_REPOSITORY=owner/name uv run --no-project ci/asan_cache_state.py

A PR restores only what master saved, and a restored archive saves about 20
minutes while it is fresh. Writes refresh=true to GITHUB_OUTPUT when master has
no archive or the newest one is older than MAX_AGE. A failed query builds
nothing: the next push asks again.
"""

from __future__ import annotations

import datetime as dt
import json
import os
import subprocess
import sys
from collections.abc import Callable

PREFIX = "zccache-sanitize-asan-ON-"
REF = "refs/heads/master"
MAX_AGE = dt.timedelta(hours=6)


def needs_refresh(entries: list[dict], now: dt.datetime) -> bool:
    created = [
        dt.datetime.fromisoformat(entry["created_at"].replace("Z", "+00:00"))
        for entry in entries
        if entry["ref"] == REF and entry["key"].startswith(PREFIX)
    ]
    return not created or now - max(created) > MAX_AGE


def query() -> list[dict]:
    path = f"repos/{os.environ['GITHUB_REPOSITORY']}/actions/caches?key={PREFIX}&ref={REF}&per_page=100"
    command = ["gh", "api", "--paginate", "--jq", ".actions_caches[]", path]
    done = subprocess.run(command, capture_output=True, text=True, check=True)
    return [json.loads(line) for line in done.stdout.splitlines()]


def decide(entries: Callable[[], list[dict]], now: dt.datetime) -> bool:
    try:
        return needs_refresh(entries(), now)
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as error:
        print(f"asan_cache_state: {error}; no refresh on this push", file=sys.stderr)
        return False


def main() -> int:
    refresh = decide(query, dt.datetime.now(dt.timezone.utc))
    print(f"refresh={str(refresh).lower()}")
    with open(os.environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as output:
        output.write(f"refresh={str(refresh).lower()}\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
