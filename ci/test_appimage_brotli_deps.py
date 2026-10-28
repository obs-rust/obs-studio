#!/usr/bin/env -S uv run --script
"""The AppImage's libcurl links the pinned brotli, not the base's 1.0.6 (#2782).

    uv run python -m unittest ci.test_appimage_brotli_deps

The image's libcurl is now the pinned one (`deps.lock.json`, fetched from
obs-studio-libs), not a rebuild from AlmaLinux's source RPM, so the brotli it
decodes with comes from the pinned 1.2.1 prefix rather than the base's 1.0.6 —
inside CVE-2026-111497's range (obs-rust/obs-studio#2782, #2774). Two things
make that true, and both are easy to lose in a Dockerfile edit:

- the artifact is in `deps.lock.json` and on the image's CMake, pkg-config and
  loader paths, so the shipped `libbrotlidec.so.1` is the pinned 1.2.1;
- the libcurl that needs it is itself locked (`ci/test_appimage_curl_deps.py`
  pins that half), so no build in the image can resolve brotli from the base.

The base stays glibc 2.28 (#926): this is a dependency substitution, not a base
bump.
"""

from __future__ import annotations

import json
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MANYLINUX = ROOT / "build-aux/appimage/manylinux"
PREFIX = "/opt/obs-deps/brotli"
FETCH = "obs-studio-libs fetch"


def dockerfile() -> str:
    return (MANYLINUX / "Dockerfile").read_text()


def image_env(name: str) -> list[str]:
    match = re.search(rf"^ENV {name}=(\S+)$", dockerfile(), re.MULTILINE)
    assert match, f"Dockerfile sets no {name}"
    return match.group(1).split(":")


def lock_entry() -> dict[str, object]:
    lock = json.loads((MANYLINUX / "deps.lock.json").read_text())
    entries = [t for t in lock["tools"] if t["tool"] == "brotli"]
    assert len(entries) == 1, "deps.lock.json has no single brotli entry"
    return entries[0]


class BrotliDepsTest(unittest.TestCase):
    def test_brotli_is_locked_past_the_cve(self):
        entry = lock_entry()
        self.assertEqual(entry["install"], PREFIX)
        # CVE-2026-111497 covers the 1.0.x line; 1.2.0 is the first safe release.
        major, minor = (int(part) for part in str(entry["version"]).split(".")[:2])
        self.assertGreaterEqual((major, minor), (1, 2))
        self.assertRegex(str(entry["sha256"]), r"^[0-9a-f]{64}$")

    def test_brotli_is_on_the_image_paths(self):
        self.assertIn(PREFIX, image_env("CMAKE_PREFIX_PATH"))
        self.assertIn(f"{PREFIX}/lib/pkgconfig", image_env("PKG_CONFIG_PATH"))
        # The AppImage bundles libbrotli*.so.1 from the loader path, like the others.
        self.assertIn(f"{PREFIX}/lib", image_env("LD_LIBRARY_PATH"))

    def test_the_dependencies_come_from_the_lock(self):
        self.assertIn(FETCH, dockerfile())
        # libcurl is prebuilt too, so nothing in the image rebuilds it against
        # the base's brotli 1.0.6.
        self.assertNotIn("rpmbuild", dockerfile())


if __name__ == "__main__":
    unittest.main()
