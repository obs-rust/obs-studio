#!/usr/bin/env -S uv run --script
"""check-appdir keeps AppRun's runtime files inside the AppDir (#962), in Python.

The rule lives in build-aux/appimage/check_appdir.py since the port off the
136-line shell script, and assemble_appdir.py and test-appimage.sh call it
through `uv run --no-project`. This builds a small AppDir and drives the port
over it, pinning the verdict the shell reached: a complete AppDir passes, and a
missing or empty exported path, a missing client.conf, a missing required
PipeWire module or SPA plugin, a missing Qt plugin, and a missing, duplicated,
too-old or second-referenced libsrt each fail with the shell's own message.

    uv run python -m unittest ci/test_check_appdir.py
"""

from __future__ import annotations

import importlib.util
import io
import shutil
import sys
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
APPIMAGE = ROOT / "build-aux" / "appimage"
MODULE = APPIMAGE / "check_appdir.py"
SHELL_SCRIPT = APPIMAGE / "check-appdir.sh"

# The top-level `export NAME="$HERE..."` lines the shell's grep picked up, plus a
# line without `$HERE` that it must ignore (GIO_MODULE_DIR is exported empty on
# purpose: see AppRun).
APPRUN = """#!/bin/sh
modes() { :; }
export APPDIR="$HERE"
export QT_PLUGIN_PATH="$HERE/usr/lib/qt6/plugins"
export SPA_PLUGIN_DIR="$HERE/usr/lib/spa-0.2"
export PIPEWIRE_MODULE_DIR="$HERE/usr/lib/pipewire-0.3"
export PIPEWIRE_CONFIG_DIR="$HERE/usr/share/pipewire"
export GIO_MODULE_DIR="$HERE/usr/lib/gio/modules"
export GIO_USE_VFS=local
exec "$APPDIR/usr/bin/obs" "$@"
"""

CLIENT_CONF = """context.modules = [
    { name = libpipewire-module-protocol-native }
    { name = libpipewire-module-client-node }
    { name = libpipewire-module-adapter }
    { name = libpipewire-module-rt  flags = [ ifexists ] }
]

context.spa-libs = {
    audio.convert.* = audioconvert/libspa-audioconvert
    support.*       = support/libspa-support
}
"""

# The six Qt plugins AppRun exports (X11 and Wayland platforms and helpers).
QT_FILES = (
    "platforms/libqxcb.so",
    "platforms/libqwayland.so",
    "wayland-shell-integration/libxdg-shell.so",
    "wayland-graphics-integration-client/libqt-plugin-wayland-egl.so",
)
PW_MODULES = ("protocol-native", "client-node", "adapter")


