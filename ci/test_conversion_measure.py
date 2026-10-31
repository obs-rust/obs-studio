#!/usr/bin/env -S uv run --script
"""tools/conversion/measure.py: the share of OBS converted to Rust (#2242).

    uv run python -m unittest discover -s ci -p test_conversion_measure.py

A fixture tree pins the exact counts and the exclusions; a check on the
real tree pins the libobs ported set to rust/tools/ported_sources.py, the
Tier 2 definition.
"""

from __future__ import annotations

import importlib.util
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


MEASURE_PATH = ROOT / "tools/conversion/measure.py"
measure = load("conversion_measure", MEASURE_PATH)
ported_sources = load("ported_sources", ROOT / "rust/tools/ported_sources.py")

EMPTY_SETS = [
    "LIBOBS_RUST_SOURCES", "LIBOBS_WINDOWS_SOURCES", "LIBOBS_WINDOWS_OBFUSCATE_SOURCES",
    "LIBOBS_WINDOWS_PORTED_SOURCES", "LIBOBS_MACOS_SOURCES", "LIBOBS_MACOS_PORTED_SOURCES",
    "LIBOBS_LINUX_PORTED_SOURCES", "LIBOBS_HEVC_PORTED_SOURCES", "LIBOBS_PULSE_PORTED_SOURCES",
    "LIBOBS_NIX_GIO_PORTED_SOURCES", "LIBOBS_WAYLAND_PORTED_SOURCES",
    "LIBOBS_WAYLAND_GIO_PORTED_SOURCES",
]


def write(root: Path, rel: str, lines: int | None = None, text: str | None = None) -> None:
    path = root / rel
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text if text is not None else "x;\n" * lines)


def group(pch: str, *members: str) -> str:
    body = "/* Unity group (BUILD.md): fixture. */\n\n"
    body += f'#include "libobs/build/{pch}"\n\n'
    return body + "".join(f'#include "libobs/{m}"\n' for m in members)


def fixture(root: Path) -> None:
    sets = {name: [] for name in EMPTY_SETS}
    sets["LIBOBS_SOURCES"] = ["obs.h", "build/libobs.core+.h.c"]
    sets["LIBOBS_PORTED_SOURCES"] = ["build/libobs.ported-util+.h.c", "build/libobs.ported-media+.h.c"]
    sets["LIBOBS_LINUX_SOURCES"] = ["build/libobs.nix+.h.c"]
    for name, members in sets.items():
        write(root, f"libobs/build/source-sets.d/{name}.json", text=json.dumps(members))
    write(root, "libobs/build/libobs.core+.h.c", text=group("libobs.pch.h", "obs.c.h", "graphics/graphics.c.h"))
    write(root, "libobs/build/libobs.ported-util+.h.c",
          text=group("libobs-util.pch.h", "util/bmem.c.h", "util/dstr.c.h"))
    write(root, "libobs/build/libobs.ported-media+.h.c", text=group("libobs-media.pch.h", "media-io/audio-io.c.h"))
    write(root, "libobs/build/libobs.nix+.h.c", text=group("libobs-nix.pch.h", "obs-nix.c.h"))
    write(root, "libobs/build/libobs.pch.h", 5)
    write(root, "libobs/obs.c.h", 100)
    write(root, "libobs/graphics/graphics.c.h", 50)
    write(root, "libobs/util/bmem.c.h", 30)
    write(root, "libobs/util/dstr.c.h", 20)
    write(root, "libobs/media-io/audio-io.c.h", 40)
    write(root, "libobs/obs-nix.c.h", 10)
    write(root, "libobs/obs.h", 77)  # header: not implementation
    write(root, "libobs/unused-freebsd.c", 33)  # in no group: not compiled
    # Exclusions.
    write(root, "rust/obs-c-oracle/legacy/libobs/obs.c", 1000)
    write(root, "test/cmocka/test-obs.c", 500)
    write(root, "frontend/tests/test-main.cpp", 400)
    write(root, "rust/obs-foo/tests/t.rs", 300)
    write(root, "rust/target/debug/build/gen.rs", 600)
    write(root, "deps/json11/json11.cpp", 999)
    # Counted.
    write(root, "rust/obs-foo/src/lib.rs", 70)
    write(root, "plugins/obs-filters/filter.c", 60)
    write(root, "frontend/main.cpp", 40)
    # Kept by design.
    write(root, "plugins/obs-websocket/src/ws.cpp", 200)


