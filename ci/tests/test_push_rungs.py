"""The attestation rungs in ./push (prooft -> ghd -> none -> refuse).

The rungs decide which component writes the `Ci-Attestation` trailers, so a
regression here silently stops producing evidence or, worse, claims evidence it
never collected. Each rung is asserted by its message and its return value.

Run: uv run --no-project python ci/tests/test_push_rungs.py
"""

from __future__ import annotations

import contextlib
import importlib.machinery
import importlib.util
import io
import tempfile
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
LOADER = importlib.machinery.SourceFileLoader("push_script", str(ROOT / "push"))
SPEC = importlib.util.spec_from_loader("push_script", LOADER)
assert SPEC is not None
push = importlib.util.module_from_spec(SPEC)
LOADER.exec_module(push)


class Rungs(unittest.TestCase):
    def attest(self, tools: dict[str, str | None], prooft_version=(0, 1, 0), soldr_ok=True, ghd_stamp=True):
        """Run attest() with a fake PATH, capturing what it printed."""

        def which(name):
            return tools.get(name)

        def supports(args, needle):
            return ghd_stamp if "ghd" in args[0] else soldr_ok

        calls: list[list[str]] = []

        def run(args, **kwargs):
            calls.append(args)

            class Result:
                returncode = 0
                stdout = '{"v":1}'
                stderr = ""

            return Result()

        out = io.StringIO()
        gitdir = tempfile.mkdtemp()

        def fake_git(*args, **kwargs):
            class R:
                stdout = gitdir + "\n"
                returncode = 0

            return R()

        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(out), \
                mock.patch.object(push, "git", fake_git), \
                mock.patch.object(push.shutil, "which", which), \
                mock.patch.object(push, "supports", supports), \
                mock.patch.object(push, "run", run), \
                mock.patch.object(push, "_prooft_version", lambda tool: prooft_version):
            result = push.attest()
        return result, out.getvalue(), calls

    def test_prooft_runs_first_and_stamps(self):
        result, text, calls = self.attest({"prooft": "/usr/bin/prooft"})
        self.assertTrue(result, text)
        self.assertEqual(calls[0], ["/usr/bin/prooft", "soldr"], calls)
        self.assertNotIn("deprecated", text)

    def test_old_prooft_falls_through_to_the_deprecated_path(self):
        result, text, calls = self.attest(
            {"prooft": "/usr/bin/prooft", "soldr": "/usr/bin/soldr", "ghd": "/usr/bin/ghd"},
            prooft_version=(0, 0, 9),
        )
        self.assertTrue(result, text)
        self.assertIn("older than", text)
        self.assertIn("deprecated path", text)
        self.assertEqual(calls[0][0], "/usr/bin/soldr")

    def test_no_writer_returns_false_without_inventing_evidence(self):
        result, text, calls = self.attest({})
        self.assertFalse(result, text)
        self.assertIn("prooft is not installed", text)
        self.assertEqual(calls, [])

    def test_enforce_refuses_when_nothing_was_attested(self):
        with mock.patch.object(push, "REQUIRE", True), mock.patch.object(push, "attest", lambda: False), \
                mock.patch.object(push, "git") as git:
            git.return_value.stdout = "feature\n"
            git.return_value.returncode = 0
            code = push.main([])
        self.assertEqual(code, 3)


class Evidence(unittest.TestCase):
    """Evidence is per commit and pruned, never a shared singleton."""

    def test_two_commits_never_share_a_file(self):
        with tempfile.TemporaryDirectory() as gitdir, \
                mock.patch.object(push, "git") as git:
            shas = iter(["abc123", "def456"])

            def fake(*args, **kwargs):
                class R:
                    returncode = 0
                    stdout = gitdir + "\n" if "--git-dir" in args else next(shas) + "\n"

                return R()

            git.side_effect = fake
            first = push.evidence_path()
            second = push.evidence_path()
        self.assertEqual(first.name, "abc123.json")
        self.assertEqual(second.name, "def456.json")

    def test_prune_keeps_the_newest_and_drops_the_rest(self):
        import os, time
        with tempfile.TemporaryDirectory() as tmp:
            directory = Path(tmp)
            made = []
            for index in range(push.ATTEST_KEEP + 5):
                path = directory / f"{index}.json"
                path.write_text("{}", encoding="utf-8")
                os.utime(path, (time.time() - index, time.time() - index))
                made.append(path)
            push.prune_evidence(directory)
            left = sorted(directory.glob("*.json"))
        self.assertEqual(len(left), push.ATTEST_KEEP)
        self.assertIn(made[0], left)
        self.assertNotIn(made[-1], left)


if __name__ == "__main__":
    unittest.main()
