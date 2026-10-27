#!/usr/bin/env -S uv run --script
"""Tests for rust/tools/linux-validate/build_errors.py.

    uv run python -m unittest ci/test_tier2_build_errors.py

A failed Tier 2 build printed only lines matching ": error" and "FAILED:".
cargo's messages ("error[E0308]: ...", "error: ...", a build script's
panic) match neither, so #814's ON build showed just
"FAILED: [code=101] .../libobs_preflight_ffi.a" and no reason.

The rule lives in rust/tools/linux-validate/build_errors.py since the port off
the shell script (#962), and validate.sh runs it through `uv run --no-project`.
This drives the port's output over canned build logs.
"""

from __future__ import annotations

import importlib.util
import io
import os
import shlex
import subprocess
import sys
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TOOLS = ROOT / "rust" / "tools" / "linux-validate"
MODULE = TOOLS / "build_errors.py"
SHELL_SCRIPT = TOOLS / "build-errors.sh"

LOG = """\
[12/900] Building C object libobs/CMakeFiles/libobs.dir/util/dstr.c.o
/src/libobs/util/dstr.c:10:5: error: unknown type name 'foo'
FAILED: libobs/CMakeFiles/libobs.dir/util/dstr.c.o
   Compiling obs-preflight v0.1.0 (/src/rust/obs-preflight)
error[E0308]: mismatched types
  --> rust/obs-preflight/src/lib.rs:42:9
   |
42 |         "nope"
   |         ^^^^^^ expected `u32`, found `&str`

error: could not compile `obs-preflight` (lib) due to 1 previous error
thread 'main' panicked at rust/obs-preflight-ffi/build.rs:7:5:
cbindgen failed
FAILED: [code=101] obs-preflight-cargo/release/libobs_preflight_ffi.a
   Compiling obs-util v0.1.0 (/src/rust/obs-util)
warning: unused variable: `x`
"""


