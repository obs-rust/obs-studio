#!/usr/bin/env -S uv run --script
"""The Linux C/C++ (CMake) builds cache compiles with zccache, not ccache (#957).

    uv run python -m unittest ci/test_zccache.py

soldr already routes rustc through zccache; the CMake builds on Linux (Tier 2
in Docker, the full Ubuntu build, the AppImage and the sanitizers) use the
same cache as their compiler launcher. zccache is pinned to one release,
installed by rust/tools/linux-validate/install-zccache.sh with its SHA-256.

Not covered, on purpose: the Windows job (zccache does not run as a cl.exe
stand-in for the Visual Studio generator; see build-project.yaml), the
Flatpak (flatpak-builder --ccache runs the SDK's own ccache inside the
sandbox) and macOS (Xcode's compilation cache, not ccache).
"""

from __future__ import annotations

import json
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TOOLS = ROOT / "rust" / "tools"
INSTALLER = TOOLS / "linux-validate" / "install-zccache.sh"
WORKFLOWS = ROOT / ".github" / "workflows"
VERSION = "1.15.5"

# Files that run a Linux CMake build and so must name zccache as its launcher
# and never ccache.
LINUX_BUILD_FILES = [
    TOOLS / "linux-validate" / "Dockerfile",
    TOOLS / "linux-validate" / "run.py",
    TOOLS / "linux-validate" / "validate.sh",
    TOOLS / "sanitize" / "sanitize.sh",
    TOOLS / "sanitize" / "shell.nix",
    ROOT / "build-aux" / "appimage" / "build.py",
    ROOT / "bosn.toml",
    ROOT / ".github" / "scripts" / ".build.zsh",
    WORKFLOWS / "sanitizers.yaml",
    WORKFLOWS / "appimage.yaml",
]


def job(workflow: str, name: str) -> str:
    text = (WORKFLOWS / workflow).read_text()
    m = re.search(rf"^  {re.escape(name)}:\n((?:    .*\n|\n)*)", text, re.M)
    assert m, f"{workflow} has no job {name}"
    return m.group(1)


def cache_steps(block: str) -> list[tuple[str, str]]:
    """(action, key) for every actions/cache step in a job."""
    out = []
    # Each step's body is its indented, non-"- " lines (\s+ would run into
    # the next step and hide every later key).
    for m in re.finditer(r"uses: actions/cache/(restore|save)@\S+.*\n((?:[ \t]+(?![ \t]*- ).*\n)*)", block):
        key = re.search(r"^\s+key: (.*)$", m.group(2), re.M)
        out.append((m.group(1), key.group(1) if key else ""))
    return out


class ZccacheTest(unittest.TestCase):
    def test_installer_pins_version_and_checksums(self):
        text = INSTALLER.read_text()
        self.assertIn(f'ZCCACHE_VERSION="{VERSION}"', text)
        # One checksum per Linux runner arch, verified before install.
        self.assertEqual(len(re.findall(r"\b[0-9a-f]{64}\b", text)), 2, text)
        self.assertIn("sha256sum -c", text)

    def test_linux_builds_launch_through_zccache(self):
        for path in LINUX_BUILD_FILES:
            with self.subTest(path=path.relative_to(ROOT)):
                text = path.read_text()
                self.assertNotRegex(text, r"(?i)(?<!z)ccache", "still uses ccache")
        for path in (TOOLS / "linux-validate" / "validate.sh", TOOLS / "sanitize" / "sanitize.sh"):
            text = path.read_text()
            self.assertIn("-DCMAKE_C_COMPILER_LAUNCHER=zccache", text)
            self.assertIn("-DCMAKE_CXX_COMPILER_LAUNCHER=zccache", text)
            # The daemon must flush its index before the cache dir is saved.
            self.assertIn("zccache stop", text)

    def test_docker_image_installs_pinned_zccache(self):
        text = (TOOLS / "linux-validate" / "Dockerfile").read_text()
        self.assertIn("COPY install-zccache.sh", text)
        self.assertRegex(text, r"RUN .*install-zccache\.sh /usr/local/bin")

    def test_ubuntu_ci_preset_uses_zccache(self):
        presets = json.loads((ROOT / "CMakePresets.json").read_text())["configurePresets"]
        ubuntu = next(p for p in presets if p["name"] == "ubuntu-ci")["cacheVariables"]
        self.assertNotIn("ENABLE_CCACHE", ubuntu)
        self.assertEqual(ubuntu.get("CMAKE_C_COMPILER_LAUNCHER"), "zccache")
        self.assertEqual(ubuntu.get("CMAKE_CXX_COMPILER_LAUNCHER"), "zccache")

    def test_cache_keys_name_zccache(self):
        jobs = [
            ("build-project.yaml", "ubuntu-build"),
            ("build-project.yaml", "rust-linux-tier2-build"),
            ("sanitizers.yaml", "sanitize"),
            ("appimage.yaml", "build"),
        ]
        for workflow, name in jobs:
            steps = cache_steps(job(workflow, name))
            compiler = [key for _, key in steps if "cache" in key and not key.startswith(("cargo-", "apt-"))]
            with self.subTest(job=f"{workflow}:{name}"):
                self.assertTrue(compiler, steps)
                for key in compiler:
                    self.assertIn("zccache-", key)
                    self.assertNotRegex(key, r"(?<!z)ccache")

    def test_only_master_pushes_save_ci_compiler_caches(self):
        # zackees/ci.yml cache policy: a PR-ref save is unreachable by other PRs.
        for name in ("ubuntu-build", "rust-linux-tier2-build"):
            block = job("build-project.yaml", name)
            saves = re.findall(r"(?:if: (.*)\n\s+uses: actions/cache/save|uses: actions/cache/save@\S+.*\n\s+if: (.*))", block)
            with self.subTest(job=name):
                self.assertTrue(saves)
                for a, b in saves:
                    self.assertIn("github.ref == 'refs/heads/master'", a or b)

    def test_tier2_lane_setup_step_renamed(self):
        text = (ROOT / "local-gate.toml").read_text()
        self.assertIn("Save Tier 2 zccache 💾", text)
        self.assertIn("name: Save Tier 2 zccache 💾", job("build-project.yaml", "rust-linux-tier2-build"))


if __name__ == "__main__":
    unittest.main()
