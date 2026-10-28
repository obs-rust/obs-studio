#!/usr/bin/env -S uv run --script
"""Unit tests for ci/local_gate.py.

    uv run python -m unittest ci/test_local_gate.py
"""

from __future__ import annotations

import importlib.util
import io
import os
import subprocess
import sys
import unittest
from pathlib import Path
from unittest import mock

_SPEC = importlib.util.spec_from_file_location("local_gate", Path(__file__).with_name("local_gate.py"))
local_gate = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(local_gate)


class SoldrCacheTest(unittest.TestCase):
    """soldr's zccache can replay stale rustc output (obs-rust/obs-studio#134),
    so every cargo command the gate attests must bypass the rustc wrapper."""

    def cargo_calls(self, lane, env=None):
        calls = []

        def fake_run(cmd, **kw):
            calls.append((cmd, kw.get("env")))
            return subprocess.CompletedProcess(cmd, 0)

        with (
            mock.patch.dict(os.environ, env or {}, clear=False),
            mock.patch.object(local_gate.subprocess, "run", side_effect=fake_run),
            mock.patch.object(local_gate, "run", side_effect=fake_run),
            mock.patch.object(local_gate, "tracked_files", return_value=[]),
            mock.patch.object(local_gate, "ensure_nextest", return_value=None),
            mock.patch.object(local_gate, "clang_format"),
            mock.patch.object(local_gate, "gersemi"),
            mock.patch.object(local_gate, "ruff_ratchet"),
            mock.patch.object(local_gate, "self_test", create=True),
            mock.patch.object(local_gate, "check_test_binary_inventory") as inventory,
            mock.patch.object(local_gate, "toolchain_gcc_ld", return_value="/sysroot/gcc-ld"),
        ):
            # The caller's own setting must not leak into the check.
            if "SOLDR_RUSTC_WRAPPER" not in (env or {}):
                os.environ.pop("SOLDR_RUSTC_WRAPPER", None)
            lane(False)
            if lane is local_gate.lane_lint:
                inventory.assert_called_once_with()
        return [(cmd, e) for cmd, e in calls if cmd[:2] == ["soldr", "cargo"]]

    def test_lanes_disable_rustc_wrapper(self):
        for lane in (local_gate.lane_lint, local_gate.lane_rust_test):
            calls = self.cargo_calls(lane)
            self.assertTrue(calls, f"{lane.__name__} runs no soldr cargo command")
            for cmd, env in calls:
                with self.subTest(lane=lane.__name__, cmd=" ".join(cmd)):
                    self.assertIsNotNone(env, "cargo runs with the caller's environment")
                    self.assertEqual(env.get("SOLDR_RUSTC_WRAPPER"), "none")

    def test_opt_in_keeps_wrapper(self):
        # OBS_GATE_ZCCACHE=1 restores the cached build for local iteration;
        # the caller's SOLDR_RUSTC_WRAPPER is then left alone.
        for cmd, env in self.cargo_calls(local_gate.lane_rust_test, {"OBS_GATE_ZCCACHE": "1"}):
            self.assertNotEqual((env or {}).get("SOLDR_RUSTC_WRAPPER"), "none", " ".join(cmd))


class SelfTestTest(unittest.TestCase):
    """Every ci/test_*.py runs, one process per file, in parallel (#949)."""

    def fake_run(self, failing=()):
        calls = []

        def fake(cmd, **kw):
            calls.append(cmd)
            name = cmd[cmd.index("-p") + 1]
            code = 1 if name in failing else 0
            return subprocess.CompletedProcess(cmd, code, f"output of {name}\n", "")

        return calls, fake

    def test_runs_every_ci_test_module_in_its_own_process(self):
        # Every ci/test_*.py, not just this file, so new checks (tier 2
        # volume, toolchain pin, registration fragments) run in the gate.
        calls, fake = self.fake_run()
        with mock.patch.object(local_gate.subprocess, "run", side_effect=fake):
            local_gate.self_test()
        names = sorted(cmd[cmd.index("-p") + 1] for cmd in calls)
        expected = sorted(p.name for p in (local_gate.ROOT / "ci").glob("test_*.py"))
        self.assertEqual(names, expected)
        self.assertGreater(len(names), 1)
        self.assertTrue(all("discover" in cmd for cmd in calls))

    def test_a_failing_module_fails_the_step_and_prints_its_output(self):
        calls, fake = self.fake_run(failing={"test_local_gate.py"})
        printed = []
        with (
            mock.patch.object(local_gate.subprocess, "run", side_effect=fake),
            mock.patch("builtins.print", side_effect=lambda *a, **k: printed.append(" ".join(map(str, a)))),
            self.assertRaises(subprocess.CalledProcessError),
        ):
            local_gate.self_test()
        self.assertTrue(any("output of test_local_gate.py" in line for line in printed), printed)

    def test_workers_follow_usable_cpus_capped(self):
        # 164 s of serial self-tests made ./lint slow (#949); a CPU-limited
        # container must not start every file at once.
        for usable, want in ((None, 1), (1, 1), (4, 4), (64, local_gate.SELF_TEST_MAX_WORKERS)):
            with mock.patch.object(local_gate.os, "process_cpu_count", create=True, return_value=usable):
                self.assertEqual(local_gate.self_test_workers(), want, usable)


