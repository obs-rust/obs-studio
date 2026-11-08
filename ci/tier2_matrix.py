#!/usr/bin/env -S uv run --script
"""Select Tier 2 scheduling without changing either backend's test registry."""

import json
import os


def matrix(event: str, *, full: bool = False) -> dict[str, list[str]]:
    # Hosted queue wait now dwarfs the serial tests' runtime (#2072).
    # Complete CTest runs retain RUN_SERIAL isolation within each backend.
    compact = event in ("pull_request", "merge_group") and not full
    return {
        "mode": ["OFF", "ON"],
        "part": ["all"] if compact else ["suite", "serial-1-of-2", "serial-2-of-2"],
    }


def main() -> None:
    result = matrix(os.environ.get("GITHUB_EVENT_NAME", ""), full=os.environ.get("TIER2_FULL") == "true")
    line = "matrix=" + json.dumps(result, separators=(",", ":"))
    print(line)
    with open(os.environ["GITHUB_OUTPUT"], "a") as output:
        output.write(line + "\n")


if __name__ == "__main__":
    main()