def load_module():
    """build-aux/appimage/check_appdir.py by path: `uv run python -m unittest
    ci/test_check_appdir.py` does not put build-aux/ on sys.path, and no file in
    this repo edits sys.path (ci/test_no_sys_path_edits.py)."""
    spec = importlib.util.spec_from_file_location("check_appdir", MODULE)
    assert spec is not None and spec.loader is not None, f"cannot load {MODULE}"
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class CheckAppdirTest(unittest.TestCase):
    """The verdict the shell reached, over a small AppDir."""

    def setUp(self):
        self.module = load_module()
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.appdir = self.root / "AppDir"
        (self.appdir / "usr" / "lib" / "gio" / "modules").mkdir(parents=True)
        write = self.write
        write(self.appdir / "AppRun", APPRUN)
        for plugin in QT_FILES:
            write(self.appdir / "usr" / "lib" / "qt6" / "plugins" / plugin, b"")
        for module in PW_MODULES:
            write(self.appdir / "usr" / "lib" / "pipewire-0.3" / f"libpipewire-module-{module}.so", b"")
        write(self.appdir / "usr" / "lib" / "spa-0.2" / "support" / "libspa-support.so", b"")
        write(self.appdir / "usr" / "lib" / "spa-0.2" / "audioconvert" / "libspa-audioconvert.so", b"")
        write(self.appdir / "usr" / "share" / "pipewire" / "client.conf", CLIENT_CONF)
        # SRT_VERSION lives as a bare x.y.z string between NULs in .rodata.
        self.write_srt(b"1.5.7")

    def write(self, path: Path, data: str | bytes) -> Path:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data.encode() if isinstance(data, str) else data)
        return path

    def write_srt(self, version: bytes) -> Path:
        return self.write(self.appdir / "usr" / "lib" / "libsrt.so.1.5.7", b"\x7fELF" + b"\x00" + version + b"\x00")

    def run_check(self) -> tuple[int, str, str]:
        out, err = io.StringIO(), io.StringIO()
        with redirect_stdout(out), redirect_stderr(err):
            code = self.module.check(self.appdir)
        return code, out.getvalue(), err.getvalue()

    def assert_fails(self, expected: str) -> None:
        code, out, err = self.run_check()
        self.assertEqual(code, 1)
        self.assertIn(f"::error::{expected}", out)
        self.assertIn("check-appdir: FAILED", err)

    def test_a_complete_appdir_passes(self):
        code, out, err = self.run_check()
        self.assertEqual(code, 0, out + err)
        self.assertNotIn("::error::", out)
        self.assertEqual(err, "")
        # APPDIR is exported as the AppDir itself, without a trailing path, so
        # the shell's `case "$APPDIR"/*` skipped it: five paths are counted.
        self.assertIn("check-appdir: all 5 AppDir paths that AppRun exports exist", out)
        self.assertIn("one libsrt (1.5.7)", out)

    def test_an_optional_pipewire_module_is_a_note(self):
        code, out, _ = self.run_check()
        self.assertEqual(code, 0)
        self.assertIn("optional PipeWire module libpipewire-module-rt is not bundled", out)

    def test_a_missing_exported_directory_fails(self):
        shutil.rmtree(self.appdir / "usr" / "lib" / "pipewire-0.3")
        self.assert_fails(
            f"AppRun exports PIPEWIRE_MODULE_DIR={self.appdir}/usr/lib/pipewire-0.3, which is not in the AppImage"
        )

    def test_an_empty_exported_directory_fails(self):
        self.write(self.appdir / "AppRun", APPRUN.replace("export GIO_USE_VFS=local", 'export EMPTY_DIR="$HERE/usr/empty"\nexport GIO_USE_VFS=local'))
        (self.appdir / "usr" / "empty").mkdir()
        self.assert_fails(f"AppRun exports EMPTY_DIR={self.appdir}/usr/empty, which is empty")

    def test_an_empty_gio_module_dir_is_allowed(self):
        # The dir stays empty in setUp; the complete-AppDir test already proves
        # it is exempt. Re-assert the specific absence here.
        code, out, _ = self.run_check()
        self.assertEqual(code, 0)
        self.assertNotIn("GIO_MODULE_DIR", out)

    def test_no_appdir_paths_in_the_block_fails(self):
        self.write(self.appdir / "AppRun", "#!/bin/sh\nexec obs\n")
        self.assert_fails("found no AppDir paths in AppRun's environment block")

    def test_a_missing_client_conf_fails(self):
        (self.appdir / "usr" / "share" / "pipewire" / "client.conf").unlink()
        self.assert_fails(
            f"PipeWire client.conf is not bundled (expected {self.appdir}/usr/share/pipewire/client.conf); "
            "pw_context_new() fails without it"
        )

    def test_a_missing_required_pipewire_module_fails(self):
        (self.appdir / "usr" / "lib" / "pipewire-0.3" / "libpipewire-module-protocol-native.so").unlink()
        self.assert_fails(
            "libpipewire-module-protocol-native.so is not bundled; OBS cannot connect a PipeWire stream without it"
        )

    def test_a_missing_required_spa_plugin_fails(self):
        (self.appdir / "usr" / "lib" / "spa-0.2" / "support" / "libspa-support.so").unlink()
        self.assert_fails(
            "support/libspa-support.so is not bundled; OBS cannot connect a PipeWire stream without it"
        )

    def test_a_module_client_conf_loads_but_that_is_not_bundled_fails(self):
        conf = self.appdir / "usr" / "share" / "pipewire" / "client.conf"
        self.write(conf, CLIENT_CONF.replace("{ name = libpipewire-module-adapter }", "{ name = libpipewire-module-jack }"))
        self.assert_fails(
            f"client.conf loads libpipewire-module-jack, which is not in {self.appdir}/usr/lib/pipewire-0.3"
        )

    def test_a_spa_factory_mapping_to_a_missing_plugin_fails(self):
        (self.appdir / "usr" / "lib" / "spa-0.2" / "audioconvert" / "libspa-audioconvert.so").unlink()
        self.assert_fails(
            f"client.conf maps a SPA factory to audioconvert/libspa-audioconvert, "
            f"which is not in {self.appdir}/usr/lib/spa-0.2"
        )

    def test_a_missing_qt_plugin_fails(self):
        (self.appdir / "usr" / "lib" / "qt6" / "plugins" / "platforms" / "libqwayland.so").unlink()
        self.assert_fails("the Qt plugin platforms/libqwayland.so is not bundled")

    def test_no_libsrt_fails(self):
        (self.appdir / "usr" / "lib" / "libsrt.so.1.5.7").unlink()
        self.assert_fails("expected exactly one libsrt in the AppImage, found 0: ")

    def test_two_libsrt_fails(self):
        self.write(self.appdir / "usr" / "lib" / "extra" / "libsrt.so.1.5.7", b"\x7fELF")
        self.assert_fails("expected exactly one libsrt in the AppImage, found 2: usr/lib/extra/libsrt.so.1.5.7 usr/lib/libsrt.so.1.5.7")

    def test_a_libsrt_older_than_the_floor_fails(self):
        self.write_srt(b"1.5.6")
        self.assert_fails("the bundled usr/lib/libsrt.so.1.5.7 is libsrt 1.5.6; at least 1.5.7 is required")

    def test_an_unreadable_libsrt_version_fails(self):
        self.write_srt(b"1.5.7\x00\x00\x00\x009.9.9")
        self.assert_fails("cannot read the version of usr/lib/libsrt.so.1.5.7 (version strings found: 1.5.7 9.9.9)")

    def test_a_second_libsrt_referenced_by_another_library_fails(self):
        self.write(self.appdir / "usr" / "lib" / "libavformat.so", b"\x7fELF\x00libsrt-gnutls.so.1.5\x00")
        self.assert_fails(
            "usr/lib/libavformat.so needs libsrt-gnutls.so.1.5, a second libsrt next to the bundled libsrt.so.1.5.7"
        )

    def test_no_appdir_is_a_usage_error(self):
        out, err = io.StringIO(), io.StringIO()
        with redirect_stdout(out), redirect_stderr(err):
            code = self.module.main([])
        self.assertEqual(code, 2)
        self.assertIn("usage: check_appdir.py <AppDir>", err.getvalue())


