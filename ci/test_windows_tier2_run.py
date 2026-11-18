#!/usr/bin/env -S uv run --script
"""Linux safety tests for the Windows run-only consumer and complete registry."""

import importlib
import json
from pathlib import Path
import tempfile
import struct
import subprocess
import unittest
from unittest.mock import patch
from test_windows_zip_check import pe

ROOT = Path(__file__).resolve().parents[1]


class RunOnlyTests(unittest.TestCase):
    def helper(self):
        return importlib.import_module("windows_tier2_run")

    def test_full_mode_uses_canonical_directory_including_fragments(self):
        text = (ROOT / "cmake/cross/windows-tier2.cmake").read_text()
        self.assertIn("OBS_CROSS_WINDOWS_TIER2_FULL", text)
        full = (ROOT / "cmake/cross/windows-tier2-full.cmake").read_text()
        self.assertIn('add_subdirectory("${OBS_SOURCE_ROOT}/test/cmocka"', full)
        self.assertNotIn("set(HAVE_OBS_OUTPUTS TRUE)", full)

    def test_inventory_requires_names_commands_and_properties(self):
        h = self.helper()
        plan = [
            {
                "name": "one",
                "command": ["@OBS_TIER2_BUILD@/test.exe"],
                "properties": [
                    {"name": "RUN_SERIAL", "value": True},
                    {"name": "TIMEOUT", "value": 12},
                ],
            }
        ]
        h.compare_inventory(plan, plan)
        for changed in (
            [],
            [{**plan[0], "command": ["other.exe"]}],
            [{**plan[0], "properties": []}],
            plan + plan,
        ):
            with self.assertRaises(ValueError):
                h.compare_inventory(plan, changed)

    def test_flat_registry_roundtrips_properties_through_real_ctest(self):
        h = self.helper()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            exe = root / "test.exe"
            exe.write_bytes(pe([]))
            tests = [
                {
                    "name": "one",
                    "command": [str(exe), "two words", "a;b"],
                    "properties": [
                        {"name": "RUN_SERIAL", "value": True},
                        {"name": "TIMEOUT", "value": 12.0},
                        {"name": "RESOURCE_LOCK", "value": ["graphics", "timing"]},
                        {
                            "name": "ENVIRONMENT",
                            "value": ["ONE=some;list", "TWO=spaces here"],
                        },
                        {"name": "WORKING_DIRECTORY", "value": str(root)},
                    ],
                }
            ]
            h.write_registry(root, tests)
            actual = json.loads(
                subprocess.check_output(
                    ["ctest", "--test-dir", str(root), "--show-only=json-v1"], text=True
                )
            )["tests"]
            h.compare_inventory(tests, actual)

    def test_incomplete_probe_never_runs(self):
        h = self.helper()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "manifest.json").write_text(
                json.dumps({"schema": 1, "complete": False})
            )
            with patch.object(h.subprocess, "run") as run:
                with self.assertRaisesRegex(ValueError, "complete"):
                    h.verify(root, "a" * 40)
                run.assert_not_called()

    def test_four_passes_keep_entire_suite_and_nonzero_status(self):
        h = self.helper()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            manifest = {
                "cores": {name: f"cores/{name}/obs.dll" for name in h.PASSES},
                "runtime_directories": ["deps"],
                "tests": [{"name": "one"}],
            }
            calls = []

            def execute(command, **kwargs):
                calls.append((command, kwargs))
                return type("Result", (), {"returncode": 0})()

            with (
                patch.object(h, "native_controls"),
                patch.object(h, "check_shadowing"),
                patch.object(h.subprocess, "run", side_effect=execute),
            ):
                h.run_passes(
                    root, manifest, "ctest", {"PATH": "system32", "OBS_CTEST_JOBS": "3"}
                )
            self.assertEqual(len(calls), 4)
            for (command, kwargs), name in zip(calls, h.PASSES):
                self.assertIn("--no-tests=error", command)
                self.assertNotIn("-R", command)
                self.assertEqual(command[-2:], ["-j", "3"])
                self.assertEqual(
                    Path(kwargs["env"]["PATH"].split(h.os.pathsep)[0]),
                    root / "cores" / name,
                )
            with (
                patch.object(h, "native_controls"),
                patch.object(h, "check_shadowing"),
                patch.object(h, "crash_stacks") as crashes,
                patch.object(
                    h.subprocess,
                    "run",
                    return_value=type("Result", (), {"returncode": 9})(),
                ),
            ):
                with self.assertRaisesRegex(RuntimeError, "9"):
                    h.run_passes(root, manifest, "ctest", {"PATH": "system32"})
                crashes.assert_called_once()

    def test_shadowing_and_path_override_are_rejected(self):
        h = self.helper()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            test = {
                "name": "one",
                "command": [str(root / "test.exe")],
                "properties": [],
            }
            h.check_shadowing(root, [test])
            (root / "obs.dll").write_bytes(b"shadow")
            with self.assertRaisesRegex(ValueError, "shadow"):
                h.check_shadowing(root, [test])
            for prop in [
                {"name": "ENVIRONMENT", "value": ["PATH=bad"]},
                {"name": "ENVIRONMENT_MODIFICATION", "value": ["PATH=reset:"]},
            ]:
                with self.assertRaises(ValueError):
                    h.check_shadowing(
                        root / "empty",
                        [{**test, "command": ["test.exe"], "properties": [prop]}],
                    )

    def test_build_commands_are_rejected_in_runtime_inventory(self):
        h = self.helper()
        for command in [
            ["cl.exe", "x.c"],
            ["cmake", "--build", "tree"],
            ["soldr", "cargo", "test"],
            ["uv", "run", "build.py"],
        ]:
            with self.assertRaisesRegex(ValueError, "runtime command"):
                h.runtime_command(command, Path("bundle"), {})

    def test_download_requires_immutable_identity_and_digest(self):
        h = self.helper()
        for identity, digest in [("latest", "a" * 64), ("0", "a" * 64), ("12", "")]:
            with self.assertRaises(ValueError):
                h.artifact_identity(identity, digest)
        self.assertEqual(h.artifact_identity("123", "a" * 64), 123)

    def test_archive_traversal_and_links_fail_before_extract(self):
        h = self.helper()
        import io
        import tarfile

        for name, link in [
            ("../escape", False),
            ("safe/link", True),
            ("C:/escape", False),
        ]:
            with tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                archive = root / "bundle.tar"
                with tarfile.open(archive, "w") as out:
                    info = tarfile.TarInfo(name)
                    if link:
                        info.type = tarfile.SYMTYPE
                        info.linkname = "../../escape"
                    out.addfile(info, io.BytesIO())
                with self.assertRaises(ValueError):
                    h.extract(archive, root / "out")

    def test_zstd_transport_roundtrip(self):
        h = self.helper()
        import io
        import tarfile

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            archive = root / "bundle.tar.zst"
            with tarfile.open(archive, "w:zst") as out:
                data = b"sealed payload"
                info = tarfile.TarInfo("data/fixture")
                info.size = len(data)
                out.addfile(info, io.BytesIO(data))
            h.extract(archive, root / "out")
            self.assertEqual((root / "out/data/fixture").read_bytes(), data)

    def test_fixture_assertion_checks_inputs_without_compiling(self):
        from windows_tier2_probe import digest
        from windows_tier2_fixtures import INPUTS

        with tempfile.TemporaryDirectory() as tmp:
            receipt = Path(tmp) / "receipt.json"
            record = {
                "schema": 1,
                "test": "test_core_module_manifest_source",
                "source_sha": "a" * 40,
                "target": "x86_64-pc-windows-msvc",
                "profile": "RelWithDebInfo",
                "exit_code": 0,
                "inputs": {name: digest(ROOT / name) for name in INPUTS},
            }
            command = [
                "cmake",
                f"-DRECEIPT={receipt}",
                f"-DSOURCE_ROOT={ROOT}",
                f"-DEXPECTED_SOURCE_SHA={'a' * 40}",
                "-P",
                str(ROOT / "ci/windows_tier2_assert_fixture.cmake"),
            ]
            receipt.write_text(json.dumps(record))
            self.assertEqual(subprocess.run(command, capture_output=True).returncode, 0)
            record["inputs"][INPUTS[0]] = "0" * 64
            receipt.write_text(json.dumps(record))
            result = subprocess.run(command, capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("Fixture input changed", result.stderr)

    def test_exports_are_read_from_the_name_table_without_llvm(self):
        h = self.helper()
        data = bytearray(pe([])).ljust(0x400, b"\0")
        # One backed section, with an export directory, function/name/ordinal
        # tables at different offsets. Confusing the function and name tables
        # must fail this test, rather than silently accepting producer symbols.
        struct.pack_into("<II", data, 0x40 + 24 + 112, 0x1080, 40)
        struct.pack_into("<I", data, 0x40 + 24 + 240 + 8, 0x200)
        struct.pack_into("<I", data, 0x40 + 24 + 240 + 16, 0x200)
        struct.pack_into("<IIII", data, 0x280 + 20, 1, 1, 0x10B0, 0x10C0)
        struct.pack_into("<I", data, 0x2B0, 0x10E0)
        struct.pack_into("<I", data, 0x2C0, 0x10D0)
        data[0x2D0:0x2D8] = b"obs_api\0"
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "obs.dll"
            path.write_bytes(data)
            self.assertEqual(h.coff_exports(path), {"obs_api"})

    def test_complete_transport_verifies_then_mutations_fail_closed(self):
        h = self.helper()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            files = {
                "build/test.exe": pe([]),
                "probes/loader.exe": pe([]),
                "probes/graphics.exe": pe([]),
            }
            files.update({f"cores/{name}/obs.dll": pe([]) for name in h.PASSES})
            for owner in ("cmake", "cargo"):
                files.update(
                    {
                        f"renderers/{owner}/{name}.dll": pe([])
                        for name in ("D3D11", "OPENGL")
                    }
                )
            for name, data in files.items():
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(data)
            tests = [
                {
                    "name": "one",
                    "command": ["@OBS_TIER2_BUILD@/test.exe"],
                    "properties": [],
                }
            ]
            manifest = {
                "schema": 2,
                "complete": True,
                "target": h.probe.TARGET,
                "coverage": "native-full",
                "source_sha": "a" * 40,
                "files": {name: h.probe.digest(root / name) for name in files},
                "cores": {name: f"cores/{name}/obs.dll" for name in h.PASSES},
                "exports": ["api"],
                "tests": tests,
                "runtime_directories": [],
                "native_baseline": {
                    "source_sha": "a" * 40,
                    "clean_configure": True,
                    "tests": tests,
                },
                "ctest_templates": {},
                "native_contract": {
                    "bundled_obs_sys": {"source_sha": "a" * 40, "passed": True},
                    "loader_probe": "probes/loader.exe",
                    "graphics_probe": "probes/graphics.exe",
                    "renderers": {
                        owner: {
                            name: f"renderers/{owner}/{name}.dll"
                            for name in ("D3D11", "OPENGL")
                        }
                        for owner in ("cmake", "cargo")
                    },
                },
            }
            actual = h.normalized(tests, {h.probe.BUILD: (root / "build").as_posix()})
            path = root / "manifest.json"
            path.write_text(json.dumps(manifest))
            with (
                patch.object(h, "coff_exports", return_value={"api"}),
                patch.object(h, "verify_renderer_exports"),
                patch.object(h, "write_registry"),
                patch.object(
                    h.subprocess,
                    "check_output",
                    return_value=json.dumps({"tests": actual}),
                ),
            ):
                h.verify(root, "a" * 40)
                for key, replacement in [
                    ("cores", {}),
                    ("exports", []),
                    ("source_sha", "b" * 40),
                ]:
                    path.write_text(json.dumps({**manifest, key: replacement}))
                    with self.assertRaises(ValueError):
                        h.verify(root, "a" * 40)
                path.write_text(json.dumps(manifest))
                (root / "build/test.exe").write_bytes(pe(["missing.dll"]))
                with self.assertRaisesRegex(ValueError, "changed artifact"):
                    h.verify(root, "a" * 40)

    def test_renderer_exports_keep_native_contract_assertions(self):
        h = self.helper()
        manifest = {
            "native_contract": {
                "renderers": {
                    owner: {
                        renderer: f"{owner}/{renderer}.dll"
                        for renderer in ("D3D11", "OPENGL")
                    }
                    for owner in ("cmake", "cargo")
                }
            }
        }
        api = {"device_create", "device_destroy"}
        with patch.object(h, "coff_exports", return_value=api):
            h.verify_renderer_exports(Path("bundle"), manifest)
        for exports in [[api, {"device_create"}], [api | {"missing"}, api]]:
            with patch.object(h, "coff_exports", side_effect=exports):
                with self.assertRaises(ValueError):
                    h.verify_renderer_exports(Path("bundle"), manifest)


if __name__ == "__main__":
    unittest.main()
