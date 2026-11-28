#!/usr/bin/env -S uv run --script
"""Dell-labelled jobs never receive repository secrets on pull_request.

PRs no longer route to the Dell pool (the merge-queue selector skips them), but
build-project.yaml keeps its secret gating so a future PR route cannot leak them.
"""

from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parent.parent


class MemberPrWorkflowTest(unittest.TestCase):
    def test_dell_pr_jobs_do_not_receive_repository_secrets(self):
        text = (ROOT / ".github/workflows/build-project.yaml").read_text()
        blocks = dict(re.findall(r"^  ([\w-]+):\n(.*?)(?=^  [\w-]+:|\Z)", text, re.M | re.S))
        for name, block in blocks.items():
            if not re.search(r"runs-on: .*inputs.mq-heavy-labels", block):
                continue
            for line in block.splitlines():
                if "secrets." in line and "secrets.GITHUB_TOKEN" not in line:
                    with self.subTest(job=name, line=line):
                        self.assertIn("(github.event_name != 'pull_request' || inputs.mq-heavy-labels == '') && secrets.", line)

    def test_caller_strips_secrets_before_dell_pr_reusable_workflow(self):
        ci = (ROOT / ".github/workflows/ci.yml").read_text()
        block = ci.split("  build-project:\n", 1)[1].split("  flatpak-runtime-reuse:\n", 1)[0]
        self.assertNotIn("secrets: inherit", block)
        build = (ROOT / ".github/workflows/build-project.yaml").read_text()
        names = set(re.findall(r"secrets\.([A-Z0-9_]+)", build))
        for name in names:
            self.assertIn(name + ": ${{ (github.event_name != 'pull_request' || needs.select-mq-runner.outputs.heavy-labels == '') && secrets." + name + " || '' }}", block)

    def test_pr_path_linux_jobs_follow_selected_pool(self):
        # Fast Gate, Cross Compile and Qt Frontend Linux use the pool whenever
        # select-mq-runner picked it (merge_group and same-repo PRs alike) and
        # stay hosted when it returned empty.
        text = (ROOT / ".github/workflows/build-project.yaml").read_text()
        for job, nxt in (("rust-tests-linux", "cross-compile"),
                         ("cross-compile", "rust-cross-lint"),
                         ("linux-qt-frontend", "rust-tier2-macos")):
            block = text.split(f"  {job}:\n", 1)[1].split(f"\n  {nxt}:", 1)[0]
            runner = re.search(r"^    runs-on: (.*)$", block, re.M)[1]
            self.assertEqual(
                "${{ inputs.mq-heavy-labels != '' && fromJSON(inputs.mq-heavy-labels) || 'ubuntu-26.04' }}",
                runner, job)

if __name__ == "__main__":
    unittest.main()
