#!/usr/bin/env -S uv run --script
"""Run-only macOS Tier 2 bundle: staging, verification and CTest execution (#5009)."""
from __future__ import annotations

import json
import re
import struct
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import macos_tier2_bundle as bundle
import test_macos_test_artifact


def build_tree(root: Path) -> Path:
    work = root / "build_cmocka_link_mac-arm64"
    (work / "test/cmocka").mkdir(parents=True)
    (work / "libobs-c-reference").mkdir()
    image = bytes(test_macos_test_artifact.SignatureTest.signed_image())
    (work / "test/cmocka/test_a").write_bytes(image)
    (work / "test/cmocka/test_a").chmod(0o755)
    (work / "obs.dylib").write_bytes(image)
    (work / "libobs-c-reference/obs.dylib").write_bytes(image)
    (work / "CTestTestfile.cmake").write_text("subdirs(test/cmocka)\n")
    (work / "test/cmocka/CTestTestfile.cmake").write_text(f'add_test([=[test_a]=] "{work}/test/cmocka/test_a")\n')
    (work / "ctest-registry.txt").write_text("  Test #1: test_a\n\nTotal Tests: 1\n")
    (work / "CMakeFiles").mkdir()
    (work / "CMakeFiles/junk.o").write_bytes(b"object")
    (work / "compiler-cache").mkdir()
    (work / "compiler-cache/ccache").write_bytes(b"\x7fELF")
    deps = root / ".deps/obs-deps-1-universal/lib"
    deps.mkdir(parents=True)
    (deps / "libavcodec.dylib").write_bytes(image)
    qt = root / ".deps/obs-deps-qt6-1-universal/lib/QtCore.framework/Versions/A"
    qt.mkdir(parents=True)
    (qt / "QtCore").write_bytes(image)
    return work


