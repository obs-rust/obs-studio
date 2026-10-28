#!/usr/bin/env -S uv run --script
"""dispatch.yaml's commit-hash step stays inside the shell budget (GEN-005, #2829).

    uv run python -m unittest ci/test_dispatch_documentation_step.py

`jobs.deploy-documentation.steps[0]` used to be a two-line block scalar whose
first line was a no-op `: Get Commit Hash` restatement of the step name, so
`ci-lint`'s GEN-005 flagged the workflow and `.github/workflows/dispatch.yaml`
carried an approved exception. The step is now one `echo` into
`$GITHUB_OUTPUT`; this test keeps it that way and keeps the exception deleted.
`ci-lint precheck` needs the network, so the rule is mirrored here over the
workflow text and ci.toml.
"""

from __future__ import annotations

import re
import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = ROOT / ".github" / "workflows" / "dispatch.yaml"
CI_TOML = ROOT / "ci.toml"
PATH = ".github/workflows/dispatch.yaml"

EXPECTED_RUN = 'echo "commitHash=${GITHUB_SHA:0:9}" >> $GITHUB_OUTPUT'


def load_ci_toml() -> dict:
    with CI_TOML.open("rb") as handle:
        return tomllib.load(handle)


def documentation_step(text: str) -> str:
    """The YAML lines of deploy-documentation's first step (through `run:`)."""

    job = re.search(r"(?ms)^  deploy-documentation:\n(.*?)(?=^  [a-z0-9-]+:\n)", text)
    assert job is not None, "dispatch.yaml has no deploy-documentation job"
    step = re.search(r"(?ms)^      - name: Get Commit Information.*?(?=^      - |\Z)", job.group(1))
    assert step is not None, "deploy-documentation has no commit-information step"
    return step.group(0)


class DispatchDocumentationStepTest(unittest.TestCase):
    def setUp(self):
        self.text = WORKFLOW.read_text(encoding="utf-8")
        self.step = documentation_step(self.text)

    def test_step_keeps_its_id_and_output(self):
        """The artifact name is built from `steps.setup.outputs.commitHash`."""

        self.assertRegex(self.step, r"(?m)^        id: setup$")
        self.assertIn("steps.setup.outputs.commitHash", self.text)

    def test_run_is_a_single_line(self):
        """GEN-005's shell budget: one line, no block scalar, no control syntax."""

        self.assertNotRegex(self.step, r"(?m)^        run: [|>]", "run: must not be a block scalar")
        run = re.search(r"(?m)^        run:(.*)$", self.step)
        self.assertIsNotNone(run, "the step has no run:")
        assert run is not None
        self.assertEqual(run.group(1).strip(), EXPECTED_RUN)

    def test_run_still_writes_the_nine_character_hash(self):
        """`${GITHUB_SHA:0:9}` is the 9-character hash the artifact name embeds."""

        self.assertIn("${GITHUB_SHA:0:9}", self.step)
        self.assertIn(">> $GITHUB_OUTPUT", self.step)

    def test_no_gen005_exception_for_this_workflow(self):
        stale = [
            entry
            for entry in load_ci_toml().get("exceptions", [])
            if entry.get("rule") == "GEN-005" and entry.get("path") == PATH
        ]
        self.assertEqual(stale, [], "dispatch.yaml's GEN-005 exception must be deleted")


if __name__ == "__main__":
    unittest.main()
