#!/usr/bin/env -S uv run --script
"""Fail when a Tier 2 test that must run was Skipped (obs-rust/obs-studio#776):

    uv run --no-project rust/tools/linux-validate/check_skips.py CTEST_LOG

The harness image once had no PipeWire daemon, dbus-daemon or X server, so
those tests exited 77 and ctest reported them Skipped, which counted as a pass.
Tests matching MUST_RUN may not skip. Tested by ci/test_tier2_skips.py.

Ported off the 16-line shell script in #962; validate.sh runs it through
`uv run --no-project`, the same way it runs the other ported helpers of that
lane.
"""

from __future__ import annotations

import re
import sys
from collections.abc import Iterable
from pathlib import Path

# The tests the harness image must make runnable: a skip among them is a hole,
# not a pass. Sleep inhibition through the portal and session D-Bus was once
# lost without a failing test (#453), so it is here too.
MUST_RUN = re.compile(r"^(test_pipewire_|test_vlc_|test_screencast_portal_|test_inhibit_sleep)")
# `sed -nE 's/^.*Test +#[0-9]+: +([^ ]+) +\.+ *(\*\*\*)?Skipped.*$/\1/p'`: the
# whole line, so a "Passed" line never matches even if it carries the dots.
SKIPPED = re.compile(r"^.*Test +#[0-9]+: +([^ ]+) +\.+ *(?:\*\*\*)?Skipped.*$")


def skipped_must_run(lines: Iterable[str]) -> list[str]:
    """The names of tests that must run but were reported Skipped, sorted."""
    names = {match.group(1) for line in lines if (match := SKIPPED.match(line))}
    return sorted(name for name in names if MUST_RUN.match(name))


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        sys.stderr.write("usage: check_skips.py CTEST_LOG\n")
        return 2
    log = Path(argv[1])
    names = skipped_must_run(log.read_text(encoding="utf-8", errors="replace").splitlines())
    if names:
        print("FAIL: Tier 2 skipped tests that must run (obs-rust/obs-studio#776):")
        for name in names:
            print(f"  {name}")
        print(
            "The harness image must provide pipewire, dbus-daemon, xvfb and xauth, "
            "and ctest must run under xvfb-run."
        )
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
