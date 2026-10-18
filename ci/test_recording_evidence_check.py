#!/usr/bin/env -S uv run --script
"""Tests for the recording evidence check (#1305).

    uv run python -m unittest ci/test_recording_evidence_check.py
"""

from __future__ import annotations

import hashlib
import io
import json
import sys
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import recording_evidence_check as rec  # noqa: E402

NVIDIA_LOG = """\
07:34:33.238: Initializing OpenGL...
07:34:33.372: Loading up OpenGL on adapter NVIDIA Corporation NVIDIA GeForce RTX 3060/PCIe/SSE2
07:34:33.372: OpenGL loaded successfully, version 3.3.0 NVIDIA 595.104.02, shading language 3.30 NVIDIA via Cg compiler
07:34:33.403: video settings reset:
"""

LLVMPIPE_LOG = """\
10:00:00.000: Initializing OpenGL...
10:00:00.100: Loading up OpenGL on adapter Mesa llvmpipe (LLVM 19.1.7, 256 bits)
10:00:00.100: OpenGL loaded successfully, version 4.5 (Core Profile) Mesa 25.0.7, shading language 4.50
"""

# Hardware adapter name, but the version string shows the software rasterizer.
SWRAST_VERSION_LOG = """\
10:00:00.100: Loading up OpenGL on adapter Mesa Intel(R) UHD Graphics
10:00:00.100: OpenGL loaded successfully, version 4.5 (Core Profile) Mesa 25.0.7 (softpipe), shading language 4.50
"""

D3D11_LOG = """\
10:00:00.100: Loading up D3D11 on adapter NVIDIA GeForce RTX 3060 (0)
"""

D3D11_WARP_LOG = """\
10:00:00.100: Loading up D3D11 on adapter Microsoft Basic Render Driver (0)
"""


def maps_line(path: Path) -> str:
    return f"7f0000000000-7f0000001000 r-xp 00001000 00:2c 1234   {path}\n"


class Fixture:
    """A fake install: a C libobs, a Rust twin and a graphics module."""

    def __init__(self, root: Path):
        lib = root / "opt/obs/lib"
        (lib / "libobs-with-rust").mkdir(parents=True)
        self.c = lib / "libobs.so.30"
        self.c.write_bytes(b"\x7fELF C libobs, no Rust in here")
        self.rust = lib / "libobs-with-rust/libobs.so.30"
        self.rust.write_bytes(b"\x7fELF ... /rustc/0123abcd/library/core/src/fmt.rs ...")
        self.opengl = lib / "libobs-opengl.so.30"
        self.opengl.write_bytes(b"\x7fELF opengl")
        self.system_opengl = root / "usr/lib/libobs-opengl.so.30"
        self.system_opengl.parent.mkdir(parents=True)
        self.system_opengl.write_bytes(b"\x7fELF stock opengl")
        self.prefix = root / "opt/obs"

    def maps(self, *paths: Path) -> str:
        return "".join(maps_line(p) for p in paths) + maps_line(Path("/usr/lib/libc.so.6"))


class RendererTest(unittest.TestCase):
    def test_accepts_nvidia_opengl(self):
        r = rec.renderer(NVIDIA_LOG)
        self.assertEqual(r["api"], "OpenGL")
        self.assertIn("RTX 3060", r["adapter"])
        self.assertIn("NVIDIA 595.104.02", r["version"])

    def test_accepts_hardware_d3d11(self):
        self.assertEqual(rec.renderer(D3D11_LOG)["api"], "D3D11")

    def test_rejects_llvmpipe(self):
        with self.assertRaisesRegex(rec.CheckError, "software renderer"):
            rec.renderer(LLVMPIPE_LOG)

    def test_rejects_software_version_string(self):
        with self.assertRaisesRegex(rec.CheckError, "software renderer"):
            rec.renderer(SWRAST_VERSION_LOG)

    def test_rejects_warp(self):
        with self.assertRaisesRegex(rec.CheckError, "software renderer"):
            rec.renderer(D3D11_WARP_LOG)

    def test_missing_renderer_line_fails_closed(self):
        with self.assertRaisesRegex(rec.CheckError, "no renderer line"):
            rec.renderer("10:00:00.000: video settings reset:\n")

    def test_two_renderers_are_ambiguous(self):
        with self.assertRaisesRegex(rec.CheckError, "more than one renderer"):
            rec.renderer(NVIDIA_LOG + LLVMPIPE_LOG)


class MappedLibobsTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.f = Fixture(Path(self.tmp.name))

    def tearDown(self):
        self.tmp.cleanup()

    def check(self, maps: str, expect: str, prefix: Path | None = None) -> dict:
        return rec.libraries(maps, expect=expect, prefix=prefix)

    def test_rust_twin_is_detected(self):
        out = self.check(self.f.maps(self.f.rust, self.f.opengl), "rust")
        self.assertEqual(out["libobs"]["backend"], "rust")
        self.assertEqual(out["libobs"]["path"], str(self.f.rust))
        self.assertEqual(
            out["libobs"]["sha256"], hashlib.sha256(self.f.rust.read_bytes()).hexdigest()
        )
        self.assertEqual(out["graphics"]["path"], str(self.f.opengl))

    def test_c_libobs_is_detected(self):
        out = self.check(self.f.maps(self.f.c, self.f.opengl), "c")
        self.assertEqual(out["libobs"]["backend"], "c")

    def test_rejects_c_when_rust_expected(self):
        with self.assertRaisesRegex(rec.CheckError, "expected the rust libobs, mapped c"):
            self.check(self.f.maps(self.f.c, self.f.opengl), "rust")

    def test_rejects_rust_when_c_expected(self):
        with self.assertRaisesRegex(rec.CheckError, "expected the c libobs, mapped rust"):
            self.check(self.f.maps(self.f.rust, self.f.opengl), "c")

    def test_rejects_two_libobs(self):
        with self.assertRaisesRegex(rec.CheckError, "2 libobs"):
            self.check(self.f.maps(self.f.c, self.f.rust, self.f.opengl), "rust")

    def test_rejects_missing_libobs(self):
        with self.assertRaisesRegex(rec.CheckError, "0 libobs"):
            self.check(self.f.maps(self.f.opengl), "c")

    def test_rejects_missing_graphics_module(self):
        with self.assertRaisesRegex(rec.CheckError, "0 graphics modules"):
            self.check(self.f.maps(self.f.rust), "rust")

    def test_rejects_graphics_module_outside_prefix(self):
        # A stock libobs-opengl from the system lib dir next to our libobs.
        with self.assertRaisesRegex(rec.CheckError, "outside"):
            self.check(
                self.f.maps(self.f.rust, self.f.system_opengl), "rust", prefix=self.f.prefix
            )

    def test_prefix_accepts_own_install(self):
        out = self.check(self.f.maps(self.f.rust, self.f.opengl), "rust", prefix=self.f.prefix)
        self.assertEqual(out["libobs"]["backend"], "rust")

    def test_unreadable_mapped_file_fails_closed(self):
        gone = Path(self.tmp.name) / "opt/obs/lib/gone/libobs.so.30"
        with self.assertRaisesRegex(rec.CheckError, "cannot read"):
            self.check(self.f.maps(gone, self.f.opengl), "c")

    def test_deleted_mapping_is_rejected(self):
        # The library was replaced on disk while the process ran.
        maps = maps_line(Path(f"{self.f.c} (deleted)")) + maps_line(self.f.opengl)
        with self.assertRaisesRegex(rec.CheckError, "deleted"):
            self.check(maps, "c")


class MainTest(unittest.TestCase):
    def test_main_passes_and_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            f = Fixture(Path(tmp))
            log = Path(tmp) / "obs.log"
            maps = Path(tmp) / "maps"
            out = Path(tmp) / "identity.json"
            maps.write_text(f.maps(f.rust, f.opengl))
            log.write_text(NVIDIA_LOG)
            args = ["--log", str(log), "--maps", str(maps), "--libobs", "rust"]
            with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                self.assertEqual(rec.main([*args, "--json", str(out)]), 0)
                self.assertEqual(json.loads(out.read_text())["libobs"]["backend"], "rust")
                log.write_text(LLVMPIPE_LOG)
                self.assertEqual(rec.main(args), 1)


if __name__ == "__main__":
    unittest.main()
