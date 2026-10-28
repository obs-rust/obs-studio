#!/usr/bin/env -S uv run --script
"""Saved-profile startup checks reject missing capture plugins and wrong CPU packages.

uv run python -m unittest ci/test_packaged_startup.py
"""

from __future__ import annotations

import importlib.util
import json
import os
import struct
import tempfile
import unittest
from unittest.mock import AsyncMock, Mock, patch
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location(
    "packaged_startup", ROOT / "test/packaged-startup.py"
)
smoke = importlib.util.module_from_spec(spec)
spec.loader.exec_module(smoke)


class PackagedReadinessTest(unittest.IsolatedAsyncioTestCase):
    def response(self, code, success=False):
        return {
            "d": {
                "requestStatus": {"code": code, "result": success},
                "responseData": {"inputKinds": ["screen_capture"]},
            }
        }

    async def test_not_ready_is_followed_by_a_real_success_response(self):
        client = Mock(
            request=AsyncMock(
                side_effect=[
                    self.response(207),
                    self.response(207),
                    self.response(100, True),
                ]
            )
        )
        result = await smoke.wait_until_ready(
            client, Mock(poll=Mock(return_value=None)), smoke.time.monotonic() + 5
        )
        self.assertEqual(result, {"inputKinds": ["screen_capture"]})
        self.assertEqual(client.request.await_count, 3)
        self.assertTrue(
            all(
                call.args == ("GetInputKindList", {"unversioned": True})
                for call in client.request.await_args_list
            )
        )

    async def test_other_protocol_failures_and_process_exit_are_fatal(self):
        client = Mock(request=AsyncMock(return_value=self.response(702)))
        with self.assertRaisesRegex(RuntimeError, "702"):
            await smoke.wait_until_ready(
                client, Mock(poll=Mock(return_value=None)), smoke.time.monotonic() + 5
            )
        self.assertEqual(client.request.await_count, 1)
        with self.assertRaisesRegex(RuntimeError, "exited"):
            await smoke.wait_until_ready(
                client,
                Mock(poll=Mock(return_value=1), returncode=1),
                smoke.time.monotonic() + 5,
            )
        self.assertEqual(client.request.await_count, 1)

    async def test_not_ready_cannot_extend_the_startup_deadline(self):
        client = Mock(request=AsyncMock(return_value=self.response(207)))
        with patch.object(smoke, "time", Mock(monotonic=Mock(side_effect=[0, 2]))):
            with self.assertRaisesRegex(RuntimeError, "readiness"):
                await smoke.wait_until_ready(
                    client, Mock(poll=Mock(return_value=None)), 1
                )
        self.assertEqual(client.request.await_count, 1)

    async def test_an_unanswered_request_names_the_frontend(self):
        # #2161: OBS connected but never answered; asyncio's bare TimeoutError
        # said nothing about which request or why.
        client = Mock(request=AsyncMock(side_effect=TimeoutError))
        with self.assertRaisesRegex(RuntimeError, "did not answer GetInputKindList"):
            await smoke.wait_until_ready(
                client, Mock(poll=Mock(return_value=None)), smoke.time.monotonic() + 5
            )


class FrontendLogReportTest(unittest.TestCase):
    def test_a_failure_prints_the_newest_frontend_log(self):
        # #2161: the fixture's cleanup deleted the only record of the hang.
        with tempfile.TemporaryDirectory() as tmp:
            logs = Path(tmp) / "logs"
            logs.mkdir()
            (logs / "2026-10-24 13-00-00.txt").write_text("older run\n")
            newest = logs / "2026-10-24 13-29-30.txt"
            newest.write_text("info: Loading module: win-dshow.dll\nhang here\n")
            os.utime(logs / "2026-10-24 13-00-00.txt", (1, 1))
            with patch("builtins.print") as printed:
                smoke.report_frontend_log(Path(tmp))
            text = "\n".join(str(c.args[0]) for c in printed.call_args_list if c.args)
            self.assertIn("hang here", text)
            self.assertNotIn("older run", text)

    def test_no_log_is_reported_without_failing(self):
        with tempfile.TemporaryDirectory() as tmp:
            with patch("builtins.print") as printed:
                smoke.report_frontend_log(Path(tmp))
            self.assertTrue(printed.called)


