#!/usr/bin/env -S uv run --script
"""The AppImage image has libvpl for the qsv build (obs-rust/obs-studio#1863).

    uv run python -m unittest ci.test_appimage_qsv_deps

appimage.yaml's qsv input builds obs-qsv11, whose CMakeLists needs VPL 2.9.
The manylinux image (AlmaLinux 8) has no oneVPL package, so libvpl comes from
obs-rust/obs-studio-libs like the other prebuilt libraries: it must be in
deps.lock.json and on the image's CMake and loader paths, or the qsv build
fails to configure ("Could NOT find VPL", run 39899437010).
"""

from __future__ import annotations

import json
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MANYLINUX = ROOT / "build-aux/appimage/manylinux"
PREFIX = "/opt/obs-deps/libvpl"


def image_env(name: str) -> list[str]:
    dockerfile = (MANYLINUX / "Dockerfile").read_text()
    match = re.search(rf"^ENV {name}=(\S+)$", dockerfile, re.MULTILINE)
    assert match, f"Dockerfile sets no {name}"
    return match.group(1).split(":")


class QsvDepsTest(unittest.TestCase):
    def test_libvpl_is_locked(self):
        lock = json.loads((MANYLINUX / "deps.lock.json").read_text())
        entries = [t for t in lock["tools"] if t["tool"] == "libvpl"]
        self.assertEqual(len(entries), 1, "deps.lock.json has no libvpl")
        entry = entries[0]
        self.assertEqual(entry["install"], PREFIX)
        # obs-qsv11: find_package(VPL 2.9 REQUIRED).
        major, minor = (int(part) for part in entry["version"].split(".")[:2])
        self.assertGreaterEqual((major, minor), (2, 9))
        self.assertRegex(entry["sha256"], r"^[0-9a-f]{64}$")

    def test_libvpl_is_on_the_image_paths(self):
        self.assertIn(PREFIX, image_env("CMAKE_PREFIX_PATH"))
        # The AppImage bundles libvpl.so.2 from the loader path, like the others.
        self.assertIn(f"{PREFIX}/lib", image_env("LD_LIBRARY_PATH"))


if __name__ == "__main__":
    unittest.main()
