#!/usr/bin/env -S uv run --script
"""assemble-appdir turns a `cmake --install` tree into an AppDir (#962), in Python.

    uv run python -m unittest ci/test_assemble_appdir.py

The rule lives in build-aux/appimage/assemble_appdir.py since the port off the
306-line shell script. This drives its decisions over a fake AppDir with stub
executables first on PATH:

- the compatibility-library audit (a missing input, the wrong libavutil SONAME
  and a leftover vaMapBuffer2 all stop the build with status 2);
- the glibc-floor decision (`getconf` at or below 2.28 leaves glibc to the host,
  above it bundles it);
- the host-driver EXCLUDE set (the AppImage excludelist, plus the C runtime when
  glibc is the host's);
- the Qt plugin requirement;
- a `not found` dependency (fail closed);
- a whole successful assembly.
"""

from __future__ import annotations

import importlib.util
import io
import os
import re
import stat
import sys
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parent.parent
APPIMAGE = ROOT / "build-aux" / "appimage"
MODULE = APPIMAGE / "assemble_appdir.py"
SHELL_SCRIPT = APPIMAGE / "assemble-appdir.sh"

# Enough of an ELF header for the `head -c4` magic test the shell used.
ELF = b"\x7fELF\x02\x01\x01\x00" + b"\x00" * 8

# The stubs are shell scripts compiled from the tools the port shells out to.
# Each one answers only the query its tool is asked, from an environment
# variable, so a case is one entry in `run_main`'s environment.
READELF = """#!/bin/sh
case "$1" in
  -d) printf '  0x000000000000000e (SONAME)  Library soname: [%s]\\n' "${READELF_SONAME:-libavutil.so.60}" ;;
  *) printf '  3: 0000000000000000     0 FUNC    GLOBAL DEFAULT  UND %s\\n' "${READELF_UNDEF:-xNotVaMap}"
     if [ -n "${READELF_VAMAP:-}" ]; then
       printf '  4: 0000000000000000     0 FUNC    GLOBAL DEFAULT  UND vaMapBuffer2\\n'
     fi ;;
esac
"""
GCC = """#!/bin/sh
printf '%s' "${GCC_MULTIARCH:-}"
"""
GETCONF = """#!/bin/sh
printf '%s\\n' "${GETCONF_GLIBC:-glibc 2.28}"
"""
QTPATHS6 = """#!/bin/sh
printf '%s\\n' "${QT_PLUGINS_DIR:-}"
"""
PKG_CONFIG = """#!/bin/sh
case "$1" in
  --variable=plugindir) printf '%s\\n' "${SPA_DIR:-}" ;;
  --variable=moduledir) printf '%s\\n' "${PW_MODULES:-}" ;;
  --variable=prefix) printf '%s\\n' "${PW_PREFIX:-}" ;;
esac
"""
LDD = """#!/bin/sh
if [ -n "${LDD_NOT_FOUND:-}" ]; then
  printf '\\tlibmissing.so.1 => not found\\n'
fi
"""
UV = """#!/bin/sh
printf '%s\\n' "$*" >> "${UV_LOG:-/dev/null}"
exit 0
"""
INSTALL_LOG = """#!/bin/sh
printf '%s\\n' "$*" >> "${INSTALL_LOG_FILE:-/dev/null}"
exit 0
"""
NO_OUTPUT = """#!/bin/sh
exit 0
"""

STUBS = {
    "readelf": READELF,
    "gcc": GCC,
    "getconf": GETCONF,
    "qtpaths6": QTPATHS6,
    "pkg-config": PKG_CONFIG,
    "ldd": LDD,
    "uv": UV,
    "install": INSTALL_LOG,
    "patchelf": NO_OUTPUT,
    "dpkg": NO_OUTPUT,
}

APPRUN = """#!/bin/sh
LOADER="@LOADER@"
BUNDLE="@BUNDLE_GLIBC@"
"@OBS_GUI_NAME@" "$@"
"""


