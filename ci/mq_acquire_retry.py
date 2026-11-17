#!/usr/bin/env -S uv run --no-project --script
# /// script
# requires-python = ">=3.11"
# ///
"""Retry only proven hosted-runner acquisition cancellations in the live queue.

    uv run --no-project --script ci/mq_acquire_retry.py [--dry-run]

Needs GH_TOKEN (github.token), actions: write, checks: read and contents: read.
Unknown evidence never permits a retry. CI Gate must still pass on the rerun.
"""

from __future__ import annotations

import argparse
import os


def maintain(repo: str, *, dry_run: bool = False) -> int:
    """Inspect live merge-group runs and rerun only acquisition failures."""
    print("queue acquisition retry is not implemented")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()
    return maintain(os.environ.get("GITHUB_REPOSITORY", "obs-rust/obs-studio"), dry_run=args.dry_run)


if __name__ == "__main__":
    raise SystemExit(main())
