#!/usr/bin/env -S uv run --script
"""The Linux C/C++ (CMake) builds cache compiles with zccache, not ccache (#957).

    uv run python -m unittest ci/test_zccache.py

soldr already routes rustc through zccache; the CMake builds on Linux (Tier 2
in Docker, the full Ubuntu build, the AppImage and the sanitizers) use the
same cache as their compiler launcher. zccache is pinned to one release,
installed by rust/tools/linux-validate/install_zccache.py with its SHA-256.

Not covered, on purpose: the Windows job (zccache does not run as a cl.exe
stand-in for the Visual Studio generator; see build-project.yaml), the
Flatpak (flatpak-builder --ccache runs the SDK's own ccache inside the
sandbox) and macOS (Xcode's compilation cache, not ccache).
"""

from __future__ import annotations

import hashlib
import importlib.util
import json
import re
import stat
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TOOLS = ROOT / "rust" / "tools"
LINUX_VALIDATE = TOOLS / "linux-validate"
INSTALLER = LINUX_VALIDATE / "install_zccache.py"
SHELL_SCRIPT = LINUX_VALIDATE / "install-zccache.sh"
WORKFLOWS = ROOT / ".github" / "workflows"
VERSION = "1.15.5"

# Files that run a Linux CMake build and so must name zccache as its launcher
# and never ccache.
LINUX_BUILD_FILES = [
    TOOLS / "linux-validate" / "Dockerfile",
    TOOLS / "linux-validate" / "run.py",
    TOOLS / "linux-validate" / "validate.sh",
    TOOLS / "sanitize" / "sanitize.py",
    TOOLS / "sanitize" / "shell.nix",
    ROOT / "build-aux" / "appimage" / "build.py",
    ROOT / "bosn.toml",
    ROOT / ".github" / "scripts" / ".build.zsh",
    WORKFLOWS / "sanitizers.yaml",
    WORKFLOWS / "appimage.yaml",
]


