#!/usr/bin/env -S uv run --script
# pyright: basic, reportImplicitRelativeImport=false
"""Which CI jobs a change needs (#436): Linux always; core code adds the
Rust-libobs OBS build; macOS or Windows code adds that platform's jobs.
Platform is decided by path alone (#897): platform code lives in
platform-named files and directories, and core files carry no platform
defines (ci/platform_ifdef_ratchet.py).

    uv run python -m unittest ci/test_ci_scope.py
"""

from __future__ import annotations

import unittest
from pathlib import Path

from ci_scope import scope
from platform_paths import LINUX, MACOS, WINDOWS, platforms

ROOT = Path(__file__).resolve().parent.parent
CPP_TESTS = {"test/cmocka/test_source_select_list.cpp"}


class CiScopeTest(unittest.TestCase):
    def check(self, files, *, core=False, macos=False, windows=False):
        got = scope(files, lambda path: path in CPP_TESTS)
        self.assertEqual(
            (got.core, got.macos, got.windows), (core, macos, windows), files
        )

    def test_docs_and_linux_only_need_linux(self):
        self.check(["docs/rust-port/naming.md"])
        self.check(["plugins/linux-pipewire/pipewire.c"])
        self.check(["libobs/obs-nix-wayland.c.h"], core=True)
        self.check(["libobs/obs-nix.c.h"], core=True)
        self.check(["frontend/widgets/OBSBasicWindow.cpp"])  # "Window", not Windows

    def test_core_code(self):
        self.check(["libobs/obs-source.c.h"], core=True)
        self.check(["rust/obs-util/src/dstr.rs"], core=True)
        self.check(["Cargo.lock"], core=True)

    def test_core_on_a_master_push_also_selects_both_platform_lanes(self):
        # #2916: a `libobs/` or `rust/` only diff selects no platform lane, so a
        # Windows-only (or macOS-only) break in core code reached master unseen
        # (#2910). A master push runs both platform Rust lanes for that diff.
        master_push: dict = {"event": "push", "ref": "master"}
        got = scope(["libobs/obs.c.h"], lambda path: path in CPP_TESTS, **master_push)
        self.assertEqual((got.core, got.macos, got.windows), (True, True, True))
        got = scope(
            ["rust/obs-util/src/dstr.rs"], lambda path: path in CPP_TESTS, **master_push
        )
        self.assertEqual((got.core, got.macos, got.windows), (True, True, True))
        # PRs add Windows; non-master pushes retain path selection.
        for event, windows in (("pull_request", True), ("push", False)):
            got = scope(
                ["libobs/obs.c.h"],
                lambda path: path in CPP_TESTS,
                event=event,
                ref="fix/2910-x",
            )
            self.assertEqual(
                (got.core, got.macos, got.windows), (True, False, windows), (event, got)
            )
        # Scope-policy changes validate their native push routing on real runners.
        got = scope(["ci/ci_scope.py"], lambda path: path in CPP_TESTS, **master_push)
        self.assertEqual((got.core, got.macos, got.windows), (False, True, True))
        # And a platform file keeps its own selection.
        got = scope(
            ["plugins/mac-capture/mac-display-capture.m"],
            lambda path: path in CPP_TESTS,
            **master_push,
        )
        self.assertEqual((got.macos, got.windows), (True, False))

    def test_core_prs_keep_routing_and_queue_adds_both_platforms(self):
        for path in (
            "libobs/obs.c.h",
            "rust/obs-util/src/dstr.rs",
            "Cargo.toml",
            "Cargo.lock",
            "rust-toolchain.toml",
            "ci/ci_scope.py",
        ):
            with self.subTest(path=path):
                pr = scope([path], event="pull_request", ref="3637/merge")
                self.assertTrue(pr.windows)
                self.assertFalse(pr.macos or pr.full)
                queued = scope([path], event="merge_group")
                self.assertTrue(queued.windows and queued.macos)
                self.assertFalse(queued.full)
                pushed = scope([path], event="push", ref="master")
                self.assertTrue(pushed.windows and pushed.macos)

    def test_non_core_prs_keep_platform_path_selection(self):
        for path in ("docs/features.md", "ci/test_ci_scope.py", "plugins/obs-ffmpeg/obs-ffmpeg.c"):
            with self.subTest(path=path):
                got = scope([path], event="pull_request")
                self.assertFalse(got.windows or got.macos or got.full)

    def test_merge_group_and_push_keep_native_core_coverage(self):
        # #3606: the effect parser batch passed with MSVC linking skipped.
        for files in (
            ["rust/obs-graphics/src/effect_parser.rs", "libobs/graphics/effect.c.h"],
            ["rust/obs-util/src/dstr.rs"],
            ["libobs/obs.c.h"],
            ["Cargo.lock"],
            ["libobs/util/platform-cocoa.m.h", "libobs/util/platform-windows.c.h"],
        ):
            with self.subTest(files=files):
                got = scope(files, event="merge_group")
                self.assertTrue(got.core)
                self.assertTrue(got.windows)
                self.assertTrue(got.macos)
                pushed = scope(files, event="push", ref="master")
                self.assertTrue(pushed.windows and pushed.macos)
                self.assertFalse(got.full)
        got = scope(["ci/ci_scope.py"], event="merge_group")
        self.assertTrue(got.windows and got.macos)

    def test_merge_group_restores_selected_heavy_lanes(self):
        files = [
            "libobs/obs.c.h", "frontend/widgets/OBSBasicWindow.cpp",
            "ci/ci_scope.py", "rust/obs-util/src/platform/windows/mod.rs",
            "rust/obs-util/src/platform/macos/mod.rs", "Cargo.lock",
        ]
        queued = scope(files, event="merge_group")
        for lane in ("macos", "windows", "qt", "tier2",
                     "cross_lint", "windows_link"):
            with self.subTest(lane=lane):
                self.assertTrue(getattr(queued, lane))
        self.assertTrue(queued.core)
        self.assertFalse(queued.docs_only)
        pr = scope(files, event="pull_request")
        self.assertTrue(pr.tier2 and pr.qt and pr.cross_lint and pr.windows_link)
        pushed = scope(files, event="push", ref="master")
        self.assertTrue(pushed.tier2 and pushed.qt and pushed.macos and pushed.windows)
        self.assertTrue(pushed.cross_lint)

    def test_queue_event_rejects_malformed_and_reads_combined_diff(self):
        import json
        import os
        import subprocess
        import tempfile
        from unittest import mock
        import ci_scope

        with tempfile.TemporaryDirectory() as directory:
            event = Path(directory) / "event.json"
            with mock.patch.dict(os.environ, {"GITHUB_EVENT_PATH": str(event)}):
                for body in ({}, {"merge_group": {}}, {"merge_group": {"base_sha": "bad"}}):
                    event.write_text(json.dumps(body))
                    with self.assertRaises(ValueError):
                        ci_scope.queue_changed_files()
                group = {"base_sha": "a" * 40, "head_sha": "b" * 40}
                event.write_text(json.dumps({"merge_group": group}))
                files = ["plugins/win-capture/game-capture.c", "plugins/mac-capture/mac-display-capture.m"]
                with mock.patch.object(ci_scope.subprocess, "run", return_value=
                                       subprocess.CompletedProcess([], 0, "\n".join(files))) as run:
                    self.assertEqual(ci_scope.queue_changed_files(), files)
                self.assertEqual(run.call_args.args[0], ["git", "diff", "--name-only", group["base_sha"], group["head_sha"], "--"])
                with mock.patch.object(ci_scope.subprocess, "run", return_value=subprocess.CompletedProcess([], 0, "")):
                    self.assertEqual(ci_scope.queue_changed_files(), [])

    def test_queue_path_fixtures(self):
        # Native platform paths stay narrow; shared/core selects both platforms.
        fixtures = (
            (["plugins/win-capture/game-capture.c"], False, True, False, False),
            (["plugins/mac-capture/mac-display-capture.m"], True, False, False, False),
            (["libobs/util/platform-windows.c.h"], False, True, False, False),
            (["libobs/util/platform-cocoa.m.h"], True, False, False, False),
            (["rust/obs-util/src/platform/windows/mod.rs"], False, True, False, False),
            (["cmake/common/helpers_common.cmake"], True, True, False, True),
            (["frontend/widgets/OBSBasicWindow.cpp"], False, False, True, False),
            (["libobs/obs.c.h"], True, True, False, True),
            (["Cargo.toml"], True, True, False, True),
            (["plugins/obs-ffmpeg/obs-ffmpeg.c"], False, False, False, False),
            (["docs/features.md"], False, False, False, False),
            ([], False, False, False, False),
            # Combined immutable diff from two PRs, rather than either PR head.
            (["plugins/win-capture/game-capture.c", "plugins/mac-capture/mac-display-capture.m"], True, True, False, False),
        )
        for files, macos, windows, qt, tier2 in fixtures:
            with self.subTest(files=files):
                got = scope(files, event="merge_group")
                self.assertEqual((got.macos, got.windows, got.qt, got.tier2),
                                 (macos, windows, qt, tier2))
                non_docs = bool(files) and not got.docs_only
                self.assertEqual(got.windows_link, non_docs)
                self.assertEqual(got.cross_lint, non_docs)
                self.assertFalse(got.full)
        self.assertEqual(scope(["plugins/obs-ffmpeg/obs-ffmpeg.c"], event="merge_group").linux_targets, "obs-ffmpeg")

    def test_queue_runner_selection_preserves_hosted_fallbacks(self):
        import re

        expected = {
            "ci.yml": {
                "cancel-superseded": "ubuntu-latest", "ci-gate": "ubuntu-26.04",
                "tier2-lane-proof": "ubuntu-26.04", "cache-janitor": "ubuntu-26.04",
            },
            "build-project.yaml": dict.fromkeys((
                "check-event", "ci-mode", "rust-tests-linux", "cross-compile",
                "rust-cross-lint", "rust-cross-lint-release", "rust-cross-proof", "rust-windows-link",
                "rust-linux-tier2-build", "rust-linux-tier2", "linux-qt-frontend",
            ), "ubuntu-26.04"),
            "check-format.yaml": {"checks": "ubuntu-latest"},
        }
        for name, jobs in expected.items():
            text = (ROOT / ".github/workflows" / name).read_text()
            selection = ("needs.select-mq-runner.outputs.labels" if name == "ci.yml"
                         else "inputs.mq-runner-labels")
            blocks = dict(re.findall(
                r"^  ([\w-]+):\n(.*?)(?=^  [\w-]+:|\Z)", text, re.MULTILINE | re.DOTALL
            ))
            for job, fallback in jobs.items():
                with self.subTest(workflow=name, job=job):
                    self.assertIn(
                        f"runs-on: ${{{{ {selection} != '' && fromJSON({selection})"
                        f" || '{fallback}' }}}}", blocks[job],
                    )
                    if name == "ci.yml":
                        self.assertRegex(blocks[job], r"needs: .*select-mq-runner")
            self.assertNotIn("fromJSON(vars.MQ_RUNNER_LABELS)", text)
            self.assertEqual(text.count(f"fromJSON({selection})"),
                             len(jobs) + (name == "build-project.yaml"))
            for runner in re.findall(r"^    runs-on: (.+)$", text, re.MULTILINE):
                if runner.startswith(("macos-", "windows-")):
                    self.assertNotIn("MQ_RUNNER_LABELS", runner)
            if name == "build-project.yaml":
                self.assertIn("startsWith(matrix.os, 'ubuntu-')", blocks["ubuntu-build"])
                self.assertIn(f"fromJSON({selection})", blocks["ubuntu-build"])
                self.assertIn(" || matrix.os }}", blocks["ubuntu-build"])
            if name != "ci.yml":
                self.assertIn("mq-runner-labels:", text)
                self.assertIn("default: ''", text)

    def test_queue_runner_probe_and_reusable_inputs(self):
        import re

        text = (ROOT / ".github/workflows/ci.yml").read_text()
        blocks = dict(re.findall(
            r"^  ([\w-]+):\n(.*?)(?=^  [\w-]+:|\Z)", text, re.MULTILINE | re.DOTALL
        ))
        probe = blocks["select-mq-runner"]
        self.assertIn("runs-on: ubuntu-latest", probe)
        self.assertNotRegex(probe, r"(?m)^    if:")  # Never skip dependants on PRs.
        self.assertIn("github.event_name == 'merge_group' && vars.MQ_RUNNER_LABELS != ''", probe)
        steps = dict(re.findall(
            r"^      - name: ([^\n]+)\n(.*?)(?=^      - |\Z)", probe, re.MULTILINE | re.DOTALL
        ))
        app = steps["Create Runner Probe App Token"]
        self.assertRegex(app, r"uses: actions/create-github-app-token@[0-9a-f]{40} # v[23]\.")
        self.assertIn("id: mq-probe-app", app)
        self.assertIn("continue-on-error: true", app)
        self.assertIn(
            "if: github.event_name == 'merge_group' && vars.MQ_RUNNER_LABELS != ''"
            " && vars.MQ_PROBE_APP_ID != ''\n", app,
        )
        for setting in (
            "app-id: ${{ vars.MQ_PROBE_APP_ID }}",
            "private-key: ${{ secrets.MQ_PROBE_APP_PRIVATE_KEY }}",
            "owner: ${{ github.repository_owner }}",
        ):
            self.assertIn(setting, app)
        self.assertLess(probe.index("id: mq-probe-app"), probe.index("id: select"))
        self.assertIn(
            "MQ_RUNNER_STATUS_TOKEN: ${{ steps.mq-probe-app.outputs.token"
            " || secrets.MQ_RUNNER_STATUS_TOKEN }}", steps["Check Runner Availability"],
        )
        self.assertIn("run: uv run --no-project ci/select_mq_runner.py", probe)
        self.assertIn("labels: ${{ steps.select.outputs.labels }}", probe)
        self.assertNotRegex(probe, r"(?m)^    continue-on-error:")
        self.assertEqual(probe.count("continue-on-error: true"), 4)
        for step in ("checkout", "uv"):
            self.assertIn(f"steps.{step}.outcome == 'success'", probe)
            self.assertIn(f"steps.{step}.outcome == 'failure'", probe)
        self.assertIn("steps.select.outcome == 'failure'", probe)
        self.assertIn("::warning::", probe)
        for job in ("check-format", "build-project"):
            self.assertIn("needs: select-mq-runner", blocks[job])
            self.assertIn("mq-runner-labels: ${{ needs.select-mq-runner.outputs.labels }}", blocks[job])

    def test_queue_runs_merge_result_tests_and_windows_link(self):
        text = (ROOT / ".github/workflows/build-project.yaml").read_text()
        job = text.split("  rust-windows-link:\n", 1)[1].split("    steps:", 1)[0]
        self.assertIn("needs.check-event.outputs.windows_link == 'true'", job)
        self.assertIn('echo "windows_link=${windows_link:-true}"', text)
        self.assertIn("(github.event_name == 'merge_group' || needs.ci-mode.outputs.rust-test_hit != 'true')", text)

    def test_core_file_content_is_not_read(self):
        # Core files carry no platform defines (the ratchet lint enforces
        # it), so a core file is Linux-only whatever it contains (#897).
        self.check(["libobs/obs.c.h"], core=True)
        self.check(["frontend/OBSApp.cpp"])
        self.check(["plugins/obs-ffmpeg/CMakeLists.txt"])

    def test_macos_code(self):
        self.check(["plugins/mac-capture/mac-display-capture.m"], macos=True)
        self.check(
            ["frontend/utility/CrashHandler_MacOS.mm"]
        )  # Qt area: follow-up, no OS Rust/Tier 2
        self.check(["libobs/util/platform-cocoa.m.h"], core=True, macos=True)
        self.check(["libobs/util/apple/cfstring-utils.h"], core=True, macos=True)
        self.check(["cmake/macos/compilerconfig.cmake"], macos=True)
        # coreaudio-encoder builds on Windows too (PLATFORMS WINDOWS MACOS, #1051).
        self.check(["plugins/coreaudio-encoder/encoder.cpp"], macos=True, windows=True)
        self.check(
            ["libobs/audio-monitoring/osx/coreaudio-output.c.h"], core=True, macos=True
        )

    def test_posix_code_reaches_macos(self):
        # macOS builds the POSIX sources and util/platform-nix.c too
        # (libobs/cmake/os-macos.cmake).
        self.check(["libobs/util/threading-posix.c.h"], core=True, macos=True)
        self.check(["libobs/util/pipe-posix.c.h"], core=True, macos=True)
        self.check(["libobs/util/platform-nix.c.h"], core=True, macos=True)
        self.check(["shared/ipc-util/ipc-util/pipe-posix.c"], macos=True)
        self.check(["plugins/text-freetype2/find-font-unix.c"], macos=True)
        self.check(["rust/obs-util/src/platform/unix/pipe.rs"], core=True, macos=True)
        # The rest of the nix family is Linux and BSD only.
        self.check(["libobs/util/platform-nix-dbus.c.h"], core=True)

    def test_windows_code(self):
        self.check(
            ["plugins/win-capture/graphics-hook/d3d11-capture.cpp"], windows=True
        )
        self.check(["frontend/utility/CrashHandler_Windows.cpp"])  # Qt area: follow-up
        self.check(["libobs/util/threading-windows.h"], core=True, windows=True)
        self.check(["libobs/obs-win-crash-handler.c.h"], core=True, windows=True)
        self.check(["deps/w32-pthreads/pthread.c"], windows=True)
        self.check(["cmake/windows/compilerconfig.cmake"], windows=True)
        self.check(
            ["rust/obs-util/src/platform/windows/pipe.rs"], core=True, windows=True
        )

    def test_rust_platform_directories_map_exactly(self):
        self.check(["rust/obs-nvenc/src/platform/macos.rs"], core=True, macos=True)
        self.check(["rust/obs-nvenc/src/platform/windows.rs"], core=True, windows=True)
        self.check(["rust/obs-media-io/src/platform/unix.rs"], core=True, macos=True)
        self.check(["rust/obs-util/src/platform/linux/x.rs"], core=True)

    def test_shared_build_and_ci_changes_need_every_platform(self):
        for path in [
            "CMakeLists.txt",
            "cmake/common/helpers_common.cmake",
            "buildspec.json",
            "libobs/cmake/rust-hidden-base.txt",
            "rust/obs-util/src/platform/mod.rs",
        ]:
            got = scope([path])
            self.assertTrue(got.macos and got.windows, path)

    def test_rust_build_scripts_select_native_platforms(self):
        # Build scripts choose native compiler flags and C sources (#1566).
        for path in (
            "rust/obs-c-oracle/build.rs",
            "rust/obs-util/build.rs",
            "rust/obs-sys/build.rs",
            "rust/future-crate/build.rs",
        ):
            with self.subTest(path=path):
                got = scope([path])
                self.assertTrue(got.core)
                self.assertTrue(got.windows)
                self.assertTrue(got.macos)
                self.assertFalse(got.full)
                self.assertFalse(got.qt)

    def test_ordinary_rust_files_do_not_select_native_platforms(self):
        for path in (
            "rust/obs-util/src/dstr.rs",
            "rust/obs-util/src/build.rs",
            "rust/obs-util/tests/fixtures/build.rs",
            "rust/obs-util/build.rs.bak",
            "docs/build.rs",
        ):
            with self.subTest(path=path):
                got = scope([path])
                self.assertFalse(got.windows)
                self.assertFalse(got.macos)

    def test_cpp_cmocka_tests_and_test_build_config_need_every_platform(self):
        # Each platform's Tier 2 compiles them with its own compiler (#610).
        for path in [
            "test/cmocka/test_source_select_list.cpp",
            "test/cmocka/tests.d/test_source_select_list.cmake",
            "test/cmocka/CMakeLists.txt",
        ]:
            got = scope([path], lambda p: p in CPP_TESTS)
            self.assertTrue(got.macos and got.windows, path)

    def test_c_cmocka_tests_stay_linux_only(self):
        self.check(["test/cmocka/test_pipe_wait_error.c"])
        self.check(["test/cmocka/tests.d/test_pipe_wait_error.cmake"])


