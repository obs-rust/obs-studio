#!/usr/bin/env -S uv run --script
"""The AppImage's libcurl links the pinned brotli, not the base's 1.0.6 (#2782).

    uv run python -m unittest ci.test_appimage_brotli_deps

`build-aux/appimage/manylinux/Dockerfile` rebuilds libcurl from AlmaLinux's curl
source RPM so HTTPS works without `/etc/pki/tls/certs/ca-bundle.crt` (#1085).
That build resolves brotli from the base image, 1.0.6 — inside
CVE-2026-111497's range — so the shipped `libbrotlidec` has to come from the
pinned obs-studio-libs brotli instead (1.2.1, `brotli-1.2.1` release). Two
things make that true, and both are easy to lose in a Dockerfile edit:

- the artifact is in `deps.lock.json` and on the image's CMake, pkg-config and
  loader paths, so `pkg-config --modversion libbrotlidec` in the image is 1.2.1;
- the dependency fetch runs **before** the RPM rebuild, because `rpmbuild` only
  sees the prefix through the environment it is given. Fetched after, curl links
  the base's 1.0.6 again with no error (#2774).

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
RPM_BUILD = "rpmbuild -bb"


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

    def test_dependencies_are_fetched_before_the_curl_rebuild(self):
        text = dockerfile()
        self.assertLess(
            text.index(FETCH),
            text.index(RPM_BUILD),
            "the prebuilt dependencies are fetched after the libcurl RPM rebuild",
        )

    def test_curl_rebuild_sees_the_brotli_prefix(self):
        text = dockerfile()
        start = text.index("dnf download -q --source")
        rpm = text[start : text.index("\n\n", text.index(RPM_BUILD))]
        self.assertIn(PREFIX, rpm, "the libcurl rebuild is not given the brotli prefix")


if __name__ == "__main__":
    unittest.main()
