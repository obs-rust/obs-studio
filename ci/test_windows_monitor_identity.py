#!/usr/bin/env -S uv run --script
"""Pin the native build inputs for monitor EDID recovery (#2839)."""
import json
from pathlib import Path

try:
    from ci.native_test_graph import load_graph
except ModuleNotFoundError:
    from native_test_graph import load_graph

import unittest

ROOT = Path(__file__).resolve().parents[1]


class MonitorIdentityBuildTest(unittest.TestCase):
    def test_setupapi_and_registry_import_libraries_are_linked(self):
        cmake = (ROOT / 'plugins/win-capture/CMakeLists.txt').read_text()
        libraries = cmake.split('target_link_libraries(', 1)[1].split(')', 1)[0]
        for library in ('setupapi', 'advapi32'):
            self.assertIn(library, libraries.split())

    def test_portable_regression_is_registered_without_windows_guard(self):
        fragment = (ROOT / 'test/cmocka/tests.d/test_windows_monitor_identity.cmake').read_text()
        self.assertIn('test-graph.d', fragment)
        graph = load_graph(ROOT / 'test/cmocka/build/test-graph.d')
        name = 'test_windows_monitor_identity'
        self.assertEqual(graph['targets'][name]['sources'], ['test/cmocka/test_windows_monitor_identity.c'])
        self.assertEqual(graph['tests'][name], {'target': name, 'command': ['${target}']})
        self.assertNotIn('requires', graph['targets'][name])
        self.assertNotIn('add_executable(', fragment)
        self.assertNotIn('add_test(', fragment)


if __name__ == '__main__':
    unittest.main()
