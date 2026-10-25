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


# One entry per line, sorted (ci/test_sorted_shared_lists.py, #1997): ports
# add rows here instead of appending assertions at the end.
# Ported on Linux, Windows and macOS.
PORTED_ON_EVERY_HOST = (
    "graphics/graphics-sprite.c.h.o",
    "obs-audio-mix.c.h.o",
    "obs-canvas-draw.c.h.o",
    "obs-context-names.c.h.o",
    "obs-encoder-audio-start.c.h.o",
    "obs-encoder-roi.c.h.o",
    "obs-encoder-size.c.h.o",
    "obs-hotkey-pair.c.h.o",
    "obs-hotkey-press.c.h.o",
    "obs-module-locale.c.h.o",
    "obs-module-names.c.h.o",
    "obs-module-paths.c.h.o",
    "obs-module-source-rules.c.h.o",
    "obs-module-type-rules.c.h.o",
    "obs-output-protocols.c.h.o",
    "obs-output-video-checks.c.h.o",
    "obs-scene-audio-actions.c.h.o",
    "obs-scene-audio-window.c.h.o",
    "obs-scene-crop.c.h.o",
    "obs-scene-extent.c.h.o",
    "obs-scene-item-names.c.h.o",
    "obs-scene-render-plan.c.h.o",
    "obs-service-getters.c.h.o",
    "obs-source-async-pacing.c.h.o",
    "obs-source-async-upload.c.h.o",
    "obs-source-audio-input.c.h.o",
    "obs-source-audio-render.c.h.o",
    "obs-source-audio-timing.c.h.o",
    "obs-source-color-space.c.h.o",
    "obs-source-conversion-plan.c.h.o",
    "obs-source-deinterlace-draw.c.h.o",
    "obs-source-deinterlace-pacing.c.h.o",
    "obs-source-deinterlace-select.c.h.o",
    "obs-source-draw-plan.c.h.o",
    "obs-source-frame-copy.c.h.o",
    "obs-source-load.c.h.o",
    "obs-source-media-admission.c.h.o",
    "obs-source-ptm-gate.c.h.o",
    "obs-source-render-route.c.h.o",
    "obs-source-size.c.h.o",
    "obs-source-transition-clock.c.h.o",
    "obs-source-transition-render.c.h.o",
    "obs-source-volume.c.h.o",
    "obs-video-copy-plan.c.h.o",
    "obs-video-output-step.c.h.o",
    "obs-view-api.c.h.o",
)
# Ported, checked on Linux.
PORTED_ON_LINUX = (
    "obs-data.c.h.o",
    "obs-hevc.c.h.o",
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


class PortedSourceTest(unittest.TestCase):
    def test_current_linux_inventory_keeps_the_existing_exclusion_floor(self):
        objects = PORTED.ported_objects(ROOT, "linux")
        self.assertGreaterEqual(len(objects), 20)
        for name in PORTED_ON_LINUX:
            self.assertIn(name, objects)
        for name in PORTED_ON_EVERY_HOST:
            for host in ("linux", "windows", "macos"):
                with self.subTest(name=name, host=host):
                    self.assertIn(name, PORTED.ported_objects(ROOT, host))
        for name in NOT_PORTED:
            self.assertNotIn(name, objects)
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
        self.assertIn("obs-nix-portal-shortcut-sets.c.h.o", objects)
        self.assertNotIn("obs-nix-portal-shortcut-sets.c.h.o",
                         PORTED.ported_objects(ROOT, "linux", gio=False))
        self.assertNotIn("obs-nix-portal-shortcut-sets.c.h.o",
                         PORTED.ported_objects(ROOT, "linux", wayland=False))
        self.assertNotIn("obs-nix-portal-shortcut-sets.c.h.o", PORTED.ported_objects(ROOT, "windows"))
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
