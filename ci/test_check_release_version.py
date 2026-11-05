#!/usr/bin/env -S uv run --script
"""A release may never sort below the newest stable (#release-versions).

    uv run python -m unittest ci/test_check_release_version.py

The cases that matter are the two real regressions: a nightly cut while the
crate still said `0.1.0` (below the `1.0.0` stable), and a nightly cut after a
stable shipped without the follow-up crate bump (a `1.0.1-beta.*` below `1.0.1`).
"""

from __future__ import annotations

import subprocess
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCRIPT = ROOT / "ci/check_release_version.py"

sys.path.insert(0, str(ROOT / "ci"))
import check_release_version as guard


def run(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(SCRIPT), *args],
        capture_output=True,
        text=True,
        check=False,
    )


class OrderingTests(unittest.TestCase):
    def test_semver_precedence(self) -> None:
        cases = [
            ("1.0.0", "0.99.99", ">"),
            ("1.0.1-beta.20261105", "1.0.0", ">"),
            ("1.0.1-beta.20261105", "1.0.1", "<"),
            ("0.1.0-beta.20261101", "1.0.0", "<"),
            ("1.0.1-beta.10", "1.0.1-beta.9", ">"),
            ("2.0.0-beta.1", "1.9.9", ">"),
            ("1.0.0-beta.2", "1.0.0-beta.10", "<"),
            ("1.0.1", "1.0.1", "="),
        ]
        for left, right, want in cases:
            got = (
                "="
                if guard.version_key(left) == guard.version_key(right)
                else (
                    ">" if guard.version_key(left) > guard.version_key(right) else "<"
                )
            )
            self.assertEqual(got, want, f"{left} vs {right}")


class RefusalTests(unittest.TestCase):
    def test_a_nightly_after_the_stable_is_allowed(self) -> None:
        self.assertIsNone(guard.ordering_problem("1.0.1-beta.20261105", "1.0.0"))

    def test_a_nightly_below_the_stable_is_refused(self) -> None:
        problem = guard.ordering_problem("0.1.0-beta.20261101", "1.0.0")
        self.assertIsNotNone(problem)
        assert problem is not None
        self.assertIn("does not sort after the newest stable 1.0.0", problem)
        self.assertIn("rust/libobs-rust/Cargo.toml", problem)
        # The suggested bump is past the *stable*, never past the stale version.
        self.assertIn("to 1.0.1", problem)

    def test_the_forgotten_bump_is_refused_with_the_next_patch(self) -> None:
        # 1.0.1 stable already shipped; the crate was never bumped, so tonight's
        # nightly would read as older than the release people are running.
        problem = guard.ordering_problem("1.0.1-beta.20261106", "1.0.1")
        self.assertIsNotNone(problem)
        assert problem is not None
        self.assertIn("1.0.2", problem)

    def test_a_stable_at_or_below_the_newest_is_refused(self) -> None:
        self.assertIsNotNone(guard.ordering_problem("1.0.0", "1.0.0"))
        self.assertIsNotNone(guard.ordering_problem("0.9.0", "1.0.0"))
        self.assertIsNone(guard.ordering_problem("1.0.1", "1.0.0"))

    def test_a_malformed_nightly_is_refused(self) -> None:
        self.assertIn(
            "must be", guard.ordering_problem("1.0.1-rc.20261105", "1.0.0") or ""
        )
        self.assertIn(
            "no -beta", guard.ordering_problem("1.0.1", "1.0.0", nightly=True) or ""
        )

    def test_garbage_is_refused_instead_of_compared(self) -> None:
        self.assertIn("is not X.Y.Z", guard.ordering_problem("v1.0.1", "1.0.0") or "")
        self.assertIn(
            "is not X.Y.Z",
            guard.ordering_problem("1.0.1-beta.20261105", "latest") or "",
        )


class CommandTests(unittest.TestCase):
    def test_exit_zero_when_the_version_is_ahead(self) -> None:
        result = run(
            "check", "--version", "1.0.1-beta.20261105", "--latest-stable", "1.0.0"
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("sorts after the newest stable", result.stdout)

    def test_exit_one_when_the_version_is_behind(self) -> None:
        result = run(
            "check", "--version", "0.1.0-beta.20261101", "--latest-stable", "1.0.0"
        )
        self.assertEqual(result.returncode, 1)
        self.assertIn("does not sort after the newest stable", result.stderr)

    def test_compare_is_a_debugging_aid(self) -> None:
        result = run("compare", "1.0.1-beta.20261105", "1.0.0")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.strip(), "1.0.1-beta.20261105 > 1.0.0")


if __name__ == "__main__":
    unittest.main()
