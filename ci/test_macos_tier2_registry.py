#!/usr/bin/env -S uv run --script
"""Registry comparison must use names and properties at one source revision."""

import unittest
import macos_tier2_registry as registry


def snapshot(revision="abc", timeout=60):
    return {
        "revision": revision,
        "source_root": "/source",
        "build_root": "/build",
        "registry": {
            "tests": [
                {
                    "name": "test_amf",
                    "command": ["/build/test_amf"],
                    "properties": [{"name": "TIMEOUT", "value": timeout}],
                }
            ]
        },
    }


class RegistryTest(unittest.TestCase):
    def test_same_revision_and_properties(self):
        self.assertEqual(registry.compare(snapshot(), snapshot()), {})
        with self.assertRaisesRegex(ValueError, "same commit"):
            registry.compare(snapshot(), snapshot("other"))
        self.assertIn("changed", registry.compare(snapshot(), snapshot(timeout=12)))

    def test_names_are_compared_not_counts(self):
        right = snapshot()
        right["registry"]["tests"][0]["name"] = "test_wrong"
        diff = registry.compare(snapshot(), right)
        self.assertEqual(diff["missing"], ["test_amf"])
        self.assertEqual(diff["extra"], ["test_wrong"])

    def test_only_paths_and_cmake_backtraces_are_normalized(self):
        right = snapshot()
        right["build_root"] = "/other"
        right["registry"]["tests"][0]["command"] = ["/other/test_amf"]
        right["registry"]["tests"][0]["backtrace"] = 123
        self.assertEqual(registry.compare(snapshot(), right), {})

    def test_real_owner_graphs_are_present(self):
        from pathlib import Path

        text = (
            Path(__file__).resolve().parents[1] / "cmake/cross/macos-tier2.cmake"
        ).read_text()
        for owner in [
            "plugins/obs-outputs",
            "shared/media-playback",
            "plugins/image-source",
            "plugins/obs-transitions",
            "plugins/obs-filters",
            "plugins/text-freetype2",
            "shared/file-updater",
        ]:
            self.assertIn(owner, text)


if __name__ == "__main__":
    unittest.main()
