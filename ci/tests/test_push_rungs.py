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
import json
import subprocess
import shutil
import sys
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
from pr_attestation import PROOFT_SOURCE


class Rungs(unittest.TestCase):
    HOST = "x86_64-unknown-linux-gnu"

    def attest(self, tools=None, version=(0, 1, 0), partial=False, fail=None, dirty=False, signed=True):
        calls = []
        def run(args, **kw):
            calls.append(args)
            output = "{}"
            if "emit" in args:
                gates = (["rust/all/fmt", f"rust/{self.HOST}/clippy"] if partial
                         else sorted(push.client_gates(self.HOST) | push.CLIENT_CHECK_GATES | ({"swift/all/fmt"} if fail != "client-swift" else set())))
                output = json.dumps({"v": 1, "attestations": [{"gate": g} for g in gates]})
            return subprocess.CompletedProcess(args, 1 if fail and fail in args else 0, output, "")
        def git(*args, **kw):
            calls.append(["git", *args])
            return subprocess.CompletedProcess(args, 0, "tree" if args == ("rev-parse", "HEAD^{tree}") else "a" * 40, "")
        out = io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(out), \
                mock.patch.object(push.shutil, "which", side_effect=lambda n: (tools or {"prooft": "prooft"}).get(n)), \
                mock.patch.object(push, "_prooft_version", return_value=version), \
                mock.patch.object(push, "host_target", return_value=self.HOST, create=True), \
                mock.patch.object(push, "run", side_effect=run), \
                mock.patch.object(push, "git", side_effect=git), \
                mock.patch.object(push, "dirty", return_value=dirty), \
                mock.patch.object(push, "sign_final", side_effect=lambda: calls.append(["sign-final"]) or signed, create=True), \
                mock.patch.object(push, "supports", return_value=True), \
                mock.patch.object(push, "record_client_context", return_value=True, create=True), \
                mock.patch.object(push, "evidence_path", return_value=Path(tempfile.gettempdir()) / "push-rungs-evidence.json"):
            result = push.attest()
        return result, out.getvalue(), calls

    def test_prooft_runs_repo_lanes_before_final_sign(self):
        result, text, calls = self.attest()
        self.assertTrue(result, text)
        self.assertEqual([c[2] for c in calls if c[:2] == ["prooft", "run"]],
                         ["client-lint", "client-checks", f"client-test-{self.HOST}", "client-swift"])
        self.assertLess(max(i for i, c in enumerate(calls) if c[:2] == ["prooft", "run"]),
                        calls.index(["sign-final"]))
        self.assertNotIn(["prooft", "soldr"], calls)

    def test_successful_partial_prooft_is_not_complete(self):
        result, text, calls = self.attest(partial=True)
        self.assertFalse(result, text)
        self.assertNotIn(["sign-final"], calls)

    def test_failed_or_not_applicable_command_cannot_prove_success(self):
        for lane in ("client-lint", "client-checks", f"client-test-{self.HOST}"):
            with self.subTest(lane=lane):
                result, text, calls = self.attest(fail=lane)
                self.assertFalse(result, text)
                self.assertNotIn(["sign-final"], calls)

    def test_dirty_inputs_cannot_be_signed_as_proven(self):
        result, text, calls = self.attest(dirty=True)
        self.assertFalse(result, text)
        self.assertNotIn(["sign-final"], calls)

    def test_old_prooft_keeps_deprecated_partial_path(self):
        result, text, calls = self.attest(
            tools={"prooft": "prooft", "soldr": "soldr", "ghd": "ghd"}, version=(0, 0, 9))
        self.assertFalse(result, text)
        self.assertIn("partial", text.lower())
        self.assertTrue(any(c[:3] == ["ghd", "attest", "stamp"] for c in calls))

    def test_unsigned_final_object_loses_fast_path(self):
        result, text, calls = self.attest(signed=False)
        self.assertFalse(result, text)
        self.assertIn(["sign-final"], calls)

    def test_strict_partial_evidence_never_pushes(self):
        with mock.patch.object(push, "REQUIRE", True), mock.patch.object(push, "dirty", return_value=False), \
                mock.patch.object(push, "attest", return_value=False), \
                mock.patch.object(push, "git", return_value=subprocess.CompletedProcess([], 0, "feature\n")), \
                mock.patch.object(push, "run") as transport:
            self.assertEqual(push.main([]), 3)
        transport.assert_not_called()

    def test_final_sign_is_verified_after_amending(self):
        events = []
        def run(args, **kw):
            events.append(args)
            return subprocess.CompletedProcess(args, 0, "key\n", "")
        with mock.patch.object(push, "run", side_effect=run):
            self.assertTrue(push.sign_final())
        sign = next(i for i, c in enumerate(events) if "--amend" in c)
        verify = next(i for i, c in enumerate(events) if "verify-commit" in c)
        self.assertLess(sign, verify)
        self.assertIn("-S", events[sign])

    def test_stale_signature_is_not_accepted(self):
        def run(args, **kw):
            return subprocess.CompletedProcess(args, int("verify-commit" in args), "key\n", "")
        with mock.patch.object(push, "run", side_effect=run):
            self.assertFalse(push.sign_final())


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


