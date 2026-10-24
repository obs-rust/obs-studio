"""Exercise the Rust test lane against a changed source after a warm build."""

from __future__ import annotations

import importlib.util
import os
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path
from unittest import mock

CI = Path(__file__).resolve().parent
_SPEC = importlib.util.spec_from_file_location("local_gate_probe_target", CI / "local_gate.py")
local_gate = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(local_gate)


CROSS_TARGET = "x86_64-pc-windows-gnu"


def is_cross_step(cmd: list[str]) -> bool:
    """The rust-test lane's windows-gnu link check (#291, #925). The probe
    skips it: the scratch crate has no windows-gnu std on CI (#971), and the
    real rust-test lane checks the cross link on the full workspace."""
    return CROSS_TARGET in cmd


def is_feature_set_step(cmd: list[str]) -> bool:
    """A FEATURE_TEST_SETS run of the rust-test lane (#2056). The probe skips
    it: it names workspace crates (`-p obs-util`) that the scratch crate does
    not have, and the real rust-test lane runs it on the full workspace."""
    return any(
        list(args) == cmd[i : i + len(args)]
        for args in local_gate.FEATURE_TEST_SETS
        for i in range(len(cmd))
    )


# Needs soldr: CI runs it in the lint step on the soldr runner, not in the
# ubuntu-latest ci-self-test job (#427).
@unittest.skipUnless(shutil.which("soldr"), "soldr not on PATH")
class RustSourceProbeTest(unittest.TestCase):
    def test_changed_source_fails_the_rust_test_lane(self):
        scratch_root = local_gate.ROOT / "target"
        scratch_root.mkdir(exist_ok=True)
        scratch_dir = tempfile.TemporaryDirectory(prefix="obs-gate-probe-", dir=scratch_root)
        self.addCleanup(scratch_dir.cleanup)
        scratch = Path(scratch_dir.name)
        (scratch / "src").mkdir()
        (scratch / "Cargo.toml").write_text(
            '[package]\nname = "obs-gate-probe"\nversion = "0.1.0"\nedition = "2021"\n\n[workspace]\n'
        )
        (scratch / "Cargo.lock").write_text('version = 3\n\n[[package]]\nname = "obs-gate-probe"\nversion = "0.1.0"\n')
        (scratch / "rust-toolchain.toml").write_bytes((local_gate.ROOT / "rust-toolchain.toml").read_bytes())
        source = scratch / "src" / "lib.rs"
        source.write_text("pub mod nested;\n#[test]\nfn current_source_is_ready() { assert!(nested::ready()); }\n")
        nested = scratch / "src" / "nested.rs"
        nested.write_text('// compile_error!("GATE-SOURCE-PROBE");\npub fn ready() -> bool { true }\n')

        output = []

        def run_in_scratch(cmd, **kw):
            if is_cross_step(cmd) or is_feature_set_step(cmd):
                return
            result = subprocess.run(
                cmd, cwd=scratch, env=kw.get("env"), capture_output=True, text=True, timeout=90, check=False
            )
            output.append(result.stdout + result.stderr)
            if result.returncode != 0:
                # Without this the lane's failure is just "exit status 101".
                print(f"$ {' '.join(cmd)}\n{output[-1][-4000:]}", file=sys.stderr)
            result.check_returncode()

        with mock.patch.object(local_gate, "run", side_effect=run_in_scratch):
            local_gate.lane_rust_test(False)
            stamp = nested.stat()
            nested.write_text("   " + nested.read_text()[3:])
            os.utime(nested, ns=(stamp.st_atime_ns, stamp.st_mtime_ns))
            # Force Cargo to invoke rustc while the changed nested module
            # retains its size/mtime: upstream zccache#1943's stale-hit case.
            now = time.time_ns() + 2_000_000_000
            os.utime(source, ns=(now, now))
            with self.assertRaises(subprocess.CalledProcessError):
                local_gate.lane_rust_test(False)

        self.assertIn("GATE-SOURCE-PROBE", output[-1])


class ProbeScopeTest(unittest.TestCase):
    """The probe checks change detection on the host; it must not depend on
    the runner having the windows-gnu std, which the scratch crate lacked on
    CI (#971). The real rust-test lane checks the cross link."""

    def test_cross_steps_are_skipped_in_the_scratch_crate(self):
        self.assertTrue(is_cross_step(["soldr", "rustup", "target", "add", "x86_64-pc-windows-gnu"]))
        self.assertTrue(is_cross_step(
            ["soldr", "cargo", "test", "--workspace", "--locked", "--no-run", "--target", "x86_64-pc-windows-gnu"]))
        self.assertFalse(is_cross_step(["soldr", "cargo", "test", "--workspace", "--locked"]))

    def test_feature_set_steps_are_skipped_in_the_scratch_crate(self):
        # The scratch crate has no obs-util (#2056).
        self.assertTrue(local_gate.FEATURE_TEST_SETS)
        for args in local_gate.FEATURE_TEST_SETS:
            self.assertTrue(is_feature_set_step(["soldr", "cargo", "test", *args, "--locked", "--doc"]))
        self.assertFalse(is_feature_set_step(["soldr", "cargo", "test", "--workspace", "--locked"]))


if __name__ == "__main__":
    unittest.main()
