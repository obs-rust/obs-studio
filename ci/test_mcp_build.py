"""The configured MCP entry point must build a native app without session setup."""
from pathlib import Path
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent
ENTRY = ROOT / "mcp"


class McpConfigurationTest(unittest.TestCase):
    def test_server_uses_the_native_launcher(self):
        server = json.loads((ROOT / ".mcp.json").read_text())["mcpServers"]["obs"]
        self.assertEqual(server["command"], "./mcp")
        self.assertEqual(server["args"], ["--mcp"])
        self.assertEqual(ENTRY.read_text().splitlines()[0], "#!/usr/bin/env -S uv run --script")
        self.assertTrue(os.access(ENTRY, os.X_OK))


@unittest.skipUnless(sys.platform == "linux", "Linux native-launcher subprocess fixtures")
class NativeMcpBuildTest(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.bin = self.root / "bin"
        self.bin.mkdir()
        self.trace = self.root / "trace"
        self.native_args = self.root / "native-args"
        native = self.root / "native"
        native.write_text("#!/usr/bin/env bash\nprintf '%s\\n' \"$*\" > \"$MCP_TEST_NATIVE_ARGS\"\ncat\n")
        native.chmod(0o755)
        soldr = self.bin / "soldr"
        soldr.write_text(r'''#!/usr/bin/env bash
set -eu
printf '%s|%s\n' "$*" "${OBS_BUILD_RUST_ARCHIVE:-unset}" >> "$MCP_TEST_TRACE"
if [ "$1" = rustc ]; then printf 'host: x86_64-unknown-linux-gnu\n'; exit 0; fi
echo 'build diagnostic on stdout'
case "$*" in
  *libobs-rust*)
    if [ "${MCP_TEST_FAIL:-}" = archive ]; then exit 7; fi
    mkdir -p "$CARGO_TARGET_DIR/release"
    printf archive > "$CARGO_TARGET_DIR/release/liblibobs_rust.a"
    ;;
  *obs-studio-rust*)
    if [ "${MCP_TEST_FAIL:-}" = native ]; then exit 7; fi
    test -s "$OBS_BUILD_RUST_ARCHIVE"
    mkdir -p "$CARGO_TARGET_DIR/debug"
    cp "$MCP_TEST_NATIVE" "$CARGO_TARGET_DIR/debug/obs-studio-rust"
    ;;
  *) exit 9 ;;
esac
''')
        soldr.chmod(0o755)
        self.env = {**os.environ, "PATH": str(self.bin) + os.pathsep + os.environ["PATH"],
                    "OBS_MCP_NIX_SHELL": "1", "OBS_MCP_TARGET_DIR": str(self.root / "native-target"),
                    "MCP_TEST_TRACE": str(self.trace), "MCP_TEST_NATIVE": str(native),
                    "MCP_TEST_NATIVE_ARGS": str(self.native_args)}
        for name in ("OBS_BUILD_RUST_ARCHIVE", "OBS_MCP_SOLDR", "CPATH", "PKG_CONFIG_PATH"):
            self.env.pop(name, None)

    def invoke(self, *args, data=""):
        return subprocess.run([shutil.which("uv"), "run", "--no-project", "--script", str(ENTRY), *args],
                              env=self.env, input=data, capture_output=True, text=True, timeout=15)

    def test_builds_archive_then_native_in_an_independent_output(self):
        result = self.invoke("--build")
        self.assertEqual(result.returncode, 0, result.stderr)
        target = self.root / "native-target"
        self.assertEqual(result.stdout.strip(), str(target / "debug/obs-studio-rust"))
        builds = [line for line in self.trace.read_text().splitlines() if line.startswith("cargo build")]
        self.assertEqual(len(builds), 2)
        self.assertIn("-p libobs-rust", builds[0])
        self.assertIn("--release", builds[0])
        self.assertTrue(builds[0].endswith("|unset"))
        self.assertIn("-p obs-studio-rust", builds[1])
        self.assertIn("--features bundled-native", builds[1])
        self.assertTrue(builds[1].endswith("|" + str(target / "release/liblibobs_rust.a")))
        self.assertIn("build diagnostic on stdout", result.stderr)

    def test_launch_preserves_protocol_stdio_and_attach_arguments(self):
        request = '{"jsonrpc":"2.0","id":1,"method":"tools/list"}\n'
        for args in (("--mcp",), ("--mcp-attach", str(self.root / "control.sock"))):
            with self.subTest(args=args):
                result = self.invoke(*args, data=request)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(result.stdout, request)
                self.assertEqual(self.native_args.read_text().strip(), " ".join(args))

    def test_failed_archive_never_attempts_the_native_build(self):
        self.env["MCP_TEST_FAIL"] = "archive"
        result = self.invoke("--build")
        self.assertEqual(result.returncode, 7, result.stderr)
        self.assertNotIn("-p obs-studio-rust", self.trace.read_text())
        self.assertFalse(self.native_args.exists())

    def test_failed_rebuild_never_runs_a_stale_executable(self):
        first = self.invoke("--build")
        self.assertEqual(first.returncode, 0, first.stderr)
        self.env["MCP_TEST_FAIL"] = "native"
        result = self.invoke("--mcp", data="must not reach stale server\n")
        self.assertEqual(result.returncode, 7, result.stderr)
        self.assertFalse(self.native_args.exists())


if __name__ == "__main__":
    unittest.main()