class PackagedStartupTest(unittest.TestCase):
    def test_windows_loaded_modules_are_reported_and_required_capture_modules_verified(
        self,
    ):
        log = (
            "info:   Loaded Modules:\n"
            + "".join(
                f"info:     {name}.dll\n"
                for name in ("win-capture", "win-dshow", "win-wasapi", "obs-websocket")
            )
            + "info: ---------------------------------\n"
        )
        with patch("builtins.print") as report:
            smoke.require_windows_modules(log)
        self.assertIn("win-dshow.dll", str(report.call_args_list))
        with self.assertRaisesRegex(RuntimeError, "win-dshow.dll"):
            smoke.require_windows_modules(log.replace("info:     win-dshow.dll\n", ""))

    def test_windows_log_without_loaded_module_checkpoint_fails(self):
        with self.assertRaisesRegex(RuntimeError, "Loaded Modules"):
            smoke.require_windows_modules("info: startup completed\n")

    def test_macos_shutdown_timeout_captures_the_owned_live_process(self):
        proc = Mock(pid=123, poll=Mock(return_value=None))
        proc.wait.side_effect = smoke.subprocess.TimeoutExpired(["owned-app"], 30)
        with patch.object(smoke.subprocess, "run") as sample:
            sample.return_value = Mock(
                stdout="owned shutdown stack", stderr="", returncode=0
            )
            with self.assertRaises(smoke.subprocess.TimeoutExpired):
                smoke.close_frontend(proc, "macos")
            sample.assert_called_once()
            self.assertEqual(sample.call_args.args[0][:3], ["sample", "123", "5"])
            self.assertLessEqual(sample.call_args.kwargs["timeout"], 15)

    def test_successful_macos_shutdown_does_not_collect_a_stack(self):
        proc = Mock(wait=Mock(return_value=0))
        with patch.object(smoke.subprocess, "run") as sample:
            smoke.close_frontend(proc, "macos")
            sample.assert_not_called()

    def test_fixture_must_target_the_frontends_actual_config_directory(self):
        with (
            tempfile.TemporaryDirectory() as root,
            patch.dict("os.environ", {"APPDATA": root}),
        ):
            smoke.require_config_directory(Path(root) / "obs-studio-rust", "windows")
            with self.assertRaisesRegex(RuntimeError, "frontend configuration"):
                smoke.require_config_directory(Path(root) / "unrelated", "windows")
        with patch.object(Path, "home", return_value=Path("/example")):
            smoke.require_config_directory(
                Path("/example/Library/Application Support/obs-studio-rust"), "macos"
            )
            with self.assertRaisesRegex(RuntimeError, "frontend configuration"):
                smoke.require_config_directory(Path("/example/unrelated"), "macos")

    def inputs(self, platform):
        return [
            {"inputName": name, "inputKind": kind, "unversionedInputKind": kind}
            for name, kind in zip(smoke.NAMES, smoke.KINDS[platform], strict=True)
        ]

    def validate(self, platform, kinds=None, inputs=None, scene=None, profile=None):
        smoke.validate_profile(
            platform,
            list(smoke.KINDS[platform]) if kinds is None else kinds,
            self.inputs(platform) if inputs is None else inputs,
            {"currentProgramSceneName": smoke.SCENE} if scene is None else scene,
            {"currentProfileName": smoke.PROFILE} if profile is None else profile,
        )

    def test_saved_fixture_contains_the_sources_and_selects_the_profile(self):
        for platform in smoke.KINDS:
            with self.subTest(platform=platform), tempfile.TemporaryDirectory() as root:
                config = Path(root) / "config"
                smoke.write_profile(config, platform, 4455, "test-password")
                collection = json.loads(
                    (config / f"basic/scenes/{smoke.PROFILE}.json").read_text()
                )
                self.assertEqual(
                    {source["id"] for source in collection["sources"]},
                    {*smoke.KINDS[platform], "scene"},
                )
                self.assertEqual(
                    {
                        item["name"]
                        for item in collection["sources"][-1]["settings"]["items"]
                    },
                    set(smoke.NAMES),
                )
                self.assertIn(
                    f"Profile={smoke.PROFILE}", (config / "user.ini").read_text()
                )
                self.assertTrue(
                    json.loads(
                        (config / "plugin_config/obs-websocket/config.json").read_text()
                    )["auth_required"]
                )

    def test_existing_user_config_is_preserved(self):
        with tempfile.TemporaryDirectory() as root:
            config = Path(root) / "config"
            config.mkdir()
            marker = config / "user.ini"
            marker.write_text("user data")
            with self.assertRaisesRegex(RuntimeError, "existing configuration"):
                smoke.write_profile(config, "windows", 4455, "test-password")
            self.assertEqual(marker.read_text(), "user data")

    def test_live_profile_validation(self):
        for platform in smoke.KINDS:
            with self.subTest(platform=platform):
                self.validate(platform)
                with self.assertRaisesRegex(
                    RuntimeError, "capture plugins are missing"
                ):
                    self.validate(platform, kinds=list(smoke.KINDS[platform])[:-1])
                with self.assertRaisesRegex(RuntimeError, "saved input"):
                    self.validate(platform, inputs=self.inputs(platform)[:-1])
                with self.assertRaisesRegex(RuntimeError, "saved scene"):
                    self.validate(
                        platform, scene={"currentProgramSceneName": "Default"}
                    )
                with self.assertRaisesRegex(RuntimeError, "saved profile"):
                    self.validate(platform, profile={"currentProfileName": "Default"})

    def test_package_architecture_is_checked_even_on_an_emulating_host(self):
        with tempfile.TemporaryDirectory() as root:
            app = Path(root) / "obs64.exe"
            for arch, machine in (("x64", 0x8664), ("arm64", 0xAA64)):
                with self.subTest(arch=arch):
                    header = bytearray(0x86)
                    header[:2] = b"MZ"
                    struct.pack_into("<I", header, 0x3C, 0x80)
                    header[0x80:0x84] = b"PE\0\0"
                    struct.pack_into("<H", header, 0x84, machine)
                    app.write_bytes(header)
                    smoke.require_windows_arch(app, arch)
                    with self.assertRaisesRegex(RuntimeError, "is not"):
                        smoke.require_windows_arch(
                            app, "arm64" if arch == "x64" else "x64"
                        )

    def test_release_runs_arm64_package_on_a_native_runner_before_publication(self):
        workflow = (ROOT / ".github/workflows/release-rust.yaml").read_text()
        self.assertTrue(
            "windows_arm64_smoke:" in workflow,
            "release never launches the arm64 package",
        )
        self.assertTrue(
            "windows-11-vs2026-arm" in workflow,
            "arm64 package has no native Windows runner",
        )
        self.assertTrue(
            "test/packaged-startup.py" in workflow,
            "release never reloads a saved capture profile",
        )
        self.assertTrue(
            "needs.windows_arm64_smoke.result == 'success'" in workflow,
            "publication skips native startup validation",
        )


