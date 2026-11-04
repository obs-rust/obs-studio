#!/usr/bin/env -S uv run --script
"""The Windows BETA MSI installer's packaging settings and release wiring (#3476).

    uv run python -m unittest ci/test_windows_msi.py

The MSI itself can only be built on Windows; the release's `windows_msi`
job builds, signs and round-trips it. These checks run anywhere: the CPack
per-generator file is evaluated by real CMake for both generators, and the
release workflow is checked to build, validate and publish the MSI.
"""

from __future__ import annotations

import importlib.util
import io
import shutil
import subprocess
import sys
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parent.parent
GENERATOR_CONFIG = ROOT / "cmake/windows/cpack-wix-generator.cmake"
RELEASE = ROOT / ".github/workflows/release-rust.yaml"

spec = importlib.util.spec_from_file_location("windows_msi", ROOT / "ci/windows_msi.py")
assert spec is not None and spec.loader is not None
windows_msi = importlib.util.module_from_spec(spec)
spec.loader.exec_module(windows_msi)


class GeneratorConfig(unittest.TestCase):
    def evaluate(self, generator: str) -> dict[str, str]:
        if not shutil.which("cmake"):
            self.skipTest("cmake is unavailable")
        with tempfile.TemporaryDirectory() as directory:
            script = Path(directory) / "probe.cmake"
            script.write_text(
                "set(CPACK_PACKAGE_NAME obs-studio)\n"
                "set(CPACK_PACKAGE_VERSION 32.1.0-beta2)\n"
                f"set(CPACK_GENERATOR {generator})\n"
                f'include("{GENERATOR_CONFIG.as_posix()}")\n'
                'message(STATUS "NAME=${CPACK_PACKAGE_NAME}")\n'
                'message(STATUS "VERSION=${CPACK_PACKAGE_VERSION}")\n'
                'message(STATUS "PRE=${CPACK_PRE_BUILD_SCRIPTS}")\n'
            )
            result = subprocess.run(
                ["cmake", "-P", str(script)], capture_output=True, text=True
            )
        self.assertEqual(result.returncode, 0, result.stderr)
        values = {}
        for line in (result.stdout + result.stderr).splitlines():
            key, _, value = line.removeprefix("-- ").partition("=")
            values[key] = value
        return values

    def test_msi_is_branded_beta_with_a_numeric_version_and_no_pdbs(self):
        values = self.evaluate("WIX")
        self.assertEqual(values["NAME"], "OBS-Studio-Rust BETA")
        self.assertEqual(values["VERSION"], "32.1.0")
        self.assertTrue(values["PRE"].endswith("cpack-wix-strip-pdb.cmake"))

    def test_portable_zip_is_unchanged(self):
        values = self.evaluate("ZIP")
        self.assertEqual(values["NAME"], "obs-studio")
        self.assertEqual(values["VERSION"], "32.1.0-beta2")
        self.assertEqual(values["PRE"], "")

    def test_pdb_strip_script_removes_only_pdbs(self):
        if not shutil.which("cmake"):
            self.skipTest("cmake is unavailable")
        with tempfile.TemporaryDirectory() as directory:
            stage = Path(directory)
            (stage / "bin/64bit").mkdir(parents=True)
            (stage / "bin/64bit/obs64.exe").write_text("exe")
            (stage / "bin/64bit/obs64.pdb").write_text("pdb")
            script = ROOT / "cmake/windows/cpack-wix-strip-pdb.cmake"
            subprocess.run(
                ["cmake", f"-DCPACK_TEMPORARY_DIRECTORY={stage}", "-P", str(script)],
                check=True,
            )
            self.assertTrue((stage / "bin/64bit/obs64.exe").exists())
            self.assertFalse((stage / "bin/64bit/obs64.pdb").exists())

    def test_windows_cpack_config_wires_the_wix_settings(self):
        text = (ROOT / "cmake/windows/cpackconfig.cmake").read_text()
        for needle in (
            "CPACK_PROJECT_CONFIG_FILE",
            "CPACK_WIX_UPGRADE_GUID",
            "CPACK_WIX_UI_REF WixUI_InstallDir",
            "CPACK_PACKAGE_EXECUTABLES obs64",
            "CPACK_WIX_ARCHITECTURE arm64",
            "set(CPACK_GENERATOR ZIP)",
        ):
            self.assertIn(needle, text)


