#!/usr/bin/env -S uv run --script
"""Native Tier 2 builds its complete platform graph in one invocation (Refs #4996).

Run with: uv run python -m unittest ci/test_tier2_native_target.py
"""

from __future__ import annotations

import re
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
HELPER = ROOT / "rust/tools/tier2-validate/targets.cmake"
COMMON = {"libobs", "libobs-c-reference", "cmocka-tests", "libobs-opengl"}
PLATFORM = {
    "WINDOWS": {"libobs-d3d11"},
    "MACOS": {"libobs-metal", "mac-capture", "mac-avcapture", "mac-avcapture-legacy"},
}


@unittest.skipUnless(shutil.which("cmake"), "cmake is required")
class NativeTier2Target(unittest.TestCase):
    def configure(self, root: Path, host: str, *, omit: str = "", fail: bool = False) -> subprocess.CompletedProcess[str]:
        commands = ["cmake_minimum_required(VERSION 3.28)", "project(native_tier2_probe NONE)", f"set(OS_{host} TRUE)"]
        # This dependency is not in the validator's target list. New tests must
        # still join the build through the existing cmocka-tests registry.
        # tier2-residue (#5848) also needs the native-only ASan aggregate and loader probe.
        names = COMMON | PLATFORM[host] | {"future-test", "shared-core", "cmocka-asan-tests", "test_bitstream"}
        for name in sorted(names - {omit}):
            action = "false" if fail and name == "future-test" else f"touch {root.as_posix()}/{name}.built"
            commands.append(f"add_custom_target({name} COMMAND ${{CMAKE_COMMAND}} -E {action})")
        (root / "count.cmake").write_text('file(APPEND "${LOG}" "visit\\n")', encoding="utf-8")
        commands += [
            f'add_custom_command(TARGET shared-core POST_BUILD COMMAND ${{CMAKE_COMMAND}} "-DLOG={root.as_posix()}/visits.txt" -P "{root.as_posix()}/count.cmake")',
            "add_dependencies(cmocka-tests future-test shared-core)",
            "add_dependencies(libobs shared-core)",
            "add_dependencies(libobs-c-reference shared-core)",
            f'include("{HELPER.as_posix()}")',
            'get_property(deps TARGET tier2-native PROPERTY MANUALLY_ADDED_DEPENDENCIES)',
            'file(WRITE "${CMAKE_BINARY_DIR}/dependencies.txt" "${deps}")',
        ]
        (root / "CMakeLists.txt").write_text("\n".join(commands), encoding="utf-8")
        return subprocess.run(["cmake", "-S", str(root), "-B", str(root / "build")], capture_output=True, text=True, timeout=30)

    def test_complete_platform_graph_builds_and_keeps_future_tests(self) -> None:
        for host in PLATFORM:
            with self.subTest(host=host), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                configured = self.configure(root, host)
                self.assertEqual(configured.returncode, 0, configured.stdout + configured.stderr)
                deps = set((root / "build/dependencies.txt").read_text().split(";"))
                self.assertEqual(deps, COMMON | PLATFORM[host])
                # Model the separate MSBuild invocations observed in the logs,
                # regardless of this probe's locally available CMake generator.
                for target in sorted(deps):
                    baseline = subprocess.run(["cmake", "--build", str(root / "build"), "--target", target], capture_output=True, text=True, timeout=30)
                    self.assertEqual(baseline.returncode, 0, baseline.stdout + baseline.stderr)
                self.assertGreater(len((root / "visits.txt").read_text().splitlines()), 1)
                (root / "visits.txt").write_text("", encoding="utf-8")
                built = subprocess.run(["cmake", "--build", str(root / "build"), "--target", "tier2-native", "--parallel", "2"], capture_output=True, text=True, timeout=30)
                self.assertEqual(built.returncode, 0, built.stdout + built.stderr)
                self.assertEqual((root / "visits.txt").read_text().splitlines(), ["visit"])
                self.assertEqual({p.stem for p in root.glob("*.built")}, COMMON | PLATFORM[host] | {"future-test", "shared-core"})

    def test_missing_required_module_fails_configuration(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            result = self.configure(Path(directory), "WINDOWS", omit="libobs-d3d11")
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("libobs-d3d11", result.stderr)

    def test_failing_test_build_is_not_hidden_by_aggregate(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            configured = self.configure(root, "MACOS", fail=True)
            self.assertEqual(configured.returncode, 0, configured.stderr)
            result = subprocess.run(["cmake", "--build", str(root / "build"), "--target", "tier2-native"], capture_output=True, text=True, timeout=30)
            self.assertNotEqual(result.returncode, 0)

    def test_validators_request_one_complete_build(self) -> None:
        for script in ("validate.sh", "validate.ps1"):
            with self.subTest(script=script):
                text = (HELPER.parent / script).read_text(encoding="utf-8")
                self.assertIn("-DOBS_BUILD_NATIVE_TIER2=ON", text)
                command = re.search(r"^[ \t]*cmake --build .*?(?=\n[^ \t]|\Z)", text, re.M | re.S)
                self.assertIsNotNone(command)
                self.assertRegex(command.group(0), r"--target tier2-native\s*(?:[\\>`\n]|$)")
        root = (ROOT / "CMakeLists.txt").read_text(encoding="utf-8")
        self.assertRegex(root, r"if\(OBS_BUILD_NATIVE_TIER2\)\s+include\([^\n]*tier2-validate/targets.cmake")


if __name__ == "__main__":
    unittest.main()
