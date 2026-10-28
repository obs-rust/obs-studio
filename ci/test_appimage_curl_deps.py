#!/usr/bin/env -S uv run --script
"""The AppImage ships the pinned curl 8.23.0 + OpenSSL 3.5.9, not the base's.

    uv run python -m unittest ci.test_appimage_curl_deps

The glibc-2.28 base (manylinux_2_28) supplies libcurl 7.61.1 (2018, EOL) and
OpenSSL 1.1.1k (EOL). `build-aux/appimage/manylinux/Dockerfile` used to rebuild
curl from AlmaLinux's source RPM to get `--with-ca-fallback` (#1085); it now
consumes the pinned obs-studio-libs artifacts instead, so the whole libcurl —
TLS included, because curl 8.18+ refuses OpenSSL < 3.0 — comes from
`deps.lock.json` (obs-rust/obs-studio#2774). Three things make that true and are
easy to lose in a Dockerfile edit:

- the artifacts are in the lock and installed at /opt/obs-deps/{curl,openssl};
- those prefixes are on the image's CMake, pkg-config and loader paths, so the
  OBS build's `find_package(CURL)` and the bundled `libcurl.so.4` are the
  pinned ones;
- nothing rebuilds curl from the base any more (no `rpmbuild`/source RPM), and
  the image asserts the pinned version and the absent RHEL CA path.

The base stays glibc 2.28 (#926): this is dependency substitution, not a bump.
"""

from __future__ import annotations

import json
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MANYLINUX = ROOT / "build-aux/appimage/manylinux"
CURL_PREFIX = "/opt/obs-deps/curl"
OPENSSL_PREFIX = "/opt/obs-deps/openssl"

# The last curl that still accepted OpenSSL 1.1.1; 8.18+ requires 3.0.
CURL_WITH_OPENSSL_1_1_1_THROUGH = (8, 17, 99)


def dockerfile() -> str:
    return (MANYLINUX / "Dockerfile").read_text()


def image_env(name: str) -> list[str]:
    match = re.search(rf"^ENV {name}=(\S+)$", dockerfile(), re.MULTILINE)
    assert match, f"Dockerfile sets no {name}"
    return match.group(1).split(":")


def lock_entry(tool: str) -> dict[str, object]:
    lock = json.loads((MANYLINUX / "deps.lock.json").read_text())
    entries = [t for t in lock["tools"] if t["tool"] == tool]
    assert len(entries) == 1, f"deps.lock.json has no single {tool} entry"
    return entries[0]


def version_of(tool: str) -> tuple[int, ...]:
    return tuple(int(part) for part in str(lock_entry(tool)["version"]).split("."))


class CurlDepsTest(unittest.TestCase):
    def test_curl_is_locked_past_the_eol_7_x_line(self):
        entry = lock_entry("curl")
        self.assertEqual(entry["install"], CURL_PREFIX)
        self.assertGreater(version_of("curl"), (7, 99, 99))
        self.assertRegex(str(entry["sha256"]), r"^[0-9a-f]{64}$")

    def test_openssl_is_locked_new_enough_for_that_curl(self):
        entry = lock_entry("openssl")
        self.assertEqual(entry["install"], OPENSSL_PREFIX)
        if version_of("curl") > CURL_WITH_OPENSSL_1_1_1_THROUGH:
            self.assertGreaterEqual(version_of("openssl"), (3, 0, 0))
        self.assertRegex(str(entry["sha256"]), r"^[0-9a-f]{64}$")

    def test_both_prefixes_are_on_the_image_paths(self):
        # CMake's find_package(CURL) and the -lcurl link follow these.
        self.assertEqual(image_env("CMAKE_PREFIX_PATH")[0], CURL_PREFIX)
        self.assertEqual(image_env("PKG_CONFIG_PATH")[0], f"{CURL_PREFIX}/lib/pkgconfig")
        # The tool has to match the library: the base's 7.61.1 curl passes option
        # numbers a libcurl 8 refuses (`curl: (48)`), which silently broke the
        # image's rustup install when only the library was swapped.
        self.assertEqual(image_env("PATH")[0], f"{CURL_PREFIX}/bin")
        loader = image_env("LD_LIBRARY_PATH")
        # libcurl's NEEDED libssl.so.3/libcrypto.so.3 live in the openssl prefix.
        self.assertEqual(loader[0], f"{OPENSSL_PREFIX}/lib")
        self.assertEqual(loader[1], f"{CURL_PREFIX}/lib")

    def test_the_base_curl_is_no_longer_rebuilt(self):
        text = dockerfile()
        for marker in ("rpmbuild -bb", "dnf download -q --source", "curl.spec"):
            self.assertNotIn(marker, text, "the base's curl is still rebuilt")

    def test_the_image_asserts_the_pinned_build(self):
        text = dockerfile()
        # A lock/path edit that falls back to the base must fail the build.
        self.assertIn('pkg-config --modversion libcurl)" = "', text)
        self.assertIn("/etc/pki/tls/certs/ca-bundle.crt", text)
        self.assertIn("libssl.so.3", text)


if __name__ == "__main__":
    unittest.main()