class CrashReportEvidenceTest(unittest.TestCase):
    """#2714: a signal exit printed only `-11`, so the frame that faulted was an
    artifact download away and the hang variant produced nothing at all."""

    HEADER = (
        '{"app_name":"OBS-Studio-Rust","timestamp":"2026-10-27 15:36:19.00 +0000",'
        '"bug_type":"309","os_version":"macOS 26.6.1 (25G76)","incident_id":"0A1B"}'
    )
    BODY = json.dumps(
        {
            "procLaunch": "2026-10-27 15:36:02.51",
            "exception": {
                "type": "EXC_BAD_ACCESS",
                "signal": "SIGSEGV",
                "subtype": "KERN_INVALID_ADDRESS at 0x0",
            },
            "faultingThread": 0,
            "threads": [
                {
                    "id": 1,
                    "name": "com.apple.main-thread",
                    "frames": [
                        {
                            "imageIndex": 0,
                            "imageOffset": 1234,
                            "symbol": "obs_shutdown",
                            "symbolLocation": 332,
                        },
                        {
                            "imageIndex": 0,
                            "imageOffset": 99,
                            "symbol": "free_module",
                            "symbolLocation": 57,
                        },
                        {"imageIndex": 1, "imageOffset": 777},
                    ],
                },
                {"id": 2, "frames": [{"imageIndex": 0, "symbol": "other"}]},
            ],
            "usedImages": [{"name": "libobs"}, {"name": "obs-websocket"}],
        }
    )

    def write_report(self, directory, age=0.0, body=None, name=None):
        report = Path(directory) / (
            name or "OBS-Studio-Rust-2026-10-27-153619.ips"
        )
        report.write_text(
            f"{self.HEADER}\n{self.BODY if body is None else body}\n",
            encoding="utf-8",
        )
        if age:
            stamp = os.stat(report).st_mtime - age
            os.utime(report, (stamp, stamp))
        return report

    def test_a_crash_report_names_the_exception_thread_and_frames(self):
        lines = smoke.summarize_diagnostic_report(f"{self.HEADER}\n{self.BODY}")
        text = "\n".join(lines)
        self.assertIn("EXC_BAD_ACCESS", text)
        self.assertIn("SIGSEGV", text)
        self.assertIn("crashed thread: 0 com.apple.main-thread", text)
        self.assertIn("frame 0: libobs obs_shutdown + 332", text)
        self.assertIn("frame 1: libobs free_module + 57", text)
        self.assertIn("frame 2: obs-websocket", text)
        self.assertNotIn("other", text, "lines past the crashed thread are noise")

    def test_the_summary_is_bounded_to_the_first_frames(self):
        body = json.loads(self.BODY)
        body["threads"][0]["frames"] = [
            {"imageIndex": 0, "symbol": f"symbol_{index}", "symbolLocation": index}
            for index in range(40)
        ]
        lines = smoke.summarize_diagnostic_report(
            f"{self.HEADER}\n{json.dumps(body)}", frames=10
        )
        frames = [line for line in lines if line.startswith("frame ")]
        self.assertEqual(len(frames), 10)
        self.assertTrue(frames[-1].startswith("frame 9:"))

    def test_the_newest_report_is_printed_into_the_step_output(self):
        with tempfile.TemporaryDirectory() as tmp, patch("builtins.print") as printed:
            report = self.write_report(tmp)
            self.assertTrue(smoke.report_crash_reports([Path(tmp)], since=0.0))
            text = "\n".join(str(c.args[0]) for c in printed.call_args_list if c.args)
        self.assertIn(report.name, text)
        self.assertIn("EXC_BAD_ACCESS", text)
        self.assertIn("frame 0: libobs obs_shutdown + 332", text)

    def test_no_report_is_a_notice_and_never_a_new_failure(self):
        for directory in (Path(tempfile.gettempdir()) / "absent-reports",):
            with patch("builtins.print") as printed:
                self.assertFalse(smoke.report_crash_reports([directory], since=0.0))
            text = "\n".join(str(c.args[0]) for c in printed.call_args_list if c.args)
            self.assertIn("no crash report", text)

    def test_an_older_report_is_not_offered_as_this_runs_evidence(self):
        with tempfile.TemporaryDirectory() as tmp:
            self.write_report(tmp, age=3600)
            with patch("builtins.print") as printed:
                started = smoke.time.time()
                self.assertFalse(
                    smoke.report_crash_reports([Path(tmp)], since=started)
                )
            text = "\n".join(str(c.args[0]) for c in printed.call_args_list if c.args)
            self.assertIn("no crash report", text)

    def test_an_unparsable_report_says_so_without_raising(self):
        with tempfile.TemporaryDirectory() as tmp, patch("builtins.print") as printed:
            self.write_report(tmp, body="not json at all", name="legacy.crash")
            self.assertTrue(smoke.report_crash_reports([Path(tmp)], since=0.0))
            text = "\n".join(str(c.args[0]) for c in printed.call_args_list if c.args)
        self.assertIn("legacy.crash", text)
        self.assertIn("no frames parsed", text)

    def test_a_signal_exit_prints_the_report_before_raising(self):
        proc = Mock(pid=123, poll=Mock(return_value=-11))
        proc.wait.return_value = -11
        with tempfile.TemporaryDirectory() as tmp, patch(
            "builtins.print"
        ) as printed, patch.object(
            smoke, "diagnostic_report_directories", return_value=[Path(tmp)]
        ):
            self.write_report(tmp)
            with self.assertRaisesRegex(RuntimeError, "-11|SIGSEGV"):
                smoke.close_frontend(proc, "macos", since=0.0)
            text = "\n".join(str(c.args[0]) for c in printed.call_args_list if c.args)
        self.assertIn("EXC_BAD_ACCESS", text)
        self.assertIn("frame 0: libobs obs_shutdown + 332", text)

    def test_a_clean_shutdown_prints_no_evidence(self):
        proc = Mock(wait=Mock(return_value=0))
        with patch.object(smoke, "report_crash_reports") as reports:
            smoke.close_frontend(proc, "macos")
            reports.assert_not_called()


