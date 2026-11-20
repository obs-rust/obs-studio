#!/usr/bin/env -S uv run --script
"""macOS Tier 2 overlaps preparation, preserves every result and joins children.

    uv run python -m unittest ci/test_tier2_macos_tail.py
"""

from __future__ import annotations

import contextlib
import importlib.util
import io
import os
import re
import subprocess
import sys
import tempfile
import threading
import unittest
from pathlib import Path
from unittest import mock

spec = importlib.util.spec_from_file_location("tier2_macos_tail", Path(__file__).with_name("tier2_macos_tail.py"))
tail = importlib.util.module_from_spec(spec)
spec.loader.exec_module(tail)


class MacosTailTest(unittest.TestCase):
    def test_workflow_sets_up_uv_before_every_overlap_validator(self):
        workflow = Path(__file__).resolve().parent.parent / ".github/workflows/build-project.yaml"
        jobs = re.split(r"(?m)^  ([\w-]+):\n", workflow.read_text())
        consumers = []
        for name, job in zip(jobs[1::2], jobs[2::2]):
            steps = job.split("\n      - ")
            for index, step in enumerate(steps):
                if "bash rust/tools/tier2-validate/validate.sh" not in step:
                    continue
                consumers.append(name)
                with self.subTest(job=name):
                    setup = [prior for prior in steps[:index]
                             if re.search(r"uses: astral-sh/setup-uv@[0-9a-f]{40}\b", prior)]
                    self.assertTrue(setup, "overlap validator requires a preceding pinned uv setup step")
                    # A macOS-only setup also covers the dormant macOS step in
                    # the Windows job; unrelated step conditions must not pass.
                    for prior in setup:
                        condition = re.search(r"(?m)^        if: (.+)$", prior)
                        self.assertIn(condition.group(1) if condition else "", ("", "runner.os == 'macOS'"))
        self.assertIn("rust-tier2-macos", consumers)

    def test_phase_reentry_selects_cmake_framework_among_native_outputs(self):
        validator = Path(__file__).resolve().parent.parent / "rust/tools/tier2-validate/validate.sh"
        discovery = next(line for line in validator.read_text().splitlines() if line.startswith("rust_lib="))
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            expected = root / "libobs/RelWithDebInfo/libobs.framework/Versions/A/libobs"
            for path in (root / "cargo-native-core/obs/libobs.framework/Versions/A/libobs",
                         root / "libobs-c-reference/RelWithDebInfo/libobs.framework/Versions/A/libobs", expected):
                path.parent.mkdir(parents=True)
                path.touch()
            result = subprocess.run(
                ["bash", "-c", 'set -eu\nbuild="$1"\n' + discovery + '\nprintf "%s" "$rust_lib"', "bash", str(root)],
                capture_output=True, text=True, check=True,
            )
            self.assertEqual(result.stdout, str(expected))

    def validator_fixture(self, root):
        source = Path(__file__).resolve().parent.parent / "rust/tools/tier2-validate/validate.sh"
        validator = root / "rust/tools/tier2-validate/validate.sh"
        validator.parent.mkdir(parents=True)
        validator.write_text(source.read_text())

        def artifact(name, contents="", executable=False):
            path = root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(contents)
            if executable:
                path.chmod(0o755)
            return path

        for directory in ("libobs", "libobs-c-reference"):
            artifact(f"build_rust/{directory}/RelWithDebInfo/libobs.framework/Versions/A/libobs")
        artifact(".deps/obs-deps/lib/libavcodec.dylib")
        artifact("build_rust/test/cmocka/test_bitstream", '''#!/bin/bash
set -e
printf '%s/libobs.framework/Versions/A/libobs\\n' "${PROBE_WRONG_PATH:-$DYLD_FRAMEWORK_PATH}" >&2
# More than a pipe buffer after the matching line: grep -q must not kill us.
head -c 1048576 /dev/zero >&2
printf 'probe %s\\n' "$DYLD_FRAMEWORK_PATH" >> events
exit "${PROBE_STATUS:-0}"
''', executable=True)
        artifact("bin/ctest", '''#!/bin/bash
set -eu
printf 'ctest %s\\n' "$DYLD_FRAMEWORK_PATH" >> events
while [ "$#" -gt 0 ]; do
  if [ "$1" = --output-log ]; then printf 'suite %s\\n' "$DYLD_FRAMEWORK_PATH" > "$2"; shift; fi
  shift
done
exit "${CTEST_STATUS:-0}"
''', executable=True)
        # Native identity probes retain their positive and negative controls.
        identity = artifact("identity", '''#!/bin/bash
name=$(basename "$1" .dylib | sed -e s/libobs-opengl/OPENGL/ -e s/libobs-metal/METAL/)
[ "$name" = "$2" ] || exit 14
case "${!#}" in */cargo-native-core/*/libobs.framework/Versions/A/libobs) exit 0 ;; *) exit 12 ;; esac
''', executable=True)
        graphics, captures = [], []
        for renderer in ("OPENGL", "METAL"):
            artifact(f"build_rust/libobs-{renderer.lower()}/libobs-{renderer.lower()}.dylib")
            module = artifact(f"modules/{renderer}.dylib")
            graphics.append(f"{renderer}\t{module}\t{identity}\n")
        for name, directory in (("mac-capture", "mac-capture"), ("mac-avcapture", "mac-avcapture"),
                                ("mac-avcapture-legacy", "mac-avcapture")):
            module = artifact(f"build_rust/plugins/{directory}/{name}.plugin/Contents/MacOS/{name}")
            data = module.parent.parent / "Resources"
            artifact(str(data.relative_to(root) / "locale/en-US.ini"))
            captures.append(f"{name}\t{module}\t{data}\t{identity}\n")
        native = "build_rust/cargo-native-core"
        artifact(f"{native}/macos-graphics-paths.txt", "".join(graphics))
        artifact(f"{native}/macos-capture-paths.txt", "".join(captures))
        for variant in ("obs-c-reference", "obs"):
            for suffix in ("libobs.dylib", "libobs.framework/Versions/A/libobs", "libobs.framework/libobs"):
                artifact(f"{native}/{variant}/{suffix}")
        return validator

    def test_real_validator_retains_suite_local_evidence_after_ctest(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            validator = self.validator_fixture(root)
            for phase in ("cmake-tests", "cargo-tests"):
                result = subprocess.run(["bash", str(validator), "--phase", phase],
                    env={**os.environ, "PATH": f"{root / 'bin'}:{os.environ['PATH']}"},
                    capture_output=True, text=True, timeout=20)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            events = (root / "events").read_text().splitlines()
            self.assertEqual(len(events), 8)
            for ctest, probe in zip(events[::2], events[1::2]):
                self.assertTrue(ctest.startswith("ctest "), events)
                self.assertEqual(probe, ctest.replace("ctest ", "probe ", 1))
            logs = root / "build_rust/tail-logs"
            for phase, variants in (("cmake-tests", ("OFF", "ON")), ("cargo-tests", ("obs-c-reference", "obs"))):
                for variant in variants:
                    suite = logs / phase / variant
                    self.assertIn("suite ", (suite / "ctest.log").read_text())
                    self.assertIn("libobs.framework/", (suite / "loader.log").read_text())
            self.assertEqual(len(list(logs.glob("*/*/loader.log"))), 4)

    def test_real_validator_rejects_bad_loader_or_ctest_without_stale_proof(self):
        for phase in ("cmake-tests", "cargo-tests"):
            for failure in ({"PROBE_WRONG_PATH": "/wrong"}, {"PROBE_STATUS": "23"}, {"CTEST_STATUS": "24"}):
                with self.subTest(phase=phase, failure=failure), tempfile.TemporaryDirectory() as directory:
                    root = Path(directory)
                    validator = self.validator_fixture(root)
                    variant = "OFF" if phase == "cmake-tests" else "obs-c-reference"
                    proof = root / "build_rust/tail-logs" / phase / variant / "loader.log"
                    proof.parent.mkdir(parents=True)
                    framework = (root / "build_rust/libobs-c-reference/RelWithDebInfo" if phase == "cmake-tests"
                                 else root / "build_rust/cargo-native-core/obs-c-reference")
                    proof.write_text(f"{framework}/libobs.framework/Versions/A/libobs\n")
                    result = subprocess.run(["bash", str(validator), "--phase", phase],
                        env={**os.environ, "PATH": f"{root / 'bin'}:{os.environ['PATH']}", **failure},
                        capture_output=True, text=True, timeout=20)
                    self.assertNotEqual(result.returncode, 0)
                    if "CTEST_STATUS" in failure:
                        events = root / "events"
                        self.assertNotIn("probe ", events.read_text() if events.exists() else "")

    def run_fake(self, root: Path, failing: str = "") -> tuple[int, str]:
        validator = root / "validate.sh"
        validator.write_text(
            '#!/bin/bash\nset -eu\nphase="$2"\n'
            'printf "%s\\n" "$@" > "$0.$phase.args"\n'
            'echo "stdout $phase"\necho "stderr $phase" >&2\n'
            'touch "$0.$phase.done"\n'
            f'if [ "$phase" = "{failing}" ]; then exit 23; fi\n',
            encoding="utf-8",
        )
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            status = tail.run_tail(validator, ["--repeat", "2", "--tests", "with spaces"], root / "logs")
        return status, output.getvalue()

    def test_success_keeps_logs_and_forwards_arguments(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            status, output = self.run_fake(root)
            self.assertEqual(status, 0)
            for phase in ("cmake-tests", "cargo-build", "cargo-tests"):
                self.assertTrue((root / f"validate.sh.{phase}.done").is_file())
                self.assertEqual(
                    (root / f"validate.sh.{phase}.args").read_text().splitlines(),
                    ["--phase", phase, "--repeat", "2", "--tests", "with spaces"],
                )
                self.assertIn(f"stdout {phase}", output)
                self.assertIn(f"stderr {phase}", (root / "logs" / f"{phase}.log").read_text())

    def test_any_failure_fails_without_abandoning_independent_work(self):
        for failing in ("cmake-tests", "cargo-build", "cargo-tests"):
            with self.subTest(failing=failing), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                status, output = self.run_fake(root, failing)
                self.assertNotEqual(status, 0)
                self.assertIn(f"{failing}: exit 23", output)
                for independent in ("cmake-tests", "cargo-build"):
                    self.assertTrue((root / f"validate.sh.{independent}.done").is_file())
                # Native suites require successful preparation; a failed CMake
                # suite must still allow them to run and report their result.
                self.assertEqual((root / "validate.sh.cargo-tests.done").exists(), failing != "cargo-build")

    def test_overlap_and_barrier_before_native_suites(self):
        barrier = threading.Barrier(2, timeout=5)
        finished: set[str] = set()

        def phase(name, command, log_dir):
            if name == "cargo-tests":
                self.assertEqual(finished, {"cmake-tests", "cargo-build"})
            else:
                barrier.wait()
                finished.add(name)
            return 0

        with tempfile.TemporaryDirectory() as directory, mock.patch.object(tail, "run_phase", side_effect=phase):
            with mock.patch.object(tail, "show_log"):
                self.assertEqual(tail.run_tail(Path("validate.sh"), [], Path(directory)), 0)

    def test_launch_error_still_runs_other_independent_phase(self):
        def launch(command, **kwargs):
            if "cargo-build" in command:
                raise OSError("launch failed")
            return mock.Mock(returncode=0)

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with mock.patch.object(tail.subprocess, "run", side_effect=launch) as run:
                with contextlib.redirect_stdout(io.StringIO()) as output:
                    self.assertNotEqual(tail.run_tail(root / "validate.sh", [], root), 0)
                self.assertEqual(run.call_count, 2)
            self.assertIn("launch failed", output.getvalue())

    def test_cli_propagates_failure_after_all_runnable_phases(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.run_fake(root, "cmake-tests")
            result = subprocess.run(
                [sys.executable, str(Path(tail.__file__)), "--validator", str(root / "validate.sh"),
                 "--log-dir", str(root / "cli-logs")],
                capture_output=True, text=True, timeout=20,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(len(list((root / "cli-logs").glob("*.log"))), 3)
            self.assertIn("cargo-tests: exit 0", result.stdout)


if __name__ == "__main__":
    unittest.main()
