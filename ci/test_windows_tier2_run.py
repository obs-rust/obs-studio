#!/usr/bin/env -S uv run --script
"""Linux safety tests for the Windows run-only consumer and complete registry."""
import importlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]


class RunOnlyTests(unittest.TestCase):
    def helper(self):
        return importlib.import_module("windows_tier2_run")

    def test_full_mode_uses_canonical_directory_including_fragments(self):
        text = (ROOT / "cmake/cross/windows-tier2.cmake").read_text()
        self.assertIn("OBS_CROSS_WINDOWS_TIER2_FULL", text)
        full = (ROOT / "cmake/cross/windows-tier2-full.cmake").read_text()
        self.assertIn('add_subdirectory("${OBS_SOURCE_ROOT}/test/cmocka"', full)
        self.assertNotIn('set(HAVE_OBS_OUTPUTS TRUE)', full)

    def test_inventory_requires_names_commands_and_properties(self):
        h = self.helper()
        plan = [{"name": "one", "command": ["@OBS_TIER2_BUILD@/test.exe"],
                 "properties": [{"name": "RUN_SERIAL", "value": True},
                                {"name": "TIMEOUT", "value": 12}]}]
        h.compare_inventory(plan, plan)
        for changed in ([], [{**plan[0], "command": ["other.exe"]}],
                        [{**plan[0], "properties": []}], plan + plan):
            with self.assertRaises(ValueError):
                h.compare_inventory(plan, changed)

    def test_incomplete_probe_never_runs(self):
        h = self.helper()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "manifest.json").write_text(json.dumps({"schema": 1, "complete": False}))
            with patch.object(h.subprocess, "run") as run:
                with self.assertRaisesRegex(ValueError, "complete"):
                    h.verify(root, "a" * 40)
                run.assert_not_called()

    def test_four_passes_keep_entire_suite_and_nonzero_status(self):
        h = self.helper()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            manifest = {"cores": {name: f"cores/{name}/obs.dll" for name in h.PASSES},
                        "runtime_directories": ["deps"], "tests": [{"name": "one"}]}
            calls = []
            def execute(command, **kwargs):
                calls.append((command, kwargs))
                return type("Result", (), {"returncode": 0})()
            with patch.object(h, "native_controls"), patch.object(h, "check_shadowing"), \
                    patch.object(h.subprocess, "run", side_effect=execute):
                h.run_passes(root, manifest, "ctest", {"PATH": "system32", "OBS_CTEST_JOBS": "3"})
            self.assertEqual(len(calls), 4)
            for (command, kwargs), name in zip(calls, h.PASSES):
                self.assertIn("--no-tests=error", command)
                self.assertNotIn("-R", command)
                self.assertEqual(command[-2:], ["-j", "3"])
                self.assertEqual(Path(kwargs["env"]["PATH"].split(h.os.pathsep)[0]), root / "cores" / name)
            with patch.object(h, "native_controls"), patch.object(h, "check_shadowing"), \
                    patch.object(h, "crash_stacks") as crashes, \
                    patch.object(h.subprocess, "run", return_value=type("Result", (), {"returncode": 9})()):
                with self.assertRaisesRegex(RuntimeError, "9"):
                    h.run_passes(root, manifest, "ctest", {"PATH": "system32"})
                crashes.assert_called_once()

    def test_shadowing_and_path_override_are_rejected(self):
        h = self.helper()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            test = {"name": "one", "command": [str(root / "test.exe")], "properties": []}
            h.check_shadowing(root, [test])
            (root / "obs.dll").write_bytes(b"shadow")
            with self.assertRaisesRegex(ValueError, "shadow"):
                h.check_shadowing(root, [test])
            for prop in [{"name": "ENVIRONMENT", "value": ["PATH=bad"]},
                         {"name": "ENVIRONMENT_MODIFICATION", "value": ["PATH=reset:"]}]:
                with self.assertRaises(ValueError):
                    h.check_shadowing(root / "empty", [{**test, "command": ["test.exe"], "properties": [prop]}])

    def test_build_commands_are_rejected_in_runtime_inventory(self):
        h = self.helper()
        for command in [["cl.exe", "x.c"], ["cmake", "--build", "tree"],
                        ["soldr", "cargo", "test"], ["uv", "run", "build.py"]]:
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
        for name, link in [("../escape", False), ("safe/link", True), ("C:/escape", False)]:
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


if __name__ == "__main__":
    unittest.main()
