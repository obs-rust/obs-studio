#!/usr/bin/env -S uv run --script
"""The Tier 2 exclusion check consumes the shared source manifest."""
import importlib.util
from pathlib import Path
import tempfile
import json
import unittest

ROOT = Path(__file__).resolve().parent.parent
SPEC = importlib.util.spec_from_file_location("ported_sources", ROOT / "rust/tools/ported_sources.py")
PORTED = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PORTED)


class PortedSourceTest(unittest.TestCase):
    def test_current_linux_inventory_keeps_the_existing_exclusion_floor(self):
        objects = PORTED.ported_objects(ROOT, "linux")
        self.assertGreaterEqual(len(objects), 20)
        self.assertIn("obs-data.c.h.o", objects)
        self.assertIn("obs-hevc.c.h.o", objects)
        self.assertIn("obs-video-plan.c.h.o", objects)
        self.assertIn("obs-hotkey-press.c.h.o", objects)
        self.assertIn("obs-hotkey-press.c.h.o", PORTED.ported_objects(ROOT, "windows"))
        self.assertIn("obs-hotkey-press.c.h.o", PORTED.ported_objects(ROOT, "macos"))
        self.assertIn("audio-monitoring/pulse/pulseaudio-stream-flags.c.h.o", objects)
        self.assertIn("audio-monitoring/pulse/pulseaudio-monitor-format.c.h.o", objects)
        self.assertIn("util/platform-nix-dbus.c.h.o", objects)
        self.assertNotIn("util/platform-nix-dbus.c.h.o", PORTED.ported_objects(ROOT, "linux", gio=False))
        self.assertIn("util/platform-nix-portal.c.h.o", objects)
        self.assertIn("util/platform-nix-portal.c.h.o", PORTED.ported_objects(ROOT, "linux", wayland=False))
        self.assertNotIn("util/platform-nix-portal.c.h.o", PORTED.ported_objects(ROOT, "linux", gio=False))
        self.assertNotIn("util/platform-nix-portal.c.h.o", PORTED.ported_objects(ROOT, "macos"))
        self.assertNotIn("audio-monitoring/pulse/pulseaudio-stream-flags.c.h.o",
                         PORTED.ported_objects(ROOT, "linux", pulse=False))
        self.assertNotIn("audio-monitoring/pulse/pulseaudio-monitor-format.c.h.o",
                         PORTED.ported_objects(ROOT, "linux", pulse=False))
        self.assertIn("obs-nix-hotkey-state.c.h.o", objects)
        self.assertNotIn("obs-nix-hotkey-state.c.h.o",
                         PORTED.ported_objects(ROOT, "linux", wayland=False))
        self.assertNotIn("obs-nix-hotkey-state.c.h.o", PORTED.ported_objects(ROOT, "windows"))
        self.assertIn("obs-nix-portal-shortcut-ids.c.h.o", objects)
        self.assertNotIn("obs-nix-portal-shortcut-ids.c.h.o",
                         PORTED.ported_objects(ROOT, "linux", gio=False))
        self.assertNotIn("obs-nix-portal-shortcut-ids.c.h.o",
                         PORTED.ported_objects(ROOT, "linux", wayland=False))
        self.assertNotIn("obs-nix-portal-shortcut-ids.c.h.o", PORTED.ported_objects(ROOT, "macos"))
        self.assertEqual(len(objects), len(set(objects)))

    def test_host_and_optional_groups_select_the_actual_members(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            build = root / "libobs/build"
            build.mkdir(parents=True)
            groups = {
                "LIBOBS_PORTED_SOURCES": ["build/common+.h.c"],
                "LIBOBS_LINUX_PORTED_SOURCES": ["build/linux+.h.c"],
                "LIBOBS_WINDOWS_PORTED_SOURCES": ["build/windows+.h.c"],
                "LIBOBS_HEVC_PORTED_SOURCES": ["build/hevc+.h.c"],
                "LIBOBS_PULSE_PORTED_SOURCES": ["build/pulse+.h.c"],
                "LIBOBS_NIX_GIO_PORTED_SOURCES": ["build/nix-gio+.h.c"],
                "LIBOBS_WAYLAND_PORTED_SOURCES": ["build/wayland+.h.c"],
                "LIBOBS_WAYLAND_GIO_PORTED_SOURCES": ["build/wayland-gio+.h.c"],
            }
            (build / "source-sets.json").write_text(json.dumps(groups))
            for group, member in [("common", "common"), ("linux", "linux"),
                                  ("windows", "windows"), ("hevc", "hevc"), ("pulse", "pulse"),
                                  ("nix-gio", "nix-gio"),
                                  ("wayland", "wayland"), ("wayland-gio", "wayland-gio")]:
                (build / f"{group}+.h.c").write_text(f'#include "libobs/util/{member}.c.h"\n')
            self.assertEqual(PORTED.ported_objects(root, "linux"),
                             ["util/common.c.h.o", "util/hevc.c.h.o", "util/linux.c.h.o", "util/nix-gio.c.h.o", "util/pulse.c.h.o",
                              "util/wayland-gio.c.h.o", "util/wayland.c.h.o"])
            self.assertEqual(PORTED.ported_objects(root, "linux", pulse=False),
                             ["util/common.c.h.o", "util/hevc.c.h.o", "util/linux.c.h.o", "util/nix-gio.c.h.o",
                              "util/wayland-gio.c.h.o", "util/wayland.c.h.o"])
            self.assertEqual(PORTED.ported_objects(root, "linux", gio=False),
                             ["util/common.c.h.o", "util/hevc.c.h.o", "util/linux.c.h.o", "util/pulse.c.h.o",
                              "util/wayland.c.h.o"])
            # The session D-Bus GIO group does not need Wayland.
            self.assertEqual(PORTED.ported_objects(root, "linux", wayland=False),
                             ["util/common.c.h.o", "util/hevc.c.h.o", "util/linux.c.h.o", "util/nix-gio.c.h.o", "util/pulse.c.h.o"])
            self.assertEqual(PORTED.ported_objects(root, "windows", hevc=False),
                             ["util/common.c.h.o", "util/windows.c.h.o"])


if __name__ == "__main__":
    unittest.main()
