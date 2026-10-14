#!/usr/bin/env -S uv run --script
"""Run AppRun's actual host-driver setup with isolated directory fixtures (#625)."""

import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent
VARIABLES = (
    "GBM_BACKENDS_PATH",
    "LIBVA_DRIVERS_PATH",
    "__EGL_VENDOR_LIBRARY_DIRS",
    "__EGL_VENDOR_LIBRARY_FILENAMES",
    "VK_DRIVER_FILES",
    "VK_ICD_FILENAMES",
    "__EGL_EXTERNAL_PLATFORM_CONFIG_DIRS",
    "__EGL_EXTERNAL_PLATFORM_CONFIG_FILENAMES",
)


class DriverEnvironmentTest(unittest.TestCase):
    def setup_environment(self, directories, overrides=None):
        source = (ROOT / "build-aux/appimage/AppRun").read_text()
        # This whole production block runs in both the launcher and GPU tests.
        block = source.split(
            'export PIPEWIRE_CONFIG_DIR="$HERE/usr/share/pipewire"\n', 1
        )[1]
        block = block.split("# gconv modules", 1)[0]
        with tempfile.TemporaryDirectory(prefix="obs-driver-paths-") as scratch:
            base = Path(scratch)
            for directory in directories:
                (base / directory.lstrip("/")).mkdir(parents=True)
            for prefix in ("/usr/", "/etc/", "/run/"):
                block = block.replace(prefix, str(base) + prefix)
            script = block + "\nenv\n"
            env = {k: v for k, v in os.environ.items() if k not in VARIABLES}
            env.update(overrides or {})
            output = subprocess.check_output(["sh", "-c", script], env=env, text=True)
            actual = dict(
                line.split("=", 1) for line in output.splitlines() if "=" in line
            )
            return {
                key: actual.get(key, "").replace(str(base), "") for key in VARIABLES
            }

    def test_arch_and_fedora_host_driver_paths(self):
        cases = {
            "GBM_BACKENDS_PATH": "/usr/lib/gbm:/usr/lib64/gbm",
            "LIBVA_DRIVERS_PATH": "/usr/lib/dri:/usr/lib64/dri",
            "__EGL_VENDOR_LIBRARY_DIRS": "/etc/glvnd/egl_vendor.d:/usr/share/glvnd/egl_vendor.d",
            "VK_DRIVER_FILES": "/etc/vulkan/icd.d:/usr/share/vulkan/icd.d",
            "__EGL_EXTERNAL_PLATFORM_CONFIG_DIRS": "/etc/egl/egl_external_platform.d:/usr/share/egl/egl_external_platform.d",
        }
        actual = self.setup_environment(
            [p for value in cases.values() for p in value.split(":")]
        )
        for key, value in cases.items():
            with self.subTest(variable=key):
                self.assertEqual(set(actual[key].split(":")), set(value.split(":")))

    def test_nix_host_driver_paths(self):
        cases = {
            "GBM_BACKENDS_PATH": "/run/opengl-driver/lib/gbm",
            "LIBVA_DRIVERS_PATH": "/run/opengl-driver/lib/dri",
            "__EGL_VENDOR_LIBRARY_DIRS": "/run/opengl-driver/share/glvnd/egl_vendor.d",
            "VK_DRIVER_FILES": "/run/opengl-driver/share/vulkan/icd.d",
            "__EGL_EXTERNAL_PLATFORM_CONFIG_DIRS": "/run/opengl-driver/share/egl/egl_external_platform.d",
        }
        actual = self.setup_environment(cases.values())
        for key, value in cases.items():
            with self.subTest(variable=key):
                self.assertEqual(actual[key], value)

    def test_user_overrides_are_preserved(self):
        overrides = {key: f"/chosen/{key}" for key in VARIABLES}
        self.assertEqual(self.setup_environment(["/usr/lib/dri"], overrides), overrides)

    def test_old_vulkan_override_does_not_get_a_new_override(self):
        actual = self.setup_environment(
            ["/usr/share/vulkan/icd.d"], {"VK_ICD_FILENAMES": "/chosen/old.json"}
        )
        self.assertEqual(actual["VK_ICD_FILENAMES"], "/chosen/old.json")
        self.assertEqual(actual["VK_DRIVER_FILES"], "")

    def test_absent_host_directories_do_not_create_empty_overrides(self):
        self.assertTrue(all(not value for value in self.setup_environment([]).values()))
