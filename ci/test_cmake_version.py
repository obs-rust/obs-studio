"""Regression coverage for OBS version detection in tagged and tagless checkouts."""

from __future__ import annotations

import re
import subprocess
import tempfile
import unittest
from pathlib import Path

# The host's git config must not change what the test repo does: tag or
# commit signing, or a missing identity, would make git tag/commit fail.
GIT_ISOLATION = ("-c", "tag.gpgSign=false", "-c", "commit.gpgSign=false", "-c", "user.name=t", "-c", "user.email=t@t")


VERSIONCONFIG = Path(__file__).resolve().parents[1] / "cmake/common/versionconfig.cmake"
OBS_CONFIG_H = Path(__file__).resolve().parents[1] / "libobs/obs-config.h"


def libobs_api_major() -> int:
    match = re.search(r"(?m)^#define LIBOBS_API_MAJOR_VER (\d+)$", OBS_CONFIG_H.read_text(encoding="utf-8"))
    assert match, "LIBOBS_API_MAJOR_VER not found in libobs/obs-config.h"
    return int(match.group(1))


class VersionConfigTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="obs-versionconfig-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.git("init", "-q")
        self.git("config", "user.email", "test@example.invalid")
        self.git("config", "user.name", "Test")
        (self.root / "marker.txt").write_text("first\n", encoding="utf-8")
        self.git("add", "marker.txt")
        self.git("commit", "-qm", "initial commit")
        self.driver = self.root / "probe.cmake"
        self.driver.write_text(
            'set(_obs_default_version "0" "0" "1")\n'
            'set(_obs_release_candidate 0)\n'
            'set(_obs_beta 0)\n'
            'include("${VERSIONCONFIG}")\n'
            'message(STATUS "OBS_TEST_VERSION=${OBS_VERSION}")\n'
            'message(STATUS "OBS_TEST_CANONICAL=${OBS_VERSION_CANONICAL}")\n',
            encoding="utf-8",
        )

    def git(self, *arguments: str) -> None:
        subprocess.run(["git", *GIT_ISOLATION, *arguments], cwd=self.root, check=True, capture_output=True)

    def probe(self, override: str | None = None) -> subprocess.CompletedProcess[str]:
        command = ["cmake", f"-DVERSIONCONFIG={VERSIONCONFIG}"]
        if override is not None:
            command.append(f"-DOBS_VERSION_OVERRIDE={override}")
        command.extend(["-P", str(self.driver)])
        return subprocess.run(command, cwd=self.root, text=True, capture_output=True)

    def assert_versions(self, result: subprocess.CompletedProcess[str], *, version: str, canonical: str) -> None:
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn(f"OBS_TEST_VERSION={version}", result.stdout + result.stderr)
        self.assertIn(f"OBS_TEST_CANONICAL={canonical}", result.stdout + result.stderr)

    def test_tagless_checkout_uses_default_version(self) -> None:
        self.assert_versions(self.probe(), version="0.0.1", canonical="0.0.1")

    def test_non_version_tag_uses_default_version(self) -> None:
        self.git("tag", "not-a-version")
        self.assert_versions(self.probe(), version="0.0.1", canonical="0.0.1")

    def test_release_tag_is_preserved(self) -> None:
        self.git("tag", "32.0.2")
        self.assert_versions(self.probe(), version="32.0.2", canonical="32.0.2")

    def test_commit_after_release_tag_keeps_canonical_version(self) -> None:
        self.git("tag", "32.0.2")
        (self.root / "marker.txt").write_text("second\n", encoding="utf-8")
        self.git("commit", "-qam", "subsequent commit")
        result = self.probe()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        output = result.stdout + result.stderr
        self.assertIn("OBS_TEST_CANONICAL=32.0.2", output)
        self.assertRegex(output, r"OBS_TEST_VERSION=32\.0\.2-1-g[0-9a-f]+")

    def test_explicit_version_override_works_without_tags(self) -> None:
        self.assert_versions(self.probe("33.0.1-rc2"), version="33.0.1-rc2", canonical="33.0.1")

    def test_invalid_version_override_is_still_rejected(self) -> None:
        result = self.probe("not-a-version")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Invalid version supplied", result.stdout + result.stderr)


    # OBS_VERSION_OVERRIDE is the OBS version libobs reports to plugins
    # (obs_get_version_string()); plugins such as DistroAV refuse to load when it
    # is older than the OBS they need. The fork's own version goes in
    # OBS_RUST_VERSION (#1087).
    def assert_fork_version_rejected(self, override: str) -> None:
        result = self.probe(override)
        output = result.stdout + result.stderr
        self.assertNotEqual(result.returncode, 0, f"OBS_VERSION_OVERRIDE={override} was accepted:\n{output}")
        self.assertIn("OBS_RUST_VERSION", output)
        self.assertIn(f"LIBOBS_API_MAJOR_VER {libobs_api_major()}", output)

    def test_fork_release_version_override_is_rejected(self) -> None:
        self.assert_fork_version_rejected("0.1.0-beta.20261014")

    def test_fork_git_package_version_override_is_rejected(self) -> None:
        self.assert_fork_version_rejected("0.0.0.r1240.g2f39af945")

    def test_override_older_than_libobs_api_major_is_rejected(self) -> None:
        self.assert_fork_version_rejected(f"{libobs_api_major() - 1}.2.2")

    def test_override_matching_libobs_api_major_is_accepted(self) -> None:
        major = libobs_api_major()
        self.assert_versions(self.probe(f"{major}.0.0"), version=f"{major}.0.0", canonical=f"{major}.0.0")
        self.assert_versions(
            self.probe(f"{major}.0.0-beta6"), version=f"{major}.0.0-beta6", canonical=f"{major}.0.0"
        )

    def test_override_newer_than_libobs_api_major_is_accepted(self) -> None:
        major = libobs_api_major() + 1
        self.assert_versions(self.probe(f"{major}.1.0"), version=f"{major}.1.0", canonical=f"{major}.1.0")


if __name__ == "__main__":
    unittest.main()
