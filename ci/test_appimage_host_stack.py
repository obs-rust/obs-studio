#!/usr/bin/env -S uv run --script
"""Exercise the AppImage dependency selector for host driver libraries (#625)."""

from pathlib import Path
import subprocess
import tempfile
import textwrap
import unittest

ROOT = Path(__file__).resolve().parent.parent


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

    def test_ffmpeg_build_dependencies_do_not_remove_obs_srt_support(self):
        workflow = (ROOT / ".github/workflows/appimage.yaml").read_text()
        bootstrap = workflow.index("build_vaapi_compat.py avutil-compat")
        configure = workflow.index("- name: Configure", bootstrap)
        self.assertIn("install", workflow[bootstrap:configure])
        self.assertIn("libsrt-openssl-dev", workflow[bootstrap:configure])
        self.assertNotIn("-DENABLE_NEW_MPEGTS_OUTPUT=OFF", workflow)

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

    def test_workflow_builds_and_passes_the_compatible_library(self):
        workflow = (ROOT / ".github/workflows/appimage.yaml").read_text()
        self.assertIn("build_vaapi_compat.py avutil-compat", workflow)
        self.assertIn(
            "assemble-appdir.sh AppDir avutil-compat/libavutil.so.60", workflow
        )
        self.assertIn("build-dep --arch-only", workflow)

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
