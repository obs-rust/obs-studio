#!/usr/bin/env -S uv run --script
"""The private-PipeWire wrapper harness is Python (obs-rust/obs-studio#962).

    uv run python -m unittest ci/test_pipewire_wrapper.py

Ported off `test/cmocka/test_pipewire_wrapper.sh`: the module checks the three
per-test wrappers (test_pipewire_cursor_hdr.py, test_pipewire_context_fail.py,
test_pulse_monitor_device.py) that start a private PipeWire daemon. It skips
(77) when pipewire is not installed, and for each wrapper asserts that a long
TMPDIR still starts the daemon and runs the test (exit 0) while a daemon that
dies is a failure, not a skip. The control case runs with a stripped PATH and
still skips.

No real daemon or test binary runs here: `uv` runs tiny stub wrappers that key
off the environment, and `pipewire` and the pulse tools are stubs on PATH.

The shell's only caller is `test/cmocka/tests.d/test_pipewire_wrapper.cmake`,
so this suite drives the module directly and pins that the wiring -- the shell
script gone, its GEN-005 exception gone, the cmake caller running the Python
through `uv run`, and the two sibling pins repointed -- is complete.
"""

from __future__ import annotations

import importlib.util
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MODULE = ROOT / "test" / "cmocka" / "test_pipewire_wrapper.py"
SHELL_SCRIPT = ROOT / "test" / "cmocka" / "test_pipewire_wrapper.sh"
CMAKE = ROOT / "test" / "cmocka" / "tests.d" / "test_pipewire_wrapper.cmake"
CI_TOML = ROOT / "ci.toml"
EXCEPTION = 'path = "test/cmocka/test_pipewire_wrapper.sh"'
# The sibling ports run this wrapper harness as a control, so their pins must
# name the Python module now (CI has no pipewire, so PR CI does not catch it).
SIBLING_PINS = (
    ROOT / "ci" / "test_pipewire_context_fail.py",
    ROOT / "ci" / "test_pulse_monitor_device.py",
)

# A stub wrapper: skips (77) with no pipewire, fails on a dying daemon, else the
# exit the run asks for. Runs under `uv run --script`, as its ctest callers do.
STUB_WRAPPER = """#!/usr/bin/env -S uv run --script
import os
import shutil
import sys

if shutil.which("pipewire") is None:
    sys.exit(77)
if "bin-dying" in os.environ.get("PATH", ""):
    sys.exit(1)
# The long-TMPDIR case must hand the wrapper a deep *directory*; if TMPDIR
# names a file the daemon cannot make its socket and the real test fails.
tmpdir = os.environ.get("TMPDIR")
if tmpdir and not os.path.isdir(tmpdir):
    print(f"stub: TMPDIR is not a directory: {tmpdir}", file=sys.stderr)
    sys.exit(4)
sys.exit(int(os.environ.get("OBS_PW_WRAPPER_STUB_EXIT", "0")))
"""

STUB_TOOL = "#!/bin/sh\nexit 0\n"
WRAPPERS = ("test_pipewire_cursor_hdr.py", "test_pipewire_context_fail.py", "test_pulse_monitor_device.py")
PULSE_TOOLS = ("wireplumber", "pipewire-pulse", "pactl", "dbus-run-session")


def write_stub(path: Path, body: str) -> None:
    path.write_text(body)
    path.chmod(0o755)


def load_module():
    """Import the module by path (it is a script, not an installed module)."""
    spec = importlib.util.spec_from_file_location("test_pipewire_wrapper", MODULE)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class Harness:
    """A scratch scripts dir of stub wrappers and a stub tool bin."""

    def __init__(self, work: Path) -> None:
        self.work = work
        self.scripts = work / "scripts"
        self.scripts.mkdir(parents=True, exist_ok=True)
        for name in WRAPPERS:
            write_stub(self.scripts / name, STUB_WRAPPER)
        self.bin = work / "bin"
        self.bin.mkdir(parents=True, exist_ok=True)
        for tool in ("pipewire", *PULSE_TOOLS):
            write_stub(self.bin / tool, STUB_TOOL)

    def env(self, **extra: str) -> dict[str, str]:
        env = dict(os.environ)
        env["PATH"] = f"{self.bin}{os.pathsep}{os.environ.get('PATH', '')}"
        env.pop("OBS_PW_WRAPPER_STUB_EXIT", None)
        env.update(extra)
        return env

    def run(self, **extra: str):
        return subprocess.run(
            [sys.executable, str(MODULE), str(self.scripts)],
            env=self.env(**extra),
            capture_output=True,
            text=True,
            timeout=300,
        )


