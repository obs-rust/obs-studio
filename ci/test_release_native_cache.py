#!/usr/bin/env -S uv run --script
"""The daily release builds its native compiler caches cold (#782, #1801).

    uv run python -m unittest ci/test_release_native_cache.py

release-rust.yaml's Windows and macOS builds restored, read-only, the
compiler caches build-project.yaml's app builds saved on master: Windows x64
took 15m34s at 0/716 ccache hits without them against 3m08s at 759/761
(#782). Phase A of #1801 took those app-build caches out of the Actions
cache, so the release has nothing to restore. It still sets up ccache and
the Xcode compilation cache for the build itself, and it never saves, so it
cannot fill the budget either. The release-asset transport (#782) can bring
the warm caches back.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
RELEASE = ROOT / ".github" / "workflows" / "release-rust.yaml"


def build_job() -> str:
    text = RELEASE.read_text(encoding="utf-8")
    m = re.search(r"^  build:\n(.*?)(?=^  [a-z0-9-]+:\n)", text, re.S | re.M)
    assert m
    return m.group(1)


class ReleaseNativeCacheTest(unittest.TestCase):
    def setUp(self):
        self.job = build_job()
        self.build_at = self.job.index("uses: ./.github/actions/build-obs")

    def test_windows_sets_up_ccache_before_the_build(self):
        setup = self.job[self.job.index("Set Up ccache"):self.build_at]
        self.assertIn("6219f3865ca59aec41ee4b678df171d5d35855ecb2b6dbbbd20690b3a68af7b4", setup)
        self.assertIn("CCACHE_DIR", setup)

    def test_macos_points_xcode_at_its_compilation_cache(self):
        self.assertLess(self.job.index("XCODE_CAS_PATH="), self.build_at)

    def test_release_restores_no_retired_app_cache(self):
        for prefix in ("windows-ccache-", "macOS-xcode-"):
            with self.subTest(prefix=prefix):
                self.assertNotIn(prefix, self.job)
        self.assertNotIn("uses: ./.github/actions/cache", self.job)

    def test_release_never_saves_native_caches(self):
        self.assertNotIn("actions/cache/save@", self.job)
        self.assertNotIn("uses: actions/cache@", self.job)


if __name__ == "__main__":
    unittest.main()
