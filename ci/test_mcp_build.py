"""The configured MCP entry point must build a native app without session setup."""
from pathlib import Path
try:
    from .host_platform import is_linux
except ImportError:  # unittest discovery imports ci modules without a package.
    from host_platform import is_linux
import json
import os
import shutil
import subprocess
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


@unittest.skipUnless(is_linux(), "Linux native-launcher subprocess fixtures")
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
        native.write_text(native.read_text().replace("\ncat\n", "\nprintf '%s\\n' \"${OBS_MCP_GRAPHICS_MODULE:-}\" \"${OBS_MCP_MODULE_BIN:-}\" \"${OBS_MCP_MODULE_DATA:-}\" \"${OBS_MCP_DATA_PATH:-}\" > \"$MCP_TEST_NATIVE_ARGS.env\"\ncat\n"))
        native.chmod(0o755)
        soldr = self.bin / "soldr"
        soldr.write_text(r'''#!/usr/bin/env bash
set -eu
printf '%s|%s\n' "$*" "${OBS_BUILD_RUST_ARCHIVE:-unset}" >> "$MCP_TEST_TRACE"
if [ "$1" = rustc ]; then printf 'host: x86_64-unknown-linux-gnu\n'; exit 0; fi
if read -r unexpected; then echo 'build consumed protocol input' >&2; exit 9; fi
echo 'build diagnostic on stdout'
case "$*" in
  *mcp-runtime*)
    if [ "${MCP_TEST_FAIL:-}" = runtime ]; then exit 7; fi
    mkdir -p "$CARGO_TARGET_DIR/runtime/graphics" "$CARGO_TARGET_DIR/runtime/plugins"
    printf renderer > "$CARGO_TARGET_DIR/runtime/graphics/libobs-opengl.so.33"
    printf '{"graphics_module":"%s/runtime/graphics/libobs-opengl.so.33","module_bin":"%s/runtime/plugins","module_data":"data/%%module%%","data_path":"data/libobs"}' "$CARGO_TARGET_DIR" "$CARGO_TARGET_DIR" > "$CARGO_TARGET_DIR/runtime/manifest.json"
    ;;
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
    printf %s "${MCP_TEST_LIBRARY:-library}" > "$CARGO_TARGET_DIR/native-libobs.so.30"
    printf '{"reason":"build-script-executed","env":[["OBS_SYS_LIBRARY","%s/native-libobs.so.30"]]}\n' "$CARGO_TARGET_DIR"
    ;;
  *) exit 9 ;;
esac
''')
        soldr.chmod(0o755)
        self.env = {**os.environ, "PATH": str(self.bin) + os.pathsep + os.environ["PATH"],
                    "OBS_MCP_SOLDR": str(soldr), "OBS_MCP_NIX_SHELL": "1", "OBS_MCP_TARGET_DIR": str(self.root / "native-target"),
                    "MCP_TEST_TRACE": str(self.trace), "MCP_TEST_NATIVE": str(native),
                    "MCP_TEST_NATIVE_ARGS": str(self.native_args)}
        for name in ("OBS_BUILD_RUST_ARCHIVE", "CPATH", "PKG_CONFIG_PATH"):
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
        self.assertEqual((target / "debug/native-libobs.so.30").read_text(), "library")

    def test_build_includes_graphics_and_probe_runtime(self):
        result = self.invoke("--build")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("mcp-runtime", self.trace.read_text())

    def test_runtime_paths_are_set_for_the_host_and_overrides_survive(self):
        result = self.invoke("--mcp", data="protocol\n")
        self.assertEqual(result.returncode, 0, result.stderr)
        paths = Path(str(self.native_args) + ".env").read_text().splitlines()
        self.assertEqual(paths, [str(self.root / "native-target/runtime/graphics/libobs-opengl.so.33"),
                                 str(self.root / "native-target/runtime/plugins"),
                                 "data/%module%", "data/libobs"])
        self.env["OBS_MCP_GRAPHICS_MODULE"] = "explicit-renderer"
        result = self.invoke("--mcp")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(Path(str(self.native_args) + ".env").read_text().splitlines()[0], "explicit-renderer")

    def test_failed_runtime_never_launches_a_stale_host(self):
        self.env["MCP_TEST_FAIL"] = "runtime"
        result = self.invoke("--mcp")
        self.assertEqual(result.returncode, 7, result.stderr)
        self.assertFalse(self.native_args.exists())

    def test_rebuild_preserves_a_library_already_loaded_by_a_listener(self):
        first = self.invoke("--build")
        self.assertEqual(first.returncode, 0, first.stderr)
        library = self.root / "native-target/debug/native-libobs.so.30"
        loaded = self.root / "loaded-library"
        os.link(library, loaded)
        self.env["MCP_TEST_LIBRARY"] = "rebuilt library"
        second = self.invoke("--build")
        self.assertEqual(second.returncode, 0, second.stderr)
        self.assertEqual(loaded.read_text(), "library")
        self.assertEqual(library.read_text(), "rebuilt library")

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
