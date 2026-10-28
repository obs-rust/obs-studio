#!/usr/bin/env -S uv run --script
"""The AppImage links a Qt Network with no GSSAPI, so no krb5 and no OpenSSL 1.1.1k.

    uv run python -m unittest ci.test_appimage_qt6_gssapi_deps

`libQt6Network.so.6` NEEDed `libgssapi_krb5.so.2` only because qtbase was built
with the GSSAPI feature on; the manylinux_2_28 base's krb5 libraries NEED
`libcrypto.so.1.1`, which put the base's EOL OpenSSL 1.1.1k back into the bundle
next to the pinned 3.5.9 (obs-rust/obs-studio#2815). OBS makes no
SPNEGO/Negotiate request, so the `qt6` recipe in obs-studio-libs now configures
qtbase with `-DFEATURE_gssapi=OFF` and republishes the Linux build under the
`r1` label.

Two things make that true, and both are easy to lose in an edit:

- the lock pins that rebuild (the `r1` flavor, not the published original), and
  the artifact it names is installed at /opt/obs-deps/qt6, so the Qt the OBS
  build links and the AppImage bundles is the GSSAPI-free one;
- that prefix is on the image's CMake, pkg-config and loader paths.

The base stays glibc 2.28 (#926): this is dependency substitution, not a bump.
"""

from __future__ import annotations

import json
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MANYLINUX = ROOT / "build-aux/appimage/manylinux"
PREFIX = "/opt/obs-deps/qt6"
FETCH = "obs-studio-libs fetch"

# The published Linux build of this version (no flavor) still carries GSSAPI;
# the r1 rebuild beside it is the one without.
GSSAPI_FREE_REBUILD = "r1"


def dockerfile() -> str:
    return (MANYLINUX / "Dockerfile").read_text()


def image_env(name: str) -> list[str]:
    match = re.search(rf"^ENV {name}=(\S+)$", dockerfile(), re.MULTILINE)
    assert match, f"Dockerfile sets no {name}"
    return match.group(1).split(":")


def lock_entry() -> dict[str, object]:
    lock = json.loads((MANYLINUX / "deps.lock.json").read_text())
    entries = [t for t in lock["tools"] if t["tool"] == "qt6"]
    assert len(entries) == 1, "deps.lock.json has no single qt6 entry"
    return entries[0]


class Qt6GssapiDepsTest(unittest.TestCase):
    def test_the_lock_pins_the_gssapi_free_rebuild(self):
        entry = lock_entry()
        self.assertEqual(entry["install"], PREFIX)
        variant = entry["variant"]
        assert isinstance(variant, dict), "the qt6 entry pins no variant"
        self.assertEqual(variant.get("flavor"), GSSAPI_FREE_REBUILD)
        self.assertRegex(str(entry["sha256"]), r"^[0-9a-f]{64}$")

    def test_qt6_is_on_the_image_paths(self):
        self.assertIn(PREFIX, image_env("CMAKE_PREFIX_PATH"))
        self.assertIn(f"{PREFIX}/lib/pkgconfig", image_env("PKG_CONFIG_PATH"))
        # The AppImage bundles libQt6*.so.6 from the loader path, like the others.
        self.assertIn(f"{PREFIX}/lib", image_env("LD_LIBRARY_PATH"))

    def test_the_dependencies_come_from_the_lock(self):
        self.assertIn(FETCH, dockerfile())
        # The base's krb5 stack is what dragged the 1.1.1k in, so nothing in the
        # image may install or rebuild krb5 for Qt to link against.
        self.assertNotIn("krb5-devel", dockerfile())


if __name__ == "__main__":
    unittest.main()