class QtScopeTest(unittest.TestCase):
    FRAGMENTS = {
        "test/cmocka/tests.d/test_source_select_list.cmake": "find_package(Qt6 QUIET COMPONENTS Core)",
        "test/cmocka/tests.d/test_pipe_wait_error.cmake": "add_executable(test_pipe_wait_error ...)",
    }

    def qt(self, *files):
        return scope(
            list(files), lambda p: False, lambda p: self.FRAGMENTS.get(p, "")
        ).qt

    def test_frontend_and_qt_tests_run_the_job(self):
        self.assertTrue(self.qt("frontend/OBSApp.cpp"))
        self.assertTrue(self.qt("shared/qt/wrappers/qt-wrappers.cpp"))
        self.assertTrue(self.qt("test/cmocka/test_source_select_list.cpp"))
        self.assertTrue(self.qt("test/cmocka/tests.d/test_source_select_list.cmake"))
        self.assertTrue(self.qt("test/cmocka/CMakeLists.txt"))
        self.assertFalse(
            self.qt(".github/workflows/build-project.yaml")
        )  # only the Qt job's own files
        self.assertTrue(self.qt("ci/linux_qt_frontend.py"))

    def test_qt_cache_contract_changes_run_the_job(self):
        for path in ("ci/test_qt_cache_budget.py", "ci/test_qt_cache_identity.py"):
            with self.subTest(path=path):
                self.assertTrue(self.qt(path))

    def test_other_changes_skip_the_job(self):
        self.assertFalse(self.qt("docs/rust-port/naming.md"))
        self.assertFalse(self.qt("libobs/obs-source.c.h", "rust/obs-util/src/dstr.rs"))
        self.assertFalse(self.qt("test/cmocka/test_pipe_wait_error.c"))
        self.assertFalse(self.qt("test/cmocka/tests.d/test_pipe_wait_error.cmake"))


