#!/usr/bin/env -S uv run --script
"""PR CI must also run in GitHub's merge queue (obs-rust/obs-studio#692).

    uv run python -m unittest ci/test_merge_queue.py

The master ruleset queues merges and requires the `CI Gate ✅` check, so the
required checks run on the merge result instead of a stale PR head. That only
works if:
- ci.yml runs on `merge_group` and on every PR (no paths-ignore, or a
  docs-only PR would never get the required check);
- one always-running job (`CI Gate ✅`) fails when any other job failed or was
  cancelled, so the ruleset needs a single, stable check name;
- build-project.yaml scopes a merge group against its base commit and never
  lets a local-gate stamp skip tests there: the queue commit is not the
  stamped head.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PULL = (ROOT / ".github" / "workflows" / "ci.yml").read_text()
BUILD = (ROOT / ".github" / "workflows" / "build-project.yaml").read_text()


def on_block(text: str) -> str:
    m = re.search(r"^on:\n((?:[ \t].*\n|\n)*)", text, re.M)
    return m.group(1) if m else ""


def job_block(text: str, job: str) -> str:
    m = re.search(rf"^  {re.escape(job)}:\n((?:    .*\n|\s*\n)*)", text, re.M)
    return m.group(1) if m else ""


def top_level_jobs(text: str) -> list[str]:
    body = text.split("\njobs:\n", 1)[1]
    return re.findall(r"^  ([A-Za-z0-9_-]+):\s*$", body, re.M)


class MergeQueue(unittest.TestCase):
    def test_pr_fallback_executes_six_targets_and_deferred_jobs_use_fast_path(self):
        for name in ("rust-cross-lint", "rust-cross-lint-release"):
            header = job_block(BUILD, name).split("    steps:", 1)[0]
            self.assertIn("github.event_name == 'pull_request'", header)
            self.assertIn("skip_rust-cross-lint", header)
            self.assertNotIn("cross_hit", header)
        for name in ("cross-compile", "rust-windows-link", "linux-qt-frontend",
                     "rust-tests-windows", "rust-tests-macos", "rust-tier2-windows",
                     "rust-tier2-macos", "rust-linux-tier2-build"):
            header = job_block(BUILD, name).split("    steps:", 1)[0]
            self.assertIn("needs: ", header)
            self.assertIn("ci-mode", header)
            self.assertIn("fast_path", header)
            self.assertIn("github.event_name", header)

    def test_pull_runs_in_merge_queue(self):
        self.assertRegex(on_block(PULL), r"(?m)^  merge_group:")

    def test_pull_runs_on_every_pr(self):
        self.assertNotIn("paths-ignore", on_block(PULL))

    def test_gate_job_needs_every_other_job(self):
        gate = job_block(PULL, "ci-gate")
        self.assertIn("name: CI Gate ✅", gate)
        self.assertIn("if: always()", gate)
        needs = re.search(r"needs: \[([^\]]*)\]", gate)
        self.assertIsNotNone(needs)
        listed = {n.strip() for n in needs.group(1).split(",")}
        # ci-pre and the end-of-queue cache-janitor (ci/test_ci_pre.py) are
        # maintenance: a sweep never gates, and the final one runs after it.
        # tier2-lane-proof publishes a proof the gate already vouched for (#3018).
        self.assertEqual(listed, set(top_level_jobs(PULL)) - {"ci-gate", "ci-pre", "cache-janitor", "tier2-lane-proof"})
        self.assertIn("contains(needs.*.result, 'failure')", gate)
        self.assertIn("contains(needs.*.result, 'cancelled')", gate)

    def test_scope_uses_merge_group_base(self):
        self.assertIn("github.event.merge_group.base_sha", job_block(BUILD, "check-event"))

    def test_queue_heavy_jobs_ignore_proofs_and_attestation(self):
        for name in ("rust-tests-linux", "rust-cross-lint", "rust-tests-macos",
                     "rust-tests-windows", "rust-tier2-macos", "rust-tier2-windows",
                     "rust-linux-tier2-build"):
            with self.subTest(job=name):
                header = job_block(BUILD, name).split("    steps:", 1)[0]
                condition = re.search(r"^    if: (.+)$", header, re.M).group(1)
                # Force every client/hosted skip output true. Queue execution
                # must still be selected; false path scope may still skip it.
                def allows(selected):
                    values = {"docs_only": "false", "deferred": "true",
                              "macos": selected, "windows": selected,
                              "tier2": selected, "cross_lint": selected}
                    expr = condition.removeprefix("${{").removesuffix("}}")
                    expr = expr.replace("github.event_name", repr("merge_group"))
                    expr = re.sub(r"needs\.check-event\.outputs\.([\w_-]+)",
                                  lambda m: repr(values[m[1]]), expr)
                    expr = re.sub(r"needs\.ci-mode\.outputs\.[\w_-]+", repr("true"), expr)
                    expr = expr.replace("needs.rust-tests-linux.result", repr("success"))
                    expr = re.sub(r"inputs\.[\w_-]+", "False", expr)
                    expr = expr.replace("always()", "True").replace("cancelled()", "False")
                    expr = expr.replace("&&", " and ").replace("||", " or ")
                    expr = re.sub(r"!(?!=)", " not ", expr)
                    return eval(expr, {"__builtins__": {}})
                self.assertTrue(allows("true"), condition)
                if name != "rust-tests-linux":
                    self.assertFalse(allows("false"), condition)
        mode = job_block(BUILD, "ci-mode")
        self.assertIn("PROOFS: ${{ github.event_name == 'pull_request' && !inputs.full }}", mode)

    def test_queue_disables_command_verdict_replay(self):
        for text in (BUILD, (ROOT / ".github/workflows/check-format.yaml").read_text()):
            self.assertIn("SOLDR_VERDICTS: ${{ github.event_name == 'merge_group' && 'off' || 'auto' }}", text)

    def test_full_queue_rust_tests_also_ignore_hosted_proof(self):
        fast = job_block(BUILD, "rust-tests-linux")
        self.assertIn(
            "if: needs.check-event.outputs.deferred != 'true' && "
            "(github.event_name == 'merge_group' || needs.ci-mode.outputs.rust-test_hit != 'true')",
            fast,
        )

    def test_gate_checks_child_coverage_from_exact_scope(self):
        gate = job_block(PULL, "ci-gate")
        self.assertIn("uv run --no-project ci/gate_jobs.py", gate)
        self.assertIn("fetch-depth: 0", gate)
        self.assertNotIn("needs.build-project.outputs.core", gate)
        extra = job_block(BUILD, "rust-cross-lint-release")
        self.assertIn("github.event_name == 'merge_group'", extra)
        self.assertIn("inputs.mq-runner-labels", extra)

    def test_cross_job_declares_host_uic_runtime_on_minimal_runners(self):
        cross = job_block(BUILD, "cross-compile")
        self.assertIn("libglib2.0-0t64", cross)

    def test_stamp_never_skips_in_merge_queue(self):
        ci_mode = job_block(BUILD, "ci-mode")
        self.assertRegex(ci_mode, r"if: github\.event_name != 'merge_group'")


if __name__ == "__main__":
    unittest.main()
