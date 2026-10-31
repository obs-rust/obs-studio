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


# Ported on Linux, Windows and macOS: derived from LIBOBS_PORTED_SOURCES (its
# set file plus the per-unit libobs/build/ported-source-sets/ files, #3236),
# so a port adds no row here. The floor keeps a broken loader from passing.
PORTED_ON_EVERY_HOST_FLOOR = 90
# Production sets: every libobs binary compiles these groups.
PRODUCTION_SETS = ("LIBOBS_SOURCES", "LIBOBS_RUST_SOURCES", "LIBOBS_LINUX_SOURCES",
                   "LIBOBS_WINDOWS_SOURCES", "LIBOBS_MACOS_SOURCES")


def ported_on_every_host():
    sets = PORTED.source_sets(ROOT)
    return sorted({member + ".o" for group in sets["LIBOBS_PORTED_SOURCES"]
                   for member in PORTED.group_members(ROOT, group)})


# Ported, checked on Linux.
PORTED_ON_LINUX = (
    "obs-data.c.h.o",
    "obs-hevc.c.h.o",
    "obs-nix-system-info.c.h.o",
    "obs-nix-x11-hotkeys.c.h.o",
    "obs-nix-x11-input.c.h.o",
    "obs-nix-x11-keys.c.h.o",
    "obs-video-plan.c.h.o",
)
# The unported remainder of a split file: still compiled as C.
NOT_PORTED = (
    "graphics/graphics.c.h.o",
    "obs-audio.c.h.o",
    "obs-encoder.c.h.o",
    "obs-module.c.h.o",
    "obs-scene-item-audio-actions.c.h.o",
    "obs-scene.c.h.o",
    "obs-source-deinterlace.c.h.o",
    "obs-source-transition.c.h.o",
    "obs-source.c.h.o",
)


def write_sets(build, sets):
    """One libobs/build/source-sets.d/<SET>.json list per set (#3235)."""
    (build / "source-sets.d").mkdir(parents=True, exist_ok=True)
    for name, members in sets.items():
        (build / "source-sets.d" / f"{name}.json").write_text(json.dumps(members))


