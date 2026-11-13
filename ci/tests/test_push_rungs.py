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
import os
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
    PROOFT = ["uv", "run", "--no-project", "--with", PROOFT_SOURCE, "prooft"]

    def attest(self, tools=None, partial=False, fail=None, dirty=False, signed=True):
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
                mock.patch.object(push.shutil, "which", side_effect=lambda n: (tools or {"uv": "uv"}).get(n)), \
                mock.patch.object(push, "host_target", return_value=self.HOST, create=True), \
                mock.patch.object(push, "run", side_effect=run), \
                mock.patch.object(push, "git", side_effect=git), \
                mock.patch.object(push, "dirty", return_value=dirty), \
                mock.patch.object(push, "sign_final", side_effect=lambda: calls.append(["sign-final"]) or signed, create=True), \
                mock.patch.object(push, "supports", return_value=True), \
                mock.patch.object(push, "soldr_environment", return_value=contextlib.nullcontext(), create=True), \
                mock.patch.object(push, "record_client_context", return_value=True, create=True), \
                mock.patch.object(push, "evidence_path", return_value=Path(tempfile.gettempdir()) / "push-rungs-evidence.json"):
            result = push.attest()
        return result, out.getvalue(), calls

    def test_admission_contract_requires_client_checks_and_six_targets(self):
        import pr_attestation
        self.assertIn("python/all/self-test", pr_attestation.CLIENT_GATES)
        self.assertIn("general/all/client-lint", pr_attestation.CLIENT_GATES)
        self.assertEqual(len(pr_attestation.TARGETS), 6)
        self.assertTrue(all(f"rust/{t}/client-dylint" in pr_attestation.CLIENT_GATES
                            for t in pr_attestation.TARGETS))

    def test_prooft_runs_repo_lanes_before_final_sign(self):
        result, text, calls = self.attest()
        self.assertTrue(result, text)
        prefix = self.PROOFT + ["run"]
        self.assertEqual([c[len(prefix)] for c in calls if c[:len(prefix)] == prefix],
                         ["client-lint", "client-checks", f"client-test-{self.HOST}", "client-swift"])
        self.assertLess(max(i for i, c in enumerate(calls) if c[:len(prefix)] == prefix),
                        calls.index(["sign-final"]))
        self.assertNotIn(self.PROOFT + ["soldr"], calls)

    def test_installed_prooft_does_not_override_verifier_pin(self):
        result, text, calls = self.attest(tools={"uv": "uv", "prooft": "obsolete-prooft"})
        self.assertTrue(result, text)
        self.assertIn(self.PROOFT + ["emit"], calls)
        self.assertFalse(any("obsolete-prooft" in c for c in calls))

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

    def test_missing_uv_keeps_deprecated_partial_path(self):
        result, text, calls = self.attest(
            tools={"soldr": "soldr", "ghd": "ghd"})
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


