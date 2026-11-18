#!/usr/bin/env -S uv run --script
"""The stale-source regression must also run without a compiler (#5009)."""

import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "test/cmocka/check_core_module_manifest_source.cmake"


class FixtureTest(unittest.TestCase):
    def invoke(self, directory, *extra, prepared=False):
        return subprocess.run(
            [
                "cmake",
                f"-DOBS_CORE_MODULES_CMAKE={ROOT}/libobs/cmake/obs-core-modules.cmake",
                f"-DTEST_FIXTURE_ROOT={directory}",
                "-DTEST_GENERATOR=Ninja",
                *extra,
                "-P",
                str(SCRIPT),
            ],
            env={**os.environ, "OBS_TIER2_FIXTURE_PREPARED": "1" if prepared else ""},
            capture_output=True,
            text=True,
            check=False,
        )

    def test_preparation_and_compiler_free_assertions(self):
        with tempfile.TemporaryDirectory() as directory:
            fixture = Path(directory) / "fixture"
            result = self.invoke(fixture, "-DTEST_PREPARE_ONLY=ON")
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            evidence = fixture / "evidence.cmake"
            self.assertTrue(evidence.is_file())
            # A poison compiler/toolchain would fail if assertions configured again.
            result = self.invoke(
                fixture, "-DTEST_C_COMPILER=/no/compiler", prepared=True
            )
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            (fixture / "source/obs-core-modules.c").write_text("int wrong_source;\n")
            result = self.invoke(fixture, prepared=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("changed prepared fixture", result.stdout + result.stderr)

    def test_missing_preparation_fails_closed(self):
        with tempfile.TemporaryDirectory() as directory:
            result = self.invoke(directory, prepared=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("missing prepared fixture", result.stdout + result.stderr)


class NativeSplitTest(unittest.TestCase):
    def test_contract_has_explicit_assert_only_receipt(self):
        source = (ROOT / "rust/obs-build/tests/libobs.rs").read_text()
        self.assertIn("OBS_BUILD_MACOS_ASSERT_ONLY", source)
        self.assertIn("prepared.finish()", source)
        self.assertIn('prepared.get("cores"', source)
        self.assertRegex(source, r'prepared.get\(\s*"capture-probe"')
        self.assertRegex(source, r'prepared.get\(\s*"graphics-probe"')

    def test_shared_runtime_script_retains_all_negative_controls(self):
        script = ROOT / "rust/tools/tier2-validate/macos-assertions.sh"
        self.assertTrue(script.is_file())
        source = script.read_text()
        self.assertEqual(source.count('"$status" -eq 14'), 2)
        self.assertEqual(source.count('"$status" -eq 12'), 2)
        self.assertIn('diff -r "$capture_data" "$reference_data"', source)
        self.assertIn("DYLD_PRINT_LIBRARIES=1", source)
        self.assertNotIn("soldr cargo", source)


class LoaderAssertionTest(unittest.TestCase):
    def exercise(self, wrong_status=14):
        import shlex

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            native = root / "native"
            for variant in ("obs", "obs-c-reference"):
                core = native / variant / "libobs.framework/Versions/A/libobs"
                core.parent.mkdir(parents=True)
                core.write_bytes(b"core")
                (core.parents[2] / "libobs").write_bytes(b"core")
                (native / variant / "libobs.dylib").write_bytes(b"core")
            probe = root / "identity-probe"
            probe.write_text("""#!/bin/bash
printf '%s\\n' "$*" >> "$CONTROL_LOG"
if [[ ${!#} != "$DYLD_FRAMEWORK_PATH/libobs.framework/Versions/A/libobs" ]]; then exit 12; fi
expected=$(basename "$1")
if [[ $expected != "$2" ]]; then exit "$WRONG_STATUS"; fi
""")
            probe.chmod(0o755)
            loader = root / "loader"
            loader.write_text(
                '#!/bin/bash\nprintf "%s/libobs.framework/\\n" "$DYLD_FRAMEWORK_PATH"\n'
            )
            loader.chmod(0o755)
            fake_ctest = root / "ctest"
            fake_ctest.write_text("#!/bin/bash\nexit 0\n")
            fake_ctest.chmod(0o755)
            graphics = []
            captures = []
            assignments = {
                "native_output": str(native),
                "build": str(root),
                "c_lib": str(root / "cmake-core"),
                "avcodec": str(root / "libavcodec.dylib"),
                "probe": str(loader),
            }
            for kind in ("OPENGL", "METAL"):
                image = root / kind
                image.write_bytes(b"image")
                assignments["reference_" + kind.lower()] = str(image)
                graphics.append(f"{kind}\t{image}\t{probe}\n")
            for kind, variable in [
                ("mac-capture", "capture"),
                ("mac-avcapture", "camera"),
                ("mac-avcapture-legacy", "legacy_camera"),
            ]:
                image = root / kind / "Contents/MacOS" / kind
                image.parent.mkdir(parents=True)
                image.write_bytes(b"image")
                data = image.parent.parent / "Resources/locale"
                data.mkdir(parents=True)
                (data / "en-US.ini").write_text("locale")
                assignments["reference_" + variable] = str(image)
                captures.append(f"{kind}\t{image}\t{data.parent}\t{probe}\n")
            (native / "macos-graphics-paths.txt").write_text("".join(graphics))
            (native / "macos-capture-paths.txt").write_text("".join(captures))
            command = "set -euo pipefail\n" + "\n".join(
                f"{key}={shlex.quote(value)}" for key, value in assignments.items()
            )
            command += (
                "\nctest_jobs=(-j 1)\nshow_crash_stacks() { return 0; }\nsource "
                + shlex.quote(
                    str(ROOT / "rust/tools/tier2-validate/macos-assertions.sh")
                )
            )
            result = subprocess.run(
                ["bash", "-c", command],
                env={
                    **os.environ,
                    "PATH": str(root) + os.pathsep + os.environ["PATH"],
                    "CONTROL_LOG": str(root / "controls"),
                    "WRONG_STATUS": str(wrong_status),
                },
                text=True,
                capture_output=True,
                check=False,
            )
            controls = (root / "controls").read_text().splitlines()
            return result, controls

    def test_both_cores_and_every_negative_control_execute(self):
        result, controls = self.exercise()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(len(controls), 40)

    def test_wrong_negative_status_fails_the_shared_assertions(self):
        result, _ = self.exercise(13)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("negative control returned 13", result.stderr)


if __name__ == "__main__":
    unittest.main()