def load_module():
    """rust/tools/linux-validate/build_errors.py by path: `uv run python -m unittest
    ci/test_tier2_build_errors.py` does not put that directory on sys.path, and no
    file in this repo edits sys.path (ci/test_no_sys_path_edits.py)."""
    spec = importlib.util.spec_from_file_location("build_errors", MODULE)
    assert spec is not None and spec.loader is not None, f"cannot load {MODULE}"
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class BuildErrorsTest(unittest.TestCase):
    def setUp(self):
        self.module = load_module()

    @staticmethod
    def recovery_script() -> str:
        source = (TOOLS / 'validate.sh').read_text()
        recovery = source.split('  tier2_targets=', 1)[1].split('  if [ -n "${OBS_EXTRA_CFLAGS:-}"', 1)[0]
        recovery = 'tier2_targets=' + recovery
        # Run the ported helper with the interpreter running this suite: the
        # recovery block is extracted from validate.sh, which reaches the module
        # through `uv run --no-project`.
        return recovery.replace('uv run --no-project "$SRC"/rust/tools/linux-validate/build_errors.py',
                                f'{shlex.quote(sys.executable)} "$SRC"/rust/tools/linux-validate/build_errors.py')

    # A launcher that cannot reach the daemon exits 125 (run 39002068564). A
    # request the daemon accepted and then lost exits 1 with a
    # "daemon response failure" line and no compiler output: zccache's
    # wrapper/ipc/lost_request.rs keeps that exit 1 on purpose and leaves the
    # replay to the build tool (runs 39376294047, 39380784311).
    FIRST_FAILURES = (
        "echo 'FAILED: [code=125] first.o'\n"
        "    echo 'zccache[err][D]: original daemon failure'",
        "echo 'zccache[err][R]: daemon response failure (closed-connection): lost connection "
        "to daemon (no response). The requested tool returned no status; run '\\''zccache status'\\'' then retry.'\n"
        "    echo 'zccache[info][Q]: daemon under load: compiling, 2 compiles in flight'\n"
        "    printf '\\033[31mFAILED: [code=1] \\033[0mfirst.o\\n'",
    )

    def test_recovery_preserves_first_failure_and_keeps_one_retry(self):
        recovery = self.recovery_script()
        for first, retry_status, daemon_status in (
                (f, r, d) for f in self.FIRST_FAILURES for r, d in ((0, 0), (0, 1), (1, 0), (1, 1))):
            with self.subTest(first=first[:40], retry_status=retry_status, daemon_status=daemon_status), \
                    tempfile.TemporaryDirectory() as directory:
                prefix = '''
cmake() {
  if [ ! -f "$build.calls" ]; then
    echo 1 > "$build.calls"
    ''' + first + '''
    return 1
  fi
  echo 2 > "$build.calls"
  echo "retry cache bypass: ${ZCCACHE_DISABLE:-unset}"
  echo 'FAILED: [code=1] second.o'
  return "$retry_status"
}
zccache() { echo "daemon diagnostic: $*"; return "$daemon_status"; }
'''
                result = subprocess.run(
                    ['bash', '-euo', 'pipefail', '-c', prefix + recovery],
                    env={**os.environ, 'SRC': str(ROOT),
                         'build': directory + '/build', 'retry_status': str(retry_status),
                         'daemon_status': str(daemon_status),
                         'GITHUB_OUTPUT': directory + '/step-output'},
                    capture_output=True, text=True)
                self.assertEqual(result.returncode, retry_status)
                self.assertEqual(Path(directory, 'build.calls').read_text().strip(), '2')
                marker = 'original daemon failure' if 'code=125' in first else 'daemon response failure (closed-connection)'
                self.assertIn(marker, result.stdout)
                self.assertIn('daemon diagnostic: status', result.stdout)
                self.assertIn('daemon diagnostic: stop', result.stdout)
                self.assertIn('retry cache bypass: ' + ('1' if daemon_status else 'unset'),
                              Path(directory, 'build.build.log').read_text())
                self.assertIn(marker, Path(directory, 'build.build-first-failure.log').read_text())
                output = Path(directory, 'step-output')
                fields = output.read_text() if output.exists() else ''
                self.assertEqual('cache-healthy=false' in fields, bool(daemon_status))

    def test_real_compile_error_is_not_retried(self):
        # A compile that failed with exit 1 and no lost daemon request is a
        # real error: report it, never rebuild it.
        with tempfile.TemporaryDirectory() as directory:
            prefix = '''
cmake() {
  echo x >> "$build.calls"
  echo 'zccache[info][Q]: daemon under load: compiling, 2 compiles in flight'
  echo "/src/a.c:1:1: error: unknown type name 'foo'"
  echo 'FAILED: [code=1] a.o'
  return 1
}
zccache() { echo "daemon diagnostic: $*"; }
'''
            result = subprocess.run(
                ['bash', '-euo', 'pipefail', '-c', prefix + self.recovery_script()],
                env={**os.environ, 'SRC': str(ROOT), 'build': directory + '/build'},
                capture_output=True, text=True)
            self.assertEqual(result.returncode, 1)
            self.assertEqual(Path(directory, 'build.calls').read_text().count('x'), 1)
            self.assertNotIn('daemon diagnostic', result.stdout)
            self.assertIn("unknown type name 'foo'", result.stdout)

    def test_unhealthy_cache_is_not_published(self):
        workflow = (ROOT / '.github/workflows/build-project.yaml').read_text()
        build = workflow.split('- name: Build Tier 2 (libobs and libobs-c-reference)', 1)[1].split('\n      - ', 1)[0]
        save = workflow.split('- name: Save Tier 2 zccache', 1)[1].split('\n      - ', 1)[0]
        self.assertIn('id: tier2-build', build)
        self.assertIn("steps.tier2-build.outputs.cache-healthy != 'false'", save)

    def run_script(self, log: str) -> str:
        with tempfile.TemporaryDirectory() as d:
            p = Path(d) / "build.log"
            p.write_text(log)
            out = io.StringIO()
            with redirect_stdout(out):
                code = self.module.main(["build_errors.py", str(p)])
        self.assertEqual(code, 0, out.getvalue())
        return out.getvalue()

    def test_keeps_c_and_ninja_errors(self):
        out = self.run_script(LOG)
        self.assertIn("dstr.c:10:5: error: unknown type name 'foo'", out)
        self.assertIn("FAILED: [code=101] obs-preflight-cargo/release/libobs_preflight_ffi.a", out)

    def test_shows_rustc_errors_with_their_location(self):
        out = self.run_script(LOG)
        self.assertIn("error[E0308]: mismatched types", out)
        self.assertIn("--> rust/obs-preflight/src/lib.rs:42:9", out)
        self.assertIn("could not compile `obs-preflight`", out)

    def test_shows_build_script_panics(self):
        out = self.run_script(LOG)
        self.assertIn("panicked at rust/obs-preflight-ffi/build.rs:7:5", out)
        self.assertIn("cbindgen failed", out)

    def test_drops_noise(self):
        out = self.run_script(LOG)
        self.assertNotIn("Compiling obs-util", out)
        self.assertNotIn("warning: unused variable", out)

    def test_shows_launcher_failure_reason_before_failed_edge_limit(self):
        reason = "zccache[err][D]: daemon unavailable at /cache/daemon.sock (cannot connect); refusing to run cc uncached."
        log = "".join(f"FAILED: [code=125] object-{i}.o\n{reason}\n" for i in range(213))
        out = self.run_script(log)
        self.assertIn(reason, out)
        self.assertEqual(out.count(reason), 1)

    def test_usage_error(self):
        err = io.StringIO()
        with redirect_stderr(err):
            code = self.module.main(["build_errors.py"])
        self.assertEqual(code, 2)
        self.assertIn("usage: build_errors.py BUILD_LOG", err.getvalue())

    def test_shell_script_removed(self):
        self.assertFalse(SHELL_SCRIPT.exists(), f"{SHELL_SCRIPT} should have been deleted")


if __name__ == "__main__":
    unittest.main()