class PlatformPathsTest(unittest.TestCase):
    def test_classification(self):
        self.assertIsNone(platforms("libobs/obs.c.h"))
        self.assertIsNone(platforms("frontend/utility/platform.hpp"))  # the facade
        self.assertEqual(platforms("frontend/utility/platform-x11.cpp"), {LINUX})
        self.assertEqual(platforms("frontend/utility/platform-osx.mm"), {MACOS})
        self.assertEqual(platforms("frontend/utility/platform-windows.cpp"), {WINDOWS})
        self.assertEqual(platforms("libobs/util/threading-posix.c.h"), {LINUX, MACOS})
        self.assertEqual(platforms("libobs/util/platform-nix.c.h"), {LINUX, MACOS})
        self.assertEqual(platforms("plugins/linux-v4l2/v4l2-input.c"), {LINUX})

    def test_modules_follow_their_cmake_platforms(self):
        """A plugin's OSes are its PLATFORMS in plugins/CMakeLists.txt, also
        when its name doesn't say them (#1051)."""
        import re
        from pathlib import Path

        names = {
            "WINDOWS": WINDOWS,
            "MACOS": MACOS,
            "LINUX": LINUX,
            "FREEBSD": LINUX,
            "OPENBSD": LINUX,
        }
        every = {LINUX, MACOS, WINDOWS}
        text = (
            Path(__file__).resolve().parent.parent / "plugins" / "CMakeLists.txt"
        ).read_text()
        rows = re.findall(r"add_\w+\((\S+)\s+PLATFORMS\s+([A-Z ]+)", text)
        self.assertGreater(len(rows), 10)
        for module, declared in rows:
            with self.subTest(module=module):
                got = platforms(f"plugins/{module}/x.c") or every
                self.assertEqual(
                    set(got), {names[d] for d in declared.split() if d in names}
                )

    def test_file_name_inside_a_module_still_narrows(self):
        self.assertEqual(
            platforms("plugins/coreaudio-encoder/encoder.cpp"), {MACOS, WINDOWS}
        )
        self.assertEqual(
            platforms("plugins/coreaudio-encoder/windows-imports.h"), {WINDOWS}
        )
        self.assertEqual(platforms("plugins/obs-text/gdiplus/obs-text.cpp"), {WINDOWS})

    def test_windows_only_directories(self):
        for path in (
            "frontend/updater/updater.cpp",
            "plugins/obs-ffmpeg/obs-amf-test/obs-amf-test.cpp",
            "plugins/obs-qsv11/obs-qsv-test/obs-qsv-test.cpp",
            "shared/obs-inject-library/inject-library.c",
            "shared/obs-shared-memory-queue/shared-memory-queue.c",
        ):
            self.assertEqual(platforms(path), {WINDOWS}, path)
        self.assertIsNone(platforms("plugins/obs-ffmpeg/obs-ffmpeg.c"))
        self.assertIsNone(platforms("plugins/obs-qsv11/obs-qsv11.c"))


