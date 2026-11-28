#!/usr/bin/env -S uv run --script
"""Windows Tier 2 runs on Linux under headless Wine in the merge queue (#5844).

    uv run --no-project python -m unittest ci/test_tier2_wine.py

The queue cross-builds the cmocka executables and runs them under Wine; the
native windows-2025-vs2026 job (with its C-reference baseline) runs everywhere
else. Wine must never open a window on a developer's desktop.
"""

from __future__ import annotations

import re
import tempfile
from unittest import mock
import unittest
from pathlib import Path

import tier2_wine
from test_merge_queue import BUILD, job_block

ROOT = Path(__file__).resolve().parent.parent


def condition(job: str) -> str:
    header = job_block(BUILD, job).split("    steps:", 1)[0]
    return re.search(r"^    if: (.+)$", header, re.M).group(1)


class WorkflowTest(unittest.TestCase):
    def test_wine_job_runs_on_prs_and_the_queue_and_runs_the_script(self):
        block = job_block(BUILD, "rust-tier2-windows-wine")
        self.assertTrue(block)
        wine = condition("rust-tier2-windows-wine")
        self.assertIn("needs.check-event.outputs.windows == 'true'", wine)
        self.assertIn("fast_path != 'true'", wine)
        self.assertNotIn("merge_group", wine)
        self.assertIn("ci/cmocka_link_check.py --alias win-x64 --all-tests", block)
        self.assertIn("ci/tier2_wine.py", block)
        self.assertNotIn("windows-2025", block)

    def test_native_windows_tier2_leaves_the_queue_and_prs(self):
        native = condition("rust-tier2-windows")
        self.assertIn("github.event_name != 'merge_group'", native)
        self.assertIn("github.event_name != 'pull_request'", native)
        self.assertNotIn("github.event_name == 'merge_group'", native)


class ScopeTest(unittest.TestCase):
    def test_the_lanes_own_files_start_the_windows_lane(self):
        from ci_scope import scope
        for path in ("ci/tier2_wine.py", "ci/cmocka_link_check.py"):
            with self.subTest(path=path):
                self.assertTrue(scope([path], event="pull_request", ref="1/merge").windows)


class WineInstallTest(unittest.TestCase):
    def test_wine_is_unpacked_in_the_workspace_not_installed_to_root(self):
        # The Dell lane root filesystem is too small for `apt-get install wine`
        # ("No space left on device", #5867): download only, dpkg -x into RUNNER_TEMP.
        for job in ("rust-tier2-windows-wine", "rust-tests-windows-wine"):
            block = job_block(BUILD, job)
            with self.subTest(job=job):
                self.assertIn("ci/install-wine.sh", block)
                self.assertNotIn("apt-get -o Acquire::Retries=3 install", block)
        script = (ROOT / "ci" / "install-wine.sh").read_text()
        self.assertIn("install --download-only", script)
        self.assertIn("Dir::Cache::archives=", script)
        self.assertIn("dpkg -x", script)
        self.assertIn("df -T", script)
        self.assertIn("WINESERVER", (ROOT / "ci" / "wine-runner.sh").read_text())


class ExecutableTest(unittest.TestCase):
    def test_suffixless_registrations_mean_the_exe(self):
        build = Path("/build")
        self.assertEqual(tier2_wine.test_exe("/build/test/cmocka/test_audio_io", build),
                         Path("/build/test/cmocka/test_audio_io.exe"))
        self.assertEqual(tier2_wine.test_exe("/build/test/cmocka/x.exe", build), Path("/build/test/cmocka/x.exe"))
        self.assertIsNone(tier2_wine.test_exe("/usr/bin/cmake", build))
        # The display tests register `xvfb-run-or-display.sh -a <exe>` when CI has xvfb.
        self.assertIsNone(tier2_wine.test_exe("/build/test/cmocka/xvfb-run-or-display.sh", build))
        self.assertIsNone(tier2_wine.test_exe("-a", build))

    def test_the_executable_is_found_behind_a_wrapper(self):
        build = Path("/build")
        with tempfile.TemporaryDirectory() as tmp:
            build = Path(tmp)
            (build / "test").mkdir()
            exe = build / "test" / "test_audio_controls.exe"
            exe.touch()
            plan = [{"name": "test_audio_controls", "command": [str(build / "test/x.sh"), "-a", str(exe)]}]
            with mock.patch.object(tier2_wine, "plan", return_value=plan):
                tests = tier2_wine.runnable_tests(build)
            self.assertEqual(tests[0]["command"], [str(exe)])

    def test_no_cross_build_exemption_list_remains(self):
        self.assertFalse(hasattr(tier2_wine, "NOT_CROSS_BUILT"))


class CrossTreeTest(unittest.TestCase):
    """The cross tree must give consumers what the native libobs target does."""

    def test_cross_core_module_manifest_is_not_empty(self):
        text = (ROOT / "cmake/cross/CMakeLists.txt").read_text()
        self.assertNotIn("set(OBS_CORE_MODULE_COUNT 0)", text)
        self.assertIn("obs-outputs", text)
        self.assertIn("obs-transitions", text)

    def test_libobs_exports_pthreads_and_uthash_to_consumers(self):
        text = (ROOT / "cmake/cross/tier2.cmake").read_text()
        self.assertIn("target_link_libraries(libobs-cross-shared INTERFACE OBS::w32-pthreads)", text)
        self.assertIn("target_link_libraries(libobs-cross-shared INTERFACE Uthash::Uthash)", text)


class HeadlessWineTest(unittest.TestCase):
    def test_environment_never_reaches_a_display(self):
        import os
        os.environ["DISPLAY"], os.environ["WAYLAND_DISPLAY"] = ":0", "wayland-0"
        try:
            env = tier2_wine.wine_env(Path("/tmp/prefix"))
        finally:
            del os.environ["DISPLAY"], os.environ["WAYLAND_DISPLAY"]
        self.assertNotIn("DISPLAY", env)
        self.assertNotIn("WAYLAND_DISPLAY", env)
        overrides = env["WINEDLLOVERRIDES"]
        for name in ("mscoree,mshtml=", "winedbg.exe=d", "winemenubuilder.exe=d"):
            self.assertIn(name, overrides)

    def test_prefix_disables_the_crash_dialog_and_debugger(self):
        # The shared runner (#5845) creates the prefix; tier2_wine.py defaults to it.
        runner = (ROOT / "ci" / "wine-runner.sh").read_text()
        self.assertIn("ShowCrashDialog", runner)
        self.assertIn("AeDebug", runner)
        # #5867: the prefix and its TEMP live under RUNNER_TEMP, not the container overlay/tmpfs.
        self.assertIn("${RUNNER_TEMP}/obs-wine-prefix", runner)
        self.assertIn("HKCU\\\\Environment", runner)
        self.assertEqual(tier2_wine.wine_command(), [str(ROOT / "ci" / "wine-runner.sh")])

    def test_quarantine_names_a_reason_for_every_test(self):
        self.assertTrue(tier2_wine.QUARANTINE)
        for name, reason in tier2_wine.QUARANTINE.items():
            self.assertGreater(len(reason), 40, name)


if __name__ == "__main__":
    unittest.main()
