#!/usr/bin/env -S uv run --script
"""libobs compiles in its lib-modules (#3259).

    uv run python -m unittest ci/test_libobs_modules.py

libobs/cmake/modules.cmake moves each unity group off libobs-objects into
libobs-mod-<module>, the module libobs/build/modules.d assigns it
(ci/test_build_groups.py checks the manifest itself). These tests check
the CMake side:

- CMake's translation of the manifest's fnmatch patterns puts every libobs
  group in the same module as Python's fnmatch (run with `cmake -P`).
- libobs/CMakeLists.txt splits last, after the host files that add sources to
  libobs-objects, and links the modules into the shared libraries.
"""

from __future__ import annotations

import fnmatch
import json
import re
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MODULES = ROOT / "libobs" / "build" / "modules.d"
CMAKELISTS = ROOT / "libobs" / "CMakeLists.txt"


def groups() -> list[str]:
    names = []
    for path in sorted((ROOT / "libobs" / "build").iterdir()):
        match = re.fullmatch(r"libobs\.([^+]+)\+\.h\.(c|m)", path.name)
        if match:
            names.append(match.group(1))
    return names


def python_assignment() -> dict[str, str]:
    modules = {p.stem: json.loads(p.read_text(encoding="utf-8"))["groups"] for p in sorted(MODULES.glob("*.json"))}
    out = {}
    for group in groups():
        owners = [name for name, patterns in modules.items() if any(fnmatch.fnmatchcase(group, p) for p in patterns)]
        out[group] = owners[0] if len(owners) == 1 else "?"
    return out


@unittest.skipUnless(shutil.which("cmake"), "cmake is not installed")
class CmakeAssignmentTest(unittest.TestCase):
    def test_cmake_and_python_agree_on_every_group(self):
        script = (
            f'include("{(ROOT / "libobs" / "cmake" / "modules.cmake").as_posix()}")\n'
            "_libobs_read_modules(_names)\n"
            f'foreach(_group IN ITEMS {" ".join(groups())})\n'
            '  _libobs_module_of("${_group}" "${_names}" _module)\n'
            '  message(STATUS "${_group}=${_module}")\n'
            "endforeach()\n"
        )
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "assign.cmake"
            path.write_text(script, encoding="utf-8")
            done = subprocess.run(
                ["cmake", f"-DLIBOBS_MODULES_DIR={MODULES.as_posix()}", "-P", str(path)],
                capture_output=True,
                text=True,
            )
        self.assertEqual(done.returncode, 0, done.stderr)
        cmake = dict(
            line.removeprefix("-- ").split("=", 1)
            for line in (done.stdout + done.stderr).splitlines()
            if "=" in line and line.startswith("-- ")
        )
        self.assertEqual(cmake, python_assignment())


class SplitPlacementTest(unittest.TestCase):
    def test_split_runs_after_the_host_files_and_links_the_shared_libraries(self):
        text = CMAKELISTS.read_text(encoding="utf-8")
        split = text.find("libobs_split_modules(${LIBOBS_SHARED_TARGETS})")
        self.assertNotEqual(split, -1, "libobs/CMakeLists.txt does not split libobs-objects into modules")
        for host in ("os-windows", "os-macos", "os-linux", "os-freebsd"):
            self.assertLess(text.find(f"include(cmake/{host}.cmake)"), split, f"{host} adds sources after the split")
        self.assertNotIn("target_sources(libobs-objects", text[split:], "a source added after the split stays unsplit")


if __name__ == "__main__":
    unittest.main()