if __name__ == "__main__":
    unittest.main()


class MigrationFixtureTest(unittest.TestCase):
    def test_custom_locations_prepare_real_saved_capture_inputs(self):
        with tempfile.TemporaryDirectory() as root:
            config = Path(root) / "obs-studio-rust"
            fixture = smoke.ConfigFixture(
                config, "windows", "migrate-custom", 4455, "test secret"
            )
            try:
                self.assertFalse(
                    config.exists(), "precreating Rust config would bypass migration"
                )
                original = Path(root) / "obs-studio"
                import configparser

                parser = configparser.ConfigParser(interpolation=None)
                parser.read(original / "global.ini")
                locations = {
                    key: Path(value) for key, value in parser["Locations"].items()
                }
                self.assertEqual(len(set(locations.values())), 4)
                scenes = (
                    locations["scenecollections"]
                    / f"obs-studio/basic/scenes/{smoke.PROFILE}.json"
                )
                collection = json.loads(scenes.read_text())
                self.assertEqual(
                    {source["id"] for source in collection["sources"]},
                    {*smoke.KINDS["windows"], "scene"},
                )
                websocket = (
                    Path(root) / "obs-studio/plugin_config/obs-websocket/config.json"
                )
                settings = json.loads(websocket.read_text())
                self.assertEqual(
                    (settings["server_port"], settings["server_password"]),
                    (4455, "test secret"),
                )
                with self.assertRaisesRegex(RuntimeError, "Rust configuration"):
                    fixture.verify()
            finally:
                fixture.cleanup()
            self.assertFalse(original.exists())
            self.assertFalse(config.exists())

    def test_existing_user_settings_are_not_fixture_owned(self):
        with tempfile.TemporaryDirectory() as root:
            config = Path(root) / "obs-studio-rust"
            config.mkdir()
            marker = config / "user.ini"
            marker.write_text("existing settings")
            with self.assertRaisesRegex(RuntimeError, "existing"):
                smoke.ConfigFixture(config, "windows", "existing", 4455, "test secret")
            self.assertEqual(marker.read_text(), "existing settings")

    def test_validator_rejects_original_settings_changes(self):
        with tempfile.TemporaryDirectory() as root:
            fixture = smoke.ConfigFixture(
                Path(root) / "obs-studio-rust",
                "macos",
                "migrate-default",
                4455,
                "test secret",
            )
            try:
                original = Path(root) / "obs-studio/global.ini"
                original.write_text("changed by the application")
                with self.assertRaisesRegex(RuntimeError, "original settings"):
                    fixture.verify()
            finally:
                fixture.cleanup()


