#!/usr/bin/env -S uv run --script
"""Exercise partial CMake transport without declaring the full suite runnable."""

import importlib
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
from unittest.mock import patch

from test_windows_zip_check import pe

ROOT = Path(__file__).resolve().parents[1]


class CMakePackTests(unittest.TestCase):
    def helper(self):
        return importlib.import_module("windows_tier2_pack")

    def fixture(self, root):
        source, build = root / "source", root / "producer"
        for base, name, data in [
            (source, "test/cmocka/data/input.dat", b"fixture"),
            (source, "libobs/data/default.effect", b"shader"),
            (source, "plugins/example/data/locale/en-US.ini", b"Name=Example"),
            (build, "test/cmocka/test.exe", pe(["dependency.dll"])),
            (build, "plugins/example/example.dll", pe([])),
            (build, "cores/shared/obs.dll", pe([])),
            (build, "cores/reference/obs.dll", pe([])),
            (build, "test/cmocka/corrupt.png", b"deliberately invalid image"),
        ]:
            path = base / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
        deps = root / "dependencies"
        deps.mkdir()
        (deps / "dependency.dll").write_bytes(pe(["kernel32.dll"]))
        (build / "test/cmocka/empty space").mkdir()
        products = sorted(build.rglob("*.exe")) + sorted(build.rglob("*.dll"))
        (build / "tier2-runtime.txt").write_text("\n".join(map(str, products)) + "\n")
        tests = [
            {
                "name": "suite: empty dir",
                "command": [
                    str(build / "test/cmocka/test.exe"),
                    str(source / "test/cmocka/data/input.dat"),
                ],
                "properties": [
                    {
                        "name": "WORKING_DIRECTORY",
                        "value": str(build / "test/cmocka/empty space"),
                    },
                    {"name": "RUN_SERIAL", "value": True},
                ],
            }
        ]
        return source, build, deps, tests

    def stage(self, root):
        h = self.helper()
        source, build, deps, tests = self.fixture(root)
        with (
            patch.object(
                h.registry,
                "inventory",
                return_value={
                    "tests": tests,
                    "unbuilt_tests": [],
                    "missing_native_tests": [],
                },
            ),
            patch.object(h.consumer, "coff_exports", return_value={"api"}),
        ):
            manifest = h.stage(build, source, root / "bundle", [deps], "a" * 40)
        return manifest

    def test_partial_closure_data_empty_directories_and_archive(self):
        h = self.helper()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = self.stage(root)
            self.assertEqual(2, manifest["schema"])
            self.assertFalse(manifest["complete"])
            self.assertEqual("cargo parts pending", manifest["pending"][0])
            self.assertEqual("suite: empty dir", manifest["tests"][0]["name"])
            self.assertIn("build/test/cmocka/empty space", manifest["directories"])
            for path in [
                "deps/dependency.dll",
                "source/libobs/data/default.effect",
                "source/plugins/example/data/locale/en-US.ini",
                "build/test/cmocka/corrupt.png",
            ]:
                self.assertIn(path, manifest["files"])
            h.verify(root / "bundle")
            report = h.archive(root / "bundle", root / "bundle.tar.zst")
            self.assertGreater(report["compressed_bytes"], 0)
            h.consumer.extract(root / "bundle.tar.zst", root / "extracted")
            h.verify(root / "extracted")
            with self.assertRaisesRegex(ValueError, "complete"):
                h.consumer.verify(root / "extracted", "a" * 40)
            (root / "extracted/build/test/cmocka/empty space").rename(
                root / "extracted/build/test/cmocka/renamed"
            )
            with self.assertRaisesRegex(ValueError, "director"):
                h.verify(root / "extracted")

    def test_producer_paths_are_rejected_in_pe_and_text_in_both_encodings(self):
        h = self.helper()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for encoding in ("utf-8", "utf-16le"):
                for name in ("image.dll", "resource.ini"):
                    path = root / name
                    path.write_bytes(b"prefix" + "/producer/root/data".encode(encoding))
                    with self.assertRaisesRegex(ValueError, "producer path"):
                        h.audit(root, ["/producer/root"])

    def test_nested_capture_helpers_are_in_the_module_data_directory(self):
        h = self.helper()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source, build, _, _ = self.fixture(root)
            data = source / "plugins/win-capture/data"
            data.mkdir(parents=True)
            (data / "locale.ini").write_text("Name=Capture")
            products = []
            for name in ("win-capture.dll", "graphics-hook/graphics-hook64.dll", "inject-helper/inject-helper64.exe", "get-graphics-offsets/get-graphics-offsets64.exe"):
                path = build / "plugins/win-capture" / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(pe([]))
                products.append(path)
            h.stage_data(build, source, root / "bundle", products)
            for product in products[1:]:
                for base in ("build/core/win-capture/data", "source/plugins/win-capture/data"):
                    self.assertEqual(product.read_bytes(), (root / "bundle" / base / product.name).read_bytes())

    def test_missing_transitive_dependency_and_case_collision_fail(self):
        h = self.helper()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.stage(root)
            (root / "bundle/deps/dependency.dll").write_bytes(pe(["unprovided.dll"]))
            manifest = json.loads((root / "bundle/manifest.json").read_text())
            manifest["files"]["deps/dependency.dll"] = h.probe.digest(
                root / "bundle/deps/dependency.dll"
            )
            (root / "bundle/manifest.json").write_text(json.dumps(manifest))
            with self.assertRaisesRegex(ValueError, "unresolved"):
                h.verify(root / "bundle")
            (root / "bundle/deps/Dependency.dll").write_bytes(pe([]))
            with self.assertRaises(ValueError):
                h.verify(root / "bundle")

    @unittest.skipUnless(shutil.which("clang"), "requires clang")
    def test_compile_time_relocation_preserves_runtime_reads(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "source"
            source.mkdir()
            (source / "payload").write_text("relocated")
            (source / "probe.c").write_text(
                '#include <stdio.h>\nconst char *file = __FILE__;\nint main(void) { FILE *f = fopen(DATA, "r"); if (!f) return 3; fclose(f); puts(file); return 0; }\n'
            )
            (source / "CMakeLists.txt").write_text(
                "cmake_minimum_required(VERSION 3.30)\nproject(relocation C)\n"
                'set(OBS_SOURCE_ROOT "${CMAKE_CURRENT_SOURCE_DIR}")\n'
                "add_executable(probe probe.c)\n"
                'target_compile_definitions(probe PRIVATE DATA="${OBS_SOURCE_ROOT}/payload")\n'
                f'include("{ROOT}/cmake/cross/windows-tier2-relocation.cmake")\n'
                'obs_windows_tier2_relocate("${CMAKE_CURRENT_SOURCE_DIR}")\n'
            )
            build = root / "build"
            subprocess.run(
                [
                    "cmake",
                    "-S",
                    str(source),
                    "-B",
                    str(build),
                    "-G",
                    "Ninja",
                    "-DCMAKE_C_COMPILER=clang",
                ],
                check=True,
                capture_output=True,
            )
            subprocess.run(
                ["cmake", "--build", str(build)], check=True, capture_output=True
            )
            self.assertNotIn(str(root).encode(), (build / "probe").read_bytes())
            relocated = root / "relocated space"
            (relocated / "build/test/cmocka").mkdir(parents=True)
            shutil.copytree(source, relocated / "source")
            shutil.copy2(build / "probe", relocated / "build/probe")
            subprocess.run(
                [str(relocated / "build/probe")],
                cwd=relocated / "build/test/cmocka",
                check=True,
                capture_output=True,
            )


if __name__ == "__main__":
    unittest.main()
