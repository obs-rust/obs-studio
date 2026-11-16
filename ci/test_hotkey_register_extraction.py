#!/usr/bin/env -S uv run --script
"""Keep single-hotkey registration extracted and bound to each fixture's core."""

import json
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
ORIGINAL = "libobs/obs-hotkey.c.h"
EXTRACTED = "libobs/obs-hotkey-register.c.h"


class HotkeyRegistrationExtractionTest(unittest.TestCase):
    def test_single_registration_definitions_are_extracted(self):
        extracted = ROOT / EXTRACTED
        self.assertTrue(extracted.is_file(), EXTRACTED)
        moved = extracted.read_text()
        original = (ROOT / ORIGINAL).read_text()
        for owner in ("frontend", "encoder", "output", "service", "source"):
            name = f"obs_hotkey_register_{owner}"
            definition = rf"^obs_hotkey_id {name}\([^;]*?\)\s*\{{"
            with self.subTest(function=name):
                self.assertEqual(len(re.findall(definition, moved, re.M | re.S)), 1)
                self.assertIsNone(re.search(definition, original, re.M | re.S))
        self.assertRegex(original, r"obs_hotkey_register_internal\([^;]*?\)\s*\{")
        group = (ROOT / "libobs/build/libobs.core+.h.c").read_text()
        self.assertEqual(group.count(f'#include "{EXTRACTED}"'), 1)

    def test_isolated_cores_compile_registration_in_the_same_translation_unit(self):
        graph = json.loads((ROOT / "test/cmocka/build/test-graph.json").read_text())
        consumers = {
            name: target
            for name, target in graph["targets"].items()
            if ORIGINAL in target.get("sources", [])
        }
        self.assertTrue(consumers)
        for name, target in consumers.items():
            with self.subTest(target=name):
                # Library fallback registers against a different `obs` global.
                self.assertIn(EXTRACTED, target["sources"])
                self.assertEqual(target.get("pch"), "libobs/build/libobs.pch.h")
                groups = list(target.get("unity_groups", {}).values())
                self.assertIn([EXTRACTED, ORIGINAL], groups)
                self.assertCountEqual(
                    [source for group in groups for source in group], target["sources"]
                )
                for group in groups:
                    if ORIGINAL not in group:
                        self.assertEqual(len(group), 1, group)

    @unittest.skipUnless(
        all(shutil.which(tool) for tool in ("cmake", "ninja", "cc")),
        "requires CMake, Ninja and a C compiler",
    )
    def test_portal_adapter_is_linked_once_with_direct_or_grouped_sources(self):
        for mode in ("missing", "direct", "grouped"):
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                fixture = root / "test/cmocka"
                fixture.mkdir(parents=True)
                (root / "pch.h").write_text("/* fixture PCH */\n")
                (fixture / "main.c").write_text(
                    "int support(void); int main(void) { return support() != 42; }\n"
                )
                adapter = "hotkey-background-error-link-support.c"
                (fixture / adapter).write_text("int support(void) { return 42; }\n")
                (root / "CMakeLists.txt").write_text(
                    "cmake_minimum_required(VERSION 3.25)\n"
                    "project(portal_link_support C)\nadd_subdirectory(test/cmocka)\n"
                )
                graph_sources = ["test/cmocka/main.c", f"test/cmocka/{adapter}"]
                graph = {"targets": {"test_portal_shortcuts": {
                    "pch": "pch.h",
                    "unity_groups": {"main": graph_sources[:1], "adapter": graph_sources[1:]},
                }}}
                cmake = f"add_executable(test_portal_shortcuts main.c {' ' if mode == 'missing' else adapter})\n"
                if mode == "grouped":
                    cmake += (
                        f'include("{ROOT.as_posix()}/test/cmocka/native-test-graph.cmake")\n'
                        f"set(graph [=[{json.dumps(graph)}]=])\n"
                        '_obs_test_graph_groups(test_portal_shortcuts "${graph}" '
                        f'"${{PROJECT_SOURCE_DIR}}" "{";".join(graph_sources)}")\n'
                    )
                cmake += f'include("{ROOT.as_posix()}/test/cmocka/tests.d/hotkey_background_error_link_support.cmake")\n'
                (fixture / "CMakeLists.txt").write_text(cmake)
                for command in (
                    ["cmake", "-S", str(root), "-B", str(root / "build"), "-G", "Ninja"],
                    ["cmake", "--build", str(root / "build"), "--parallel", "2"],
                ):
                    result = subprocess.run(command, capture_output=True, text=True, timeout=60)
                    self.assertEqual(result.returncode, 0, result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