class StageTest(unittest.TestCase):
    def test_stage_copies_the_run_closure_and_seals_it(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "src"
            root.mkdir()
            work = build_tree(root)
            out = Path(temporary) / "bundle"
            manifest = bundle.stage(root, work, out, source_sha="abc")
            names = set(manifest["files"])
            self.assertIn("build_cmocka_link_mac-arm64/test/cmocka/test_a", names)
            self.assertIn("build_cmocka_link_mac-arm64/libobs-c-reference/obs.dylib", names)
            self.assertIn(".deps/obs-deps-1-universal/lib/libavcodec.dylib", names)
            self.assertIn(".deps/obs-deps-qt6-1-universal/lib/QtCore.framework/Versions/A/QtCore", names)
            self.assertFalse([n for n in names if "CMakeFiles" in n or "compiler-cache" in n or n.endswith(".o")])
            self.assertEqual(json.loads((out / "manifest.json").read_text())["source_sha"], "abc")
            self.assertEqual(manifest["work"], "build_cmocka_link_mac-arm64")
            self.assertTrue((out / "build_cmocka_link_mac-arm64/test/cmocka/test_a").stat().st_mode & 0o100)

    def test_stage_refuses_an_unsigned_image_and_an_empty_registry(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "src"
            root.mkdir()
            work = build_tree(root)
            (work / "test/cmocka/test_b").write_bytes(struct.pack("<8I", 0xFEEDFACF, 0x100000C, 0, 2, 0, 0, 0, 0) + bytes(64))
            with self.assertRaises(ValueError):
                bundle.stage(root, work, Path(temporary) / "bundle", source_sha="abc")
            (work / "test/cmocka/test_b").unlink()
            (work / "ctest-registry.txt").write_text("Total Tests: 0\n")
            with self.assertRaisesRegex(ValueError, "empty CTest registry"):
                bundle.stage(root, work, Path(temporary) / "bundle2", source_sha="abc")


class VerifyTest(unittest.TestCase):
    def staged(self, temporary: str) -> tuple[Path, Path]:
        root = Path(temporary) / "src"
        root.mkdir()
        work = build_tree(root)
        out = Path(temporary) / "bundle"
        bundle.stage(root, work, out, source_sha="abc")
        return root, out

    def test_verify_rejects_missing_changed_extra_files_and_wrong_source(self):
        with tempfile.TemporaryDirectory() as temporary:
            _, out = self.staged(temporary)
            bundle.verify(out, source_sha="abc")
            with self.assertRaisesRegex(ValueError, "source"):
                bundle.verify(out, source_sha="other")
            extra = out / "build_cmocka_link_mac-arm64/extra"
            extra.write_text("x")
            with self.assertRaisesRegex(ValueError, "extra"):
                bundle.verify(out, source_sha="abc")
            extra.unlink()
            (out / "build_cmocka_link_mac-arm64/test/cmocka/test_a").write_bytes(b"changed")
            with self.assertRaisesRegex(ValueError, "hash"):
                bundle.verify(out, source_sha="abc")
            (out / "build_cmocka_link_mac-arm64/test/cmocka/test_a").unlink()
            with self.assertRaisesRegex(ValueError, "missing"):
                bundle.verify(out, source_sha="abc")


class RunTest(unittest.TestCase):
    def test_both_suites_run_against_their_own_libobs_and_the_staged_dependencies(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "src"
            root.mkdir()
            work = build_tree(root)
            calls = []

            def fake(args, **kwargs):
                calls.append((args, kwargs["env"]))
                return mock.Mock(returncode=0)

            with mock.patch.object(bundle.subprocess, "run", side_effect=fake):
                bundle.run_suites(root, work.name, jobs=3)
            ctest = [(a, e) for a, e in calls if a[0] == "ctest"]
            self.assertEqual(len(ctest), 2)
            off, on = (e for _, e in ctest)
            self.assertTrue(off["DYLD_LIBRARY_PATH"].startswith(str(work / "libobs-c-reference")))
            self.assertTrue(on["DYLD_LIBRARY_PATH"].startswith(str(work)))
            for env in (off, on):
                self.assertIn(str(root / ".deps/obs-deps-1-universal/lib"), env["DYLD_LIBRARY_PATH"])
                self.assertIn(str(work / "sanitizer-runtimes"), env["DYLD_LIBRARY_PATH"])
                self.assertEqual(env["DYLD_FRAMEWORK_PATH"], str(root / ".deps/obs-deps-qt6-1-universal/lib"))

    def test_a_failing_suite_fails_the_run_and_no_compiler_is_ever_invoked(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "src"
            root.mkdir()
            work = build_tree(root)
            with mock.patch.object(bundle.subprocess, "run", return_value=mock.Mock(returncode=8)):
                with self.assertRaises(SystemExit) as stop:
                    bundle.run_suites(root, work.name, jobs=1)
            self.assertEqual(stop.exception.code, 8)


class WorkflowTest(unittest.TestCase):
    @staticmethod
    def job(name: str) -> str:
        workflow = (Path(__file__).resolve().parents[1] / ".github/workflows/build-project.yaml").read_text()
        return re.search(rf"^  {name}:\n(.*?)(?=^  [a-z0-9-]+:)", workflow, re.S | re.M).group(1)

    def test_consumer_only_downloads_verifies_and_runs(self):
        job = self.job("rust-tier2-macos-ctest")
        self.assertIn("rust-tier2-macos-build", job)
        self.assertIn("runs-on: macos-26", job)
        self.assertIn("actions/download-artifact@", job)
        self.assertIn("ci/macos_tier2_bundle.py run", job)
        for compiler in ("soldr", "setup-soldr", "brew install", "xcodebuild", "cmake --build", "cmake --preset"):
            self.assertNotIn(compiler, job)
        self.assertNotIn("merge_group'", job.split("if:")[1].split("\n")[0].replace("!= 'merge_group'", ""))

    def test_producer_builds_on_linux_from_the_common_root_and_uploads_the_bundle(self):
        job = self.job("rust-tier2-macos-build")
        self.assertNotIn("macos-", job.split("runs-on:")[1].split("\n")[0])
        for needle in ("macos_test_workspace.py place", "ci/cmocka_link_check.py --alias mac-arm64",
                       "ci/macos_tier2_bundle.py stage", "macos_test_workspace.py pack",
                       "actions/upload-artifact@", "macos_test_workspace.py restore"):
            self.assertIn(needle, job)


if __name__ == "__main__":
    unittest.main()