class Signature(unittest.TestCase):
    def test_probe_returns_signature_status(self):
        for status in ("Valid", "NotSigned"):
            with (
                self.subTest(status=status),
                patch.object(
                    windows_msi.subprocess,
                    "run",
                    return_value=subprocess.CompletedProcess([], 0, status + "\n", ""),
                ),
            ):
                self.assertEqual(
                    windows_msi.signature_status(Path("probe.msi")), status
                )

    # A pwsh (PowerShell 7) step shell exports its own PSModulePath; a
    # Windows PowerShell 5.1 child that inherits it cannot autoload
    # Microsoft.PowerShell.Security (CouldNotAutoloadMatchingModule).
    PS7_ENVIRON = {
        "SystemRoot": r"C:\Windows",
        "ProgramFiles": r"C:\Program Files",
        "USERPROFILE": r"C:\Users\runneradmin",
        "PSModulePath": ";".join(
            [
                r"C:\Users\runneradmin\Documents\PowerShell\Modules",
                r"C:\Program Files\PowerShell\Modules",
                r"c:\program files\powershell\7\Modules",
                r"C:\Modules\az_12.1.0",
            ]
        ),
    }

    def test_probe_uses_a_windows_powershell_module_path_under_pwsh(self):
        with (
            patch.object(
                windows_msi.subprocess,
                "run",
                return_value=subprocess.CompletedProcess([], 0, "Valid\n", ""),
            ) as run,
            patch.dict(windows_msi.os.environ, self.PS7_ENVIRON, clear=True),
        ):
            windows_msi.signature_status(Path("probe.msi"))
        command = run.call_args.args[0]
        self.assertEqual(
            command[0], r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe"
        )
        self.assertIn("Import-Module Microsoft.PowerShell.Security", command[-1])
        paths = run.call_args.kwargs["env"]["PSModulePath"].split(";")
        self.assertIn(r"C:\Windows\System32\WindowsPowerShell\v1.0\Modules", paths)
        self.assertIn(r"C:\Program Files\WindowsPowerShell\Modules", paths)
        self.assertIn(r"C:\Modules\az_12.1.0", paths)
        self.assertFalse([p for p in paths if "\\powershell\\" in p.lower()], paths)

    def test_scheduled_probe_failure_reports_diagnostics_and_stops(self):
        for required in (False, True):
            with (
                self.subTest(required=required),
                patch.object(
                    windows_msi.subprocess,
                    "run",
                    return_value=subprocess.CompletedProcess(
                        [],
                        1,
                        "provider diagnostic\n",
                        "Get-AuthenticodeSignature: Access denied\n",
                    ),
                ),
                patch.object(windows_msi, "_install") as install,
            ):
                output = io.StringIO()
                arguments = ["roundtrip", "--msi", "scheduled.msi"]
                if required:
                    arguments.append("--require-signature")
                with redirect_stdout(output):
                    self.assertEqual(windows_msi.main(arguments), 1)
                diagnostic = output.getvalue()
                self.assertIn("Get-AuthenticodeSignature failed", diagnostic)
                self.assertIn("PowerShell exit code 1", diagnostic)
                self.assertIn(
                    "stderr: Get-AuthenticodeSignature: Access denied", diagnostic
                )
                self.assertIn("stdout: provider diagnostic", diagnostic)
                self.assertIn("scheduled.msi", diagnostic)
                self.assertNotIn("returned NotSigned", diagnostic)
                install.assert_not_called()

    def test_valid_always_passes(self):
        self.assertIsNone(windows_msi.check_signature("Valid", required=True))

    def test_unsigned_fails_only_when_required(self):
        self.assertIsNone(windows_msi.check_signature("NotSigned", required=False))
        self.assertIsNotNone(windows_msi.check_signature("NotSigned", required=True))

    def test_a_bad_signature_always_fails(self):
        for status in ("HashMismatch", "NotTrusted", "UnknownError"):
            self.assertIsNotNone(windows_msi.check_signature(status, required=False))


