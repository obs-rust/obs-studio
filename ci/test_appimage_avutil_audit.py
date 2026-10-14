#!/usr/bin/env -S uv run --script
"""Reject ABI or backend loss in the source-built AppImage libavutil (#625)."""

import importlib.util
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent
SPEC = importlib.util.spec_from_file_location(
    "vaapi_build", ROOT / "build-aux/appimage/build_vaapi_compat.py"
)
BUILD = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BUILD)


class AvutilAuditTest(unittest.TestCase):
    def test_actual_loaded_backends_and_exported_data_layouts_are_preserved(self):
        with tempfile.TemporaryDirectory(prefix="obs-avutil-audit-") as scratch:
            directory = Path(scratch)
            (directory / "libavutil").mkdir()
            (directory / "libavutil/hwcontext.h").write_text(
                "enum AVHWDeviceType { AV_HWDEVICE_TYPE_NONE, VAAPI, CUDA };\n"
            )
            features = directory / "features"
            subprocess.run(
                [
                    "cc",
                    "-I",
                    str(directory),
                    str(ROOT / "build-aux/appimage/avutil-features.c"),
                    "-ldl",
                    "-o",
                    str(features),
                ],
                check=True,
                capture_output=True,
            )
            source = directory / "library.c"
            source.write_text(
                '#include "libavutil/hwcontext.h"\n'
                "int exported_layout[SIZE];\n"
                "unsigned avutil_version(void) { return 123; }\n"
                "enum AVHWDeviceType av_hwdevice_iterate_types(enum AVHWDeviceType t) { return t ? 0 : BACKEND; }\n"
                'const char *av_hwdevice_get_type_name(enum AVHWDeviceType t) { return t==VAAPI ? "vaapi" : "cuda"; }\n'
                "int unchanged_api(void) { return VALUE; }\n"
            )
            libraries = {}
            for name, size, backend, value in (
                ("original", 4, "VAAPI", 0),
                ("compatible", 4, "VAAPI", 1),
                ("layout", 8, "VAAPI", 1),
                ("backend", 4, "CUDA", 1),
            ):
                library = directory / f"{name}.so"
                subprocess.run(
                    [
                        "cc",
                        "-fPIC",
                        "-shared",
                        "-I",
                        str(directory),
                        f"-DSIZE={size}",
                        f"-DBACKEND={backend}",
                        f"-DVALUE={value}",
                        str(source),
                        "-o",
                        str(library),
                    ],
                    check=True,
                    capture_output=True,
                )
                libraries[name] = library
            BUILD.audit_library(
                libraries["original"], libraries["compatible"], features
            )
            with self.assertRaisesRegex(RuntimeError, "layouts=.*exported_layout"):
                BUILD.audit_library(
                    libraries["original"], libraries["layout"], features
                )
            with self.assertRaisesRegex(RuntimeError, "version/backends changed"):
                BUILD.audit_library(
                    libraries["original"], libraries["backend"], features
                )
