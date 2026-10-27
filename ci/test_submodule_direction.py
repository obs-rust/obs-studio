#!/usr/bin/env -S uv run --script
"""A change may move a submodule pointer only forward (obs-rust/obs-studio#693).

    uv run python -m unittest ci/test_submodule_direction.py

PRs from stale checkouts moved plugins/obs-websocket back and silently undid
merged fixes (#492, #496, #503, #615). ci/submodule_direction.py fails when a
gitlink changes to a commit that does not descend from the base's pointer,
unless the PR title says `submodule:`.
"""

from __future__ import annotations

import importlib.util
import os
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location("submodule_direction", ROOT / "ci" / "submodule_direction.py")
submodule_direction = importlib.util.module_from_spec(spec)
spec.loader.exec_module(submodule_direction)

ENV = {
    **os.environ,
    "GIT_AUTHOR_NAME": "t",
    "GIT_AUTHOR_EMAIL": "t@example.com",
    "GIT_COMMITTER_NAME": "t",
    "GIT_COMMITTER_EMAIL": "t@example.com",
    "GIT_CONFIG_GLOBAL": os.devnull,
    "GIT_CONFIG_NOSYSTEM": "1",
}


def git(cwd: Path, *args: str) -> str:
    return subprocess.run(["git", *args], cwd=cwd, env=ENV, capture_output=True, text=True, check=True).stdout.strip()


class Fixture:
    """A superproject with one submodule whose history is a -> b -> c, plus x off a."""

    def __init__(self, tmp: Path):
        self.sub = tmp / "sub"
        self.sub.mkdir()
        git(self.sub, "init", "-q", "-b", "main")
        self.a = self.commit("a")
        self.b = self.commit("b")
        self.c = self.commit("c")
        git(self.sub, "checkout", "-q", "-b", "side", self.a)
        self.x = self.commit("x")
        git(self.sub, "checkout", "-q", "main")
        self.top = tmp / "top"
        self.top.mkdir()
        git(self.top, "init", "-q", "-b", "master")
        git(self.top, "-c", "protocol.file.allow=always", "submodule", "add", "-q", str(self.sub), "plugins/mod")
        self.point(self.b)
        self.base = git(self.top, "rev-parse", "HEAD")

    def commit(self, name: str) -> str:
        (self.sub / name).write_text(name)
        git(self.sub, "add", name)
        git(self.sub, "commit", "-q", "-m", name)
        return git(self.sub, "rev-parse", "HEAD")

    def point(self, sha: str) -> str:
        mod = self.top / "plugins" / "mod"
        git(mod, "checkout", "-q", sha)
        git(self.top, "add", "plugins/mod")
        git(self.top, "commit", "-q", "--allow-empty", "-m", f"point at {sha[:7]}")
        return git(self.top, "rev-parse", "HEAD")

    def check(self, title: str = "") -> list[str]:
        return submodule_direction.backwards(self.top, self.base, "HEAD", title)


class SubmoduleDirection(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.f = Fixture(Path(self.tmp.name))

    def tearDown(self):
        self.tmp.cleanup()

    def test_unchanged_pointer_passes(self):
        git(self.f.top, "commit", "-q", "--allow-empty", "-m", "other")
        self.assertEqual(self.f.check(), [])

    def test_forward_bump_passes(self):
        self.f.point(self.f.c)
        self.assertEqual(self.f.check(), [])

    def test_backwards_move_fails(self):
        self.f.point(self.f.a)
        problems = self.f.check()
        self.assertEqual(len(problems), 1)
        self.assertIn("plugins/mod", problems[0])
        self.assertIn("backwards", problems[0])
        self.assertIn("submodule:", problems[0])

    def test_sideways_move_fails(self):
        self.f.point(self.f.x)
        self.assertEqual(len(self.f.check()), 1)

    def test_submodule_title_allows_it(self):
        self.f.point(self.f.a)
        self.assertEqual(self.f.check("submodule: revert obs-websocket to a"), [])

    def test_missing_commits_are_fetched(self):
        # The superproject's checkout lacks the target commit until fetched.
        mod = self.f.top / "plugins" / "mod"
        self.f.point(self.f.c)
        git(mod, "checkout", "-q", self.f.b)
        for ref in git(mod, "for-each-ref", "--format=%(refname)").split():
            git(mod, "update-ref", "-d", ref)
        git(mod, "reflog", "expire", "--expire=now", "--all")
        git(mod, "gc", "-q", "--prune=now")
        self.assertEqual(self.f.check(), [])


class Wiring(unittest.TestCase):
    def test_pr_ci_runs_the_check(self):
        workflow = (ROOT / ".github" / "workflows" / "build-project.yaml").read_text()
        self.assertIn("uv run --no-project ci/submodule_direction.py", workflow)
        self.assertIn("PR_TITLE: ${{ github.event.pull_request.title }}", workflow)


if __name__ == "__main__":
    unittest.main()
