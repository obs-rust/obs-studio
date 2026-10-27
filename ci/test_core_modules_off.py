#!/usr/bin/env -S uv run --script
"""The ENABLE_CORE_MODULES=OFF build produces an empty core module list (#2280).

    uv run python -m unittest ci/test_core_modules_off.py

ci/core_modules_off.py builds libobs for each backend with core modules off and
requires the generated list to name no module. These tests drive it with
stubbed `cmake` and `ninja`, so they check the flags each backend is configured
with and the exact list the check accepts.
"""

from __future__ import annotations

import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCRIPT = ROOT / "ci" / "core_modules_off.py"

EMPTY = 'const unsigned int obs_core_modules_count = 0;\nconst char *obs_core_modules[] = {\n\tNULL\n};\n'
# The two halves of the check, each wrong on its own.
BAD_COUNT = EMPTY.replace("count = 0", "count = 2")
BAD_ARRAY = EMPTY.replace("\tNULL\n", '\t"core",\n\tNULL\n')

CMAKE_STUB = """
printf '%s\\n' "$*" >>"$CMAKE_CALLS"
build=''; previous=''
for arg in "$@"; do [[ "$previous" == -B ]] && build=$arg; previous=$arg; done
mkdir -p "$build/libobs"
printf '%b' "$LIST" >"$build/libobs/obs-core-modules.c"
"""
NINJA_STUB = "echo \"$*\" >>\"$NINJA_CALLS\"\n"


class CoreModulesOffTest(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.bin = self.root / "bin"
        self.bin.mkdir()
        for name, body in (("cmake", CMAKE_STUB), ("ninja", NINJA_STUB)):
            stub = self.bin / name
            stub.write_text("#!/usr/bin/env bash\nset -euo pipefail\n" + body)
            stub.chmod(0o755)
        self.build_root = self.root / "builds"
        self.cmake_calls = self.root / "cmake.log"
        self.ninja_calls = self.root / "ninja.log"

    def run_script(self, *args: str, list_text: str = EMPTY, **env: str) -> subprocess.CompletedProcess:
        environment = {k: v for k, v in os.environ.items() if k != "VIRTUAL_ENV"}
        environment.update(PATH=f"{self.bin}{os.pathsep}{environment['PATH']}",
                           CMAKE_CALLS=str(self.cmake_calls), NINJA_CALLS=str(self.ninja_calls),
                           LIST=list_text, **env)
        return subprocess.run([sys.executable, str(SCRIPT), str(self.build_root), *args],
                              capture_output=True, text=True, env=environment)

    def calls(self, path: Path) -> list[str]:
        return path.read_text(encoding="utf-8").splitlines() if path.exists() else []

    def test_each_backend_is_configured_off_and_built_then_checked(self) -> None:
        result = self.run_script("-DRust_TOOLCHAIN=stable")
        self.assertEqual(result.returncode, 0, result.stderr)
        configures = self.calls(self.cmake_calls)
        self.assertEqual(len(configures), 2)
        self.assertIn(f"-B {self.build_root / 'C'}", configures[0])
        self.assertIn("-DOBS_LIBOBS_BACKEND=C", configures[0])
        self.assertIn("-DOBS_BUILD_C_REFERENCE=OFF", configures[0])
        self.assertIn(f"-B {self.build_root / 'HYBRID'}", configures[1])
        self.assertIn("-DOBS_LIBOBS_BACKEND=HYBRID", configures[1])
        self.assertIn("-DOBS_BUILD_C_REFERENCE=ON", configures[1])
        for configure in configures:
            self.assertIn("-DENABLE_CORE_MODULES=OFF", configure)
            # Extra arguments reach every configure.
            self.assertIn("-DRust_TOOLCHAIN=stable", configure)
        builds = self.calls(self.ninja_calls)
        self.assertIn(f"-C {self.build_root / 'C'} libobs", builds[0])
        self.assertIn(f"-C {self.build_root / 'HYBRID'} libobs libobs-c-reference", builds[1])
        self.assertIn("== C: ok", result.stdout)
        self.assertIn("== HYBRID: ok", result.stdout)

    def test_jobs_reaches_ninja(self) -> None:
        result = self.run_script(JOBS="3")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("-j 3", self.calls(self.ninja_calls)[0])

    def test_a_non_empty_list_fails_and_shows_the_file(self) -> None:
        result = self.run_script(list_text='const unsigned int obs_core_modules_count = 1;\n'
                                           'const char *obs_core_modules[] = {\n\t"core",\n\tNULL\n};\n')
        self.assertEqual(result.returncode, 1)
        self.assertIn("is not an empty, NULL-terminated list", result.stderr)
        self.assertIn('"core"', result.stderr)

    def test_both_halves_of_the_check_are_required(self) -> None:
        for list_text in (BAD_COUNT, BAD_ARRAY):
            with self.subTest(list_text=list_text):
                result = self.run_script(list_text=list_text)
                self.assertEqual(result.returncode, 1)
                self.assertIn("is not an empty", result.stderr)

    def test_missing_argument_prints_usage(self) -> None:
        environment = {k: v for k, v in os.environ.items() if k != "VIRTUAL_ENV"}
        result = subprocess.run([sys.executable, str(SCRIPT)], capture_output=True, text=True, env=environment)
        self.assertEqual(result.returncode, 2)
        self.assertIn("usage: core_modules_off.py", result.stderr)

    def test_unknown_backend_is_refused(self) -> None:
        result = self.run_script(BACKENDS="C FOO")
        self.assertEqual(result.returncode, 2)
        self.assertIn("unknown backend FOO", result.stderr)


if __name__ == "__main__":
    unittest.main()