class PullRequestTransport(unittest.TestCase):
    """Exercise main's transport decisions without publishing or running proof lanes."""

    BRANCH = "fix/fork-head"
    BASE = "obs-rust/obs-studio"

    def transport(self, *, destination=BASE, push_urls=None, existing="", pressure=0,
                  lookup_error=False, resolve_error=False, attested=False):
        calls = []
        push_urls = push_urls or [f"git@github.com:{destination}.git"]

        def command(args, **kwargs):
            calls.append(args)
            output, status = "", 0
            if args[:3] == ["git", "rev-parse", "--abbrev-ref"]:
                output = self.BRANCH
            elif args[:3] == ["git", "remote", "get-url"]:
                self.assertIn("--push", args)
                self.assertIn("--all", args)
                output = "\n".join(push_urls)
            elif args[:2] == ["git", "log"]:
                output = "Fix fork PR creation"
            elif args[:2] == ["git", "push"]:
                pass
            elif args[:3] == ["gh", "repo", "view"]:
                selected = destination if args[3] in push_urls else self.BASE
                output = json.dumps({"nameWithOwner": selected, "url": f"https://github.com/{selected}"})
                status = int(resolve_error)
            elif args[:3] == ["gh", "pr", "view"]:
                # Old main never knows which repository received the branch.
                status = 1
            elif args[:2] == ["gh", "api"]:
                output, status = existing, int(lookup_error)
            elif args[:3] == ["gh", "pr", "create"]:
                pass
            elif "backpressure.py" in " ".join(args):
                output, status = "backpressure closed", pressure
            else:
                self.fail(f"unexpected command: {args}")
            if status and kwargs.get("check"):
                raise subprocess.CalledProcessError(status, args, output, "private transport error")
            return subprocess.CompletedProcess(args, status, output, "")

        output = io.StringIO()
        with contextlib.redirect_stdout(output), contextlib.redirect_stderr(output), \
                mock.patch.object(push, "REQUIRE", False), \
                mock.patch.object(push, "dirty", return_value=False), \
                mock.patch.object(push, "attest", return_value=attested), \
                mock.patch.object(push.subprocess, "run", side_effect=command):
            result = push.main([])
        return result, calls, output.getvalue()

    def creation(self, calls):
        commands = [c for c in calls if c[:3] == ["gh", "pr", "create"]]
        self.assertEqual(len(commands), 1)
        return commands[0]

    def assert_target(self, calls, head):
        create = self.creation(calls)
        self.assertIn("--head", create)
        self.assertEqual(create[create.index("--head") + 1], head)
        for flag in ("--repo", "--base"):
            self.assertIn(flag, create)
        self.assertEqual(create[create.index("--repo") + 1], f"https://github.com/{self.BASE}")
        self.assertEqual(create[create.index("--base") + 1], "master")

    def test_unattested_publish_keeps_title_and_backpressure(self):
        result, calls, output = self.transport(pressure=75)
        self.assertEqual(result, 0, output)
        self.assertIn(["git", "push", "-u", "origin", self.BRANCH], calls)
        create = self.creation(calls)
        self.assertIn("--fill", create)
        self.assertIn("--draft", create)
        self.assertEqual(create[create.index("--title") + 1], "Fix fork PR creation")
        self.assertIn("ordinary PR CI remains required", output)

    def test_same_repository_keeps_unqualified_head(self):
        result, calls, output = self.transport(attested=True)
        self.assertEqual(result, 0, output)
        self.assert_target(calls, self.BRANCH)
        self.assertNotIn("--draft", self.creation(calls))

    def test_fork_uses_destination_owner_and_preserves_fallback_draft(self):
        result, calls, output = self.transport(destination="contributor/renamed-obs", pressure=75)
        self.assertEqual(result, 0, output)
        self.assert_target(calls, f"contributor:{self.BRANCH}")
        self.assertIn("--draft", self.creation(calls))
        self.assertIn("ordinary PR CI remains required", output)
        self.assertFalse(any(c[:3] == ["gh", "api", "user"] for c in calls))

    def test_separate_push_url_selects_fork_not_fetch_repository(self):
        for url in ("git@github.com:other-owner/obs-studio.git",
                    "https://github.com/other-owner/obs-studio.git",
                    "ssh://git@github.com/other-owner/obs-studio.git"):
            with self.subTest(url=url):
                result, calls, output = self.transport(destination="other-owner/obs-studio", push_urls=[url])
                self.assertEqual(result, 0, output)
                self.assert_target(calls, f"other-owner:{self.BRANCH}")
                self.assertTrue(any(url in c for c in calls if c[:3] == ["gh", "repo", "view"]))

    def test_existing_fork_pr_is_reused_including_a_stacked_base(self):
        result, calls, output = self.transport(destination="contributor/obs-studio", existing="123\n")
        self.assertEqual(result, 0, output)
        self.assertFalse(any(c[:3] == ["gh", "pr", "create"] for c in calls))
        self.assertIn("PR #123 already exists", output)
        lookup = next(c for c in calls if c[:2] == ["gh", "api"])
        self.assertIn(f"repos/{self.BASE}/pulls", lookup)
        self.assertIn(f"head=contributor:{self.BRANCH}", lookup)
        self.assertEqual(lookup[lookup.index("-X") + 1], "GET")
        self.assertIn("state=open", lookup)
        self.assertFalse(any(a.startswith("base=") for a in lookup))

    def test_lookup_failure_does_not_create_a_duplicate_pr(self):
        result, calls, output = self.transport(lookup_error=True)
        self.assertNotEqual(result, 0, output)
        self.assertFalse(any(c[:3] == ["gh", "pr", "create"] for c in calls))

    def test_ambiguous_destinations_fail_before_push(self):
        result, calls, output = self.transport(push_urls=["git@github.com:a/obs.git", "git@github.com:b/obs.git"])
        self.assertNotEqual(result, 0, output)
        self.assertFalse(any(c[:2] == ["git", "push"] for c in calls))

    def test_resolution_failure_does_not_push_or_expose_credentials(self):
        secret_url = "https://user:private-token@github.com/contributor/obs-studio.git"
        result, calls, output = self.transport(push_urls=[secret_url], resolve_error=True)
        self.assertNotEqual(result, 0, output)
        self.assertFalse(any(c[:2] == ["git", "push"] for c in calls))
        self.assertNotIn("private-token", output)