class WiringTest(unittest.TestCase):
    """The shell script is gone; its callers run the Python through uv."""

    def test_assemble_appdir_runs_the_python_module_through_uv(self):
        assemble = (APPIMAGE / "assemble_appdir.py").read_text(encoding="utf-8")
        self.assertIn(
            '["uv", "run", "--no-project", str(here / "check_appdir.py"), str(appdir)]',
            assemble,
            "assemble_appdir.py does not run check_appdir.py through uv",
        )

    def test_test_appimage_runs_the_python_module_through_uv(self):
        test_appimage = (APPIMAGE / "test-appimage.sh").read_text(encoding="utf-8")
        self.assertTrue(
            'uv run --no-project "$HERE_DIR/check_appdir.py" "$APPDIR"' in test_appimage,
            "test-appimage.sh does not run check_appdir.py through uv",
        )

    def test_the_shell_script_is_gone(self):
        self.assertFalse(SHELL_SCRIPT.exists(), "the shell script is the ported one")

    def test_no_exception_is_left_for_the_ported_script(self):
        ci_toml = (ROOT / "ci.toml").read_text(encoding="utf-8")
        self.assertFalse(
            "build-aux/appimage/check-appdir.sh" in ci_toml,
            "the GEN-005 exception for check-appdir.sh is still in ci.toml",
        )


if __name__ == "__main__":
    unittest.main()
