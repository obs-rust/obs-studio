"""The Windows hosted experiment stays opt-in and outside required jobs."""
import unittest

import gate_jobs
from test_windows_lf_checkout import job


class ProofWorkflowTest(unittest.TestCase):
    def test_proof_is_opt_in_path_gated_and_nonrequired(self):
        for name in ("windows-run-only-proof-build", "windows-run-only-proof"):
            body = job(name)
            self.assertIn("vars.WINDOWS_RUN_ONLY_PROOF_REF == github.ref", body)
            self.assertIn("needs.check-event.outputs.windows_link == 'true'", body)
            self.assertIn("needs.check-event.outputs.docs_only != 'true'", body)
            self.assertIn("github.event_name != 'merge_group'", body)
            self.assertIn("github.event_name == 'workflow_dispatch'", body)
            self.assertIn("continue-on-error: true", body)
        # Existing required Windows lane retains its original native coverage.
        required = job("rust-tests-windows")
        self.assertIn("cargo test", required)
        self.assertIn("soldr cargo", required)
        self.assertNotIn("windows_test_proof", required)

    def test_consumer_uses_immutable_producer_and_never_compiles(self):
        body = job("windows-run-only-proof")
        self.assertIn("needs.windows-run-only-proof-build.result == 'success'", body)
        self.assertIn("artifact-ids: ${{ needs.windows-run-only-proof-build.outputs.artifact-id }}", body)
        for output in ("run-id", "attempt", "source-sha", "archive-sha", "logical-root"):
            self.assertIn(f"needs.windows-run-only-proof-build.outputs.{output}", body)
        self.assertIn("python-version: '3.14'", body)
        self.assertIn("shell: pwsh", body)
        self.assertIn("working-directory: ${{ steps.workspace.outputs.real-root }}", body)
        self.assertIn("core.autocrlf input", body)
        for forbidden in ("setup-soldr", "cargo ", "vcpkg", "pkgconf", "Install Qt"):
            self.assertNotIn(forbidden, body)

    def test_failed_optional_proof_does_not_weaken_or_block_required_windows(self):
        required = "Build 🧱 / Rust Tests 🦀 (windows-2025-vs2026)"
        for name in ("Windows Run-only Proof Producer", "Windows Run-only Hosted Proof"):
            for conclusion in ("failure", "cancelled", "timed_out"):
                optional = {"name": f"Build 🧱 / {name} (optional)", "status": "completed", "conclusion": conclusion}
                self.assertEqual(gate_jobs.blocking([optional]), [])
                failed = {"name": required, "status": "completed", "conclusion": conclusion}
                self.assertEqual(gate_jobs.blocking([optional, failed], expected={required}), [required])


if __name__ == "__main__":
    unittest.main()
