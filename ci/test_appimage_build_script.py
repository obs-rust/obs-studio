#!/usr/bin/env -S uv run --script
"""build-aux/appimage/build.py runs the workflow's AppImage build steps (#899).

    uv run python -m unittest ci/test_appimage_build_script.py

- The scripts are the workflow's former inline `run:` blocks, byte for byte
  (checked against the last workflow that had them, when git has that commit).
- The workflow calls every step, in order, and passes every input.
- --dry-run prints the plan; --skip-deps drops only the dependency install.
- version + configure, run against a stub cmake, pass the flags through.
- `bosn run --task appimage` builds on the same base as the workflow.
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
from unittest import mock

ROOT = Path(__file__).resolve().parent.parent
SCRIPT = ROOT / "build-aux/appimage/build.py"
WORKFLOW = ROOT / ".github/workflows/appimage.yaml"
# origin/master just before the move: the last appimage.yaml with inline scripts.
BEFORE = "2c38ec153b622ec6873b90b8ebe5c359ff3d4129"
OLD_STEPS = {
    "version": "Determine version",
    "configure": "Configure",
    "build": "Build 🧱",
    "appdir": "Assemble AppDir",
    "appimage": "Create AppImage",
}
# Deliberate changes to the scripts since the move, as (old, new) edits.
JOURNAL_START = (
    "# Journal each compilation's hit or miss and the miss reason; the workflow\n"
    "# summarizes it (#2182). A failed session start only loses the journal.\n"
    'if ZCCACHE_SESSION_ID="$(zccache session-start --cwd "$PWD" --stats --journal "${RUNNER_TEMP:-/tmp}/appimage-zccache.jsonl" |\n'
    "  sed -n 's/.*\"session_id\": *\"\\([^\"]*\\)\".*/\\1/p')\" && [ -n \"$ZCCACHE_SESSION_ID\" ]; then\n"
    "  export ZCCACHE_SESSION_ID\n"
    "fi\n"
)
STABLE_MTIMES = (
    "# rcc writes each resource file's mtime into qrc_*.cpp, and a checkout gives\n"
    "# every file a new one, so the qrc objects missed the cache on every build of\n"
    "# the same commit (#2182). Not SOURCE_DATE_EPOCH: zccache keys on it.\n"
    'epoch="$(git log -1 --format=%ct)"\n'
    "{ git ls-files -z; git submodule foreach --quiet --recursive 'git ls-files -z | sed -z \"s|^|$sm_path/|\"'; } |\n"
    '  xargs -0 touch -h -c -d "@$epoch"\n'
)
JOURNAL_END = 'if [ -n "${ZCCACHE_SESSION_ID:-}" ]; then zccache session-end "$ZCCACHE_SESSION_ID" || true; fi\n'

LATER = {
    # #1744: the AppImage ships obs-webrtc.
    "configure": [
        ("-DENABLE_WEBRTC=OFF", "-DENABLE_WEBRTC=ON"),
    ],
    "build": [
        ("ccache -s\n", "zccache status || true\n"
         "zccache stop || true # flush the cache index before the workflow saves it\n"),
        # #2182: the build runs in a journaled zccache session.
        ("cmake --build build_ubuntu\n", JOURNAL_START + "cmake --build build_ubuntu\n" + JOURNAL_END),
        # #2182: tracked files get the commit time, so rcc's output is stable.
        ("cmake --build build_ubuntu\n", STABLE_MTIMES + "cmake --build build_ubuntu\n"),
    ],
    # #1881: the update information follows the certified release, like
    # Windows and macOS, not the daily continuous build.
    "appimage": [
        ('# A build published as the "continuous" pre-release (release job) carries\n'
         "# update information: AppImageUpdate and friends can then fetch the next\n"
         "# continuous build as a delta against this one (.zsync).\n",
         "# A build published by the release job carries update information that\n"
         '# follows the certified ("latest") release, as the Windows and macOS updaters\n'
         "# do (#1881): AppImageUpdate and friends fetch the next certified build as a\n"
         "# delta against this one (.zsync), never an uncertified daily.\n"),
        ("|continuous|OBS-Studio-Rust-", "|latest|OBS-Studio-Rust-"),
    ],
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


class ManylinuxLauncherTest(unittest.TestCase):
    def capture_launch(self):
        with tempfile.TemporaryDirectory(prefix="appimage-launcher-") as directory:
            root = Path(directory)
            executable = root / "docker"
            executable.write_text(
                '#!/bin/sh\nprintf \'%s\\0\' "$@" > "$DOCKER_ARGS_CAPTURE"\n'
            )
            executable.chmod(0o755)
            workspace = root / "workspace with spaces"
            temporary = root / "runner temp"
            home = root / "home"
            capture = root / "arguments"
            environment = {
                "PATH": f"{root}{os.pathsep}{os.environ.get('PATH', os.defpath)}",
                "HOME": str(home),
                "GITHUB_WORKSPACE": str(workspace),
                "RUNNER_TEMP": str(temporary),
                "ZCCACHE_CACHE_DIR": str(workspace / ".zccache"),
                "CCACHE_BASEDIR": str(workspace),
                "RUST_LIBOBS": "true",
                "OBS_APPIMAGE_IMAGE": "fixture:image",
                "ZCCACHE_REMOTE_STORAGE": "must-not-be-forwarded",
                "UNRELATED_SETTING": "must-not-be-forwarded",
                "DOCKER_ARGS_CAPTURE": str(capture),
            }
            command = ["printf", "value with spaces", "literal;$(not-a-command)"]
            result = subprocess.run(
                [sys.executable, str(ROOT / "build-aux/appimage/manylinux/run.py"), *command],
                cwd=ROOT, env=environment, capture_output=True, text=True, timeout=10,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            argv = capture.read_bytes().decode().split("\0")[:-1]
            forwarded = {argv[index + 1] for index, value in enumerate(argv) if value == "-e"}
            mounts = {argv[index + 1] for index, value in enumerate(argv) if value == "-v"}
            return argv, forwarded, mounts, environment, command

    def test_manylinux_forwards_configured_compiler_store(self):
        _, forwarded, mounts, environment, _ = self.capture_launch()
        self.assertIn("ZCCACHE_CACHE_DIR", forwarded)
        workspace = environment["GITHUB_WORKSPACE"]
        self.assertIn(f"{workspace}:{workspace}", mounts)
        self.assertEqual(environment["ZCCACHE_CACHE_DIR"], f"{workspace}/.zccache")

    def test_manylinux_keeps_environment_filter_and_arguments(self):
        argv, forwarded, mounts, environment, command = self.capture_launch()
        self.assertTrue({"CCACHE_BASEDIR", "RUST_LIBOBS", "GITHUB_WORKSPACE", "RUNNER_TEMP"} <= forwarded)
        self.assertNotIn("ZCCACHE_REMOTE_STORAGE", forwarded)
        self.assertNotIn("UNRELATED_SETTING", forwarded)
        self.assertEqual(argv[:2], ["run", "--rm"])
        self.assertEqual(argv[-4:], ["fixture:image", *command])
        temporary = environment["RUNNER_TEMP"]
        self.assertIn(f"{temporary}:{temporary}", mounts)
        self.assertIn(f"{environment['HOME']}/.cargo/registry:/usr/local/cargo/registry", mounts)


class BosnTaskTest(unittest.TestCase):
    def test_local_task_uses_the_workflow_base(self):
        import tomllib

        bases = set(re.findall(r"build\.py --base (\w+)", WORKFLOW.read_text()))
        self.assertEqual(bases, {"manylinux"})
        bosn = tomllib.loads((ROOT / "bosn.toml").read_text())
        task = bosn["task"]["appimage"]
        self.assertIn("--base manylinux", task["cmd"])
        stack = bosn["stack"][task["stack"]]
        self.assertEqual(stack["dockerfile"], "build-aux/appimage/manylinux/Dockerfile")

    def test_the_ubuntu_base_is_retired(self):
        # #1999: no workflow built on the Ubuntu 26.04 base, and it no longer
        # built (Ubuntu 26.04 has no libdatachannel-dev). Only the glibc 2.28
        # image remains.
        args = build.parse_args(["--sha", "0" * 40])
        self.assertEqual(args.base, "manylinux")
        self.assertNotEqual(run_script("--dry-run", "--base", "ubuntu").returncode, 0)
        self.assertNotIn("deps", build.STEP_NAMES)
        self.assertNotIn("libavutil", build.STEP_NAMES)
        self.assertFalse((ROOT / "build-aux/appimage/Dockerfile").exists())
        import tomllib

        stacks = tomllib.loads((ROOT / "bosn.toml").read_text())["stack"]
        self.assertNotIn("build-aux/appimage/Dockerfile", [s["dockerfile"] for s in stacks.values()])


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
                for old_text, new_text in LATER.get(step.name, []):
                    self.assertIn(old_text, expected)
                    expected = expected.replace(old_text, new_text)
                self.assertEqual(step.script.strip("\n"), expected.strip("\n"))

    def test_workflow_calls_every_step_in_order(self):
        called = []
        for line in WORKFLOW.read_text().splitlines():
            if "build-aux/appimage/build.py " in line and "--steps " in line:
                called += line.split("--steps ", 1)[1].split()[0].split(",")
        # The workflow builds on the glibc 2.28 image, which has the dependencies (#926).
        self.assertIn("--base manylinux --steps", WORKFLOW.read_text())
        self.assertEqual(called, list(build.STEP_NAMES))

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
            env = {"PATH": f"{scratch}:{os.environ['PATH']}", "GITHUB_OUTPUT": str(output)}
            # The build runs only on the glibc 2.28 image (#926); this host is not it.
            with mock.patch.dict(os.environ, env), \
                    mock.patch.object(build.os, "confstr", return_value="glibc 2.28"):
                code = build.main(["--steps", "version,configure", "--rust-libobs", "false", "--qsv", "true",
                                   "--sha", "0123456789abcdef"])
            self.assertEqual(code, 0)
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
