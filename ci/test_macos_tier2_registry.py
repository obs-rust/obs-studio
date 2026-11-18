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

    def test_plugin_registration_order_matches_native(self):
        from pathlib import Path
        source = (Path(__file__).resolve().parents[1] / 'cmake/cross/macos-tier2.cmake').read_text()
        self.assertIn('_obs_cross_tier2_tests()\nenable_testing()', source)

    def test_ctest_omits_plugin_tests_registered_before_testing_is_enabled(self):
        import json
        import subprocess
        import tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'plugin').mkdir()
            (root / 'plugin/CMakeLists.txt').write_text('add_test(NAME plugin-probe COMMAND "${CMAKE_COMMAND}" -E true)\n')
            names = []
            for before in (False, True):
                (root / 'CMakeLists.txt').write_text('cmake_minimum_required(VERSION 3.28)\nproject(order NONE)\n'
                    + ('enable_testing()\n' if before else '') + 'add_subdirectory(plugin)\nenable_testing()\n')
                build = root / str(before)
                subprocess.run(['cmake', '-S', str(root), '-B', str(build)], check=True, capture_output=True)
                result = subprocess.run(['ctest', '--test-dir', str(build), '--show-only=json-v1'], check=True, capture_output=True, text=True)
                names.append([t['name'] for t in json.loads(result.stdout)['tests']])
            self.assertEqual(names, [[], ['plugin-probe']])


if __name__ == "__main__":
    unittest.main()