class PipewireWrapperTest(unittest.TestCase):
    def test_wiring_is_complete(self) -> None:
        self.assertFalse(SHELL_SCRIPT.exists(), "test_pipewire_wrapper.sh should be gone")
        self.assertTrue(MODULE.is_file(), "test_pipewire_wrapper.py should exist")
        caller = CMAKE.read_text(encoding="utf-8")
        self.assertIn("uv run --script", caller)
        self.assertIn("test_pipewire_wrapper.py", caller)
        self.assertNotIn("test_pipewire_wrapper.sh", caller)
        for sibling in SIBLING_PINS:
            text = sibling.read_text(encoding="utf-8")
            self.assertIn("test_pipewire_wrapper.py", text, sibling.name)
            self.assertNotIn("test_pipewire_wrapper.sh", text, sibling.name)
        self.assertNotIn(
            EXCEPTION,
            CI_TOML.read_text(encoding="utf-8"),
            "the GEN-005 exception for test/cmocka/test_pipewire_wrapper.sh should be removed",
        )

    def test_the_module_exposes_the_shells_contract(self) -> None:
        module = load_module()
        self.assertEqual(module.SKIP, 77)
        self.assertEqual(module.TIMEOUT_SECONDS, 60)
        self.assertEqual(module.WRAPPERS, ("test_pipewire_cursor_hdr.py", "test_pipewire_context_fail.py"))
        self.assertEqual(module.PULSE_WRAPPER, "test_pulse_monitor_device.py")
        self.assertEqual(module.PULSE_TOOLS, PULSE_TOOLS)

    def test_a_missing_pipewire_skips(self) -> None:
        with tempfile.TemporaryDirectory() as work:
            harness = Harness(Path(work))
            empty = Path(work) / "empty-bin"
            empty.mkdir()
            env = dict(os.environ)
            env["PATH"] = str(empty)
            result = subprocess.run(
                [sys.executable, str(MODULE), str(harness.scripts)],
                env=env,
                capture_output=True,
                text=True,
                timeout=60,
            )
            self.assertEqual(result.returncode, 77)
            self.assertIn("pipewire not found: skipping", result.stdout)

    def test_every_wrapper_passes_with_a_long_tmpdir_and_a_dying_daemon(self) -> None:
        with tempfile.TemporaryDirectory() as work:
            harness = Harness(Path(work))
            result = harness.run()
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertIn("ok:   test_pipewire_cursor_hdr.py, long TMPDIR", result.stdout)
            self.assertIn("ok:   test_pipewire_context_fail.py, long TMPDIR", result.stdout)
            self.assertIn("ok:   test_pulse_monitor_device.py, long TMPDIR", result.stdout)
            self.assertIn("daemon exits at once: fails (exit 1)", result.stdout)
            self.assertIn("ok:   test_pipewire_cursor_hdr.py, no pipewire binary: skips (exit 77)", result.stdout)
            self.assertNotIn("FAIL:", result.stdout)

    def test_a_wrapper_that_returns_nonzero_fails_the_harness(self) -> None:
        with tempfile.TemporaryDirectory() as work:
            harness = Harness(Path(work))
            result = harness.run(OBS_PW_WRAPPER_STUB_EXIT="5")
            self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
            self.assertIn(
                "FAIL: test_pipewire_cursor_hdr.py, long TMPDIR: the daemon starts and the test runs: exit 5",
                result.stdout,
            )


if __name__ == "__main__":
    unittest.main()
