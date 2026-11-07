"""The local gate runs Dylint through soldr's native `soldr dylint` (#2293).

`soldr cargo dylint` defers to whatever cargo-dylint is on PATH ("deferring
to cargo-dylint on PATH"), so it skips soldr's managed driver and the verdict
ledger (soldr#3894) that replays a Dylint pass whose inputs already passed.

    uv run python -m unittest discover -s ci -p test_gate_dylint.py
"""

import os
import contextlib
import io
import urllib.error
import re
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import local_gate

GATE = Path(local_gate.__file__)


class DylintProofTest(unittest.TestCase):
    def exercise(self, ledger_key=None, error=False):
        import lane_proof
        def fetch(path, token):
            if error:
                raise urllib.error.URLError("offline")
            if "/artifacts?" in path:
                return {"artifacts": [] if ledger_key is None else [{
                    "name": lane_proof.artifact_name("dylint", ledger_key),
                    "workflow_run": {"id": 1},
                }]}
            from datetime import datetime, timezone
            return {"path": lane_proof.TRUSTED_WORKFLOW, "event": "pull_request",
                    "head_repository": {"full_name": "obs-rust/obs-studio"},
                    "created_at": datetime.now(timezone.utc).isoformat()}
        output = io.StringIO()
        with (
            mock.patch.dict(os.environ, {"GH_TOKEN": "fake"}, clear=True),
            mock.patch.object(lane_proof, "git", return_value=""),
            mock.patch.object(lane_proof, "touches_surface", return_value=[]),
            mock.patch.object(lane_proof, "lane_key", return_value="matching"),
            mock.patch.object(lane_proof, "api", side_effect=fetch),
            mock.patch.object(local_gate, "soldr_dylint") as run,
            contextlib.redirect_stdout(output),
        ):
            local_gate.lane_dylint(False)
        return run, output.getvalue()

    def test_matching_ledger_key_skips_dylint(self):
        run, output = self.exercise("matching")
        run.assert_not_called()
        self.assertEqual(output.strip(),
                         "dylint: attested by lane-proof-v1-dylint-matching; skipping the local pass")

    def test_different_key_or_ledger_error_runs_dylint(self):
        for key, error in (("different", False), (None, True), (None, False)):
            with self.subTest(key=key, error=error):
                run, output = self.exercise(key, error)
                run.assert_called_once()
                self.assertNotIn("attested", output)


class GateDylintTest(unittest.TestCase):
    def test_missing_toolchain_uses_root_pin(self):
        for inherited in (None, ""):
            for beside_clippy in (False, True):
                with self.subTest(inherited=inherited, beside_clippy=beside_clippy), tempfile.TemporaryDirectory() as tmp:
                    root = Path(tmp)
                    (root / "rust-toolchain.toml").write_text('[toolchain]\nchannel = "1.99.7"\n', encoding="utf-8")
                    with (
                        mock.patch.dict(os.environ, {}, clear=True),
                        mock.patch.object(local_gate, "ROOT", root),
                        mock.patch.object(local_gate, "run") as run,
                    ):
                        if inherited is not None:
                            os.environ["RUSTUP_TOOLCHAIN"] = inherited
                        local_gate.soldr_dylint(["--all"], beside_clippy=beside_clippy)
                    self.assertEqual(run.call_args.kwargs["env"].get("RUSTUP_TOOLCHAIN"), "1.99.7")

    def test_explicit_toolchain_is_preserved(self):
        with (
            mock.patch.dict(os.environ, {"RUSTUP_TOOLCHAIN": "custom-driver"}, clear=True),
            mock.patch.object(local_gate, "run") as run,
        ):
            local_gate.soldr_dylint(["--all"])
        self.assertEqual(run.call_args.kwargs["env"]["RUSTUP_TOOLCHAIN"], "custom-driver")

    def test_no_cargo_dylint_call(self):
        text = GATE.read_text(encoding="utf-8")
        self.assertIsNone(re.search(r'soldr_cargo\(\s*\[\s*"dylint"', text),
                          "run Dylint with soldr_dylint(), not soldr_cargo([\"dylint\", ...])")

    def test_soldr_dylint_runs_the_native_entry_point(self):
        gate = local_gate
        with mock.patch.object(gate, "run") as run:
            gate.soldr_dylint(["--all", "--", "--workspace", "--lib", "--bins"])
        argv = run.call_args.args[0]
        self.assertEqual(argv[:2], ["soldr", "dylint"])
        self.assertEqual(argv[2:], ["--all", "--", "--workspace", "--lib", "--bins"])
        # Same cache policy as soldr_cargo (zackees/zccache#1943).
        env = run.call_args.kwargs["env"]
        self.assertEqual(env.get("SOLDR_RUSTC_WRAPPER"), "none")
        # soldr's managed driver even where a cargo-dylint is on PATH, as in CI.
        self.assertEqual(env.get("SOLDR_FORCE_MANAGED_CARGO_SUBCOMMANDS"), "1")
        # Run alone, it shares target/ (and its CI cache) with Clippy.
        self.assertNotIn("CARGO_TARGET_DIR", env)

    def test_beside_clippy_it_uses_its_own_target_root(self):
        # soldr's no-cache preflight refuses a target dir while the Clippy
        # beside it holds a .cargo-lock there.
        with mock.patch.object(local_gate, "run") as run:
            local_gate.soldr_dylint(["--all"], beside_clippy=True)
        env = run.call_args.kwargs["env"]
        self.assertEqual(env.get("CARGO_TARGET_DIR"), str(local_gate.ROOT / "target-dylint"))

    def test_only_the_lint_lane_runs_it_beside_clippy(self):
        # The cross lane runs Dylint after Clippy; a cold target-dylint/ there
        # rebuilt the driver and every check for each target.
        text = GATE.read_text(encoding="utf-8")
        calls = re.findall(r"soldr_dylint[,(][^\n]*", text)
        self.assertEqual(sum("beside_clippy=True" in c for c in calls), 1, calls)
        self.assertTrue(any("--target" in c and "beside_clippy" not in c for c in calls), calls)


if __name__ == "__main__":
    unittest.main()