class PortedSourceTest(unittest.TestCase):
    def test_per_unit_fragments_add_independent_ported_groups(self):
        with tempfile.TemporaryDirectory() as temporary:
            fixture = Path(temporary)
            build = fixture / "libobs/build"
            fragments = build / "ported-source-sets"
            fragments.mkdir(parents=True)
            write_sets(build, {"LIBOBS_PORTED_SOURCES": []})
            (fragments / "alpha.json").write_text(json.dumps({"LIBOBS_PORTED_SOURCES": ["build/alpha+.h.c"]}))
            (fragments / "beta.json").write_text(json.dumps({"LIBOBS_PORTED_SOURCES": ["build/beta+.h.c"]}))
            self.assertEqual(PORTED.source_sets(fixture)["LIBOBS_PORTED_SOURCES"], [
                "build/alpha+.h.c", "build/beta+.h.c",
            ])

    def test_current_linux_inventory_keeps_the_existing_exclusion_floor(self):
        objects = PORTED.ported_objects(ROOT, "linux")
        self.assertGreaterEqual(len(objects), 20)
        for name in PORTED_ON_LINUX:
            self.assertIn(name, objects)
        every_host = ported_on_every_host()
        self.assertGreaterEqual(len(every_host), PORTED_ON_EVERY_HOST_FLOOR)
        for name in ("media-io/audio-resampler-drift.c.h.o", "graphics/libnsgif/gif.c.h.o"):
            self.assertIn(name, every_host)
        for host in ("linux", "windows", "macos"):
            host_objects = PORTED.ported_objects(ROOT, host)
            for name in every_host:
                with self.subTest(name=name, host=host):
                    self.assertIn(name, host_objects)
        for name in NOT_PORTED:
            self.assertNotIn(name, objects)
        self.assertIn("audio-monitoring/pulse/pulseaudio-stream-flags.c.h.o", objects)
        self.assertIn("audio-monitoring/pulse/pulseaudio-monitor-format.c.h.o", objects)
        self.assertIn("audio-monitoring/pulse/pulseaudio-monitor-steps.c.h.o", objects)
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
        self.assertNotIn("audio-monitoring/pulse/pulseaudio-monitor-steps.c.h.o",
                         PORTED.ported_objects(ROOT, "linux", pulse=False))
        self.assertIn("obs-nix-hotkey-state.c.h.o", objects)
        self.assertNotIn("obs-nix-hotkey-state.c.h.o",
                         PORTED.ported_objects(ROOT, "linux", wayland=False))
        self.assertNotIn("obs-nix-hotkey-state.c.h.o", PORTED.ported_objects(ROOT, "windows"))
        self.assertIn("obs-nix-wayland-keys.c.h.o", objects)
        self.assertNotIn("obs-nix-wayland-keys.c.h.o",
                         PORTED.ported_objects(ROOT, "linux", wayland=False))
        self.assertNotIn("obs-nix-wayland-keys.c.h.o", PORTED.ported_objects(ROOT, "windows"))
        self.assertNotIn("obs-nix-wayland-keys.c.h.o", PORTED.ported_objects(ROOT, "macos"))
        self.assertIn("obs-nix-portal-shortcut-ids.c.h.o", objects)
        self.assertNotIn("obs-nix-portal-shortcut-ids.c.h.o",
                         PORTED.ported_objects(ROOT, "linux", gio=False))
        self.assertNotIn("obs-nix-portal-shortcut-ids.c.h.o",
                         PORTED.ported_objects(ROOT, "linux", wayland=False))
        self.assertNotIn("obs-nix-portal-shortcut-ids.c.h.o", PORTED.ported_objects(ROOT, "macos"))
        self.assertIn("obs-nix-portal-shortcut-sets.c.h.o", objects)
        self.assertNotIn("obs-nix-portal-shortcut-sets.c.h.o",
                         PORTED.ported_objects(ROOT, "linux", gio=False))
        self.assertNotIn("obs-nix-portal-shortcut-sets.c.h.o",
                         PORTED.ported_objects(ROOT, "linux", wayland=False))
        self.assertNotIn("obs-nix-portal-shortcut-sets.c.h.o", PORTED.ported_objects(ROOT, "windows"))
        self.assertIn("obs-nix-portal-shortcut-status.c.h.o", objects)
        self.assertNotIn("obs-nix-portal-shortcut-status.c.h.o",
                         PORTED.ported_objects(ROOT, "linux", gio=False))
        self.assertNotIn("obs-nix-portal-shortcut-status.c.h.o",
                         PORTED.ported_objects(ROOT, "linux", wayland=False))
        self.assertNotIn("obs-nix-portal-shortcut-status.c.h.o", PORTED.ported_objects(ROOT, "macos"))
        self.assertNotIn("obs-nix-x11-hotkeys.c.h.o", PORTED.ported_objects(ROOT, "windows"))
        self.assertNotIn("obs-nix-x11-hotkeys.c.h.o", PORTED.ported_objects(ROOT, "macos"))
        self.assertNotIn("obs-nix-x11-input.c.h.o", PORTED.ported_objects(ROOT, "windows"))
        self.assertNotIn("obs-nix-x11-input.c.h.o", PORTED.ported_objects(ROOT, "macos"))
        self.assertNotIn("obs-nix-x11-keys.c.h.o", PORTED.ported_objects(ROOT, "windows"))
        self.assertNotIn("obs-nix-x11-keys.c.h.o", PORTED.ported_objects(ROOT, "macos"))
        self.assertIn("obs-cocoa-keys.c.h.o", PORTED.ported_objects(ROOT, "macos"))
        self.assertNotIn("obs-cocoa-keys.c.h.o", objects)
        self.assertNotIn("obs-cocoa-keys.c.h.o", PORTED.ported_objects(ROOT, "windows"))
        self.assertIn("obs-win-crash-report.c.h.o", PORTED.ported_objects(ROOT, "windows"))
        self.assertNotIn("obs-win-crash-report.c.h.o", objects)
        self.assertNotIn("obs-win-crash-report.c.h.o", PORTED.ported_objects(ROOT, "macos"))
        self.assertIn("obs-windows-keys.c.h.o", PORTED.ported_objects(ROOT, "windows"))
        self.assertNotIn("obs-windows-keys.c.h.o", objects)
        self.assertNotIn("obs-windows-keys.c.h.o", PORTED.ported_objects(ROOT, "macos"))
        self.assertIn("audio-monitoring/monitor-delay.c.h.o", PORTED.ported_objects(ROOT, "windows"))
        self.assertNotIn("audio-monitoring/monitor-delay.c.h.o", objects)
        self.assertNotIn("audio-monitoring/monitor-delay.c.h.o", PORTED.ported_objects(ROOT, "macos"))
        self.assertIn("audio-monitoring/monitor-speaker-layout.c.h.o", PORTED.ported_objects(ROOT, "windows"))
        self.assertNotIn("audio-monitoring/monitor-speaker-layout.c.h.o", objects)
        self.assertNotIn("audio-monitoring/monitor-speaker-layout.c.h.o", PORTED.ported_objects(ROOT, "macos"))
        self.assertIn("audio-monitoring/monitor-video-sync.c.h.o", PORTED.ported_objects(ROOT, "windows"))
        self.assertNotIn("audio-monitoring/monitor-video-sync.c.h.o", objects)
        self.assertNotIn("audio-monitoring/monitor-video-sync.c.h.o", PORTED.ported_objects(ROOT, "macos"))
        self.assertIn("util/windows/window-match.c.h.o", PORTED.ported_objects(ROOT, "windows"))
        self.assertNotIn("util/windows/window-match.c.h.o", objects)
        self.assertNotIn("util/windows/window-match.c.h.o", PORTED.ported_objects(ROOT, "macos"))
        self.assertEqual(len(objects), len(set(objects)))

    def test_no_ported_c_links_into_production(self):
        """A ported implementation file is in no group every binary compiles."""
        sets = PORTED.source_sets(ROOT)
        production = {member + ".o" for name in PRODUCTION_SETS for group in sets[name]
                      if group.endswith((".c", ".m"))
                      for member in PORTED.group_members(ROOT, group)}
        self.assertTrue(production)
        for host in ("linux", "windows", "macos"):
            with self.subTest(host=host):
                self.assertEqual(sorted(production & set(PORTED.ported_objects(ROOT, host))), [])

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
            write_sets(build, groups)
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