class PinnedSoldr(unittest.TestCase):
    def test_pin_matches_every_ci_setup_step(self):
        from test_soldr_pin import workflow_pins
        pins = workflow_pins()
        self.assertTrue(pins)
        for workflow, versions in pins.items():
            with self.subTest(workflow=workflow):
                self.assertEqual({push.soldr_pin()}, versions)

    def test_bootstrap_scrubs_overrides_and_restores_callers_environment(self):
        original = {
            "PATH": os.defpath, "HOME": str(Path.home()),
            "SOLDR_DYLINT_TOOLCHAIN": "nightly-broken",
            "SOLDR_DYLINT_DRIVER": "/bad/driver", "SOLDR_RUSTC_WRAPPER": "/bad/wrapper",
            "RUSTUP_TOOLCHAIN": "nightly-broken", "RUSTC": "/bad/rustc",
            "RUSTDOC": "/bad/rustdoc", "RUSTC_WRAPPER": "/bad/wrapper",
            "RUSTC_WORKSPACE_WRAPPER": "/bad/workspace-wrapper",
            "CARGO_BUILD_RUSTC": "/bad/rustc", "CARGO_BUILD_RUSTC_WRAPPER": "/bad/wrapper",
            "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER": "/bad/workspace-wrapper",
        }
        calls = []
        executable = str(Path(tempfile.gettempdir()) / "pinned-soldr" / "soldr")
        def command(args, **kwargs):
            calls.append((args, dict(os.environ)))
            output = f"soldr {push.soldr_pin()}\n" if args[-1] == "--version" else executable + "\n"
            return subprocess.CompletedProcess(args, 0, output, "")
        with mock.patch.dict(os.environ, original, clear=True), \
                mock.patch.object(push.subprocess, "run", side_effect=command):
            with push.soldr_environment("uv"):
                self.assertEqual(os.environ["PATH"].split(os.pathsep)[0], str(Path(executable).parent))
                self.assertEqual(os.environ["PATH"].split(os.pathsep)[1:], original["PATH"].split(os.pathsep))
                self.assertFalse(any(k.startswith("SOLDR_DYLINT_") for k in os.environ))
                self.assertNotIn("RUSTUP_TOOLCHAIN", os.environ)
                self.assertNotIn("RUSTC", os.environ)
            self.assertEqual(dict(os.environ), original)
        self.assertIn(f"soldr=={push.soldr_pin()}", calls[0][0])
        self.assertIn("--no-build", calls[0][0])
        for _, environment in calls:
            for key in original.keys() - {"PATH", "HOME"}:
                self.assertNotIn(key, environment)

    def test_explicit_path_still_requires_exact_ci_version(self):
        executable = str(Path(tempfile.gettempdir()) / "custom" / "soldr")
        for version, succeeds in (("0.0.0", False), (None, True)):
            with self.subTest(version=version), \
                    mock.patch.dict(os.environ, {"PUSH_SOLDR": executable}), \
                    mock.patch.object(push.subprocess, "run", return_value=subprocess.CompletedProcess(
                        [], 0, f"soldr {version or push.soldr_pin()}\n", "")) as command:
                if succeeds:
                    with push.soldr_environment("uv"):
                        self.assertEqual(os.environ["PATH"].split(os.pathsep)[0], str(Path(executable).parent))
                else:
                    with self.assertRaisesRegex(OSError, "soldr.*requires"):
                        with push.soldr_environment("uv"):
                            self.fail("mismatched override reached proof commands")
                self.assertEqual(command.call_args.args[0], [executable, "--version"])

    def test_failure_restores_environment_and_never_runs_proof(self):
        before = dict(os.environ)
        with mock.patch.object(push, "dirty", return_value=False), \
                mock.patch.object(push, "git", return_value=subprocess.CompletedProcess([], 0, "tree")), \
                mock.patch.object(push.shutil, "which", return_value="uv"), \
                mock.patch.object(push.subprocess, "run", side_effect=OSError("bootstrap unavailable")), \
                mock.patch.object(push, "complete_prooft") as proof:
            with self.assertRaises(OSError):
                push.attest()
        self.assertEqual(dict(os.environ), before)
        proof.assert_not_called()


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
    It exercises the writer selected by ./push against the repository policy.
    """
    def test_emitted_inventory_and_source_only_invalidation(self):
        with mock.patch.object(push, "dirty", return_value=False), \
                mock.patch.object(push, "git", return_value=subprocess.CompletedProcess([], 0, "tree")), \
                mock.patch.object(push, "complete_prooft", return_value=True) as writer:
            self.assertTrue(push.attest())
        selected = writer.call_args.args[0]
        tool = [selected] if isinstance(selected, str) else selected
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
                command(*tool, "run", "client-checks", "--", sys.executable, "-c", "pass")
                command(*tool, "run", "client-swift", "--", sys.executable, "-c", "pass")
                bundle = json.loads(command(*tool, "emit").stdout)
                self.assertEqual({a["gate"] for a in bundle["attestations"]},
                                 push.client_gates(host) | push.CLIENT_CHECK_GATES | {"swift/all/fmt"})
                self.assertIn("Ci-Attestation:", command("git", "log", "-1", "--format=%B").stdout)
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
