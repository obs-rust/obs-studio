#!/usr/bin/env -S uv run --script
"""The libobs lib-module cache keys modules by what they compile (#3262).

    uv run python -m unittest ci/test_libobs_modules_cache.py

ci/libobs_modules.py keys each lib-module by its compile commands, the
files they read and the compiler. These tests build a tiny fake tree (a
compile_commands.json over two groups in two modules, compiled by the host
C compiler) and check that the key:

- is the same in two build dirs of one checkout;
- changes for the module whose header changed, and only for it;
- ignores the compiler launcher and a cache hit's probe target name.

They also check that libobs/cmake/modules.cmake imports a hit.
"""

from __future__ import annotations

import importlib.util
import json
import shutil
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SPEC = importlib.util.spec_from_file_location("libobs_modules", ROOT / "ci" / "libobs_modules.py")
assert SPEC and SPEC.loader
mods = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(mods)

CC = shutil.which("cc") or shutil.which("gcc") or shutil.which("clang")


def fake_tree(base: Path, build_name: str, launcher: str | None = None, target_suffix: str = "") -> tuple[Path, Path]:
    source = base / "src"
    (source / "libobs" / "build").mkdir(parents=True, exist_ok=True)
    (source / "libobs" / "util").mkdir(parents=True, exist_ok=True)
    (source / "libobs" / "util" / "a.h").write_text("int a(void);\n") if not (
        source / "libobs" / "util" / "a.h"
    ).exists() else None
    (source / "libobs" / "graphics.h").write_text("int g(void);\n") if not (
        source / "libobs" / "graphics.h"
    ).exists() else None
    groups = {"util-cf": "util/a.h", "graphics": "graphics.h"}
    build = base / build_name
    build.mkdir(exist_ok=True)
    entries = []
    for group, header in groups.items():
        src = source / "libobs" / "build" / f"libobs.{group}+.h.c"
        src.write_text(f'#include "{header}"\n')
        module = "core" if group.startswith("util") else "graphics"
        obj = f"libobs/CMakeFiles/libobs-mod-{module}{target_suffix}.dir/build/libobs.{group}+.h.c.o"
        args = [*([launcher] if launcher else []), CC, f"-I{source}/libobs", f"-I{build}/config", "-O2", "-o", obj, "-c", str(src)]
        entries.append({"directory": str(build), "file": str(src), "output": obj, "arguments": args})
    (build / "compile_commands.json").write_text(json.dumps(entries))
    return source, build


@unittest.skipUnless(CC, "no C compiler")
class KeyTest(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.tmp)

    def keys(self, build: Path, source: Path) -> dict[str, str]:
        return mods.keys(build, source)

    def test_two_build_dirs_agree(self):
        source, one = fake_tree(self.tmp, "b1")
        _, two = fake_tree(self.tmp, "b2")
        k1 = self.keys(one, source)
        self.assertEqual(set(k1), {"core", "graphics"})
        self.assertEqual(k1, self.keys(two, source))

    def test_a_header_edit_changes_only_its_module(self):
        source, build = fake_tree(self.tmp, "b")
        before = self.keys(build, source)
        (source / "libobs" / "graphics.h").write_text("int g(void);\nint h(void);\n")
        after = self.keys(build, source)
        self.assertNotEqual(before["graphics"], after["graphics"])
        self.assertEqual(before["core"], after["core"])

    def test_launcher_and_probe_name_do_not_change_the_key(self):
        source, plain = fake_tree(self.tmp, "b1")
        _, probed = fake_tree(self.tmp, "b2", launcher="zccache", target_suffix="-keyprobe")
        self.assertEqual(self.keys(plain, source), self.keys(probed, source))


MANIFESTS = {
    # libobs-rust -> obs-a (normal) -> obs-b (build, via the workspace);
    # obs-c is only a dev-dependency.
    "libobs-rust": '[package]\nname = "libobs-rust"\n[dependencies]\nobs-a = { path = "../obs-a" }\n',
    "obs-a": '[package]\nname = "obs-a"\n[build-dependencies]\nobs-b = { workspace = true }\n'
    '[dev-dependencies]\nobs-c = { path = "../obs-c" }\n',
    "obs-b": '[package]\nname = "obs-b"\n',
    "obs-c": '[package]\nname = "obs-c"\n',
}


