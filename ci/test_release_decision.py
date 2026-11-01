#!/usr/bin/env -S uv run --script
"""release-rust.yaml publishes only complete, gated betas (#622-#624).

    uv run python -m unittest ci/test_release_decision.py
"""

from __future__ import annotations

import contextlib
import io
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
import release_decision as rd

WF = (ROOT / ".github" / "workflows" / "release-rust.yaml").read_text(encoding="utf-8")
APPIMAGE = (ROOT / ".github" / "workflows" / "appimage.yaml").read_text(encoding="utf-8")
M = rd.MASTER_REF
V = "0.1.0-beta.20261013"


class Decision(unittest.TestCase):
    def test_publish_needs_everything_green(self):
        self.assertTrue(rd.may_publish("success", "success", "success"))
        for bad in ("failure", "cancelled", "skipped"):
            self.assertFalse(rd.may_publish("success", bad, "success"))
            self.assertFalse(rd.may_publish("success", "success", bad))
            self.assertFalse(rd.may_publish(bad, "success", "success"))

    def test_required_assets(self):
        names = rd.required_assets(V)
        self.assertEqual(len(names), 7)
        self.assertIn(f"OBS-Studio-Rust-{V}-linux-x86_64.flatpak", names)
        self.assertEqual(rd.missing_assets(V, names), [])
        short = [n for n in names if not n.endswith(".zip")]
        self.assertEqual(
            rd.missing_assets(V, short),
            [f"OBS-Studio-Rust-{V}-windows-x64.zip", f"OBS-Studio-Rust-{V}-windows-arm64.zip"],
        )

    def test_assets_cli(self):
        with tempfile.TemporaryDirectory() as d:
            for n in rd.required_assets(V):
                (Path(d) / n).write_text("x")
            self.assertEqual(rd.main(["assets", "--version", V, d]), 0)
            (Path(d) / rd.required_assets(V)[0]).unlink()
            self.assertEqual(rd.main(["assets", "--version", V, d]), 1)

    def test_check(self):
        d = "20261013"
        self.assertEqual(rd.check("schedule", M, False, False, d, d), (False, False))
        self.assertEqual(rd.check("schedule", M, False, True, d, d), (True, False))
        self.assertEqual(rd.check("workflow_dispatch", M, True, True, d, d), (False, True))
        self.assertEqual(rd.check("workflow_dispatch", M, True, False, d, d), (False, False))

    def test_check_refusals(self):
        d = "20261013"
        with self.assertRaises(ValueError):
            rd.check("workflow_dispatch", "refs/heads/feature", False, False, d, d)
        with self.assertRaises(ValueError):
            rd.check("workflow_dispatch", M, True, True, "20261012", d)
        with self.assertRaises(ValueError):
            rd.check("schedule", M, True, True, d, d)

    def test_verification_builds_a_branch_without_repairing_a_release(self):
        d = "20261013"
        self.assertEqual(
            rd.check("workflow_dispatch", "refs/heads/fix/629", False, True, d, d, verify_only=True),
            (False, False),
        )
        for event, repair, date in (("schedule", False, d), ("workflow_dispatch", True, d),
                                    ("workflow_dispatch", False, "20261012")):
            with self.subTest(event=event, repair=repair, date=date), self.assertRaises(ValueError):
                rd.check(event, M, repair, True, date, d, verify_only=True)

    def test_verification_cannot_publish_even_when_every_build_passes(self):
        self.assertFalse(rd.may_publish("success", "success", "success", verify_only=True))
        with contextlib.redirect_stdout(io.StringIO()):
            result = rd.main(["publish", "--gate", "success", "--build", "success",
                              "--appimage", "success", "--verify-only", "true"])
        self.assertEqual(result, 1)

    def test_verification_cli_exposes_its_nonpublishing_mode(self):
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            result = rd.main(["check", "--event", "workflow_dispatch", "--ref", "refs/heads/fix/629",
                              "--tag-exists", "true", "--tag-date", "20261013", "--today", "20261013",
                              "--verify-only", "true"])
        self.assertEqual(result, 0)
        self.assertEqual(set(output.getvalue().splitlines()), {"skip=false", "repair=false", "verify_only=true"})

    def test_stable_release_has_no_date_and_master_dispatch_only(self):
        # obs-studio-rust-v1.0.0: a stable dispatch carries no beta date.
        self.assertEqual(rd.check("workflow_dispatch", M, False, False, "", "20261101", stable=True), (False, False))
        self.assertEqual(rd.check("workflow_dispatch", M, True, True, "", "20261101", stable=True), (False, True))
        for event, ref, repair, exists, verify in (
            ("schedule", M, False, False, False),
            ("workflow_dispatch", "refs/heads/fix/x", False, False, False),
            ("workflow_dispatch", M, False, True, False),  # never silently skip or replace
            ("workflow_dispatch", M, False, False, True),
        ):
            with self.subTest(event=event, ref=ref, exists=exists, verify=verify), self.assertRaises(ValueError):
                rd.check(event, ref, repair, exists, "", "20261101", verify_only=verify, stable=True)

    def test_stable_assets(self):
        names = rd.required_assets("1.0.0")
        self.assertIn("OBS-Studio-Rust-1.0.0-windows-x64.zip", names)
        self.assertIn("OBS-Studio-Rust-1.0.0-macos-arm64.dmg", names)
        self.assertEqual(len(names), 7)

    def test_stable_cli_needs_no_tag_date(self):
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            result = rd.main(["check", "--event", "workflow_dispatch", "--ref", M, "--tag-exists", "false",
                              "--today", "20261101", "--stable", "true"])
        self.assertEqual(result, 0)
        self.assertIn("stable=true", output.getvalue().splitlines())


class Workflow(unittest.TestCase):
    def release_job(self) -> str:
        return WF[WF.index("\n  release:") :]

    def test_release_requires_all_builds(self):
        job = self.release_job()
        self.assertIn("needs.build.result == 'success'", job)
        self.assertIn("needs.appimage.result == 'success'", job)

    def test_downloads_are_strict(self):
        self.assertNotIn("continue-on-error", self.release_job())

    def test_release_checks_assets_and_has_repair(self):
        job = self.release_job()
        self.assertIn("release_decision.py assets", job)
        self.assertIn("--clobber", job)
        self.assertIn("repair:", WF)

    def test_continuous_moves_after_gate(self):
        job = self.release_job()
        self.assertIn("continuous", job)
        self.assertIn("--clobber", job)
        self.assertNotIn("gh release delete continuous", APPIMAGE)
        self.assertNotIn("gh release delete continuous", WF)

    def test_verification_cannot_enter_the_release_job(self):
        job = self.release_job()
        self.assertTrue("needs.check.outputs.verify_only != 'true'" in job, "verification can enter publishing")
        self.assertTrue('--verify-only "$VERIFY_ONLY"' in job, "publisher lacks the verification safety check")

    def test_stable_dispatch_tags_the_crate_version(self):
        self.assertIn("      stable:\n", WF)
        check = WF[WF.index("\n  check:") : WF.index("\n  build:")]
        self.assertIn('STABLE: ${{ inputs.stable }}', check)
        self.assertIn('version="$crate"', check)
        self.assertIn('--stable "$STABLE"', check)

    def test_notes_describe_the_shipped_hybrid(self):
        job = self.release_job()
        self.assertNotIn("It ships the C libobs", job)
        self.assertIn("Stable release", job)


if __name__ == "__main__":
    unittest.main()