def load_module():
    """rust/tools/linux-validate/install_zccache.py by path: `uv run python -m
    unittest ci/test_zccache.py` does not put that directory on sys.path, and no
    file in this repo edits sys.path (ci/test_no_sys_path_edits.py)."""
    spec = importlib.util.spec_from_file_location("install_zccache", INSTALLER)
    assert spec is not None and spec.loader is not None, f"cannot load {INSTALLER}"
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def executable(directory: Path, text: str) -> Path:
    """A stand-in for an installed zccache: an executable printing `text`."""
    binary = directory / "zccache"
    binary.write_text("#!/bin/sh\necho " + text)
    binary.chmod(binary.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
    return binary


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
        self.assertIn(f'ZCCACHE_VERSION = "{VERSION}"', text)
        # One checksum per Linux runner arch, verified before install.
        self.assertEqual(len(re.findall(r"\b[0-9a-f]{64}\b", text)), 2, text)
        self.assertIn("hashlib.sha256", text)

    def test_linux_builds_launch_through_zccache(self):
        for path in LINUX_BUILD_FILES:
            with self.subTest(path=path.relative_to(ROOT)):
                text = path.read_text()
                self.assertNotRegex(text, r"(?i)(?<!z)ccache", "still uses ccache")
        for path in (TOOLS / "linux-validate" / "validate.sh", TOOLS / "sanitize" / "sanitize.py"):
            text = path.read_text()
            self.assertIn("-DCMAKE_C_COMPILER_LAUNCHER=zccache", text)
            self.assertIn("-DCMAKE_CXX_COMPILER_LAUNCHER=zccache", text)
            # The daemon must flush its index before the cache dir is saved
            # (`zccache stop` in the shell, ["zccache", "stop"] in Python).
            self.assertRegex(text, r"""["']?zccache["']?[,\s]+["']?stop""")

    def test_docker_image_installs_pinned_zccache(self):
        text = (TOOLS / "linux-validate" / "Dockerfile").read_text()
        self.assertIn("COPY install_zccache.py", text)
        self.assertRegex(text, r"RUN .*install_zccache\.py /usr/local/bin")

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


class PortTest(unittest.TestCase):
    """The ported installer answers offline: pinned version, arch map, digest."""

    def setUp(self):
        self.module = load_module()

    def test_every_linux_runner_arch_maps_to_its_release(self):
        for machine in ("x86_64", "amd64"):
            self.assertEqual(self.module.arch_for(machine), "x86_64")
        for machine in ("aarch64", "arm64"):
            self.assertEqual(self.module.arch_for(machine), "aarch64")

    def test_an_unsupported_arch_fails(self):
        with self.assertRaises(SystemExit) as caught:
            self.module.arch_for("riscv64")
        self.assertIn("unsupported arch riscv64", str(caught.exception))

    def test_each_release_pins_a_sha256(self):
        self.assertEqual(
            set(self.module.SHA256),
            {"x86_64", "aarch64"},
        )
        for digest in self.module.SHA256.values():
            self.assertRegex(digest, r"^[0-9a-f]{64}$")

    def test_the_digest_is_the_archive_sha256(self):
        with tempfile.TemporaryDirectory() as tmp:
            data = Path(tmp) / "release.tar.gz"
            data.write_bytes(b"zccache release bytes")
            # The same hash `sha256sum` would print for those bytes.
            expected = hashlib.sha256(b"zccache release bytes").hexdigest()
            self.assertEqual(self.module.sha256_of(data), expected)

    def test_an_installed_pinned_zccache_is_left_alone(self):
        with tempfile.TemporaryDirectory() as tmp:
            executable(Path(tmp), f"zccache {VERSION}\n")
            self.assertTrue(self.module.installed(Path(tmp)))

    def test_a_stale_or_missing_zccache_reinstalls(self):
        with tempfile.TemporaryDirectory() as tmp:
            executable(Path(tmp), "zccache 1.14.0\n")
            self.assertFalse(self.module.installed(Path(tmp)))
        with tempfile.TemporaryDirectory() as tmp:
            self.assertFalse(self.module.installed(Path(tmp)))

    def test_an_unknown_version_line_does_not_count(self):
        with tempfile.TemporaryDirectory() as tmp:
            executable(Path(tmp), "grep: zccache 1.15.5-extra\n")
            self.assertFalse(self.module.installed(Path(tmp)))


class WiringTest(unittest.TestCase):
    """The shell installer is gone; every caller runs the Python port (#962)."""

    def test_the_shell_script_is_ported(self):
        self.assertFalse(SHELL_SCRIPT.exists(), "install-zccache.sh is the ported one and must go")

    def test_no_gen_005_exception_is_left(self):
        ci_toml = (ROOT / "ci.toml").read_text(encoding="utf-8")
        self.assertNotIn(
            "rust/tools/linux-validate/install-zccache.sh",
            ci_toml,
            "the GEN-005 exception for install-zccache.sh is still in ci.toml",
        )

    def test_the_linux_validate_dockerfile_runs_the_python(self):
        text = (LINUX_VALIDATE / "Dockerfile").read_text()
        # assertTrue with a short message: assertIn would echo the whole file.
        self.assertTrue(
            "install_zccache.py" in text and "install-zccache.sh" not in text,
            "the Tier 2 Dockerfile does not run install_zccache.py",
        )

    def test_the_manylinux_dockerfile_runs_the_python(self):
        text = (ROOT / "build-aux" / "appimage" / "manylinux" / "Dockerfile").read_text()
        self.assertTrue(
            "install_zccache.py /usr/local/bin" in text and "install-zccache.sh" not in text,
            "the manylinux Dockerfile does not run install_zccache.py",
        )

    def test_the_dockerignore_lists_the_python(self):
        text = (ROOT / ".dockerignore").read_text()
        self.assertIn("!rust/tools/linux-validate/install_zccache.py", text)

    def test_setup_zccache_runs_the_python_through_uv(self):
        text = (ROOT / ".github" / "scripts" / "utils.zsh" / "setup_zccache").read_text()
        self.assertTrue(
            "uv run --no-project" in text
            and "rust/tools/linux-validate/install_zccache.py" in text
            and "install-zccache.sh" not in text,
            "setup_zccache does not run the installer through uv",
        )

    def test_the_build_workflow_runs_the_python_through_uv(self):
        text = (WORKFLOWS / "build-project.yaml").read_text()
        self.assertEqual(
            text.count("uv run --no-project rust/tools/linux-validate/install_zccache.py"),
            2,
            "build-project.yaml must run the installer through uv at both sites",
        )
        self.assertNotIn("install-zccache.sh", text)

    def test_the_sanitizers_run_the_python_through_uv(self):
        text = (TOOLS / "sanitize" / "sanitize.py").read_text()
        self.assertTrue(
            '"uv", "run", "--no-project"' in text
            and "install_zccache.py" in text
            and "install-zccache.sh" not in text,
            "sanitize.py does not run the installer through uv",
        )


if __name__ == "__main__":
    unittest.main()