LIBOBS_RUST = """[package]
name = "libobs-rust"
[dependencies]
obs-util = { path = "../obs-util" }
obs-data = { path = "../obs-data" }
[dev-dependencies]
obs-c-oracle = { path = "../obs-c-oracle" }
"""
CARGO = {
    "rust/libobs-rust/Cargo.toml": LIBOBS_RUST,
    "rust/obs-data/Cargo.toml": '[dependencies]\nobs-util = { path = "../obs-util" }\n',
    "rust/obs-util/Cargo.toml": "[dependencies]\n",
}
SUBS = frozenset(
    {"plugins/obs-websocket", "plugins/obs-browser", "deps/libdshowcapture/src"}
)


class OsLaneTest(unittest.TestCase):
    """The one platform rule and the OS lanes it starts (#992)."""

    def os(self, path):
        from ci_scope import os_lanes

        return set(os_lanes(path))

    def test_platform_rule_per_os(self):
        self.assertEqual(
            self.os("rust/obs-util/src/platform/windows/mod.rs"), {"windows"}
        )
        self.assertEqual(self.os("rust/obs-util/src/platform/macos/mod.rs"), {"macos"})
        self.assertEqual(self.os("rust/obs-util/src/platform/linux/mod.rs"), {"linux"})
        self.assertEqual(
            self.os("rust/obs-util/src/platform/posix/mod.rs"), {"linux", "macos"}
        )
        self.assertEqual(
            self.os("plugins/text-freetype2/text-freetype2-host-windows.c"), {"windows"}
        )
        self.assertEqual(
            self.os("shared/ipc-util/platform/posix/pipe.c"), {"linux", "macos"}
        )

    def test_core_is_linux_only(self):
        for f in (
            "libobs/obs-source.c.h",
            "rust/obs-util/src/dstr.rs",
            "frontend/OBSApp.cpp",
            "shared/x/platform.c",
        ):
            self.assertEqual(self.os(f), {"linux"}, f)

    def test_legacy_names_still_fall_back(self):
        self.assertEqual(self.os("plugins/win-capture/a.c"), {"windows"})
        self.assertEqual(self.os("libobs/obs-cocoa.m.h"), {"macos"})

    def test_outside_every_area_starts_no_os(self):
        self.assertEqual(self.os("docs/platform-windows.md"), set())

    def test_libobs_group_files_start_their_module_os(self):
        # #3264: a libobs group file and the precompiled header its groups
        # include start the OS of the lib-module libobs/build/modules.d puts
        # the group in. By name alone, libobs.windows+.h.c started only Linux,
        # so a Windows-only group change never built on Windows in a PR.
        cases = {
            "libobs/build/libobs.windows+.h.c": {"windows"},
            "libobs/build/libobs.obfuscate-windows+.h.c": {"windows"},
            "libobs/build/libobs-obfuscate.pch.h": {"windows"},
            "libobs/build/libobs.graphics-ffmpeg-host-windows+.h.c": {"windows"},
            "libobs/build/libobs.macos+.h.c": {"macos"},
            "libobs/build/libobs.graphics-ffmpeg-host-posix+.h.c": {"linux", "macos"},
            "libobs/build/libobs.nix+.h.c": {"linux"},
        }
        for path, want in cases.items():
            self.assertEqual(self.os(path), want, path)

    def test_core_module_group_files_stay_linux(self):
        for path in ("libobs/build/libobs.core+.h.c", "libobs/build/libobs.pch.h"):
            self.assertEqual(self.os(path), {"linux"}, path)


