#!/usr/bin/env -S uv run --script
"""build-aux/appimage/build.py runs the workflow's AppImage build steps (#899).

    uv run python -m unittest ci/test_appimage_build_script.py

- The scripts are the workflow's former inline `run:` blocks, byte for byte
  (checked against the last workflow that had them, when git has that commit).
- The workflow calls every step, in order, and passes every input.
- --dry-run prints the plan; --skip-deps drops only the dependency install.
- version + configure, run against a stub cmake, pass the flags through.
"""

from __future__ import annotations

import importlib.util
import os
import re
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCRIPT = ROOT / "build-aux/appimage/build.py"
WORKFLOW = ROOT / ".github/workflows/appimage.yaml"
# origin/master just before the move: the last appimage.yaml with inline scripts.
BEFORE = "2c38ec153b622ec6873b90b8ebe5c359ff3d4129"
OLD_STEPS = {
    "deps": "Install build dependencies",
    "libavutil": "Build libavutil with host VA compatibility",
    "version": "Determine version",
    "configure": "Configure",
    "build": "Build 🧱",
    "appdir": "Assemble AppDir",
    "appimage": "Create AppImage",
}

TAGLESS_COMMENT = (
    "# It keeps the libobs API version: plugins compare the OBS version against the one\n"
    "# they need, and CMake rejects an override older than the libobs API major (#1087).\n"
)
TAGLESS_FALLBACK = (
    "  api=\"$(sed -n 's/^#define LIBOBS_API_\\(MAJOR\\|MINOR\\|PATCH\\)_VER \\([0-9]*\\)$/\\2/p' "
    "libobs/obs-config.h | paste -sd.)\"\n"
    '  echo "OBS_VERSION=${api}-g${GITHUB_SHA::9}" >> "$GITHUB_ENV"\n'
)

spec = importlib.util.spec_from_file_location("appimage_build", SCRIPT)
build = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = build
spec.loader.exec_module(build)


def run_script(*args: str, env: dict[str, str] | None = None) -> subprocess.CompletedProcess:
    return subprocess.run(["uv", "run", "--no-project", str(SCRIPT), *args],
                          capture_output=True, text=True, env=env, timeout=120)


def old_run_block(workflow: str, name: str) -> str:
    block = workflow.split(f"      - name: {name}", 1)[1].split("\n      - ", 1)[0]
    return textwrap.dedent(block.split("        run: |\n", 1)[1])


