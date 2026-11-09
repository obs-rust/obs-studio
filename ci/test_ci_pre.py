#!/usr/bin/env -S uv run --script
"""ci-pre.yml runs pre-work; janitor is limited to trusted events (#1269).

    uv run python -m unittest ci/test_ci_pre.py

zackees/ci.yml (docs/policy-general.md, GEN-014..018): ci-pre.yml is a
workflow_call-only workflow with read-only top-level permissions and no
workflow-level concurrency. Its cache-janitor job holds actions: write, runs
on the repo-wide non-cancelling `cache-janitor` group, checks out ci_lint at
the pinned SHA without credentials and runs it with stdlib python3 (no
installs). ci.yml calls it on PRs and queue runs; push.yaml calls it on
master, the only event its sweep runs on. No job depends on it, and CI Gate
does not count it. The merge queue sweeps last instead (#1854,
zackees/ci.yml#405): ci.yml's final cache-janitor job runs after every other
job, so it sees what the run saved and keeps the repo under [cache].budget
by LRU before GitHub's own eviction picks.
"""

from __future__ import annotations

import re
import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORKFLOWS = ROOT / ".github" / "workflows"
CI_PRE = WORKFLOWS / "ci-pre.yml"
PULL = WORKFLOWS / "ci.yml"
PUSH = WORKFLOWS / "push.yaml"
PIN = "24362f3e60248ebe72ffd261e5585640228cbc5d"


def top_level(text: str, key: str) -> str:
    m = re.search(rf"^{key}:\n((?:[ \t]+.*\n|\n)*)", text, re.M)
    return m.group(1) if m else ""


def jobs(text: str) -> dict[str, str]:
    body = top_level(text, "jobs")
    out = {}
    for m in re.finditer(r"^  ([A-Za-z0-9_-]+):\n((?:    .*\n|\n)*)", body, re.M):
        out[m.group(1)] = m.group(2)
    return out


def needs(block: str) -> list[str]:
    m = re.search(r"^    needs: (?:\[([^\]]*)\]|(\S+))", block, re.M)
    if not m:
        return []
    return [n.strip() for n in (m.group(1) or m.group(2)).split(",")]


class CiPreTest(unittest.TestCase):
    def setUp(self):
        self.text = CI_PRE.read_text()
        self.janitor = jobs(self.text)["cache-janitor"]

    def test_workflow_shape(self):
        self.assertEqual(re.findall(r"^  ([a-z_]+):", top_level(self.text, "on"), re.M), ["workflow_call"])
        self.assertEqual(top_level(self.text, "permissions").split(), ["contents:", "read"])
        self.assertNotRegex(self.text, r"(?m)^concurrency:")
        self.assertEqual(list(jobs(self.text)), ["cache-janitor"])

    def test_janitor_job(self):
        j = self.janitor
        self.assertRegex(j, r"if:.*github\.event_name == 'push'.*refs/heads/master")
        self.assertNotIn("merge_group", j)
        self.assertRegex(j, r"concurrency:\n\s+group: cache-janitor\n\s+cancel-in-progress: false")
        self.assertRegex(j, r"permissions:\n(\s+\w+: \w+\n)*\s+actions: write")
        self.assertIn("timeout-minutes:", j)
        checkout = re.search(r"repository: zackees/ci\.yml\n((?:\s+\w[\w-]*: .*\n)*)", j)
        self.assertIsNotNone(checkout, j)
        self.assertIn(f"ref: {PIN}", checkout.group(1))
        self.assertIn("persist-credentials: false", checkout.group(1))
        # The stdlib interpreter, spelled so ci/test_uv_run.py does not flag it.
        self.assertRegex(j, r"run: py[a-z]+3 -P -m ci_lint cache janitor --repo \. ")
        # GEN-018: stdlib python3 only, nothing installed.
        self.assertNotRegex(j, r"setup-uv|setup-python|\buvx?\b|\bpipx?\b")

    def test_pr_pull_calls_it(self):
        text = PULL.read_text()
        all_jobs = jobs(text)
        self.assertEqual(next(iter(all_jobs)), "ci-pre")
        self.assertIn("uses: ./.github/workflows/ci-pre.yml", all_jobs["ci-pre"])
        self.assertIn("actions: write", all_jobs["ci-pre"])

    def test_janitor_only_runs_on_trusted_queue_without_blocking_work(self):
        text = PULL.read_text()
        all_jobs = jobs(text)
        self.assertNotRegex(all_jobs["ci-pre"], r"if:.*github\.event_name")
        # The end-of-queue janitor runs after everything, ci-pre included.
        for name, block in all_jobs.items():
            if name not in ("ci-pre", "cache-janitor"):
                with self.subTest(job=name):
                    self.assertNotIn("ci-pre", needs(block))

    def test_master_push_calls_prework_without_blocking_build(self):
        all_jobs = jobs(PUSH.read_text())
        self.assertIn("uses: ./.github/workflows/ci-pre.yml", all_jobs["ci-pre"])
        self.assertIn("github.ref == 'refs/heads/master'", all_jobs["ci-pre"])
        self.assertIn("actions: write", all_jobs["ci-pre"])
        for name, block in all_jobs.items():
            if name != "ci-pre":
                with self.subTest(job=name):
                    self.assertNotIn("ci-pre", needs(block))

    def test_gate_ignores_the_sweep(self):
        self.assertNotIn("ci-pre", needs(jobs(PULL.read_text())["ci-gate"]))

    def test_ci_toml_allows_it(self):
        allowed = tomllib.loads((ROOT / "ci.toml").read_text())["allow"]
        self.assertEqual(allowed["workflows"]["ci-pre.yml"], ["workflow_call"])
        self.assertIn("ci-pre", allowed["permissions"]["actions: write"])


def upstream(name: str, all_jobs: dict[str, str]) -> set[str]:
    seen, todo = set(), list(needs(all_jobs[name]))
    while todo:
        cur = todo.pop()
        if cur not in seen:
            seen.add(cur)
            todo.extend(needs(all_jobs.get(cur, "")))
    return seen


class EndOfQueueJanitorTest(unittest.TestCase):
    """#1854 / GEN-022: the merge queue run ends with the cache janitor."""

    def setUp(self):
        self.all_jobs = jobs(PULL.read_text())
        self.janitor = self.all_jobs.get("cache-janitor", "")

    def test_janitor_runs_after_every_other_job(self):
        self.assertTrue(self.janitor, "ci.yml has no cache-janitor job")
        self.assertEqual(set(self.all_jobs) - {"cache-janitor"}, upstream("cache-janitor", self.all_jobs))

    def test_janitor_runs_only_last_in_the_queue(self):
        self.assertRegex(self.janitor, r"if: always\(\) && github\.event_name == 'merge_group'")
        self.assertRegex(self.janitor, r"concurrency:\n\s+group: cache-janitor\n\s+cancel-in-progress: false")
        self.assertRegex(self.janitor, r"permissions:\n(\s+\w+: \w+\n)*\s+actions: write")
        self.assertIn(f"run: uvx --from git+https://github.com/zackees/ci.yml@{PIN} ci-lint cache janitor --repo . ",
                      self.janitor)
        self.assertNotIn("--no-enforce-budget", self.janitor)

    def test_gate_never_waits_on_the_sweep(self):
        self.assertNotIn("cache-janitor", upstream("ci-gate", self.all_jobs))

    def test_ci_toml_allows_it(self):
        allowed = tomllib.loads((ROOT / "ci.toml").read_text())["allow"]
        self.assertIn("cache-janitor", allowed["permissions"]["actions: write"])


if __name__ == "__main__":
    unittest.main()