class GpuStatusTest(unittest.TestCase):
    """obs-rust/obs-studio#941: the packaged app reports a real adapter in
    GetStats.gpuAcceleration and logs its GPU state at startup."""

    def status(self, **overrides):
        status = {
            "state": "yellow",
            "adapter": "Microsoft Basic Render Driver",
            "detectedGpus": [],
            "summary": "Rendering on the CPU.",
            "causes": [],
        }
        status.update(overrides)
        return status

    def test_a_real_adapter_in_any_state_passes(self):
        for state in ("green", "yellow", "red"):
            smoke.validate_gpu_status(self.status(state=state))
        smoke.validate_gpu_status(self.status(adapter="Apple M1 (Virtual)"))

    def test_a_missing_status_fails(self):
        with self.assertRaisesRegex(RuntimeError, "gpuAcceleration is missing"):
            smoke.validate_gpu_status(None)

    def test_a_bare_api_name_is_not_an_adapter(self):
        for api in ("OpenGL", "Direct3D 11", "Metal"):
            with self.assertRaisesRegex(RuntimeError, "API name"):
                smoke.validate_gpu_status(self.status(adapter=api))

    def test_unknown_state_or_shape_fails(self):
        with self.assertRaisesRegex(RuntimeError, "state"):
            smoke.validate_gpu_status(self.status(state="blue"))
        with self.assertRaisesRegex(RuntimeError, "detectedGpus"):
            smoke.validate_gpu_status(self.status(detectedGpus=None))

    def test_startup_log_reports_the_gpu_state(self):
        smoke.require_gpu_log("info: GPU acceleration: green: Rendering on X.\n")
        smoke.require_gpu_log("warning: ==== GPU acceleration: YELLOW ====\n")
        with self.assertRaisesRegex(RuntimeError, "GPU acceleration"):
            smoke.require_gpu_log("info: Loading up OpenGL on adapter X\n")