class LaneTableTest(unittest.TestCase):
    """One test per LANES row: adding a module is adding a row (#992)."""

    def sc(self, *files):
        return scope(list(files), read=lambda p: CARGO.get(p, ""), submodules=SUBS)

    def test_linked_crates_follow_non_dev_dependencies(self):
        from ci_scope import linked_crates

        self.assertEqual(
            linked_crates(lambda p: CARGO.get(p, "")),
            {"libobs-rust", "obs-util", "obs-data"},
        )

    def test_tier2_rows(self):
        for f in (
            "libobs/obs-source.c.h",
            "test/cmocka/test_x.c",
            "deps/jansson/x.c",
            "rust/tools/linux-validate/validate.sh",
            "Cargo.lock",
            "rust/obs-data/src/lib.rs",  # linked into libobs
            "rust/libobs-rust/src/lib.rs",
            # The in-process MCP cases of obs-tests-app run in Tier 2 (#3319).
            "rust/obs-mcp/src/host.rs",
            "rust/obs-mcp/tests/libobs_video.rs",
            "rust/obs-studio-rust/src/main.rs",
            "rust/obs-studio-rust/tests/cli/mcp_stdio.rs",
            "rust/obs-sys/src/ffi/mod.rs",
        ):
            self.assertTrue(self.sc(f).tier2, f)

    def test_tier2_skips_unrelated_paths(self):
        for f in (
            "docs/ci-tooling.md",
            "frontend/OBSApp.cpp",
            "plugins/obs-filters/expander-filter.c",
            "ci/ci_scope.py",
            ".github/workflows/build-project.yaml",
            "rust/obs-gpu-status/src/lib.rs",  # not linked into libobs
            "rust/obs-c-oracle/src/lib.rs",  # dev-dependency only
        ):
            self.assertFalse(self.sc(f).tier2, f)

    def test_no_pr_path_escalates_to_full(self):
        for f in (
            "build-aux/appimage/build.sh",
            ".gitmodules",
            "plugins/obs-websocket",
            "plugins/obs-browser/x.cpp",
            ".github/workflows/build-project.yaml",
        ):
            self.assertFalse(self.sc(f).full, f)

    def test_workflow_edits_start_no_platform_job(self):
        got = self.sc(".github/workflows/build-project.yaml")
        self.assertFalse(got.macos or got.windows or got.qt or got.tier2)

    def test_libobs_os_cmake_starts_only_its_os(self):
        # libobs/cmake/os-<os>.cmake picks one OS's sources (#1051).
        win = self.sc("libobs/cmake/os-windows.cmake")
        self.assertEqual((win.windows, win.macos, win.tier2), (True, False, False))
        mac = self.sc("libobs/cmake/os-macos.cmake")
        self.assertEqual((mac.windows, mac.macos, mac.tier2), (False, True, False))
        for path in ("libobs/cmake/os-linux.cmake", "libobs/cmake/os-freebsd.cmake"):
            got = self.sc(path)
            self.assertEqual(
                (got.windows, got.macos, got.tier2), (False, False, True), path
            )
        # Any other OS-named libobs/cmake file starts only its OS too (#3237).
        for path in (
            "libobs/cmake/rust-os-windows.cmake",
            "libobs/cmake/windows/obs-module.rc.in",
        ):
            got = self.sc(path)
            self.assertEqual(
                (got.windows, got.macos, got.tier2), (True, False, False), path
            )
        for path in (
            "libobs/cmake/rust-os-macos.cmake",
            "libobs/cmake/macos/entitlements.plist",
        ):
            got = self.sc(path)
            self.assertEqual(
                (got.windows, got.macos, got.tier2), (False, True, False), path
            )
        for path in (
            "libobs/cmake/rust-os-nix.cmake",
            "libobs/cmake/linux/libobs.pc.in",
        ):
            got = self.sc(path)
            self.assertEqual(
                (got.windows, got.macos, got.tier2), (False, False, True), path
            )
        # Every other libobs/cmake file stays shared.
        got = self.sc("libobs/cmake/rust.cmake")
        self.assertEqual((got.windows, got.macos, got.tier2), (True, True, True))

    def test_tier2_follows_sources_cmocka_tests_compile(self):
        # A cmocka test that builds plugin or frontend sources must run when
        # those sources change, including a submodule bump (#1996 skipped
        # Tier 2 while bumping obs-websocket's RequestBatchHandler.cpp).
        from ci_scope import cmocka_source_paths

        refs = cmocka_source_paths(
            [
                'set(_ws_src "${CMAKE_SOURCE_DIR}/plugins/obs-websocket/src")\n',
                'add_executable(t "${CMAKE_SOURCE_DIR}/frontend/utility/Foo.cpp")\n',
                "list(APPEND s ${CMAKE_SOURCE_DIR}/plugins/linux-v4l2/${f})\n",
                'target_include_directories(t PRIVATE "${CMAKE_SOURCE_DIR}/frontend")\n',
            ]
        )
        self.assertNotIn("frontend", refs)
        sc = lambda f: scope([f], read=lambda p: CARGO.get(p, ""), cmocka_paths=refs)
        for f in (
            "plugins/obs-websocket",  # gitlink
            "plugins/obs-websocket/src/requesthandler/RequestBatchHandler.cpp",
            "frontend/utility/Foo.cpp",
            "plugins/linux-v4l2/v4l2-input.c",
        ):
            self.assertTrue(sc(f).tier2, f)
        for f in (
            "frontend/utility/Bar.cpp",
            "plugins/obs-websocket-extra/x.cpp",
            "plugins/obs-filters/x.c",
        ):
            self.assertFalse(sc(f).tier2, f)

    def test_tier2_reads_the_real_cmocka_tests(self):
        from ci_scope import repo_cmocka_source_paths

        refs = repo_cmocka_source_paths()
        self.assertTrue(scope(["plugins/obs-websocket"], cmocka_paths=refs).tier2)

    def test_platform_only_libobs_file_skips_linux_tier2(self):
        self.assertFalse(self.sc("libobs/util/platform-windows.c.h").tier2)
        self.assertFalse(self.sc("libobs/util/platform-cocoa.m.h").tier2)
        self.assertTrue(self.sc("libobs/util/platform-nix.c.h").tier2)
        self.assertTrue(self.sc("libobs/util/threading-posix.c.h").tier2)

    def test_submodule_root_selects_its_linux_target(self):
        for module in ("obs-websocket", "obs-browser"):
            self.assertEqual(self.sc(f"plugins/{module}").linux_targets, module)

    def test_linux_targets_are_the_touched_modules(self):
        self.assertEqual(
            self.sc("plugins/obs-filters/expander-filter.c").linux_targets,
            "obs-filters",
        )
        self.assertEqual(
            self.sc(
                "plugins/obs-filters/a.c",
                "shared/happy-eyeballs/b.c",
                "plugins/obs-filters/c.c",
            ).linux_targets,
            "happy-eyeballs,obs-filters",
        )

    def test_linux_targets_skip_other_os_qt_and_non_modules(self):
        for f in (
            "plugins/win-capture/a.c",
            "plugins/mac-capture/a.m",
            "plugins/frontend-tools/a.cpp",
            "shared/qt/a.cpp",
            "plugins/CMakeLists.txt",
            "libobs/a.c",
            "docs/a.md",
        ):
            self.assertEqual(self.sc(f).linux_targets, "", f)

    def test_qt_row_is_frontend_only(self):
        self.assertTrue(self.sc("frontend/OBSApp.cpp").qt)
        self.assertFalse(self.sc("libobs/obs-source.c.h").qt)
        self.assertFalse(self.sc("rust/obs-util/src/a.rs").qt)

    def test_docs_start_nothing(self):
        got = vars(self.sc("docs/rust-port/naming.md"))
        # Only docs_only differs from an empty diff (#2079).
        self.assertTrue(got.pop("docs_only"))
        empty = vars(scope([]))
        empty.pop("docs_only")
        self.assertEqual(got, empty)