class EnvironmentHintTest(unittest.TestCase):
    """Environment failures get a one-line hint naming the documented fix
    (obs-rust/obs-studio#694, docs/rust-port/troubleshooting.md)."""

    CASES = {
        "soldr: IO error: Permission denied (os error 13)": "ownership",
        "soldr: warning: disk space is low (6.2 GiB free). Run `soldr gc` to review": "soldr gc",
        "Could NOT find FFmpeg (missing: avcodec) (Required is at least version \"8.0\")": "./unittest",
        "Cannot connect to the Docker daemon at unix:///var/run/docker.sock": "Docker",
    }

    def test_each_error_maps_to_a_hint(self):
        for line, words in self.CASES.items():
            with self.subTest(line=line):
                hints = local_gate.hints_for(line)
                self.assertEqual(len(hints), 1, hints)
                self.assertIn(words, hints[0])
                self.assertIn("troubleshooting.md", hints[0])

    def test_ordinary_output_has_no_hint(self):
        self.assertEqual(local_gate.hints_for("test result: ok. 12 passed\nerror[E0308]: mismatched types"), [])

    def test_hints_are_not_repeated(self):
        line = "soldr: IO error: Permission denied (os error 13)"
        self.assertEqual(len(local_gate.hints_for(f"{line}\n{line}")), 1)

    def test_missing_soldr_fails_the_lane_with_a_hint(self):
        def missing(cmd, **kw):
            raise FileNotFoundError(2, "No such file or directory", "soldr")

        out = []
        with mock.patch.object(local_gate.subprocess, "Popen", side_effect=missing), \
                mock.patch("builtins.print", side_effect=lambda *a, **k: out.append(" ".join(map(str, a)))), \
                mock.patch("sys.argv", ["local_gate.py", "--lane", "rust-test"]):
            self.assertEqual(local_gate.main(), 1)
        text = "\n".join(out)
        self.assertIn("lane rust-test: FAILED", text)
        self.assertIn("soldr is not on PATH", text)
        self.assertIn("troubleshooting.md", text)

    def test_run_streams_output_and_reports_hints(self):
        out = []
        with mock.patch("builtins.print", side_effect=lambda *a, **k: out.append(" ".join(map(str, a)))):
            with self.assertRaises(subprocess.CalledProcessError):
                local_gate.run(["sh", "-c", "echo 'soldr: IO error: Permission denied (os error 13)'; exit 3"])
        text = "\n".join(out)
        self.assertIn("Permission denied", text)
        self.assertIn("hint:", text)


class CrossTargetsTest(unittest.TestCase):
    """PR gate lints win-x64, mac-arm64 and linux-x64 (the host); the release
    gate adds win-arm64, mac-x64 and linux-arm64 (obs-rust/obs-studio#955)."""

    def test_pr_gate_targets(self):
        self.assertEqual(local_gate.CROSS_TARGETS, ("x86_64-pc-windows-msvc", "aarch64-apple-darwin"))

    def test_release_gate_targets(self):
        self.assertEqual(
            local_gate.RELEASE_CROSS_TARGETS,
            ("aarch64-pc-windows-msvc", "x86_64-apple-darwin", "aarch64-unknown-linux-gnu"),
        )
        self.assertIn("cross-release", local_gate.LANES)


if __name__ == "__main__":
    unittest.main()


