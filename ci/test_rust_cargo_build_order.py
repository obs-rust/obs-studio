"""Exercise parallel CMake builds against a nonblocking Cargo-style lock (#1407)."""

import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MODULE = ROOT / "libobs/cmake/rust-target.cmake"


@unittest.skipUnless(shutil.which("cmake") and shutil.which("ninja"), "CMake and Ninja required")
class RustCargoBuildOrderTest(unittest.TestCase):
    def test_parallel_cargo_targets_share_the_build_directory_safely(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            # The first Cargo process holds the lock until an independent native
            # target releases it. A second Cargo process must not start meanwhile.
            (root / "probe.py").write_text('''\
import pathlib
import sys
import threading
import time
root = pathlib.Path(sys.argv[1])
role = sys.argv[2]
def wait_for(path, timeout=10):
    deadline = time.monotonic() + timeout
    while not path.exists():
        if time.monotonic() >= deadline:
            raise RuntimeError(f"timed out waiting for {path}")
        threading.Event().wait(0.01)
if role == "native":
    wait_for(root / "first-started")
    # Native compilation runs alongside Cargo; give a wrongly unordered second
    # invocation time to reach the preflight before releasing the first one.
    threading.Event().wait(0.5)
    (root / "release-first").touch()
else:
    if role != "first":
        wait_for(root / "first-started")
    try:
        (root / "cargo-lock").mkdir()
    except FileExistsError:
        raise SystemExit("no-cache preflight: Cargo build lock is busy")
    if role == "first":
        (root / "first-started").touch()
        wait_for(root / "release-first")
    (root / "cargo-lock").rmdir()
    (root / (role + "-done")).touch()
''')
            (root / "CMakeLists.txt").write_text(
                'cmake_minimum_required(VERSION 3.20)\nproject(CargoLock NONE)\n'
                f'include("{MODULE.as_posix()}")\n'
                f'set(probe uv run --no-project "{Path(sys.executable).as_posix()}" "${{CMAKE_SOURCE_DIR}}/probe.py" "${{CMAKE_BINARY_DIR}}")\n'
                'foreach(crate IN ITEMS first second third)\n'
                '  add_custom_target(cargo-prebuild_${crate})\n'
                '  add_custom_target(cargo-build_${crate} ALL COMMAND ${probe} ${crate} VERBATIM)\n'
                '  add_dependencies(cargo-build_${crate} cargo-prebuild_${crate})\n'
                '  if(COMMAND obs_serialize_cargo_build)\n'
                '    obs_serialize_cargo_build(${crate})\n'
                '  endif()\n'
                'endforeach()\n'
                'add_custom_target(native ALL COMMAND ${probe} native VERBATIM)\n'
            )
            build = root / "build"
            configure = subprocess.run(["cmake", "-S", str(root), "-B", str(build), "-G", "Ninja"], capture_output=True, text=True, timeout=30)
            self.assertEqual(configure.returncode, 0, configure.stdout + configure.stderr)
            result = subprocess.run(["cmake", "--build", str(build), "--parallel", "8"], capture_output=True, text=True, timeout=30)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            for crate in ("first", "second", "third"):
                self.assertTrue((build / (crate + "-done")).exists())

    def launcher(self, soldr_body: str, *args: str) -> tuple[subprocess.CompletedProcess, Path, Path]:
        """Run ci/native_build.py against a stub soldr; return it, the log, the wrapper marker."""
        directory = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, directory, True)
        soldr = directory / "soldr"
        soldr.write_text('#!/bin/bash\nprintf \'%s\\n\' "$*" >> "$OBS_TEST_SOLDR_LOG"\n' + soldr_body)
        soldr.chmod(0o755)
        log, marker = directory / "soldr.log", directory / "wrapper"
        env = dict(os.environ, PATH=str(directory) + os.pathsep + os.environ["PATH"],
                   OBS_TEST_SOLDR_LOG=str(log), OBS_TEST_WRAPPER_LOG=str(marker),
                   OBS_NATIVE_TOOL_OUTPUT=str(directory / "target"))
        env.pop("SOLDR_RUSTC_WRAPPER", None)
        result = subprocess.run([sys.executable, str(ROOT / "ci/native_build.py"), *args],
                                env=env, capture_output=True, text=True, timeout=60)
        return result, log, marker

    HOST = """if [[ "$*" == *rustc* ]]; then
  echo 'host: x86_64-unknown-linux-gnu'
  exit 0
fi"""

    def test_application_launcher_uses_the_native_rust_cache_safeguard(self):
        result, _, marker = self.launcher(self.HOST + """
printf '%s' "${SOLDR_RUSTC_WRAPPER:-unset}" > "$OBS_TEST_WRAPPER_LOG"
exit 19
""", "--help")
        self.assertEqual(result.returncode, 19, result.stdout + result.stderr)
        self.assertEqual(marker.read_text(), "none")

    def test_the_tool_is_built_for_the_host_and_run_with_the_arguments(self):
        result, log, _ = self.launcher(self.HOST + """
mkdir -p "$OBS_NATIVE_TOOL_OUTPUT/x86_64-unknown-linux-gnu/debug"
printf '%s\\n' '#!/bin/sh' 'printf "%s\\n" "$*"' > "$OBS_NATIVE_TOOL_OUTPUT/x86_64-unknown-linux-gnu/debug/obs-build"
chmod +x "$OBS_NATIVE_TOOL_OUTPUT/x86_64-unknown-linux-gnu/debug/obs-build"
""", "build", "--profile", "relwithdebinfo")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(result.stdout, "build --profile relwithdebinfo\n")
        rustc, build = log.read_text().splitlines()
        self.assertIn("rustc -vV", rustc)
        for expected in ("--locked", "--target x86_64-unknown-linux-gnu", "--target-dir",
                         "-p obs-build --bin obs-build", "Cargo.toml"):
            self.assertIn(expected, build)

    def test_a_launcher_that_cannot_read_the_host_target_stops(self):
        result, log, _ = self.launcher("exit 0\n")
        self.assertEqual(result.returncode, 1)
        self.assertIn("could not determine the Rust host target", result.stderr)
        self.assertNotIn("cargo build", log.read_text())

    def test_the_root_shell_launcher_is_gone_and_the_module_is_the_entry_point(self):
        # GEN-005 (#962): the logic lives in ci/native_build.py, so no root
        # shell shim is tracked for `ci-lint precheck` to flag -- the root
        # command is `uv run --script ci/native_build.py`. This replaces the
        # two-line `./native-build` shim, which the linter reports as a
        # [GEN-005] VIOLATION `native-build` finding.
        self.assertFalse(
            (ROOT / "native-build").exists(),
            "the root `native-build` shell shim is a GEN-005 violation (#962); "
            "run `uv run --script ci/native_build.py` instead",
        )
        self.assertTrue((ROOT / "ci" / "native_build.py").exists())
        for doc in ("docs/rust-port/cc-build.md",):
            text = (ROOT / doc).read_text()
            self.assertNotIn("./native-build", text, doc)
            self.assertIn("ci/native_build.py", text, doc)

    def test_every_corrosion_import_joins_the_cargo_build_order(self):
        imports = {
            "libobs/cmake/rust.cmake": "libobs-rust",
            "frontend/cmake/frontend-rust.cmake": "obs-frontend",
            "plugins/obs-nvenc/CMakeLists.txt": "obs-nvenc-plugin",
        }
        for path, crate in imports.items():
            with self.subTest(path=path):
                source = (ROOT / path).read_text()
                self.assertIn(f"obs_serialize_cargo_build({crate})", source)
                self.assertLess(source.index("corrosion_import_crate("), source.index(f"obs_serialize_cargo_build({crate})"))
