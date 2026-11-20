#!/usr/bin/env -S uv run --script
"""CMake's soldr pin (cmake/common/soldr.cmake) matches CI's.

cmake/common/soldr.cmake downloads soldr v${OBS_SOLDR_VERSION} when soldr is
not on PATH (the cross-compile check, cmake/cross/). It must be the version
every zackees/setup-soldr step in .github/workflows/ installs, with a SHA256
for each host it can download for.

    uv run python -m unittest ci/test_soldr_pin.py
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SOLDR_CMAKE = ROOT / "cmake/common/soldr.cmake"
HOSTS = {
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
    "aarch64-pc-windows-msvc",
}


def cmake_pin() -> str:
    m = re.search(r"^set\(OBS_SOLDR_VERSION ([0-9.]+)\)$", SOLDR_CMAKE.read_text(), re.M)
    assert m, "OBS_SOLDR_VERSION not found"
    return m.group(1)


def workflow_pins() -> dict[str, set[str]]:
    """version: under each zackees/setup-soldr step, per workflow file."""
    pins: dict[str, set[str]] = {}
    for wf in sorted((ROOT / ".github/workflows").glob("*.y*ml")):
        lines = wf.read_text().splitlines()
        for i, line in enumerate(lines):
            if "uses: zackees/setup-soldr@" not in line:
                continue
            # The step's keys (uses:, with:) share this indent; the step ends at
            # a shallower line (the next "- " step or the next job).
            indent = len(line) - len(line.lstrip())
            version = "implicit action default"
            for follow in lines[i + 1 :]:
                if follow.strip() and len(follow) - len(follow.lstrip()) < indent:
                    break
                m = re.match(r"\s+version:\s*['\"]?([0-9.]+)", follow)
                if m:
                    version = m.group(1)
            pins.setdefault(wf.name, set()).add(version)
    return pins


class SoldrPinTest(unittest.TestCase):
    def test_linux_hosted_darwin_toolchain_is_available(self):
        self.assertGreaterEqual(
            tuple(map(int, cmake_pin().split("."))), (0, 9, 54),
            "macOS run-only preparation requires soldr toolchain darwin (soldr#4168)",
        )

    def test_source_mtimes_keep_nanosecond_precision(self):
        self.assertGreaterEqual(
            tuple(map(int, cmake_pin().split("."))), (0, 9, 38),
            "Older Soldr source manifests truncate mtimes to milliseconds (#1359)",
        )

    def test_cmake_pin_matches_every_setup_soldr_step(self):
        pins = workflow_pins()
        self.assertTrue(pins, "no setup-soldr version: pins found")
        for wf, versions in pins.items():
            with self.subTest(workflow=wf):
                self.assertEqual(versions, {cmake_pin()})

    def test_every_downloadable_host_has_a_sha256(self):
        text = SOLDR_CMAKE.read_text()
        hashes = dict(re.findall(r"^set\(_obs_soldr_sha256_(\S+) ([0-9a-f]+)\)$", text, re.M))
        self.assertEqual(set(hashes), HOSTS)
        for host, digest in hashes.items():
            with self.subTest(host=host):
                self.assertRegex(digest, r"^[0-9a-f]{64}$")


if __name__ == "__main__":
    unittest.main()