class ProoftIntegration(unittest.TestCase):
    """Real pinned writer/parser, cheap commands, isolated repository, no remote.

    The command-capture tests prove the expensive lane's Rust commands. This
    fixture tests the policy/writer contract without building those targets.
    It uses the verifier's exact source pin, independent of host installations.
    """
    def test_emitted_inventory_and_source_only_invalidation(self):
        tool = ["uv", "run", "--no-project", "--with", PROOFT_SOURCE, "prooft"]
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            def command(*args, check=True):
                result = subprocess.run(args, cwd=root, text=True, capture_output=True)
                if check and result.returncode:
                    self.fail(f"{args}: {result.stdout}\n{result.stderr}")
                return result
            command("git", "init", "-q")
            command("git", "config", "user.name", "Client proof fixture")
            command("git", "config", "user.email", "fixture@example.invalid")
            command("git", "config", "commit.gpgsign", "false")
            shutil.copy(ROOT / "ci-attestations.yml", root)
            shutil.copy(ROOT / "local-gate.toml", root)
            source = root / "rust" / "fixture" / "src" / "lib.rs"
            source.parent.mkdir(parents=True)
            source.write_text("// original source\n")
            command("git", "add", "ci-attestations.yml", "local-gate.toml", "rust/fixture/src/lib.rs")
            command("git", "commit", "-qm", "Fixture input")
            for host in push.CLIENT_TARGETS:
                # Clear previous host proofs: never let a Linux lint pass
                # manufacture macOS/Windows native runtime evidence.
                command("git", "commit", "--amend", "-qm", "Fixture input")
                command(*tool, "run", "client-lint", "--", sys.executable, "-c", "pass")
                command(*tool, "run", f"client-test-{host}", "--", sys.executable, "-c", "pass")
                bundle = json.loads(command(*tool, "emit").stdout)
                self.assertEqual({a["gate"] for a in bundle["attestations"]}, push.client_gates(host))
            # Exit 75 is not success, even if a caller marks its lane optional.
            command("git", "commit", "--amend", "-qm", "Fixture input")
            result = command(*tool, "run", "client-test-x86_64-unknown-linux-gnu", "--",
                             sys.executable, "-c", "raise SystemExit(75)", check=False)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(json.loads(command(*tool, "emit").stdout)["attestations"], [])
            command(*tool, "run", "client-lint", "--", sys.executable, "-c", "pass")
            message = command("git", "log", "-1", "--format=%B").stdout
            source.write_text("// changed source only\n")
            command("git", "add", "rust/fixture/src/lib.rs")
            command("git", "commit", "-qm", message)
            self.assertEqual(json.loads(command(*tool, "emit").stdout)["attestations"], [])
            # A successful command that changes a tracked input stamps nothing.
            result = command(*tool, "run", "client-lint", "--", sys.executable, "-c",
                             "from pathlib import Path; Path('rust/fixture/src/lib.rs').write_text('changed')",
                             check=False)
            self.assertNotEqual(result.returncode, 0)


if __name__ == "__main__":
    unittest.main()