class BuildScriptTest(unittest.TestCase):
    def test_scripts_are_the_former_inline_workflow_steps(self):
        old = subprocess.run(["git", "show", f"{BEFORE}:.github/workflows/appimage.yaml"],
                             cwd=ROOT, capture_output=True, text=True)
        if old.returncode:
            self.skipTest(f"{BEFORE} not in this clone")
        args = build.parse_args(["--sha", "0" * 40])
        for step in build.plan(args):
            with self.subTest(step=step.name):
                expected = old_run_block(old.stdout, OLD_STEPS[step.name])
                # The only expression in a script became an environment variable.
                expected = expected.replace("${{ inputs.publish_continuous }}", "${PUBLISH_CONTINUOUS}")
                # The libavutil the AppDir gets is per base (#926).
                expected = expected.replace("avutil-compat/libavutil.so.60", '"${AVUTIL_LIB}"')
                # Where appimagetool comes from is per base too (#926).
                expected = expected.replace(
                    '"https://github.com/AppImage/appimagetool/releases/download/${APPIMAGETOOL_VERSION}/appimagetool-x86_64.AppImage"',
                    '"${APPIMAGETOOL_URL}"')
                # A tagless checkout keeps the libobs API version (#1087).
                expected = expected.replace(
                    'commit hash, which CMake rejects as a project VERSION, so use a dev version then.\n',
                    'commit hash, which CMake rejects as a project VERSION, so use a dev version then.\n'
                    + TAGLESS_COMMENT)
                expected = expected.replace(
                    '  echo "OBS_VERSION=0.0.0-g${GITHUB_SHA::9}" >> "$GITHUB_ENV"\n', TAGLESS_FALLBACK)
                self.assertEqual(step.script.strip("\n"), expected.strip("\n"))

    def test_workflow_calls_every_step_in_order(self):
        called = []
        for line in WORKFLOW.read_text().splitlines():
            if "build-aux/appimage/build.py " in line and "--steps " in line:
                called += line.split("--steps ", 1)[1].split()[0].split(",")
        # The workflow builds on the glibc 2.28 image, which has the dependencies (#926).
        self.assertIn("--base manylinux --steps", WORKFLOW.read_text())
        self.assertEqual(called, [s for s in build.STEP_NAMES if s not in build.MANYLINUX_SKIPPED])

    def test_workflow_passes_every_input(self):
        text = WORKFLOW.read_text()
        for flag, var in [("--rust-libobs", "RUST_LIBOBS"), ("--qsv", "QSV"),
                          ("--rust-version", "OBS_RUST_VERSION"), ("--publish-continuous", "PUBLISH_CONTINUOUS")]:
            with self.subTest(flag=flag):
                self.assertIn(f'{flag} "${var}"', text)
        self.assertIn("PUBLISH_CONTINUOUS: ${{ inputs.publish_continuous == true }}", text)

    def test_dry_run_prints_the_plan(self):
        result = run_script("--dry-run", "--sha", "abc")
        self.assertEqual(result.returncode, 0, result.stderr)
        headers = [l for l in result.stdout.splitlines() if l.startswith("==> ")]
        self.assertEqual([h.split("] ", 1)[1].split(":")[0] for h in headers], list(build.STEP_NAMES))
        self.assertIn("cmake --preset ubuntu-ci", result.stdout)
        self.assertIn("env RUST_LIBOBS=true", result.stdout)
        self.assertIn(f"env APPIMAGETOOL_SHA256={build.APPIMAGETOOL_SHA256}", result.stdout)

    def test_skip_deps_drops_only_the_install(self):
        result = run_script("--dry-run", "--skip-deps", "--qsv", "true", "--rust-libobs", "false")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertNotIn("deps: Install build dependencies", result.stdout)
        self.assertIn("libavutil: Build libavutil", result.stdout)
        self.assertIn("env QSV=true", result.stdout)
        self.assertIn("env RUST_LIBOBS=false", result.stdout)

    def test_rejects_unknown_steps_and_values(self):
        self.assertNotEqual(run_script("--dry-run", "--steps", "bogus").returncode, 0)
        self.assertNotEqual(run_script("--dry-run", "--rust-libobs", "yes").returncode, 0)

    def test_configure_passes_flags_to_cmake(self):
        with tempfile.TemporaryDirectory() as scratch:
            stub = Path(scratch) / "cmake"
            log = Path(scratch) / "cmake.log"
            stub.write_text(f'#!/bin/sh\nprintf "%s\\n" "$@" > "{log}"\n')
            stub.chmod(0o755)
            output = Path(scratch) / "output"
            env = {**os.environ, "PATH": f"{scratch}:{os.environ['PATH']}", "GITHUB_OUTPUT": str(output)}
            result = run_script("--steps", "version,configure", "--rust-libobs", "false", "--qsv", "true",
                                "--sha", "0123456789abcdef", env=env)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            argv = log.read_text().splitlines()
            self.assertEqual(argv[:2], ["--preset", "ubuntu-ci"])
            self.assertIn("-DENABLE_RUST_LIBOBS=FALSE", argv)
            self.assertIn("-DENABLE_QSV11=TRUE", argv)
            version = next(a for a in argv if a.startswith("-DOBS_VERSION_OVERRIDE="))
            self.assertRegex(version, r"=\d+\.\d+\.\d+.*$")
            self.assertEqual(output.read_text(), "rust=FALSE\n")

    def test_tagless_checkout_version_keeps_libobs_api_version(self):
        # Without OBS release tags the version step must still give CMake an
        # OBS version it accepts: the libobs API version plus the commit (#1087).
        with tempfile.TemporaryDirectory() as scratch:
            repo = Path(scratch)
            (repo / "libobs").mkdir()
            (repo / "libobs/obs-config.h").write_text((ROOT / "libobs/obs-config.h").read_text())
            subprocess.run(["git", "init", "-q"], cwd=repo, check=True)
            env_file = repo / "github-env"
            env = {**os.environ, "GITHUB_ENV": str(env_file), "GITHUB_SHA": "0123456789abcdef"}
            result = subprocess.run(["bash", "-c", build.VERSION], cwd=repo, env=env, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            header = (ROOT / "libobs/obs-config.h").read_text()
            api = ".".join(re.search(rf"(?m)^#define LIBOBS_API_{part}_VER (\d+)$", header).group(1)
                           for part in ("MAJOR", "MINOR", "PATCH"))
            self.assertEqual(env_file.read_text(), f"OBS_VERSION={api}-g012345678\n")


if __name__ == "__main__":
    sys.exit(unittest.main())
