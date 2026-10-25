#!/usr/bin/env -S uv run --script
"""Exercise the AppImage dependency selector for host driver libraries (#625)."""

from pathlib import Path
import importlib.util
import subprocess
import sys
import tempfile
import textwrap
import unittest

ROOT = Path(__file__).resolve().parent.parent
HOSTLIB = ROOT / "build-aux/appimage/hostlib"


def hostlib_build():
    """build-aux/appimage/hostlib/build.py as a module (it only runs from main())."""
    spec = importlib.util.spec_from_file_location("hostlib_build", HOSTLIB / "build.py")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module  # dataclasses look their module up
    spec.loader.exec_module(module)
    return module


def recorded_host_needs() -> dict[str, list[str]]:
    """ci/data/appimage-host-needed.txt as consumer -> sonames."""
    deps: dict[str, list[str]] = {}
    for line in (ROOT / "ci/data/appimage-host-needed.txt").read_text().splitlines():
        if line and not line.startswith("#"):
            soname, consumer = line.split()
            deps.setdefault(consumer, []).append(soname)
    return deps


class HostDriverStackTest(unittest.TestCase):
    def test_metadata_check_installs_every_unbundled_va_library(self):
        workflow = (ROOT / ".github/workflows/appimage.yaml").read_text()
        block = workflow.split(
            "      - name: Install host GPU runtime for metadata check", 1
        )[1].split("\n      - name:", 1)[0]
        source = textwrap.dedent(block.split("        run: |\n", 1)[1])
        # Capture the arguments of the actual install commands; do not alter
        # the host package manager or replace the workflow's dependency list.
        capture = 'sudo() { [ "$1" != -E ] || shift; "$@"; }; apt-get() { printf "%s\\n" "$@"; };\n'
        with tempfile.TemporaryDirectory() as directory:
            result = subprocess.run(
                ["bash", "-c", capture + source],
                cwd=directory,
                capture_output=True,
                text=True,
                timeout=10,
            )
        self.assertEqual(result.returncode, 0, result.stderr)
        for package in ("libva2", "libva-drm2", "libva-x11-2"):
            self.assertIn(package, result.stdout.splitlines())

    def test_assembly_requires_the_source_built_compatibility_library(self):
        with tempfile.TemporaryDirectory(prefix="obs-appdir-compat-") as scratch:
            root = Path(scratch)
            result = subprocess.run(
                [
                    "bash",
                    str(ROOT / "build-aux/appimage/assemble-appdir.sh"),
                    str(root),
                ],
                capture_output=True,
                text=True,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("validated VA-compatible libavutil", result.stderr)
            self.assertEqual(
                list(root.iterdir()), [], "invalid input must fail before assembly"
            )

    def test_workflow_passes_the_image_ffmpeg_libavutil(self):
        workflow = (ROOT / "build-aux/appimage/build.py").read_text()
        self.assertIn('assemble-appdir.sh AppDir "${AVUTIL_LIB}"', workflow)
        # The glibc 2.28 base (#926) passes its own FFmpeg's, built against
        # AlmaLinux 8's libva (the Ubuntu base was retired in #1999).
        self.assertIn('"AVUTIL_LIB": "/opt/obs-deps/ffmpeg/lib/libavutil.so.60"', workflow)

    def test_driver_libraries_are_excluded_without_excluding_app_dependencies(self):
        source = (ROOT / "build-aux/appimage/assemble-appdir.sh").read_text()
        selector = next(
            line for line in source.splitlines() if line.startswith("EXCLUDE=")
        )
        host = (
            "libEGL.so.1",
            "libGLX.so.0",
            "libGLdispatch.so.0",
            "libOpenGL.so.0",
            "libgbm.so.1",
            "libdrm.so.2",
            "libdrm_amdgpu.so.1",
            "libvulkan.so.1",
            "libwayland-client.so.0",
            "libwayland-egl.so.1",
            "libwayland-server.so.0",
            "libva.so.2",
            "libva-drm.so.2",
            "libva-x11.so.2",
            "libnvidia-egl-gbm.so.1",
            "libcuda.so.1",
            "libnvcuvid.so.1",
        )
        bundled = (
            "libavutil.so.60",
            "libavcodec.so.62",
            "libobs.so.0",
            "libQt6Core.so.6",
            "libpipewire-0.3.so.0",
            "libvamp-hostsdk.so.3",
        )
        # Run the actual Bash selector used by the dependency-copy loop.
        script = (
            selector
            + '\nfor name in "$@"; do if [[ "$name" =~ $EXCLUDE ]]; then echo host; else echo bundled; fi; done'
        )
        selected = subprocess.check_output(
            ["bash", "-c", script, "selector", *host, *bundled], text=True
        ).splitlines()
        for library, actual in zip(host + bundled, selected, strict=True):
            with self.subTest(library=library):
                self.assertEqual(actual, "host" if library in host else "bundled")

    def test_preflight_knows_every_library_the_release_takes_from_the_host(self):
        # A host library that host-libs.json does not list is never checked by
        # obs-hostlib-check: a host without libwayland-egl.so.1 got "Failed to
        # initialize video: Graphics module not found" instead of the
        # preflight's message naming the package (obs-rust/obs-studio#2298);
        # a host without libGLX.so.0 got the dynamic loader's error (#2337).
        build = hostlib_build()
        deps = recorded_host_needs()
        self.assertIn("libwayland-egl.so.1", deps["usr/lib/obs-studio-rust/libobs-opengl.so.30"])
        self.assertIn("libGLX.so.0", deps["usr/bin/obs-studio-rust-gui"])
        unchecked = build.unchecked_host_needs(
            deps, set(), build.parse_host_libs(HOSTLIB / "host-libs.json")
        )
        self.assertEqual(
            unchecked,
            {},
            "the AppImage takes these from the host, and host-libs.json does not list them",
        )

    def test_recorded_host_libraries_are_the_ones_assembly_excludes(self):
        # The recorded list is what assemble-appdir.sh's EXCLUDE keeps out of
        # the bundle (built at the glibc floor: the C runtime too).
        source = (ROOT / "build-aux/appimage/assemble-appdir.sh").read_text()
        selector = [
            line.strip()
            for line in source.splitlines()
            if line.strip().startswith("EXCLUDE=")
        ]
        self.assertEqual(len(selector), 2, selector)
        sonames = sorted({s for names in recorded_host_needs().values() for s in names})
        script = (
            "\n".join(selector)
            + '\nfor name in "$@"; do [[ "$name" =~ $EXCLUDE ]] || echo "$name"; done'
        )
        bundled = subprocess.check_output(
            ["bash", "-c", script, "selector", *sonames], text=True
        ).split()
        self.assertEqual(bundled, [], "recorded as host libraries, but assembly bundles them")

    def test_unchecked_host_needs_names_only_unknown_host_libraries(self):
        build = hostlib_build()
        libs = build.parse_host_libs(HOSTLIB / "host-libs.json")
        deps = {
            "usr/bin/a": ["libc.so.6", "libstdc++.so.6", "libEGL.so.1", "libGLX.so.0", "libfoo.so.1"],
            "usr/lib/b.so": ["libbundled.so.2", "libfoo.so.1", "libva.so.2"],
        }
        self.assertEqual(
            build.unchecked_host_needs(deps, {"libbundled.so.2"}, libs),
            {"libfoo.so.1": ["usr/bin/a", "usr/lib/b.so"]},
        )
