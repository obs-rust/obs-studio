#!/usr/bin/env -S uv run --script
"""The daily beta identifies itself as OBS-Studio-Rust <full version> (#631).

    uv run python -m unittest ci/test_release_identity.py

The release string (0.1.0-beta.20261013) is set once, by release-rust.yaml's
build job (from the check job's version), and reaches the log banner and
About, the macOS bundle and DMG volume, the Windows VERSIONINFO and the
AppImage metainfo; every platform's packaged artifact is then checked against
it. These tests cover the CMake plumbing and that each check is wired in.
"""

from __future__ import annotations

import os
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
VERSIONCONFIG = ROOT / "cmake" / "common" / "versionconfig.cmake"


def text(rel: str) -> str:
    return (ROOT / rel).read_text(encoding="utf-8")


class ReleaseVersionCMake(unittest.TestCase):
    def probe(self, env_value: str | None = None, define: str | None = None):
        with tempfile.TemporaryDirectory(prefix="release-identity-") as d:
            driver = Path(d) / "driver.cmake"
            driver.write_text(
                "set(_obs_default_version 0 0 1)\n"
                f'include("{VERSIONCONFIG.as_posix()}")\n'
                'message(STATUS "RELEASE=[${OBS_RUST_RELEASE_VERSION}]")\n',
                encoding="utf-8",
            )
            env = {k: v for k, v in os.environ.items() if k != "OBS_RUST_VERSION"}
            if env_value is not None:
                env["OBS_RUST_VERSION"] = env_value
            args = ["cmake", *([f"-DOBS_RUST_VERSION={define}"] if define else []), "-P", str(driver)]
            return subprocess.run(args, cwd=d, env=env, capture_output=True, text=True)

    def release(self, **kw) -> str:
        out = self.probe(**kw)
        self.assertEqual(out.returncode, 0, out.stderr)
        for line in (out.stdout + out.stderr).splitlines():
            if "RELEASE=[" in line:
                return line.split("RELEASE=[", 1)[1].rstrip("]")
        self.fail(out.stdout + out.stderr)

    def test_env_sets_release_version(self):
        self.assertEqual(self.release(env_value="0.1.0-beta.20261013"), "0.1.0-beta.20261013")

    def test_define_sets_release_version(self):
        self.assertEqual(self.release(define="0.2.0-beta.20270101"), "0.2.0-beta.20270101")

    def test_unset_is_empty_so_local_builds_use_the_crate_version(self):
        self.assertEqual(self.release(), "")

    def test_rejects_junk(self):
        self.assertNotEqual(self.probe(env_value="nonsense; rm -rf").returncode, 0)


class CrateVersion(unittest.TestCase):
    def test_crate_is_the_1_0_release(self):
        cargo = text("rust/libobs-rust/Cargo.toml")
        self.assertIn('\nversion = "1.0.0"\n', cargo)
        lock = text("Cargo.lock")
        self.assertIn('name = "libobs-rust"\nversion = "1.0.0"\n', lock)


class Wiring(unittest.TestCase):
    def setUp(self):
        self.release = text(".github/workflows/release-rust.yaml")

    def test_build_job_sets_the_version_from_check(self):
        self.assertIn("OBS_RUST_VERSION: ${{ needs.check.outputs.version }}", self.release)

    def test_linux_checks_the_full_string(self):
        self.assertIn('want="OBS-Studio-Rust $VERSION (based on OBS "', self.release)

    def test_macos_checks_the_dmg(self):
        self.assertIn("CFBundleShortVersionString", self.release)
        self.assertIn("CFBundleVersion", self.release)
        self.assertIn("hdiutil attach", self.release)

    def test_windows_zip_check_gets_the_version(self):
        self.assertIn("--expect-version", text(".github/actions/package-obs/action.yaml"))

    def test_appimage_gets_and_checks_the_version(self):
        wf = text(".github/workflows/appimage.yaml")
        self.assertIn("rust_version", wf)
        self.assertIn("EXPECT_RUST_VERSION", wf)
        self.assertIn("EXPECT_RUST_VERSION", text("build-aux/appimage/test-appimage.sh"))
        self.assertIn("rust_version: ${{ needs.check.outputs.version }}", self.release)

    def test_resources_use_the_release_version(self):
        self.assertIn("obs_bundle_versions", text("cmake/macos/helpers.cmake"))
        self.assertIn("OBS_RUST_RELEASE_VERSION", text("cmake/macos/bundle-version.cmake"))
        self.assertIn("OBS_RC_PRODUCT_VERSION", text("frontend/cmake/windows/obs.rc.in"))
        self.assertIn("OBS_RUST_VERSION", text("frontend/cmake/os-linux.cmake"))
        self.assertIn("OBS_RUST_VERSION", text(".github/scripts/.package.zsh"))


if __name__ == "__main__":
    unittest.main()