class DocsOnlyTest(unittest.TestCase):
    """A diff of only documentation skips Fast Gate's Rust lint (#2079).

    Markdown is still read by ci/test_*.py, which the Format job runs on
    every PR, so only the Rust lint (rustfmt, clippy, Dylints) is skipped.
    """

    def docs_only(self, *files):
        return scope(list(files)).docs_only

    def test_markdown_anywhere_is_docs_only(self):
        self.assertTrue(self.docs_only("docs/rust-port/coordination.md"))
        self.assertTrue(
            self.docs_only(
                "AGENTS.md", ".claude/skills/obs-worker/SKILL.md", "rust/README.md"
            )
        )
        self.assertTrue(self.docs_only("build-aux/appimage/README.md"))

    def test_ci_evidence_license_and_gitignore_are_docs_only(self):
        self.assertTrue(
            self.docs_only(
                "docs/ci-evidence/2026-10-22-runtime-package-experiment.json"
            )
        )
        self.assertTrue(
            self.docs_only("LICENSE", "COPYING", ".gitignore", "docs/features.md")
        )

    def test_any_other_file_keeps_the_lint(self):
        self.assertFalse(
            self.docs_only("docs/rust-port/coordination.md", "ci/test_claim_bot.py")
        )
        self.assertFalse(self.docs_only("AGENTS.md", "rust/obs-util/src/lib.rs"))
        self.assertFalse(self.docs_only(".github/ISSUE_TEMPLATE/bug.yml"))
        self.assertFalse(self.docs_only("docs/rust-port/diagram.svg"))
        self.assertFalse(self.docs_only("rust/obs-util/src/notes.md.rs"))

    def test_empty_diff_is_not_docs_only(self):
        self.assertFalse(self.docs_only())

    def test_fast_gate_skips_only_deferred_docs_only_runs(self):
        text = (ROOT / ".github/workflows/build-project.yaml").read_text()
        self.assertIn("docs_only: ${{ steps.native.outputs.docs_only }}", text)
        # A master push, the release gate and full runs never skip it.
        self.assertIn(
            'if [[ "${deferred}" != true ]]; then\n            qt=true tier2=true docs_only=false',
            text,
        )
        self.assertIn('echo "docs_only=${docs_only:-false}"', text)
        job = text.split("  rust-tests-linux:\n", 1)[1].split("    steps:", 1)[0]
        self.assertIn("needs.check-event.outputs.docs_only != 'true'", job)


