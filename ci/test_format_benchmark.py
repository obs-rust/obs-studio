#!/usr/bin/env -S uv run --script
"""Native format measurement must stay opt-in and restore its checkout on failure."""
import importlib.util
import json
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parent.parent
SPEC = importlib.util.spec_from_file_location("compare_format", ROOT / "rust/tools/benchmarks/compare_format.py")
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class NativeFormatBenchmarkTest(unittest.TestCase):
    def test_untrusted_baseline_is_rejected_before_git(self):
        for value in ("--help", "master", "HEAD~1", "$(touch injected)", "a" * 39):
            with self.subTest(value=value), mock.patch.object(MODULE, "git") as git:
                with self.assertRaises(ValueError):
                    MODULE.validate_base(ROOT, value)
                git.assert_not_called()

    def test_failed_baseline_preserves_evidence_and_restores_candidate(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "repo"
            root.mkdir()
            def git(*args):
                return subprocess.check_output(["git", *args], cwd=root, text=True, stderr=subprocess.DEVNULL).strip()
            git("init", "-q")
            git("config", "user.email", "test@example.invalid")
            git("config", "user.name", "Format test")
            source = root / "source"
            source.write_text("baseline")
            git("add", "source")
            git("commit", "-qm", "baseline")
            baseline = git("rev-parse", "HEAD")
            source.write_text("candidate")
            git("commit", "-qam", "candidate")
            candidate = git("rev-parse", "HEAD")
            output = Path(temporary) / "evidence"
            def fail_measure(_root, directory, label):
                (directory / f"{label}.log").write_text("compiler failure")
                raise RuntimeError("probe failed")
            with mock.patch.object(MODULE, "measure", side_effect=fail_measure):
                with self.assertRaisesRegex(RuntimeError, "probe failed"):
                    MODULE.compare(root, baseline, output)
            self.assertEqual(git("rev-parse", "HEAD"), candidate)
            self.assertEqual(source.read_text(), "candidate")
            self.assertEqual((output / "baseline.log").read_text(), "compiler failure")
            manifest = json.loads((output / "comparison.json").read_text())
            self.assertEqual(manifest, {"baseline": baseline, "candidate": candidate, "status": "failed"})

    def test_workflow_has_only_an_explicit_readonly_native_measurement(self):
        text = (ROOT / ".github/workflows/dispatch.yaml").read_text()
        self.assertIn("          - format-benchmark\n", text)
        job = text.split("\n  format-benchmark:\n", 1)[1]
        self.assertIn("github.repository == 'obs-rust/obs-studio'", job)
        self.assertIn("github.event_name == 'workflow_dispatch'", job)
        self.assertIn("inputs.job == 'format-benchmark'", job)
        self.assertIn("runs-on: macos-26", job)
        self.assertIn("permissions:\n      contents: read", job)
        self.assertIn("FORMAT_BASE: ${{ inputs.ref }}", job)
        self.assertIn('compare_format.py --base "$FORMAT_BASE"', job)
        self.assertIn("if: always()", job)
        self.assertNotIn("secrets.", job)
        self.assertNotIn("contents: write", job)
        self.assertNotIn("run: |", job)


if __name__ == "__main__":
    unittest.main()