class FixtureTree(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        cls.root = Path(cls.tmp.name)
        fixture(cls.root)
        cls.data = measure.measure(cls.root, commit="abc1234", date="2026-10-24")

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    def test_libobs_linux(self):
        linux = self.data["libobs"]["hosts"]["linux"]
        self.assertEqual(linux["total_lines"], 250)
        self.assertEqual(linux["ported_lines"], 90)
        self.assertEqual(linux["hybrid_lines"], 160)
        self.assertEqual(linux["percent"], 36.0)
        self.assertEqual(linux["ported_files"],
                         ["media-io/audio-io.c.h", "util/bmem.c.h", "util/dstr.c.h"])

    def test_libobs_other_hosts_lack_nix(self):
        for host in ("windows", "macos"):
            data = self.data["libobs"]["hosts"][host]
            self.assertEqual((data["total_lines"], data["ported_lines"], data["percent"]), (240, 90, 37.5))

    def test_libobs_areas(self):
        areas = self.data["libobs"]["hosts"]["linux"]["areas"]
        self.assertEqual({k: (v["total_lines"], v["ported_lines"]) for k, v in areas.items()},
                         {"util": (50, 50), "media-io": (40, 40), "graphics": (50, 0), "core": (110, 0)})

    def test_libobs_headline_is_all_hosts(self):
        everything = self.data["libobs"]["all"]
        self.assertEqual((everything["total_lines"], everything["ported_lines"], everything["percent"]),
                         (250, 90, 36.0))

    def test_whole_app(self):
        app = self.data["app"]
        self.assertEqual(app["total_lines"], 350)
        self.assertEqual(app["ported_lines"], 90)
        self.assertEqual(app["percent"], 25.71)
        self.assertEqual(app["by_dir"], {"libobs": 250, "frontend": 40, "plugins": 60})

    def test_websocket_kept_by_design(self):
        self.assertEqual(self.data["kept_by_design"], {"plugins/obs-websocket": 200})

    def test_rust_lines_secondary(self):
        self.assertEqual(self.data["rust"]["lines"], 70)

    def test_exclusions_listed(self):
        for prefix in ("rust/obs-c-oracle/legacy/", "test/", "deps/"):
            self.assertIn(prefix, " ".join(self.data["excluded"]))

    def test_svg_is_text_and_themed(self):
        svg = measure.render_svg(self.data)
        self.assertTrue(svg.startswith("<svg"))
        self.assertNotIn("<image", svg)
        self.assertIn("prefers-color-scheme: dark", svg)
        for text in ("36.0%", "25.71%", "abc1234", "2026-10-24"):
            self.assertIn(text, svg)

    def test_deterministic_and_check(self):
        again = measure.measure(self.root, commit="abc1234", date="2026-10-24")
        self.assertEqual(json.dumps(self.data, sort_keys=True), json.dumps(again, sort_keys=True))
        out = self.root / "out"
        base = [sys.executable, str(MEASURE_PATH), "--root", str(self.root), "--commit", "abc1234",
                "--date", "2026-10-24", "--json", str(out / "conversion.json"), "--svg", str(out / "conversion.svg")]
        subprocess.run(base, check=True)
        self.assertEqual(subprocess.run(base + ["--check"]).returncode, 0)
        (out / "conversion.json").write_text("{}")
        self.assertEqual(subprocess.run(base + ["--check"], capture_output=True).returncode, 1)


class RealTree(unittest.TestCase):
    def test_ported_set_matches_tier2_inventory(self):
        data = measure.measure(ROOT, commit="x", date="x")
        for host in ("linux", "windows", "macos"):
            expected = sorted(o.removesuffix(".o") for o in ported_sources.ported_objects(ROOT, host))
            self.assertEqual(data["libobs"]["hosts"][host]["ported_files"], expected, host)

    def test_every_libobs_group_is_counted(self):
        groups = {p.name for p in (ROOT / "libobs/build").glob("libobs.*+.h.*")}
        sets = ported_sources.source_sets(ROOT)
        listed = {Path(e).name for v in sets.values() for e in v if "+." in e}
        listed |= {Path(e).name for v in measure.REFERENCE_SETS.values() for e in v if e.startswith("build/")}
        # Hybrid-only glue and the pulse-off alternative are not in the reference.
        self.assertEqual(groups - listed, {"libobs.null-audio+.h.c"} - listed)


if __name__ == "__main__":
    unittest.main()
