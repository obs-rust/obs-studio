#!/usr/bin/env -S uv run --script
"""The NixOS AppImage lane's verdict, checked without Nix (#962).

build-aux/appimage/test-appimage-nixos.sh decides from OBS's log whether the
packaged AppImage started through appimageTools.wrapType2. Its port keeps
those decisions, so compare them with the shell's own grep calls on sample
logs.

    uv run --no-project ci/test_appimage_nixos_checks.py
"""

from __future__ import annotations

import importlib.util
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCRIPT = ROOT / "build-aux/appimage/test_appimage_nixos.py"
WORKFLOW = ROOT / ".github/workflows/appimage.yaml"
MARKER = "/run/opengl-driver/lib/libnvidia-ml.so.1"

GOOD = """\
info: hostlib: va unavailable (not found): searched 2 candidates
info: ---------------------------------
info: Loading up OpenGL on adapter Mesa Intel(R) UHD Graphics (TGL GT2)
info: OBS-Studio-Rust 32.0.0 (linux)
info: Created scene collection 'Untitled'
"""


def module():
    spec = importlib.util.spec_from_file_location("test_appimage_nixos", SCRIPT)
    loaded = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(loaded)
    return loaded


def shell_verdict(log: Path, state: Path, marker: Path) -> set[str]:
    """The decisions the removed shell script made, run as it made them."""
    errors = set()

    def grep(pattern: str, fixed: bool) -> bool:
        args = ["grep", "-q", "-x" if fixed else "-E", pattern, str(log)]
        return subprocess.run(args).returncode == 0

    if state.read_text().strip() != "running":
        errors.add("state")
    if grep("cannot start|needs these libraries from your system", False):
        errors.add("preflight")
    if not grep("Switched to scene|Loaded scenes|Created scene collection", False):
        errors.add("scene")
    if not grep("Loading up OpenGL on adapter", False):
        errors.add("opengl")
    loaded = marker.exists() and MARKER in marker.read_text().splitlines()
    if not loaded:
        errors.add("nvml")
    if grep("error while loading shared libraries|Segmentation fault|SIGSEGV", False):
        errors.add("crash")
    return errors


class NixosChecksTest(unittest.TestCase):
    def verdict(self, log: str, state: str = "running", marker: str | None = MARKER) -> set[str]:
        return set(module().errors(log, state, marker))

    def test_a_good_start_has_no_errors(self) -> None:
        self.assertEqual(self.verdict(GOOD), set())

    def test_each_failure_is_named(self) -> None:
        cases = {
            "state": (GOOD, "exited 1", MARKER),
            "preflight": (GOOD + "error: cannot start OBS\n", "running", MARKER),
            "scene": (GOOD.replace("Created scene collection 'Untitled'\n", ""), "running", MARKER),
            "opengl": (GOOD.replace("Loading up OpenGL on adapter", "Loading up GL on adapter"), "running", MARKER),
            "nvml": (GOOD, "running", None),
            "crash": (GOOD + "obs: error while loading shared libraries: libfoo.so\n", "running", MARKER),
        }
        for expected, (log, state, marker) in cases.items():
            with self.subTest(error=expected):
                self.assertEqual(self.verdict(log, state, marker), {expected})

    def test_the_port_decides_as_the_shell_did(self) -> None:
        samples = [
            (GOOD, "running", MARKER),
            (GOOD, "exited 1", MARKER),
            (GOOD + "error: cannot start OBS\n", "running", MARKER),
            (GOOD.replace("Created scene collection 'Untitled'\n", ""), "running", None),
            (GOOD + "Segmentation fault\n", "exited 139", None),
            ("", "unknown", None),
        ]
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for log, state, marker in samples:
                with self.subTest(state=state):
                    log_path, state_path, marker_path = root / "log", root / "state", root / "marker"
                    log_path.write_text(log)
                    state_path.write_text(state)
                    if marker is None:
                        marker_path.unlink(missing_ok=True)
                    else:
                        marker_path.write_text(marker + "\n")
                    shell = shell_verdict(log_path, state_path, marker_path)
                    ours = set(module().errors(log, state, marker))
                    self.assertEqual(ours, shell)

    def test_the_nixos_lane_runs_the_python_script(self) -> None:
        text = WORKFLOW.read_text()
        self.assertIn("test_appimage_nixos.py", text)
        self.assertNotIn("test-appimage-nixos.sh", text)


if __name__ == "__main__":
    unittest.main()
