#!/usr/bin/env -S uv run --script
"""Unit tests for ci/local_gate.py.

    uv run python -m unittest ci/test_local_gate.py
"""

from __future__ import annotations

import importlib.util
import contextlib
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
        with mock.patch.object(local_gate, "ensure_nextest", side_effect=AssertionError("tool acquisition must not run without soldr")) as acquire, \
                mock.patch.object(local_gate.subprocess, "Popen", side_effect=missing), \
                mock.patch("builtins.print", side_effect=lambda *a, **k: out.append(" ".join(map(str, a)))), \
                mock.patch("sys.argv", ["local_gate.py", "--lane", "rust-test"]):
            self.assertEqual(local_gate.main(), 1)
        acquire.assert_not_called()
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

    def test_ci_cross_targets_have_separate_runners_and_one_combined_proof(self):
        import re
        workflow = (local_gate.ROOT / ".github/workflows/build-project.yaml").read_text()
        def job(name):
            return re.search(rf"^  {name}:\n(.*?)(?=^  [a-z0-9-]+:|\Z)", workflow, re.M | re.S).group(1)
        cross = job("rust-cross-lint")
        self.assertIn("fail-fast: false", cross)
        self.assertIn("target: [x86_64-pc-windows-msvc, aarch64-apple-darwin]", cross)
        self.assertIn("--cross-target ${{ matrix.target }}", cross)
        self.assertNotIn("actions/upload-artifact", cross)
        proof = job("rust-cross-proof")
        self.assertIn("needs: [ci-mode, rust-cross-lint]", proof)
        self.assertIn("needs.rust-cross-lint.result == 'success'", proof)
        self.assertIn("name: ${{ needs.ci-mode.outputs.cross_name }}", proof)

    def test_cross_lane_changes_select_both_targets_and_scope_policy_pr_windows(self):
        from ci_scope import scope
        for event in ("pull_request", "merge_group"):
            for path in ("ci/local_gate.py", "ci/test_local_gate.py", "ci/ci_scope.py",
                         "local-gate.toml", ".github/workflows/build-project.yaml"):
                with self.subTest(event=event, path=path):
                    selected = scope([path], event=event)
                    self.assertEqual(selected.cross_lint, event == "pull_request")
                    self.assertFalse(selected.macos)
                    self.assertEqual(selected.windows, event == "pull_request" and path == "ci/ci_scope.py")
            for path in ("docs/ci-tooling.md", "rust/obs-util/src/dstr.rs"):
                self.assertFalse(scope([path], event=event).cross_lint)
        workflow = (local_gate.ROOT / ".github/workflows/build-project.yaml").read_text()
        self.assertIn("cross_lint: ${{ steps.native.outputs.cross_lint }}", workflow)
        self.assertIn("needs.check-event.outputs.cross_lint == 'true'", workflow)

    def test_selected_target_keeps_clippy_dylint_and_test_linking(self):
        for target in local_gate.CROSS_TARGETS:
            with self.subTest(target=target), mock.patch.object(sys, "argv", [
                "local_gate.py", "--lane", "cross", "--cross-target", target,
            ]), mock.patch.object(local_gate, "add_target") as add, \
                    mock.patch.object(local_gate, "soldr_cargo") as cargo, \
                    mock.patch.object(local_gate, "soldr_dylint") as dylint:
                self.assertEqual(local_gate.main(), 0)
            add.assert_called_once_with(target)
            scope = ["--workspace", "--exclude", "obs-c-oracle", "--lib", "--bins"]
            self.assertEqual(cargo.call_args_list, [
                mock.call(["clippy", *scope, "--locked", "--target", target, "--", "-D", "warnings"]),
                mock.call(["test", "--workspace", "--no-run", "--locked", "--target", target]),
            ])
            dylint.assert_called_once_with(["--all", "--", *scope, "--target", target])

    def test_release_gate_targets(self):
        self.assertEqual(
            local_gate.RELEASE_CROSS_TARGETS,
            ("aarch64-pc-windows-msvc", "x86_64-apple-darwin", "aarch64-unknown-linux-gnu"),
        )
        self.assertIn("cross-release", local_gate.LANES)


