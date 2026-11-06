"""Contracts for opt-in packaged startup diagnostics."""
import importlib.util
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


class DiagnosticsTest(unittest.TestCase):
    def test_capture_is_default_off_and_wraps_original_command(self):
        spec = importlib.util.spec_from_file_location("diagnostics", ROOT / "ci/macos_startup_diagnostics.py")
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        command = ["/App With Spaces/obs", "--profile", "PackagedSmoke"]
        self.assertEqual(module.launch_command(command, "fresh", {}), command)
        captured = module.launch_command(command, "fresh", {"OBS_MACOS_STARTUP_LLDB": "1"})
        self.assertEqual(captured[-len(command):], command)
        self.assertIn("--", captured)

    def test_both_matrix_legs_always_upload(self):
        workflow = (ROOT / ".github/workflows/release-rust.yaml").read_text()
        job = workflow.split("  macos_smoke:")[1].split("  gate:")[0]
        self.assertIn("if: always()\n        uses: actions/upload-artifact", job)
        self.assertIn("run: uv run --no-project ci/macos_dmg_startup.py", job)
