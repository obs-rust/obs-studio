#!/usr/bin/env -S uv run --script
"""Exercise the AppImage dependency selector for host driver libraries (#625)."""

from pathlib import Path
import subprocess
import unittest

ROOT = Path(__file__).resolve().parent.parent


class HostDriverStackTest(unittest.TestCase):
    def test_driver_libraries_are_excluded_without_excluding_app_dependencies(self):
        source = (ROOT / "build-aux/appimage/assemble-appdir.sh").read_text()
        selector = next(line for line in source.splitlines() if line.startswith("EXCLUDE="))
        host = (
            "libEGL.so.1", "libGLX.so.0", "libGLdispatch.so.0", "libOpenGL.so.0",
            "libgbm.so.1", "libdrm.so.2", "libdrm_amdgpu.so.1", "libvulkan.so.1",
            "libwayland-client.so.0", "libwayland-egl.so.1", "libwayland-server.so.0",
            "libva.so.2", "libva-drm.so.2", "libva-x11.so.2",
            "libnvidia-egl-gbm.so.1", "libcuda.so.1", "libnvcuvid.so.1",
        )
        bundled = ("libavutil.so.60", "libavcodec.so.62", "libobs.so.0",
                   "libQt6Core.so.6", "libpipewire-0.3.so.0", "libvamp-hostsdk.so.3")
        # Run the actual Bash selector used by the dependency-copy loop.
        script = selector + '\nfor name in "$@"; do if [[ "$name" =~ $EXCLUDE ]]; then echo host; else echo bundled; fi; done'
        selected = subprocess.check_output(["bash", "-c", script, "selector", *host, *bundled], text=True).splitlines()
        for library, actual in zip(host + bundled, selected, strict=True):
            with self.subTest(library=library):
                self.assertEqual(actual, "host" if library in host else "bundled")
