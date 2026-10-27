#!/usr/bin/env -S uv run --script
"""The Linux Qt frontend lane configures, links and tests against prebuilt Qt.

    PYTHONPATH=ci uv run --no-project python -m unittest test_linux_qt_frontend

ci/linux_qt_frontend.py must keep using the prebuilt Qt and FFmpeg prefixes,
refuse a distro Qt in the cache, build only the Qt-linked cmocka targets that
exist, and hand ctest exactly those tests. These tests stub cmake,
pkg-config, ninja, uv and xvfb-run, so nothing is configured or built.
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCRIPT = ROOT / "ci" / "linux_qt_frontend.py"
FRAGMENTS = ROOT / "test" / "cmocka" / "tests.d"

RECORD = """import json, os, sys
with open(os.path.join(os.environ["RECORD"], "calls.jsonl"), "a") as f:
    f.write(json.dumps({"name": os.path.basename(sys.argv[0]), "argv": sys.argv[1:], "env": dict(os.environ)}) + "\\n")
"""

CMAKE_STUB = RECORD + """
print("Found Qt6: 6.11.0 (found version \\"6.11.0\\")")
print("Found FFmpeg: avcodec")
build = sys.argv[sys.argv.index("-B") + 1]
open(os.path.join(build, "CMakeCache.txt"), "w").write(os.environ.get("CACHE_LINE", "") + "\\n")
"""

PKG_CONFIG_STUB = RECORD + """
args = sys.argv[1:]
if args and args[0] == "--variable=prefix":
    print(os.environ.get("PC_PREFIX", ""))
elif args and args[0] == "--modversion":
    print(os.environ.get("PC_VERSION", "60.16.100"))
"""

NINJA_STUB = RECORD + """
if "-t" in sys.argv[1:]:
    print(os.environ.get("NINJA_TARGETS", ""))