class ClangFormatScopeTest(unittest.TestCase):
    """CI's clang-format job checks every changed C/C++/ObjC file
    (.github/actions/run-clang-format, checkGlob '*.c' '*.h' ...), but the
    lint lane only formatted the upstream OBS directories. C under rust/
    (obs-c-oracle wrappers, the platform_windows SEH shim) passed the local
    gate and then failed CI's clang-format (#707)."""

    def test_rust_c_sources_are_checked(self):
        files = [
            "rust/obs-util/src/platform/windows/plugin_seh.c",
            "rust/obs-c-oracle/oracle/platform_windows.c",
            "libobs/obs.c.h",
        ]
        checked = [
            f
            for f in files
            if local_gate.top_dir_matches(f, local_gate.CLANG_DIRS)
            and f.endswith(local_gate.CLANG_EXTS)
            and not local_gate.excluded(f, local_gate.CLANG_EXCLUDE)
        ]
        self.assertEqual(checked, files)


class FormatScopeTest(unittest.TestCase):
    """./lint formats only changed files locally; CI checks every file (#1050)."""

    FILES = ["libobs/obs.c.h", "frontend/a.cpp", "frontend/b.cpp", "CMakeLists.txt"]

    def test_ci_checks_every_file(self):
        self.assertEqual(local_gate.format_scope(self.FILES, ["frontend/a.cpp"], ci=True), self.FILES)

    def test_local_checks_only_changed_files(self):
        self.assertEqual(
            local_gate.format_scope(self.FILES, ["frontend/a.cpp", "docs/x.md"], ci=False), ["frontend/a.cpp"]
        )

    def test_unknown_changes_check_every_file(self):
        # No merge-base with origin/master: nothing to compare against.
        self.assertEqual(local_gate.format_scope(self.FILES, None, ci=False), self.FILES)

    def test_formatter_config_change_checks_every_file(self):
        for config in (".clang-format", "libobs/.clang-format", ".gersemirc", "ci/local_gate.py"):
            with self.subTest(config=config):
                self.assertEqual(local_gate.format_scope(self.FILES, [config], ci=False), self.FILES)


class FormatScopeUpstreamConfigTest(unittest.TestCase):
    """A formatter change that landed on master after the branch point must
    widen the local check too (#1050 review)."""

    def test_upstream_config_change_counts_as_changed(self):
        outputs = {
            ("merge-base", "HEAD", "origin/master"): "base\n",
            ("diff", "--name-only", "base"): "frontend/a.cpp\n",
            ("ls-files", "--others", "--exclude-standard"): "",
            ("diff", "--name-only", "base", "origin/master", "--", *local_gate.FORMAT_CONFIG_PATHS,
             *(f":(glob)**/{n}" for n in local_gate.FORMAT_CONFIG_NAMES)): "ci/local_gate.py\n",
        }

        def fake(cmd, **kw):
            return subprocess.CompletedProcess(cmd, 0, outputs.get(tuple(cmd[1:]), ""), "")

        with mock.patch.object(local_gate.subprocess, "run", side_effect=fake):
            changed = local_gate.changed_since_master()
        self.assertIn("ci/local_gate.py", changed)
        files = ["frontend/a.cpp", "frontend/b.cpp"]
        self.assertEqual(local_gate.format_scope(files, changed, ci=False), files)


class EmptyTestDiffTest(unittest.TestCase):
    """A shell that loses the base must not turn the changed-crates step into
    the whole gate. On a Windows runner PowerShell reads `${BASE}` as an unset
    variable, so the step passed `--test-diff ""`, argparse saw a falsy value
    and the gate ran every lane — the POSIX-only ci/ self-tests included
    (#2743)."""

    def test_an_empty_base_is_refused(self):
        ran = []
        lanes = {name: (lambda fix, name=name: ran.append(name)) for name in local_gate.LANES}
        with (
            mock.patch.object(sys, "argv", ["local_gate.py", "--test-diff", ""]),
            mock.patch.object(local_gate, "LANES", lanes),
            mock.patch.object(local_gate, "test_diff") as test_diff,
            mock.patch.object(sys, "stderr", new=io.StringIO()) as stderr,
            self.assertRaises(SystemExit) as raised,
        ):
            local_gate.main()
        self.assertEqual(raised.exception.code, 2)
        self.assertIn("--test-diff needs a base commit", stderr.getvalue())
        self.assertEqual(ran, [])
        test_diff.assert_not_called()
