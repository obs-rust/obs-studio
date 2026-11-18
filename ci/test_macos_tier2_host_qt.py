#!/usr/bin/env -S uv run --script
"""Qt's copied per-config tool locations must never select a Darwin generator."""

import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


class HostQtTest(unittest.TestCase):
    def test_relwithdebinfo_and_parser_use_the_linux_sdk(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            host = root / "Linux Qt SDK/libexec"
            host.mkdir(parents=True)
            names = ("moc", "uic", "rcc", "cmake_automoc_parser")
            for name in names:
                (host / name).write_bytes(b"\x7fELF")
            (root / "CMakeLists.txt").write_text(
                "cmake_minimum_required(VERSION 3.28)\nproject(host_tools NONE)\n"
                + f'include("{ROOT}/cmake/cross/tier2-host-qt.cmake")\n'
                + "".join(
                    f'add_executable(Qt6::{name} IMPORTED)\n'
                    f'set_target_properties(Qt6::{name} PROPERTIES IMPORTED_CONFIGURATIONS "RELEASE;RELWITHDEBINFO" '
                    f'IMPORTED_LOCATION_RELEASE "/Darwin/{name}" IMPORTED_LOCATION_RELWITHDEBINFO "/Darwin/{name}")\n'
                    for name in names
                )
                + f'obs_tier2_bind_host_qt("{host.parent}")\n'
                + 'file(GENERATE OUTPUT "tools.txt" CONTENT "'
                + "\\n".join(f"$<TARGET_FILE:Qt6::{name}>" for name in names) + '")\n'
            )
            result = subprocess.run(["cmake", "-S", str(root), "-B", str(root / "build"), "-G", "Ninja",
                                     "-DCMAKE_BUILD_TYPE=RelWithDebInfo"], capture_output=True, text=True, check=False)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertEqual((root / "build/tools.txt").read_text().splitlines(), [str(host / name) for name in names])


if __name__ == "__main__":
    unittest.main()