"""

RECORDER_STUB = RECORD


def qt6_fragments() -> list[str]:
    """The cmocka fragments that find Qt6, in the order the driver reads them."""
    return sorted(p.stem for p in FRAGMENTS.glob("*.cmake") if "Qt6" in p.read_text(encoding="utf-8"))


class LinuxQtFrontendTest(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.bin = self.root / "bin"
        self.bin.mkdir()
        self.record = self.root / "record"
        self.record.mkdir()
        for name, body in (
            ("cmake", CMAKE_STUB),
            ("pkg-config", PKG_CONFIG_STUB),
            ("ninja", NINJA_STUB),
            ("uv", RECORDER_STUB),
            ("xvfb-run", RECORDER_STUB),
        ):
            stub = self.bin / name
            stub.write_text(f"#!{sys.executable}\n{body}")
            stub.chmod(0o755)
        self.qt = self.root / "qt"
        (self.qt / "lib" / "cmake" / "Qt6").mkdir(parents=True)
        (self.qt / "lib" / "cmake" / "Qt6" / "Qt6Config.cmake").write_text("", encoding="utf-8")
        (self.qt / "plugins").mkdir()
        self.ff = self.root / "ffmpeg"
        (self.ff / "lib" / "pkgconfig").mkdir(parents=True)
        self.build = self.root / "build"
        self.build.mkdir()

    def run_script(self, *args: str, **env: str) -> subprocess.CompletedProcess:
        environment = {
            k: v for k, v in os.environ.items() if k not in ("PKG_CONFIG_PATH", "LD_LIBRARY_PATH", "CTEST_JOBS")
        }
        environment.update(
            {
                "PATH": f"{self.bin}{os.pathsep}{environment['PATH']}",
                "RECORD": str(self.record),
                "CACHE_LINE": f"Qt6Widgets_DIR:PATH={self.qt}/lib/cmake/Qt6",
                "PC_PREFIX": str(self.ff),
                "NINJA_TARGETS": "\n".join(f"{name}: phony" for name in qt6_fragments()[:2]),
            }
        )
        environment.update(env)
        return subprocess.run(
            [sys.executable, str(SCRIPT), *args], capture_output=True, text=True, env=environment, timeout=120
        )

    def calls(self, name: str) -> list[dict]:
        path = self.record / "calls.jsonl"
        if not path.exists():
            return []
        entries = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]
        return [entry for entry in entries if entry["name"] == name]

    def prefixes(self) -> tuple[str, str, str]:
        return str(self.qt), str(self.ff), str(self.build)

    def test_usage_needs_the_three_prefixes(self) -> None:
        for args in ((), (str(self.qt),)):
            with self.subTest(args=args):
                result = self.run_script(*args)
                self.assertEqual(result.returncode, 2)
                self.assertIn("usage: linux_qt_frontend.py QT_PREFIX FFMPEG_PREFIX BUILD_DIR", result.stderr)
                self.assertEqual(self.calls("cmake"), [])

    def test_a_prefix_without_qt6_is_refused(self) -> None:
        empty = self.root / "empty"
        empty.mkdir()
        result = self.run_script(str(empty), str(self.ff), str(self.build))
        self.assertEqual(result.returncode, 1)
        self.assertIn(f"no Qt6 in {empty}", result.stderr)
        self.assertEqual(self.calls("cmake"), [])

    def test_the_configure_step_uses_both_prebuilt_prefixes(self) -> None:
        result = self.run_script(*self.prefixes())
        self.assertEqual(result.returncode, 0, result.stderr)
        cmake = self.calls("cmake")[0]
        self.assertEqual(cmake["argv"][:3], ["-S", str(ROOT), "-B"])
        self.assertEqual(cmake["argv"][3], str(self.build))
        self.assertIn("-G", cmake["argv"])
        self.assertEqual(cmake["argv"][cmake["argv"].index("-G") + 1], "Ninja")
        self.assertIn(f"-DCMAKE_PREFIX_PATH={self.qt};{self.ff}", cmake["argv"])
        self.assertIn(f"-DQt6_DIR={self.qt}/lib/cmake/Qt6", cmake["argv"])
        for flag in ("-DCMAKE_C_COMPILER_LAUNCHER=ccache", "-DCMAKE_CXX_COMPILER_LAUNCHER=ccache",
                     "-DENABLE_UNIT_TESTS=ON", "-DENABLE_RUST_LIBOBS=OFF", "-DENABLE_FRONTEND=ON",
                     "-DENABLE_SCRIPTING=OFF", "-DENABLE_BROWSER=OFF", "-DENABLE_WEBSOCKET=OFF"):
            self.assertIn(flag, cmake["argv"])
        self.assertEqual(cmake["env"]["PKG_CONFIG_PATH"], f"{self.ff}/lib/pkgconfig")
        self.assertEqual(cmake["env"]["LD_LIBRARY_PATH"], f"{self.ff}/lib")
        # The configure output is both logged and shown, as `tee` did.
        log = (self.build / "build.configure.log").read_text(encoding="utf-8")
        self.assertIn("Found Qt6", log)
        self.assertIn("Found Qt6", result.stdout)
        self.assertIn("qt-phase phase=configure seconds=", result.stdout)

    def test_a_distro_qt_in_the_cache_is_refused(self) -> None:
        result = self.run_script(
            *self.prefixes(), CACHE_LINE="Qt6Widgets_DIR:PATH=/usr/lib/x86_64-linux-gnu/cmake/Qt6"
        )
        self.assertEqual(result.returncode, 1)
        self.assertIn("Qt6Widgets_DIR is not in the prebuilt prefix:", result.stderr)
        self.assertIn("Qt6Widgets_DIR:PATH=/usr/lib/x86_64-linux-gnu/cmake/Qt6", result.stderr)
        self.assertEqual(self.calls("ninja"), [])

    def test_libavcodec_must_come_from_the_ffmpeg_prefix(self) -> None:
        result = self.run_script(*self.prefixes(), PC_PREFIX="/usr")
        self.assertEqual(result.returncode, 1)
        self.assertIn(f"libavcodec is not from {self.ff} (/usr)", result.stderr)
        self.assertEqual(self.calls("ninja"), [])

    def test_only_qt_targets_that_exist_are_built_and_tested(self) -> None:
        fragments = qt6_fragments()
        built, skipped = fragments[:2], fragments[2:]
        self.assertTrue(built and skipped)
        result = self.run_script(*self.prefixes())
        self.assertEqual(result.returncode, 0, result.stderr)
        for name in skipped:
            self.assertIn(f"== skipped (no target, missing Qt module?): {name}", result.stdout)
        build = self.calls("ninja")[-1]
        self.assertEqual(build["argv"], ["-C", str(self.build), "obs-studio", *built])
        self.assertIn(f"== Qt-linked tests: {' '.join(built)}", result.stdout)
        # The shared Qt facade runs against the same real prefix, offscreen.
        unittest_call = self.calls("uv")[0]
        self.assertIn("unittest", unittest_call["argv"])
        self.assertEqual(unittest_call["env"]["OBS_QT_TEST_PREFIX"], str(self.qt))
        self.assertEqual(unittest_call["env"]["PYTHONPATH"], "ci")
        ctest = self.calls("xvfb-run")[0]
        self.assertEqual(
            ctest["argv"],
            ["-a", "ctest", "--test-dir", str(self.build), "--output-on-failure", "-j", "4",
             "-R", f"^({'|'.join(built)})$"],
        )
        self.assertEqual(ctest["env"]["LD_LIBRARY_PATH"], f"{self.qt}/lib:{self.ff}/lib")
        self.assertEqual(ctest["env"]["QT_PLUGIN_PATH"], f"{self.qt}/plugins")
        self.assertIn("qt-phase phase=build seconds=", result.stdout)
        self.assertIn("qt-phase phase=test seconds=", result.stdout)

    def test_the_ctest_job_count_is_configurable(self) -> None:
        self.assertEqual(self.run_script(*self.prefixes(), CTEST_JOBS="7").returncode, 0)
        ctest = self.calls("xvfb-run")[0]["argv"]
        self.assertEqual(ctest[ctest.index("-j") + 1], "7")

    def test_the_workflow_runs_the_python_driver(self) -> None:
        workflow = (ROOT / ".github" / "workflows" / "build-project.yaml").read_text(encoding="utf-8")
        self.assertIn("uv run --no-project ci/linux_qt_frontend.py", workflow)
        self.assertNotIn("run: ci/linux_qt_frontend.sh", workflow)


if __name__ == "__main__":
    unittest.main()