PR_1292_FILES = [
    "ci/platform-ifdef-baseline.json",
    "cmake/cross/CMakeLists.txt",
    "plugins/frontend-tools/CMakeLists.txt",
    "plugins/frontend-tools/auto-scene-switcher-nix.cpp.hpp",
    "plugins/frontend-tools/auto-scene-switcher-osx.mm.hpp",
    "plugins/frontend-tools/auto-scene-switcher-win.cpp.hpp",
    "plugins/frontend-tools/auto-scene-switcher.cpp.hpp",
    "plugins/frontend-tools/build/frontend-tools-c.pch.h",
    "plugins/frontend-tools/build/frontend-tools-captions-windows.pch.hpp",
    "plugins/frontend-tools/build/frontend-tools-host-macos.pch.h",
    "plugins/frontend-tools/build/frontend-tools-host-posix.pch.h",
    "plugins/frontend-tools/build/frontend-tools-host-windows.pch.h",
    "plugins/frontend-tools/build/frontend-tools-linux.pch.hpp",
    "plugins/frontend-tools/build/frontend-tools-macos.pch.hpp",
    "plugins/frontend-tools/build/frontend-tools-scripts.pch.hpp",
    "plugins/frontend-tools/build/frontend-tools-windows.pch.hpp",
    "plugins/frontend-tools/build/frontend-tools.captions-windows+.hpp.cpp",
    "plugins/frontend-tools/build/frontend-tools.host-macos+.h.c",
    "plugins/frontend-tools/build/frontend-tools.host-posix+.h.c",
    "plugins/frontend-tools/build/frontend-tools.host-windows+.h.c",
    "plugins/frontend-tools/build/frontend-tools.pch.hpp",
    "plugins/frontend-tools/captions-host-windows.hpp",
    "plugins/frontend-tools/captions-mssapi-stream.hpp",
    "plugins/frontend-tools/captions-mssapi.hpp",
    "plugins/frontend-tools/captions.cpp.hpp",
    "plugins/frontend-tools/frontend-tools-host-macos.c.h",
    "plugins/frontend-tools/frontend-tools-host-posix.c.h",
    "plugins/frontend-tools/frontend-tools-host-windows.c.h",
    "plugins/frontend-tools/frontend-tools-host.h",
    "plugins/frontend-tools/frontend-tools.c.h",
    "plugins/frontend-tools/scripts.cpp.hpp",
    "test/cmocka/test_frontend_tools_host.c",
    "test/cmocka/test_frontend_tools_host_macos.c",
    "test/cmocka/tests.d/test_frontend_tools_host.cmake",
]