class RustKeyTest(unittest.TestCase):
    """The Rust staticlib is one more cached module (#3265)."""

    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.tmp)
        self.source = self.tmp / "src"
        for name in ("libobs-rust", "obs-a", "obs-b", "obs-c"):
            (self.source / "rust" / name / "src").mkdir(parents=True)
            (self.source / "rust" / name / "src" / "lib.rs").write_text(f"// {name}\n")
            (self.source / "rust" / name / "tests").mkdir()
            (self.source / "rust" / name / "tests" / "t.rs").write_text("// test\n")
            (self.source / "rust" / name / "Cargo.toml").write_text(MANIFESTS[name])
        (self.source / "Cargo.toml").write_text('[workspace.dependencies]\nobs-b = { path = "rust/obs-b" }\n')
        (self.source / "Cargo.lock").write_text("lock 1\n")
        (self.source / "libobs").mkdir()
        (self.source / "libobs" / "obs-hotkeys.h").write_text("hotkeys\n")
        self.build = self.tmp / "build"
        self.build.mkdir()
        self.config = {"location": "/x/liblibobs_rust.a", "features": "gio", "target": "", "cc": "/usr/bin/cc"}
        self.write_config()
        self.crates = mods.linked_crate_dirs(self.source)

    def write_config(self):
        (self.build / mods.RUST_JSON).write_text(json.dumps(self.config))

    def key(self):
        return mods.rust_key(self.build, self.source, self.crates)

    def test_linked_crates_follow_normal_and_build_deps(self):
        self.assertEqual([c.name for c in self.crates], ["libobs-rust", "obs-a", "obs-b"])

    def test_inputs_change_the_key(self):
        base = self.key()
        self.assertEqual(base, self.key())
        for path in ("rust/obs-b/src/lib.rs", "Cargo.lock", "libobs/obs-hotkeys.h"):
            file = self.source / path
            old = file.read_text()
            file.write_text(old + "// edit\n")
            self.assertNotEqual(base, self.key(), path)
            file.write_text(old)
        self.config["features"] = ""
        self.write_config()
        self.assertNotEqual(base, self.key(), "features")

    def test_unlinked_crates_tests_and_paths_do_not(self):
        base = self.key()
        (self.source / "rust" / "obs-c" / "src" / "lib.rs").write_text("// edit\n")
        (self.source / "rust" / "obs-a" / "tests" / "t.rs").write_text("// edit\n")
        self.config["location"] = "/elsewhere/liblibobs_rust.a"
        self.config["cc"] = "/opt/bin/cc"
        self.write_config()
        self.assertEqual(base, self.key())

    def test_no_rust_config_means_no_key(self):
        (self.build / mods.RUST_JSON).unlink()
        self.assertIsNone(self.key())


class CmakeImportTest(unittest.TestCase):
    def test_a_hit_is_imported_not_compiled(self):
        text = (ROOT / "libobs" / "cmake" / "modules.cmake").read_text(encoding="utf-8")
        self.assertIn('include("${OBS_LIBOBS_MODULE_CACHE}/hits.cmake")', text)
        self.assertRegex(text, r"add_library\(\$\{_target\} STATIC IMPORTED\)")
        self.assertIn("EXCLUDE_FROM_ALL", text, "a hit's commands must stay visible to the key")

    def test_a_rust_hit_links_the_cached_archive(self):
        text = (ROOT / "libobs" / "cmake" / "rust.cmake").read_text(encoding="utf-8")
        self.assertIn("libobs-modcache-rust.json", text)
        self.assertIn("add_library(libobs_rust-cached STATIC IMPORTED GLOBAL)", text)
        self.assertIn("WHOLE_ARCHIVE,${_obs_rust_static}", text)


if __name__ == "__main__":
    unittest.main()