class WindowsCloseFrontendTest(unittest.TestCase):
    def run_windows_close(self, visible, owners=None, ticks=None):
        import ctypes

        api = Mock()
        api.EnumWindows.side_effect = lambda callback, _: callback(42, 0)
        api.GetWindowThreadProcessId.side_effect = (
            lambda hwnd, pid: setattr(pid._obj, "value", next(owner_values))
        )
        owner_values = iter(owners or [123] * 10)
        api.IsWindowVisible.side_effect = visible
        api.PostMessageW.return_value = 1
        proc = Mock(pid=123, poll=Mock(return_value=None), wait=Mock(return_value=0))
        with (
            patch.object(ctypes, "WinDLL", return_value=api, create=True),
            patch.object(ctypes, "WINFUNCTYPE", side_effect=lambda *args: lambda fn: fn, create=True),
            patch.object(smoke.time, "monotonic", side_effect=ticks or [0] * 10),
            patch.object(smoke.time, "sleep"),
        ):
            smoke.close_frontend(proc, "windows")
        return api, proc

    def test_waits_for_a_visible_window_of_the_owned_process(self):
        api, proc = self.run_windows_close([False, True])
        self.assertEqual(api.EnumWindows.call_count, 2)
        api.PostMessageW.assert_called_once_with(42, 0x0010, 0, 0)
        proc.wait.assert_called_once_with(timeout=30)

    def test_never_closes_an_unrelated_process(self):
        with self.assertRaisesRegex(RuntimeError, "owned frontend window"):
            self.run_windows_close([True] * 10, owners=[456] * 10, ticks=[0, 31])

    def test_permanently_invisible_owned_window_times_out(self):
        with self.assertRaisesRegex(RuntimeError, "owned frontend window"):
            self.run_windows_close([False] * 10, ticks=[0, 31])