class ClientLanesTest(unittest.TestCase):
    TARGETS = (
        "x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu",
        "x86_64-pc-windows-msvc", "aarch64-pc-windows-msvc",
        "x86_64-apple-darwin", "aarch64-apple-darwin",
    )

    def test_all_six_targets_lint_once_without_test_linking(self):
        for host in self.TARGETS:
            calls = []
            def capture(cmd, **kw):
                calls.append(cmd)
            with self.subTest(host=host), contextlib.ExitStack() as stack:
                for name in ("tracked_files", "changed_since_master"):
                    stack.enter_context(mock.patch.object(local_gate, name, return_value=[]))
                for name in ("clang_format", "gersemi", "ruff_ratchet", "self_test",
                             "check_test_binary_inventory", "check_feature_sets", "platform_ifdef_ratchet",
                             "perf_guard_lint", "knowledge_lint", "add_target"):
                    stack.enter_context(mock.patch.object(local_gate, name))
                stack.enter_context(mock.patch.object(local_gate, "host_target", return_value=host, create=True))
                stack.enter_context(mock.patch.object(local_gate, "run", side_effect=capture))
                local_gate.LANES["client-lint"](False)
            for target in self.TARGETS:
                clippy = [c for c in calls if c[:3] == ["soldr", "cargo", "clippy"]
                          and (c[c.index("--target") + 1] if "--target" in c else host) == target]
                dylint = [c for c in calls if c[:2] == ["soldr", "dylint"]
                          and (c[c.index("--target") + 1] if "--target" in c else host) == target]
                self.assertEqual(len(clippy), 1 + (len(local_gate.FEATURE_SETS) if target == host else 0))
                self.assertEqual(len(dylint), 1)
                for command in clippy + dylint:
                    if target != host:
                        self.assertIn("obs-c-oracle", command)
            self.assertFalse(any(c[:3] == ["soldr", "cargo", "test"] for c in calls))

    def test_host_features_and_dedicated_dylint_are_preserved(self):
        with mock.patch.object(local_gate, "host_target", return_value=self.TARGETS[0], create=True), \
                mock.patch.object(local_gate, "lane_lint") as lint, \
                mock.patch.object(local_gate, "lane_dylint") as dylint, \
                mock.patch.object(local_gate, "cross_lint") as cross:
            local_gate.LANES["client-lint"](False)
        lint.assert_called_once_with(False, self_tests=False)
        dylint.assert_called_once_with(False, self_tests=False)
        cross.assert_called_once_with(self.TARGETS[1:], link=False)

    def test_failed_target_fails_client_lane(self):
        with mock.patch.object(local_gate, "host_target", return_value=self.TARGETS[0], create=True), \
                mock.patch.object(local_gate, "lane_lint"), mock.patch.object(local_gate, "lane_dylint"), \
                mock.patch.object(local_gate, "add_target"), \
                mock.patch.object(local_gate, "soldr_cargo", side_effect=subprocess.CalledProcessError(1, "clippy")):
            with self.assertRaises(subprocess.CalledProcessError):
                local_gate.LANES["client-lint"](False)

    def test_unsupported_host_does_not_prove_any_lane(self):
        with mock.patch.object(local_gate, "host_target", return_value="riscv64-unknown-linux-gnu", create=True), \
                mock.patch.object(local_gate, "lane_lint") as lint:
            with self.assertRaises(local_gate.NotApplicable):
                local_gate.LANES["client-lint"](False)
        lint.assert_not_called()

    def test_client_test_preserves_workspace_features_and_linux_gnu_link(self):
        for host in self.TARGETS:
            with self.subTest(host=host), \
                    mock.patch.object(local_gate, "host_target", return_value=host, create=True), \
                    mock.patch.object(local_gate.host_platform, "is_linux", return_value=host.endswith("linux-gnu")), \
                    mock.patch.object(local_gate, "run"), \
                    mock.patch.object(local_gate, "rust_tests") as tests, \
                    mock.patch.object(local_gate, "windows_gnu_no_run") as gnu:
                local_gate.LANES["client-test"](False)
            self.assertEqual(tests.call_args_list, [mock.call(["--workspace"])] +
                             [mock.call(list(args)) for args in local_gate.FEATURE_TEST_SETS])
            self.assertEqual(gnu.call_count, int(host.endswith("linux-gnu")))

    def test_off_host_client_test_exits_not_applicable_without_running_tests(self):
        with mock.patch.object(sys, "argv", ["local_gate.py", "--lane", "client-test",
                                           "--client-target", "aarch64-apple-darwin"]), \
                mock.patch.object(local_gate, "host_target", return_value=self.TARGETS[0]), \
                mock.patch.object(local_gate, "lane_rust_test") as tests:
            self.assertEqual(local_gate.main(), 75)
        tests.assert_not_called()


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


