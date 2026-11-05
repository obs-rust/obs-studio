#!/usr/bin/env -S uv run --script
"""Only unflagged daily betas are selected for pre-release hygiene.

    uv run python -m unittest ci/test_release_hygiene.py

The five betas that were published before the release workflow passed
`--prerelease` are the reason this exists: they rendered as ordinary releases
directly under the stable one.
"""

from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def load(name: str, path: Path):
    """Import a ci/ script by path: nothing here edits sys.path (test_no_sys_path_edits)."""
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


hygiene = load("release_hygiene", ROOT / "ci/release_hygiene.py")


def release(tag: str, *, pre: bool = False, draft: bool = False) -> dict[str, object]:
    return {"tagName": tag, "isPrerelease": pre, "isDraft": draft}


class SelectionTests(unittest.TestCase):
    def test_flags_an_unflagged_daily(self) -> None:
        self.assertEqual(
            hygiene.select_dailies([release("obs-studio-rust-v0.1.0-beta.20261023")]),
            ["obs-studio-rust-v0.1.0-beta.20261023"],
        )

    def test_leaves_the_stable_and_flagged_dailies_alone(self) -> None:
        releases = [
            release("obs-studio-rust-v1.0.0"),
            release("obs-studio-rust-v0.1.0-beta.20261101", pre=True),
            release("continuous", pre=True),
            release("obs-stream-rust-v0.1.0"),
            release("ci-native-cache-v1-windows-x64", pre=True),
        ]
        self.assertEqual(hygiene.select_dailies(releases), [])

    def test_leaves_drafts_alone(self) -> None:
        self.assertEqual(
            hygiene.select_dailies(
                [release("obs-studio-rust-v1.0.1-beta.20261106", draft=True)]
            ),
            [],
        )

    def test_ignores_tags_that_only_look_like_dailies(self) -> None:
        for tag in (
            "obs-studio-rust-v1.0.1-beta",  # no date
            "obs-studio-rust-v1.0.1-beta.2026110",  # short date
            "obs-studio-rust-v1.0.1-rc.20261106",  # not a beta
            "obs-studio-rust-v1.0.1",
            "obs-stream-rust-v0.1.0-beta.20261106",  # a different product
        ):
            self.assertEqual(hygiene.select_dailies([release(tag)]), [], tag)

    def test_reports_every_stale_daily_once_sorted(self) -> None:
        releases = [
            release("obs-studio-rust-v0.1.0-beta.20261022"),
            release("obs-studio-rust-v0.1.0-beta.20261014"),
            release("obs-studio-rust-v0.1.0-beta.20261018"),
        ]
        self.assertEqual(
            hygiene.select_dailies(releases),
            [
                "obs-studio-rust-v0.1.0-beta.20261014",
                "obs-studio-rust-v0.1.0-beta.20261018",
                "obs-studio-rust-v0.1.0-beta.20261022",
            ],
        )


if __name__ == "__main__":
    unittest.main()