def load_module():
    """build-aux/appimage/assemble_appdir.py by path: `uv run python -m unittest
    ci/test_assemble_appdir.py` does not put build-aux/ on sys.path, and no file
    in this repo edits sys.path (ci/test_no_sys_path_edits.py)."""
    spec = importlib.util.spec_from_file_location("assemble_appdir", MODULE)
    assert spec is not None and spec.loader is not None, f"cannot load {MODULE}"
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class AssembleAppdirTest(unittest.TestCase):
    """The decisions the shell made, over a fake AppDir and stub tools."""

    def setUp(self):
        self.module = load_module()
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)

        self.bin = self.root / "bin"
        self.bin.mkdir()
        for name, source in STUBS.items():
            self.stub(name, source)

        # The scripts assemble_appdir.py calls by absolute path (AppRun and
        # check_appdir.py live next to it in the real tree).
        self.here = self.root / "appimage"
        self.here.mkdir()
        self.script("check_appdir.py", "#!/bin/sh\nexit 0\n")
        (self.here / "AppRun").write_text(APPRUN, encoding="utf-8")
        setattr(self.module, "HERE", self.here)  # the port's `$HERE`, the script's directory

        # The AppDir a `cmake --install` left behind.
        self.appdir = self.root / "AppDir"
        (self.appdir / "usr" / "bin").mkdir(parents=True)
        self.elf(self.appdir / "usr" / "bin" / "obs", executable=True)
        self.elf(self.appdir / "usr" / "lib" / "obs-modules" / "libtest.so")
        for relative in (
            "usr/share/applications/com.obsproject.Studio.desktop",
            "usr/share/icons/hicolor/256x256/apps/com.obsproject.Studio.png",
        ):
            path = self.appdir / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("x", encoding="utf-8")

        # What qtpaths6 and pkg-config point the port at.
        self.qtplugins = self.root / "qtplugins"
        for relative in (
            "platforms/libqxcb.so",
            "imageformats/libqsvg.so",
            "iconengines/libqsvgicon.so",
            "platformthemes/libqgtk3.so",
        ):
            self.touch(self.qtplugins / relative)
        self.spa = self.root / "spa"
        self.touch(self.spa / "support" / "libspa-support.so")
        self.pw_modules = self.root / "pipewire-modules"
        self.touch(self.pw_modules / "libpipewire-module-protocol-native.so")
        self.pw_prefix = self.root / "pipewire-prefix"
        self.touch(self.pw_prefix / "share" / "pipewire" / "client.conf")
        self.noqt = self.root / "noqt"
        self.noqt.mkdir()

        self.compat = self.root / "libavutil.so.60"
        self.compat.write_bytes(b"libavutil")
        self.uv_log = self.root / "uv.log"
        self.install_log = self.root / "install.log"
        self.empty = self.root / "Empty"
        self.empty.mkdir()

    def stub(self, name: str, source: str) -> None:
        path = self.bin / name
        path.write_text(source, encoding="utf-8")
        path.chmod(0o755)

    def script(self, name: str, source: str) -> None:
        path = self.here / name
        path.write_text(source, encoding="utf-8")
        path.chmod(0o755)

    def elf(self, path: Path, executable: bool = False) -> Path:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(ELF)
        if executable:
            path.chmod(path.stat().st_mode | stat.S_IEXEC)
        return path

    def touch(self, path: Path) -> Path:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text("x", encoding="utf-8")
        return path

    def environment(self, **overrides: str) -> dict[str, str]:
        """The stub environment for a passing assembly; one entry per case."""
        values = {
            "PATH": f"{self.bin}{os.pathsep}{os.environ.get('PATH', os.defpath)}",
            "READELF_SONAME": "libavutil.so.60",
            "GCC_MULTIARCH": "",
            "GETCONF_GLIBC": "glibc 2.28",
            "QT_PLUGINS_DIR": str(self.qtplugins),
            "SPA_DIR": str(self.spa),
            "PW_MODULES": str(self.pw_modules),
            "PW_PREFIX": str(self.pw_prefix),
            "UV_LOG": str(self.uv_log),
            "INSTALL_LOG_FILE": str(self.install_log),
        }
        values.update(overrides)
        return values

    def run_main(self, *args: str, **overrides: str) -> tuple[int, str, str]:
        out, err = io.StringIO(), io.StringIO()
        with patch.dict(os.environ, self.environment(**overrides)):
            with redirect_stdout(out), redirect_stderr(err):
                code = self.module.main(list(args))
        return code, out.getvalue(), err.getvalue()

    def uv_calls(self) -> list[str]:
        if not self.uv_log.exists():
            return []
        return self.uv_log.read_text(encoding="utf-8").splitlines()

    # --- the compatibility library ------------------------------------------

    def test_a_missing_compatibility_library_is_a_usage_error(self):
        for args in ((), (str(self.empty),), (str(self.empty), str(self.root / "absent.so"))):
            with self.subTest(args=args):
                code, out, err = self.run_main(*args)
                self.assertEqual(code, 2)
                self.assertEqual(out, "")
                self.assertIn("provide a validated VA-compatible libavutil", err)
                self.assertIn("build.py's AVUTIL_LIB", err)
        # The audit runs before anything is written.
        self.assertEqual(list(self.empty.iterdir()), [])

    def test_the_wrong_libavutil_soname_stops_the_build(self):
        code, out, err = self.run_main(
            str(self.empty), str(self.compat), READELF_SONAME="libavutil.so.59"
        )
        self.assertEqual(code, 2)
        self.assertEqual(out, "")
        self.assertIn("::error::compatibility library has the wrong libavutil SONAME", err)

    def test_a_leftover_vamapbuffer2_stops_the_build(self):
        code, out, err = self.run_main(str(self.empty), str(self.compat), READELF_VAMAP="1")
        self.assertEqual(code, 2)
        self.assertEqual(out, "")
        self.assertIn("::error::compatibility library still requires vaMapBuffer2", err)

    # --- the glibc floor ----------------------------------------------------

    def test_the_glibc_floor_decides_whether_glibc_is_bundled(self):
        # (the empty plugin directory stops the run right after the decision,
        # before either branch touches the file system)
        for version, bundle in (("glibc 2.17", 0), ("glibc 2.28", 0), ("glibc 2.29", 1), ("glibc 2.43", 1)):
            with self.subTest(version=version):
                code, out, err = self.run_main(
                    str(self.appdir), str(self.compat),
                    QT_PLUGINS_DIR=str(self.noqt), GETCONF_GLIBC=version,
                )
                self.assertEqual(code, 1, err)
                self.assertIn(
                    f"build glibc {version.split()[1]}, floor 2.28: bundle glibc={bundle}", out
                )
                self.assertIn("::error::no Qt 6 plugins", err)

    def test_the_exclude_set_is_the_host_driver_stack(self):
        base = self.module.HOST_EXCLUDE
        bundled_glibc = self.module.GLIBC_EXCLUDE
        host = (
            "libEGL.so.1", "libGLX.so.0", "libGLdispatch.so.0", "libOpenGL.so.0",
            "libgbm.so.1", "libdrm.so.2", "libdrm_amdgpu.so.1", "libvulkan.so.1",
            "libwayland-client.so.0", "libwayland-egl.so.1", "libwayland-server.so.0",
            "libva.so.2", "libva-drm.so.2", "libva-x11.so.2",
            "libnvidia-egl-gbm.so.1", "libcuda.so.1", "libnvcuvid.so.1",
        )
        bundled = (
            "libavutil.so.60", "libavcodec.so.62", "libobs.so.0", "libQt6Core.so.6",
            "libpipewire-0.3.so.0", "libvamp-hostsdk.so.3",
        )
        for name in host:
            with self.subTest(library=name):
                self.assertIsNotNone(re.search(base, name))
        for name in bundled:
            with self.subTest(library=name):
                self.assertIsNone(re.search(base, name))
        # Bundling no glibc also excludes glibc and the C++ runtime.
        for name in ("ld-linux-x86-64.so.2", "libc.so.6", "libm.so.6", "libpthread.so.0",
                     "libdl.so.2", "librt.so.1", "libresolv.so.2", "libutil.so.1",
                     "libstdc++.so.6", "libgcc_s.so.1"):
            with self.subTest(library=name):
                self.assertIsNotNone(re.search(bundled_glibc, name))
                self.assertIsNone(re.search(base, name))
        for name in bundled:
            with self.subTest(library=name):
                self.assertIsNone(re.search(bundled_glibc, name))

    # --- Qt plugins ---------------------------------------------------------

    def test_no_qt_plugins_fails_the_build(self):
        code, out, err = self.run_main(
            str(self.appdir), str(self.compat), QT_PLUGINS_DIR=str(self.noqt)
        )
        self.assertEqual(code, 1)
        self.assertIn(f"::error::no Qt 6 plugins (looked at {self.noqt})", err)
        self.assertFalse((self.appdir / "usr" / "lib" / "qt6").exists())

    def test_a_required_qt_plugin_is_searched_for_and_fails_the_build(self):
        if list(Path("/usr/lib").glob("*/qt6")) or Path("/usr/lib64/qt6").is_dir():
            self.skipTest("a system Qt 6 plugin directory would satisfy the search")
        # Only the X11 platform plugin: the SVG ones OBS needs are missing, and
        # neither the plugin's own tree nor the system has them. The tree sits in
        # a directory of its own: the search also looks beside it.
        incomplete = self.root / "incomplete" / "plugins"
        self.touch(incomplete / "platforms" / "libqxcb.so")
        code, out, err = self.run_main(
            str(self.appdir), str(self.compat), QT_PLUGINS_DIR=str(incomplete)
        )
        self.assertEqual(code, 1)
        self.assertIn(
            "::error::Qt plugin imageformats/libqsvg.so not found; without it OBS "
            "has no SVG icons or loads the host's copy",
            err,
        )

    # --- the library closure ------------------------------------------------

    def test_an_unresolved_dependency_fails_closed(self):
        code, out, err = self.run_main(str(self.appdir), str(self.compat), LDD_NOT_FOUND="1")
        self.assertEqual(code, 1)
        self.assertIn("::error::unresolved dependency for", err)
        self.assertIn("libmissing.so.1 => not found", err)

    # --- the whole thing ----------------------------------------------------

    def test_a_successful_assembly(self):
        code, out, err = self.run_main(str(self.appdir), str(self.compat))
        self.assertEqual(code, 0, err)
        self.assertEqual(err, "")
        self.assertIn("build glibc 2.28, floor 2.28: bundle glibc=0", out)
        self.assertIn(f"Qt plugins from: {self.qtplugins}", out)
        self.assertIn("qt6/plugins/platforms/libqxcb.so", out)
        self.assertIn("::group::Libraries still resolved from the host", out)
        self.assertIn("::endgroup::", out)
        # At the floor nothing of glibc is bundled and no interpreter is patched.
        self.assertFalse((self.appdir / "glibc").exists())
        self.assertFalse((self.appdir / "lib64").exists())
        self.assertNotIn("interpreter patched", out)

        # Qt plugins, minus the GTK 3 platform theme.
        plugins = self.appdir / "usr" / "lib" / "qt6" / "plugins"
        self.assertTrue((plugins / "platforms" / "libqxcb.so").is_file())
        self.assertTrue((plugins / "imageformats" / "libqsvg.so").is_file())
        self.assertFalse((plugins / "platformthemes" / "libqgtk3.so").exists())
        self.assertEqual(
            (self.appdir / "usr" / "bin" / "qt.conf").read_text(encoding="utf-8"),
            "[Paths]\nPrefix = ..\nPlugins = lib/qt6/plugins\n",
        )
        self.assertTrue((self.appdir / "usr" / "lib" / "gio" / "modules").is_dir())

        # The dlopen()ed pieces.
        self.assertTrue(
            (self.appdir / "usr" / "lib" / "spa-0.2" / "support" / "libspa-support.so").is_file()
        )
        self.assertTrue(
            (self.appdir / "usr" / "lib" / "pipewire-0.3"
             / "libpipewire-module-protocol-native.so").is_file()
        )
        self.assertTrue((self.appdir / "usr" / "share" / "pipewire" / "client.conf").is_file())

        # The modules the build installed, for test-appimage.sh.
        self.assertEqual(
            (self.appdir / "usr" / "share" / "obs" / "appimage-modules.txt").read_text(
                encoding="utf-8"
            ),
            "libtest.so\n",
        )
        self.assertEqual(self.install_log.read_text(encoding="utf-8").strip(),
                         f"-m 644 {self.compat} {self.appdir / 'usr' / 'lib' / 'libavutil.so.60'}")

        # AppRun, the .desktop/.png pair and the .DirIcon symlink.
        app_run = (self.appdir / "AppRun").read_text(encoding="utf-8")
        for marker in ("@LOADER@", "@BUNDLE_GLIBC@", "@OBS_GUI_NAME@"):
            self.assertNotIn(marker, app_run)
        self.assertIn('LOADER=""', app_run)
        self.assertIn('BUNDLE="0"', app_run)
        self.assertIn('"obs" "$@"', app_run)
        self.assertEqual(os.access(self.appdir / "AppRun", os.X_OK), True)
        self.assertTrue((self.appdir / "com.obsproject.Studio.desktop").is_file())
        self.assertTrue((self.appdir / "com.obsproject.Studio.png").is_file())
        diricon = self.appdir / ".DirIcon"
        self.assertTrue(diricon.is_symlink())
        self.assertEqual(os.readlink(diricon), "com.obsproject.Studio.png")

        # The four helper steps went through uv, in order.
        calls = self.uv_calls()
        self.assertEqual(
            calls,
            [
                f"run --no-project {self.here / 'hostlib' / 'build.py'} {self.appdir}",
                f"run --no-project {self.here / 'set_runpaths.py'} {self.appdir}",
                f"run --no-project {self.here / 'check_appdir.py'} {self.appdir}",
                f"run --script {self.here / 'check-glibc-floor.py'} {self.appdir} 2.28",
            ],
        )


class WiringTest(unittest.TestCase):
    """The shell script is gone; build.py and ci.toml name the Python file."""

    def test_the_shell_script_is_replaced(self):
        self.assertFalse(SHELL_SCRIPT.exists(), "the shell script is the ported one")
        # assertTrue with a short message: assertIn would echo the whole module.
        self.assertTrue(
            '"uv", "run", "--no-project"' in MODULE.read_text(encoding="utf-8"),
            "assemble_appdir.py does not run its helper scripts through uv",
        )

    def test_build_py_runs_the_python_module(self):
        build = (APPIMAGE / "build.py").read_text(encoding="utf-8")
        self.assertTrue(
            'uv run --no-project build-aux/appimage/assemble_appdir.py AppDir "${AVUTIL_LIB}"' in build,
            "build.py's appdir step does not run assemble_appdir.py through uv",
        )

    def test_no_exception_is_left_for_the_ported_script(self):
        ci_toml = (ROOT / "ci.toml").read_text(encoding="utf-8")
        self.assertTrue(
            "build-aux/appimage/assemble-appdir.sh" not in ci_toml,
            "the GEN-005 exception for assemble-appdir.sh is still in ci.toml",
        )


if __name__ == "__main__":
    unittest.main()
