#!/usr/bin/env -S uv run --script
"""Refuse a release whose version does not sort after the newest stable (#release-versions).

Nightlies are tagged `<crate>-beta.<YYYYMMDD>` and stables `<crate>`. Both are
derived from `rust/libobs-rust/Cargo.toml`, which means the ordering depends on
a human remembering to bump the crate after a stable ships:

- bump the crate after `1.0.0` -> nightly `1.0.1-beta.20261105` sorts **above**
  `1.0.0` and **below** `1.0.1`; the nightly channel keeps updating;
- forget the bump -> nightly `1.0.1-beta.20261105` sorts **below** the stable
  `1.0.1`; the nightly channel silently stops offering anything, and the
  Releases list shows a "newer" build whose version reads as older.

That second case happened for real: dailies cut while the crate still said
`0.1.0` were published after `1.0.0`, so `obs-studio-rust-v0.1.0-beta.*`
releases sit below the stable release. This check makes the mistake a red gate
instead of a Discord question:

    uv run --no-project --script ci/check_release_version.py check \\
        --version 1.0.1-beta.20261105 --newest-stable 1.0.0

Exit 0 when `version` sorts strictly after `latest-stable`; exit 1 (with what to
do about it) when it does not, when a nightly is malformed, or when either side
is not a version this scheme can order.

Standard library only: the release `check` job runs it before anything builds.
"""

from __future__ import annotations

import argparse
import re
import sys

#: `X.Y.Z` with an optional `-<prerelease>` (`beta.20261105`), as the release
#: workflow composes it from the crate version.
VERSION_RE = re.compile(r"^(?P<core>\d+\.\d+\.\d+)(?:-(?P<pre>[0-9A-Za-z.-]+))?$")
#: A nightly adds exactly this shape to the crate version.
NIGHTLY_PRE_RE = re.compile(r"^beta\.\d{8}$")


class VersionError(ValueError):
    """The value is not a version this scheme can order."""


def parse_version(value: str) -> tuple[tuple[int, int, int], tuple[str, ...] | None]:
    """`(core, prerelease identifiers)`; `None` for a stable release."""
    match = VERSION_RE.match(value.strip())
    if match is None:
        raise VersionError(f"{value!r} is not X.Y.Z or X.Y.Z-<prerelease>")
    major, minor, patch = (int(part) for part in match.group("core").split("."))
    core = (major, minor, patch)
    pre = match.group("pre")
    return core, (tuple(pre.split(".")) if pre else None)


def version_key(
    value: str,
) -> tuple[tuple[int, int, int], int, tuple[tuple[int, int, str], ...]]:
    """A sortable key with semver precedence (semver.org §11).

    A stable release outranks every prerelease of the same core version, and
    prerelease identifiers compare left to right: numeric identifiers
    numerically, numeric before alphanumeric, others by ASCII order.
    """
    core, pre = parse_version(value)
    if pre is None:
        # Stable: higher than any prerelease of this core.
        return core, 1, ()
    identifiers: list[tuple[int, int, str]] = []
    for identifier in pre:
        if identifier.isdigit():
            identifiers.append((0, int(identifier), ""))
        else:
            identifiers.append((1, 0, identifier))
    return core, 0, tuple(identifiers)


def ordering_problem(
    version: str, latest_stable: str, *, nightly: bool | None = None
) -> str | None:
    """Why *version* may not be published after *latest_stable*, or None when it may."""
    try:
        new_key = version_key(version)
        stable_key = version_key(latest_stable)
    except VersionError as error:
        return str(error)
    core, pre = parse_version(version)
    if nightly is None:
        nightly = pre is not None
    if nightly:
        if pre is None:
            return f"{version} has no -beta.<date> suffix: a nightly must be <crate>-beta.<YYYYMMDD>"
        if not NIGHTLY_PRE_RE.match(".".join(pre)):
            return f"{version} is not a nightly: the suffix must be beta.<YYYYMMDD>"
    if new_key <= stable_key:
        # The bump is always relative to the *stable*, not to the version this
        # run computed: the crate has to move past what users already have.
        stable_core, _stable_pre = parse_version(latest_stable)
        bump = ".".join(
            str(part) for part in (stable_core[0], stable_core[1], stable_core[2] + 1)
        )
        return (
            f"{version} does not sort after the newest stable {latest_stable} "
            f"(it would publish a release that reads as older than the one already out).\n"
            f"  bump rust/libobs-rust/Cargo.toml past {latest_stable} "
            f"(for example to {bump}) so this {('nightly' if nightly else 'stable')} sorts after it"
        )
    return None


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    sub = parser.add_subparsers(dest="command", required=True)

    check = sub.add_parser(
        "check", help="Fail when the version does not sort after the newest stable"
    )
    check.add_argument(
        "--version",
        required=True,
        help="Version this run would publish (e.g. 1.0.1-beta.20261105)",
    )
    check.add_argument(
        "--newest-stable",
        required=True,
        help="Newest stable release version (e.g. 1.0.0)",
    )
    check.add_argument(
        "--nightly",
        choices=("yes", "no"),
        default=None,
        help="Force the nightly/stable reading instead of inferring it from the suffix",
    )

    compare = sub.add_parser(
        "compare", help="Print the ordering of two versions (debugging aid)"
    )
    compare.add_argument("left")
    compare.add_argument("right")

    args = parser.parse_args(argv)
    if args.command == "compare":
        try:
            left, right = version_key(args.left), version_key(args.right)
        except VersionError as error:
            print(error, file=sys.stderr)
            return 1
        relation = "=" if left == right else (">" if left > right else "<")
        print(f"{args.left} {relation} {args.right}")
        return 0

    nightly = {"yes": True, "no": False}.get(args.nightly or "", None)
    problem = ordering_problem(args.version, args.newest_stable, nightly=nightly)
    if problem is not None:
        print(f"check_release_version: {problem}", file=sys.stderr)
        return 1
    print(
        print(f"check_release_version: {args.version} sorts after the newest stable {args.newest_stable}")
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