class TestDiffGnuOracleTest(unittest.TestCase):
    """--test-diff also builds the C oracle for windows-gnu when the diff
    touches its C inputs, on a Linux host only (#4168)."""

    def run_test_diff(self, files, platform="linux"):
        cargo = []

        def fake_subprocess(cmd, **kw):
            if cmd[:2] == ["git", "diff"]:
                return subprocess.CompletedProcess(cmd, 0, stdout="\n".join(files))
            if "metadata" in cmd:
                root = local_gate.ROOT
                packages = [
                    {"name": "obs-util", "manifest_path": str(root / "rust/obs-util/Cargo.toml"), "dependencies": []},
                    {"name": "obs-c-oracle", "manifest_path": str(root / "rust/obs-c-oracle/Cargo.toml"),
                     "dependencies": [{"name": "obs-util"}]},
                ]
                return subprocess.CompletedProcess(cmd, 0, stdout=__import__("json").dumps({"packages": packages}))
            raise AssertionError(cmd)

        with (
            mock.patch.object(local_gate.subprocess, "run", side_effect=fake_subprocess),
            mock.patch.object(local_gate, "rust_tests"),
            mock.patch.object(local_gate, "run", side_effect=lambda cmd, *a, **k: cargo.append(cmd)),
            mock.patch.object(local_gate, "soldr_cargo", side_effect=lambda args, env=None: cargo.append(["soldr", "cargo", *args])),
            mock.patch.object(local_gate, "toolchain_gcc_ld", return_value="/sysroot/gcc-ld"),
            mock.patch.object(local_gate.host_platform, "is_linux", return_value=platform == "linux"),
            mock.patch.object(sys, "stdout", new=io.StringIO()),
        ):
            local_gate.test_diff("base")
        return [c for c in cargo if "x86_64-pc-windows-gnu" in c]

    def test_an_oracle_change_builds_the_oracle_for_windows_gnu(self):
        calls = self.run_test_diff(["rust/obs-c-oracle/oracle/output_pause_queries.c"])
        self.assertIn(["soldr", "rustup", "target", "add", "x86_64-pc-windows-gnu"], calls)
        self.assertIn(
            ["soldr", "cargo", "test", "-p", "obs-c-oracle", "--locked", "--no-run", "--target", "x86_64-pc-windows-gnu"], calls
        )

    def test_a_libobs_change_builds_it_too(self):
        self.assertTrue(self.run_test_diff(["libobs/obs-output.c.h"]))

    def test_a_crate_change_without_oracle_inputs_does_not(self):
        self.assertEqual(self.run_test_diff(["rust/obs-util/src/lib.rs"]), [])

    def test_not_on_a_macos_or_windows_host(self):
        for platform in ("darwin", "win32"):
            with self.subTest(platform=platform):
                self.assertEqual(self.run_test_diff(["rust/obs-c-oracle/build.rs"], platform), [])


class LaneGroupTest(unittest.TestCase):
    """The Docker Tier 2 lane shares no build directory with the host lanes, so
    it runs as its own heavy chain beside them instead of after them (#2769)."""

    def lanes(self):
        import tomllib
        return tomllib.loads((Path(__file__).resolve().parent.parent / "local-gate.toml").read_text())["gate"]["lanes"]

    def test_tier2_linux_runs_beside_the_host_lanes(self):
        lanes = self.lanes()
        groups = {name: lanes[name].get("group", "") for name in ("lint", "rust-test", "cross")}
        self.assertEqual(set(groups.values()), {""}, "the host lanes must stay in one chain")
        docker = lanes["tier2-linux"].get("group", "")
        self.assertTrue(docker, "tier2-linux needs its own heavy group to overlap the host lanes")
        self.assertNotIn(docker, groups.values())

    def test_the_new_group_stays_a_heavy_lane(self):
        self.assertEqual(self.lanes()["tier2-linux"].get("weight", "heavy"), "heavy")


if __name__ == "__main__":
    unittest.main()
