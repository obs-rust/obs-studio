#!/usr/bin/env -S uv run --script
"""Keep daily betas out of the release list (#release-versions).

Every daily publishes `obs-studio-rust-v<crate>-beta.<YYYYMMDD>`. They are meant
to be pre-releases, and the release workflow creates them with `--prerelease`,
but a release created before that flag existed (or edited by hand) shows up in
the Releases list as an ordinary release — indistinguishable from a shipped
version, and confusing next to the stable it predates.

This marks any such leftover as a pre-release. It never deletes a release and
never touches a tag: the download URLs people already have keep working.

    uv run --no-project --script ci/release_hygiene.py            # report only
    uv run --no-project --script ci/release_hygiene.py --apply    # mark them

Standard library only; `--apply` shells out to `gh release edit`.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys

#: A daily beta tag: `obs-studio-rust-v<crate>-beta.<YYYYMMDD>`.
BETA_TAG_RE = re.compile(r"^obs-studio-rust-v\d+\.\d+\.\d+-beta\.\d{8}$")


def select_dailies(releases: list[dict[str, object]]) -> list[str]:
    """Tags that are daily betas but are not flagged as pre-releases.

    *releases* is `gh release list --json tagName,isPrerelease,isDraft` output.
    Drafts are left alone: a draft is not visible in the list anyway.
    """
    stale: list[str] = []
    for release in releases:
        tag = str(release.get("tagName") or "")
        if not BETA_TAG_RE.match(tag):
            continue
        if release.get("isPrerelease") or release.get("isDraft"):
            continue
        stale.append(tag)
    return sorted(stale)


def list_releases(repo: str) -> list[dict[str, object]]:
    result = subprocess.run(
        [
            "gh",
            "release",
            "list",
            "--repo",
            repo,
            "--limit",
            "200",
            "--json",
            "tagName,isPrerelease,isDraft",
        ],
        capture_output=True,
        text=True,
        check=True,
    )
    return json.loads(result.stdout or "[]")


def mark_prerelease(repo: str, tag: str) -> None:
    subprocess.run(
        [
            "gh",
            "release",
            "edit",
            tag,
            "--repo",
            repo,
            "--prerelease",
            "--latest=false",
        ],
        capture_output=True,
        text=True,
        check=True,
    )


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument(
        "--repo", required=True, help="owner/repo whose releases to inspect"
    )
    parser.add_argument(
        "--apply",
        action="store_true",
        help="Mark the stale dailies (default: report only)",
    )
    args = parser.parse_args(argv)

    stale = select_dailies(list_releases(args.repo))
    if not stale:
        print("release_hygiene: every daily beta is already flagged as a pre-release")
        return 0
    for tag in stale:
        if args.apply:
            mark_prerelease(args.repo, tag)
            print(f"release_hygiene: flagged {tag} as a pre-release")
        else:
            print(f"release_hygiene: would flag {tag} (run with --apply)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
