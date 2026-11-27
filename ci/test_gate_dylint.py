"""The local gate runs Dylint through soldr's native `soldr dylint` (#2293).

`soldr cargo dylint` defers to whatever cargo-dylint is on PATH ("deferring
to cargo-dylint on PATH"), so it skips soldr's managed driver and the verdict
ledger (soldr#3894) that replays a Dylint pass whose inputs already passed.

    uv run python -m unittest discover -s ci -p test_gate_dylint.py
"""

import os
import re
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import local_gate

GATE = Path(local_gate.__file__)


class GateDylintTest(unittest.TestCase):
    def setUp(self):
        self.enterContext(mock.patch.dict(os.environ, {}, clear=True))

    def test_client_proofs_cover_linted_sources_and_runner_inputs(self):
        import tomllib
        lanes = tomllib.loads((local_gate.ROOT / "local-gate.toml").read_text())["gate"]["lanes"]
        for name in ("client-lint", "client-test"):
            self.assertIn(name, lanes)
            # No exclusions means every tracked source/config/runner is keyed.
            self.assertFalse(lanes[name].get("exclude"), name)
        policy = (local_gate.ROOT / "ci-attestations.yml").read_text()
        self.assertIn('inputs: ["**"]', policy)

    def test_lint_library_pin_selects_the_driver(self):
        # soldr ranks RUSTUP_TOOLCHAIN above the lint libraries' own
        # rust-toolchain.toml. setup-soldr exports the root channel (1.98.1),
        # which made soldr 0.9.54 pick the version map's nightly-2026-05-28,
        # a nightly cargo-dylint 6.2.0 has no driver for (#5336). The lint
        # libraries' pin is the one source of truth for the Dylint channel.
        for inherited in (None, "", "1.98.1", "custom-driver"):
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
                    self.assertNotIn("RUSTUP_TOOLCHAIN", run.call_args.kwargs["env"])

    def test_soldr_dylint_override_is_preserved(self):
        # soldr's own knob still overrides the lint libraries' pin.
        with (
            mock.patch.dict(os.environ, {"SOLDR_DYLINT_TOOLCHAIN": "nightly-2099-01-01"}, clear=True),
            mock.patch.object(local_gate, "run") as run,
        ):
            local_gate.soldr_dylint(["--all"])
        self.assertEqual(run.call_args.kwargs["env"]["SOLDR_DYLINT_TOOLCHAIN"], "nightly-2099-01-01")

    def test_lint_libraries_pin_the_catalogued_nightly(self):
        # Every lint library pins one dated nightly; soldr reads these files.
        import tomllib
        channels = {
            tomllib.loads(path.read_text(encoding="utf-8"))["toolchain"]["channel"]
            for path in (local_gate.ROOT / "dylints").glob("*/rust-toolchain.toml")
        }
        self.assertEqual(len(channels), 1, channels)
        self.assertRegex(channels.pop(), r"^nightly-\d{4}-\d{2}-\d{2}$")

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


class ClientDylintTest(unittest.TestCase):
    HOST = "x86_64-unknown-linux-gnu"

    def setUp(self):
        self.enterContext(mock.patch.dict(os.environ, {}, clear=True))
        self.enterContext(mock.patch.object(local_gate, "host_target", return_value=self.HOST))
        for name in ("tracked_files", "changed_since_master"):
            self.enterContext(mock.patch.object(local_gate, name, return_value=[]))
        for name in ("clang_format", "gersemi", "ruff_ratchet", "self_test",
                     "check_test_binary_inventory", "check_feature_sets", "platform_ifdef_ratchet",
                     "perf_guard_lint", "knowledge_lint", "cpp_lint", "add_target"):
            self.enterContext(mock.patch.object(local_gate, name))
        self.commands = self.enterContext(mock.patch.object(local_gate, "run"))
        self.print = self.enterContext(mock.patch("builtins.print"))

    def lint_step(self, command):
        check = command[2] if command[:2] == ["soldr", "cargo"] else command[1]
        target = command[command.index("--target") + 1] if "--target" in command else self.HOST
        return check, target

    def test_client_lint_reuses_caller_paths_in_order(self):
        paths = {name: f"/caller/{name.lower()}" for name in
                 ("CARGO_TARGET_DIR", "SOLDR_CACHE_DIR", "CARGO_HOME", "RUSTUP_HOME", "XDG_CACHE_HOME")}
        expected = [("clippy", self.HOST)] * (1 + len(local_gate.FEATURE_SETS)) + [("dylint", self.HOST)]
        for target in local_gate.CLIENT_TARGETS:
            if target != self.HOST:
                expected.extend((("clippy", target), ("dylint", target)))
        for inherited in ({}, paths):
            with self.subTest(inherited=inherited), mock.patch.dict(os.environ, inherited, clear=True):
                self.commands.reset_mock()
                local_gate.lane_client_lint(False)
                steps = []
                for call in self.commands.call_args_list:
                    check, target = self.lint_step(call.args[0])
                    env = call.kwargs["env"]
                    self.assertEqual({name: env[name] for name in paths if name in env}, inherited)
                    self.assertEqual(env["SOLDR_RUSTC_WRAPPER"], "none")
                    if check == "dylint":
                        self.assertEqual(env["SOLDR_FORCE_MANAGED_CARGO_SUBCOMMANDS"], "1")
                    if check in ("clippy", "dylint"):
                        steps.append((check, target))
                self.assertEqual(steps, expected)

    def test_failed_or_busy_target_cannot_pass_client_lint(self):
        for target in (self.HOST, "aarch64-unknown-linux-gnu"):
            for check, code in (("clippy", 1), ("dylint", 75)):
                with self.subTest(target=target, check=check), \
                        mock.patch.object(sys, "argv", ["local_gate.py", "--lane", "client-lint"]):
                    def fail(command, **kwargs):
                        if self.lint_step(command) == (check, target):
                            raise subprocess.CalledProcessError(code, command)
                    self.commands.reset_mock()
                    self.print.reset_mock()
                    self.commands.side_effect = fail
                    self.assertEqual(local_gate.main(), 1)
                    self.assertEqual(self.lint_step(self.commands.call_args.args[0]), (check, target))
                    messages = [call.args[0] for call in self.print.call_args_list]
                    self.assertIn(f"== lane client-lint: FAILED ({code})", messages)
                    self.assertNotIn(f"== client {target}: {check} passed", messages)
                    self.assertFalse(any(message.startswith("== lane client-lint: ok") for message in messages))


if __name__ == "__main__":
    unittest.main()