class HostNamedFilesTest(unittest.TestCase):
    """Host-named files (docs/rust-port/naming.md: `-host-<os>`, `hosts/<os>/`,
    `-<os>.pch.<ext>`) start their OS lane in any area, Qt areas included (#1294)."""

    def test_pr_1292_selects_windows_and_macos(self):
        got = scope(PR_1292_FILES)
        self.assertTrue(got.windows)
        self.assertTrue(got.macos)
        self.assertTrue(got.qt)

    def test_hosts_directory(self):
        got = scope(["plugins/frontend-tools/hosts/windows/x.c"])
        self.assertEqual((got.windows, got.macos, got.qt), (True, False, True))
        got = scope(["frontend/hosts/macos/x.mm"])
        self.assertEqual((got.windows, got.macos), (False, True))
        self.assertTrue(scope(["plugins/obs-filters/hosts/windows/x.c"]).windows)

    def test_qt_host_files_each_select_their_os(self):
        for path, win, mac in (
            ("plugins/frontend-tools/frontend-tools-host-windows.c.h", True, False),
            ("plugins/frontend-tools/frontend-tools-host-macos.c.h", False, True),
            ("plugins/frontend-tools/frontend-tools-host-posix.c.h", False, True),
            (
                "plugins/frontend-tools/build/frontend-tools-windows.pch.hpp",
                True,
                False,
            ),
            (
                "plugins/frontend-tools/build/frontend-tools.host-windows+.h.c",
                True,
                False,
            ),
            ("plugins/frontend-tools/captions-host-windows.hpp", True, False),
        ):
            got = scope([path])
            self.assertEqual((got.windows, got.macos), (win, mac), path)

    def test_plain_qt_file_still_selects_no_os(self):
        for path in (
            "plugins/frontend-tools/frontend-tools.c.h",
            "plugins/frontend-tools/scripts.cpp.hpp",
            "frontend/OBSApp.cpp",
        ):
            got = scope([path])
            self.assertEqual((got.windows, got.macos), (False, False), path)
        # Upstream platform tokens in a Qt area keep their old behaviour.
        got = scope(["plugins/frontend-tools/auto-scene-switcher-win.cpp.hpp"])
        self.assertEqual((got.windows, got.macos), (False, False))

    def test_host_linux_selects_neither(self):
        for path in (
            "plugins/frontend-tools/frontend-tools-host-linux.c.h",
            "frontend/hosts/linux/x.cpp",
            "plugins/frontend-tools/build/x-linux.pch.hpp",
        ):
            got = scope([path])
            self.assertEqual((got.windows, got.macos), (False, False), path)

    def test_qt_sdk_paths_fetch_published_sdks(self):
        # rust/obs-build's Qt SDK resolver runs against the published
        # Windows and macOS SDKs on their own runners (#1878).
        for path in (
            "rust/obs-build/src/qt.rs",
            "rust/obs-build/src/qt/sdk.rs",
            "rust/obs-build/src/platform/qt_sdk.rs",
            "rust/obs-build/tests/qt_sdk.rs",
            "rust/obs-build/tests/native.rs",
            "rust/obs-build/src/validation/graph.rs",
            "rust/obs-build/src/platform/shared_library.rs",
        ):
            got = scope([path])
            self.assertEqual(
                (got.qt_sdk, got.macos, got.windows), (True, True, True), path
            )
        for path in ("rust/obs-build/src/libobs.rs", "rust/obs-util/src/dstr.rs"):
            self.assertFalse(scope([path]).qt_sdk, path)

    def test_no_event_selects_a_sanitizer_lane(self):
        # Maintainer directive 2026-11-09: ASan runs nightly only
        # (sanitizers.yaml schedule), never on a PR or in the merge queue, so
        # the scope has no sanitizer lane at all, ratcheted target or not.
        files = ["frontend/OBSApp.cpp", "plugins/decklink-captions/CMakeLists.txt"]
        for event in ("pull_request", "merge_group", "push", ""):
            got = scope(files, event=event, ref="master")
            self.assertNotIn("asan", vars(got), event)

if __name__ == "__main__":
    unittest.main()
