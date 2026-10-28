#!/usr/bin/env -S uv run --script
"""Pin the native build inputs for monitor EDID recovery (#2839)."""
from pathlib import Path
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
        self.assertIn('add_dependencies(cmocka-tests test_windows_monitor_identity)', fragment)
        self.assertIn('add_test(NAME test_windows_monitor_identity COMMAND test_windows_monitor_identity)', fragment)
        self.assertNotIn('if(OS_WINDOWS)', fragment)


if __name__ == '__main__':
    unittest.main()
