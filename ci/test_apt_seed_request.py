#!/usr/bin/env -S uv run --script
"""A PR may request a seed, but only master may write the cache.

The cache key is hashFiles('rust/tools/linux-validate/apt-packages.txt'), so a
PR that edits that list changes the key and starts against an empty archive.
These tests pin the two halves of the answer:

* apt-seed.yaml takes a source_ref and checks out only that ref's package list,
  so the key matches the consumer's while the script stays master's.
* apt-seed-request.yaml never writes a cache itself. It only dispatches a
  master-ref run, because a cache written by a pull_request run is scoped to
  that ref and invisible to the merge queue.
"""

from __future__ import annotations

from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parent.parent
SEED = ROOT / ".github" / "workflows" / "apt-seed.yaml"
REQUEST = ROOT / ".github" / "workflows" / "apt-seed-request.yaml"
CI_TOML = ROOT / "ci.toml"


def text(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def strip_comments(source: str) -> str:
    return "\n".join(
        line for line in source.splitlines() if not line.lstrip().startswith("#")
    )


class SeedInputTest(unittest.TestCase):
    def setUp(self):
        self.seed = text(SEED)

    def test_dispatch_takes_a_source_ref_defaulting_to_master(self):
        self.assertRegex(self.seed, r"(?m)^  workflow_dispatch:\n")
        self.assertIn("source_ref:", self.seed)
        self.assertRegex(self.seed, r"(?m)^\s+default: master$")

    def test_package_list_step_is_skipped_for_master(self):
        step = self.seed.split("- name: Check Out Package List", 1)[1].split(
            "\n      - ", 1
        )[0]
        self.assertIn("inputs.source_ref != 'master'", step)

    def test_only_the_package_list_is_checked_out_from_the_requested_ref(self):
        step = self.seed.split("- name: Check Out Package List", 1)[1].split(
            "\n      - ", 1
        )[0]
        self.assertIn(
            "git checkout FETCH_HEAD -- rust/tools/linux-validate/apt-packages.txt",
            step,
        )
        # A second path in the checkout would let PR code reach the seed script.
        self.assertEqual(step.count("git checkout"), 1)

    def test_seed_script_is_never_checked_out_from_the_requested_ref(self):
        step = self.seed.split("- name: Check Out Package List", 1)[1].split(
            "\n      - ", 1
        )[0]
        self.assertNotIn("ci/seed_apt_cache.py", step)

    def test_token_is_passed_in_the_url_not_persisted(self):
        step = self.seed.split("- name: Check Out Package List", 1)[1].split(
            "\n      - ", 1
        )[0]
        self.assertIn("x-access-token:${GITHUB_TOKEN}@github.com", step)
        self.assertIn("persist-credentials: false", self.seed)


class RequestWorkflowTest(unittest.TestCase):
    def setUp(self):
        self.request = strip_comments(text(REQUEST))

    def test_triggers_on_labeled_pull_requests_only(self):
        self.assertIn("types: [labeled]", self.request)
        self.assertIn("pull_request:", self.request)

    def test_guards_on_the_label_name(self):
        self.assertIn("github.event.label.name == 'seed-apt'", self.request)

    def test_checks_the_actor_permission_before_spending_minutes(self):
        self.assertIn("/collaborators/", self.request)
        self.assertIn("admin|write", self.request)

    def test_dispatches_at_master_not_at_the_pr_ref(self):
        self.assertIn("gh workflow run apt-seed.yaml", self.request)
        self.assertIn("--ref master", self.request)
        self.assertIn("source_ref=refs/pull/", self.request)

    def test_never_writes_a_cache_itself(self):
        # A pull_request-scoped cache is invisible to the merge queue, so this
        # workflow must not try to save one.
        self.assertNotIn("actions/cache", self.request)
        self.assertNotIn("mode: save", self.request)

    def test_declares_actions_write_for_the_dispatch(self):
        self.assertIn("actions: write", self.request)


class AllowListTest(unittest.TestCase):
    def test_request_workflow_is_allowlisted(self):
        toml = text(CI_TOML)
        self.assertIn('"apt-seed-request.yaml" = ["pull_request"]', toml)
        self.assertIn('"apt-seed.yaml" = ["workflow_dispatch", "schedule"]', toml)


if __name__ == "__main__":
    unittest.main()
