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
            for args, _ in [(a, e) for a, e in calls if a[0] == "ctest"]:
                self.assertEqual(args[args.index("-E") + 1], "^test_core_module_manifest_source$")
                self.assertIn("--timeout", args)
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


class CoreSuiteTest(unittest.TestCase):
    """The native Cargo cores run the same prebuilt registry (the #5848 residue stays native)."""

    def test_core_is_presented_as_obs_dylib_and_must_actually_load(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "src"
            root.mkdir()
            work = build_tree(root)
            core = Path(temporary) / "cargo/obs/libobs.dylib"
            core.parent.mkdir(parents=True)
            core.write_bytes(b"core")
            calls = []

            def fake(args, **kwargs):
                calls.append((args, kwargs["env"]))
                if kwargs.get("stdout") is not None:
                    kwargs["stdout"].write(f"dyld: loaded {work}/core-obs/obs.dylib\n".encode())
                return mock.Mock(returncode=0)

            with mock.patch.object(bundle.subprocess, "run", side_effect=fake):
                bundle.run_core_suite(root, work.name, core, "obs", jobs=2)
            link = work / "core-obs/obs.dylib"
            self.assertTrue(link.is_symlink() and link.resolve() == core.resolve())
            ctest = [e for a, e in calls if a[0] == "ctest"]
            self.assertEqual(len(ctest), 1)
            self.assertTrue(ctest[0]["DYLD_LIBRARY_PATH"].startswith(str(work / "core-obs")))

    def test_a_probe_that_loads_another_libobs_fails(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "src"
            root.mkdir()
            work = build_tree(root)
            core = Path(temporary) / "libobs.dylib"
            core.write_bytes(b"core")

            def fake(args, **kwargs):
                if kwargs.get("stdout") is not None:
                    kwargs["stdout"].write(b"dyld: loaded /somewhere/else/obs.dylib\n")
                return mock.Mock(returncode=0)

            with mock.patch.object(bundle.subprocess, "run", side_effect=fake):
                with self.assertRaisesRegex(ValueError, "did not load"):
                    bundle.run_core_suite(root, work.name, core, "obs", jobs=1)


class ProbeTest(unittest.TestCase):
    def test_a_hung_test_is_sampled_and_reported_not_waited_for(self):
        import subprocess as real

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "src"
            root.mkdir()
            work = build_tree(root)
            process = mock.Mock(pid=7)
            process.communicate.side_effect = [real.TimeoutExpired("t", 1), ("late output", None)]
            with mock.patch.object(bundle.subprocess, "Popen", return_value=process) as popen, \
                 mock.patch.object(bundle.subprocess, "run") as run:
                self.assertEqual(bundle.probe(root, work.name, "test_a", seconds=1), 124)
            self.assertEqual(popen.call_args.kwargs["env"]["ASAN_OPTIONS"].split(":")[0], "verbosity=1")
            self.assertEqual(run.call_args.args[0][:2], ["sample", "7"])
            process.kill.assert_called_once()


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


class ResidueTest(unittest.TestCase):
    """rust-tier2-macos keeps only what the Linux bundle cannot cover (#5848)."""

    root = Path(__file__).resolve().parents[1]

    def test_validator_builds_no_test_executables_in_residue_mode(self):
        script = (self.root / "rust/tools/tier2-validate/validate.sh").read_text()
        targets = (self.root / "rust/tools/tier2-validate/targets.cmake").read_text()
        self.assertIn("tier2-residue", script)
        self.assertIn("OBS_TIER2_RESIDUE", script)
        self.assertIn("ci/macos_tier2_bundle.py ctest-core", script)
        residue = re.search(r"add_dependencies\(\s*tier2-residue\s+([^)]*)\)", targets).group(1)
        self.assertIn("libobs-c-reference", residue)
        self.assertNotIn("cmocka-tests", residue)
        self.assertIn("cmocka-asan-tests", residue)

    def test_asan_targets_are_recorded_and_the_cross_build_drops_only_address(self):
        cmake = (self.root / "test/cmocka/CMakeLists.txt").read_text()
        self.assertIn("OBS_TEST_SANITIZE_ADDRESS", cmake)
        self.assertIn("asan-tests.txt", cmake)
        script = (self.root / "rust/tools/tier2-validate/validate.sh").read_text()
        self.assertIn("asan-tests.txt", script)

    def test_native_job_consumes_the_bundle_instead_of_compiling_it(self):
        job = WorkflowTest.job("rust-tier2-macos")
        self.assertIn("rust-tier2-macos-build", job)
        self.assertIn("OBS_TIER2_RESIDUE", job)
        self.assertIn("actions/download-artifact@", job)
        self.assertIn("macos_test_workspace.py unpack", job)


if __name__ == "__main__":
    unittest.main()