class MsiExecution(unittest.TestCase):
    def test_timeout_classifies_log_state_and_preserves_evidence(self):
        for data, state, expected in (
            (None, "ABSENT", 121),
            (b"", "EMPTY", 122),
            ("Action start: blocked".encode("utf-16"), "WRITTEN", 124),
        ):
            with self.subTest(state=state), tempfile.TemporaryDirectory() as directory:
                log = Path(directory) / "install.log"
                if data is not None:
                    log.write_bytes(data)
                with patch.object(
                    windows_msi.subprocess,
                    "run",
                    side_effect=subprocess.TimeoutExpired("msiexec", 300),
                ):
                    output = io.StringIO()
                    with redirect_stdout(output):
                        code = windows_msi.run_msi(["msiexec"], log)
                self.assertEqual(code, expected)
                self.assertIn(f"TIMEOUT_LOG_{state}", output.getvalue())
                self.assertIn(
                    f"TIMEOUT_LOG_{state}",
                    log.with_suffix(".diagnostic.txt").read_text(encoding="utf-8"),
                )

    def test_bom_logs_are_safe_on_a_strict_windows_console(self):
        for encoding in ("utf-16", "utf-16-be", "utf-8-sig"):
            with (
                self.subTest(encoding=encoding),
                tempfile.TemporaryDirectory() as directory,
            ):
                log = Path(directory) / "uninstall.log"
                data = "Installer detail: camera \u6444\u50cf\u5934".encode(encoding)
                if encoding == "utf-16-be":
                    data = b"\xfe\xff" + data
                log.write_bytes(data)
                raw = io.BytesIO()
                console = io.TextIOWrapper(raw, encoding="cp1252", errors="strict")
                with redirect_stdout(console):
                    windows_msi.msi_log_tail(log)
                console.flush()
                text = raw.getvalue().decode("cp1252")
                self.assertIn("Installer detail: camera", text)
                self.assertNotIn("\ufeff", text)
                self.assertNotIn("\x00", text)

    def test_broken_console_does_not_mask_installer_failure(self):
        with tempfile.TemporaryDirectory() as directory:
            log = Path(directory) / "install.log"
            log.write_text("failure", encoding="utf-16")
            with patch.object(
                windows_msi.sys.stdout, "write", side_effect=OSError("closed pipe")
            ):
                windows_msi.msi_log_tail(log)

    def test_install_quotes_the_property_value_for_the_msi_parser(self):
        with patch.object(windows_msi, "run_msi", return_value=0) as run:
            windows_msi._install(
                Path("signed package.msi"), Path("Chosen Dir"), Path("logs")
            )
        command = run.call_args.args[0]
        self.assertIsInstance(command, str)
        self.assertIn('INSTALL_ROOT="Chosen Dir"', command)
        self.assertNotIn('"INSTALL_ROOT=', command)

    def test_roundtrip_preserves_install_failure_classification(self):
        with (
            tempfile.TemporaryDirectory() as directory,
            patch.object(windows_msi, "signature_status", return_value="Valid"),
            patch.object(windows_msi, "_install", return_value=121),
            patch.object(windows_msi, "_uninstall", return_value=1605),
            redirect_stdout(io.StringIO()),
        ):
            self.assertEqual(
                windows_msi.main(
                    ["roundtrip", "--msi", "signed.msi", "--log-dir", directory]
                ),
                121,
            )

    def test_exit_codes_are_classified(self):
        for code, expected in ((0, 0), (3010, 0), (1603, 1603)):
            with self.subTest(code=code), tempfile.TemporaryDirectory() as directory:
                logs = Path(directory)
                (logs / "install.log").write_text(
                    "MSI failure detail", encoding="utf-16"
                )
                with patch.object(
                    windows_msi.subprocess,
                    "run",
                    return_value=subprocess.CompletedProcess([], code),
                ) as run:
                    output = io.StringIO()
                    with redirect_stdout(output):
                        result = windows_msi._install(
                            Path("signed.msi"), Path("Chosen Dir"), logs
                        )
                    self.assertEqual(result, expected)
                    self.assertGreater(run.call_args.kwargs["timeout"], 0)
                    self.assertIn(f"exit code {code}", output.getvalue())
                    if code == 1603:
                        self.assertIn("MSI failure detail", output.getvalue())

    def test_hang_is_bounded_and_prints_utf16_log_tail(self):
        with tempfile.TemporaryDirectory() as directory:
            logs = Path(directory)
            (logs / "install.log").write_text(
                "Action start: blocked custom action", encoding="utf-16"
            )
            with patch.object(
                windows_msi.subprocess,
                "run",
                side_effect=subprocess.TimeoutExpired("msiexec", 300),
            ):
                output = io.StringIO()
                with redirect_stdout(output):
                    self.assertNotEqual(
                        windows_msi._install(
                            Path("signed.msi"), Path("Chosen Dir"), logs
                        ),
                        0,
                    )
                self.assertIn("timed out", output.getvalue())
                self.assertIn("Action start: blocked custom action", output.getvalue())

    def test_logs_are_uploaded_on_every_conclusion(self):
        job = (
            RELEASE.read_text()
            .split("\n  windows_msi:\n", 1)[1]
            .split("\n  macos_smoke:", 1)[0]
        )
        upload = job.split("- uses: actions/upload-artifact@", 1)[1]
        self.assertIn("if: always()", upload)


class CommandLine(unittest.TestCase):
    """The release runs `windows_msi.py build|roundtrip`; a script that never
    dispatches them exits 0 having done nothing (release run 41509343216:
    `expected one MSI, found 0`)."""

    def invoke(self, *arguments: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [sys.executable, str(ROOT / "ci/windows_msi.py"), *arguments],
            capture_output=True,
            text=True,
        )

    def test_no_subcommand_is_an_error(self):
        self.assertNotEqual(self.invoke().returncode, 0)

    def test_every_subcommand_parses(self):
        for command in ("build", "sign", "roundtrip"):
            result = self.invoke(command, "--help")
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("usage", result.stdout)

    def test_build_without_a_build_dir_is_an_error(self):
        self.assertNotEqual(self.invoke("build").returncode, 0)


class ReleaseWiring(unittest.TestCase):
    def test_release_builds_round_trips_and_publishes_the_msi(self):
        text = RELEASE.read_text()
        self.assertIn("ci/windows_msi.py build", text)
        # The MSI is built before the Azure signing catalog, which picks it up.
        self.assertLess(
            text.index("ci/windows_msi.py build"),
            text.index("- name: Prepare Windows signing catalog"),
        )
        self.assertIn("build_${{ matrix.target }}/*.msi", text)
        self.assertNotIn("WINDOWS_SIGNING_PFX", text)
        self.assertIn("ci/windows_msi.py roundtrip", text)
        self.assertIn("windows-$env:TARGET.msi", text)
        release_needs = (
            text.split("\n  release:\n", 1)[1].split("needs:", 1)[1].splitlines()[0]
        )
        self.assertIn("windows_msi", release_needs)


if __name__ == "__main__":
    unittest.main()
