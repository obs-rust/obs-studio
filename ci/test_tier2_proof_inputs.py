#!/usr/bin/env -S uv run --script
"""The tier2-linux lane proof ignores Rust Tier 2 never builds (#930 A5).

    uv run python -m unittest ci/test_tier2_proof_inputs.py

The proof key (ci/lane_proof.py) hashes every file the lane's `exclude`
does not drop, so a change to an integration test or to a crate Tier 2
never compiles reran all of Tier 2 Linux (a build and 6 shards). Tier 2
builds libobs-with-rust, obs-nvenc-with-rust and the preflight ABI test:
the cargo closure of the crates those CMake files import. `cargo build`
never compiles tests/, benches/ or examples/, and crates outside that
closure not at all; their Cargo.toml stays an input (cargo reads every
workspace manifest).

The safety check: no crate the lane drops may be in that closure, with the
roots found from the CMake inputs, not listed by hand.
"""

from __future__ import annotations

import re
import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
import ci_scope

# Where Tier 2 Linux's CMake can import a crate (validate.sh: frontend off).
CMAKE_INPUTS = ("libobs", "plugins", "test/cmocka", "cmake", "deps")


def lane() -> dict:
    return tomllib.loads((ROOT / "local-gate.toml").read_text(encoding="utf-8"))["gate"]["lanes"]["tier2-linux"]


def glob_re(pattern: str) -> re.Pattern[str]:
    out = ""
    i = 0
    while i < len(pattern):
        if pattern.startswith("**/", i):
            out += "(?:.*/)?"
            i += 3
        elif pattern.startswith("**", i):
            out += ".*"
            i += 2
        elif pattern[i] == "*":
            out += "[^/]*"
            i += 1
        else:
            out += re.escape(pattern[i])
            i += 1
    return re.compile(out + r"\Z")


def excluded(path: str) -> bool:
    ln = lane()
    drop = any(glob_re(p).match(path) for p in ln.get("exclude", []))
    return drop and not any(glob_re(p).match(path) for p in ln.get("keep", []))


def crates() -> list[str]:
    return sorted(p.parent.name for p in (ROOT / "rust").glob("*/Cargo.toml"))


def mentioned_crates(text: str, names: list[str]) -> set[str]:
    # Native C oracle paths are build inputs, not Cargo imports. Preserve
    # manifest and Rust source references when inferring the Rust build roots.
    text = re.sub(r"\brust/[\w-]+/[^\s\"';}]+\.(?:c|h|cpp|hpp)(?=[\s\"';)]|$)", "", text)
    return {c for c in names if re.search(rf"rust/{re.escape(c)}\b|\b{re.escape(c)}\b(?!-)", text)}


def tier2_roots() -> set[str]:
    names = crates()
    roots = set()
    for top in CMAKE_INPUTS:
        for f in (ROOT / top).rglob("*"):
            if f.suffix not in (".cmake", ".txt") or not f.is_file():
                continue
            text = f.read_text(encoding="utf-8", errors="replace")
            roots.update(mentioned_crates(text, names))
    return roots


def tier2_closure() -> set[str]:
    read = lambda p: (ROOT / p).read_text(encoding="utf-8")
    built: set[str] = set()
    for root in tier2_roots():
        built |= ci_scope.linked_crates(read, root)
    return built


class Tier2ProofInputsTest(unittest.TestCase):
    def test_native_oracle_path_is_not_a_cargo_root(self):
        text = '\n'.join((
            'file(READ "${CMAKE_SOURCE_DIR}/rust/obs-c-oracle/legacy/encoder.c" baseline)',
            'corrosion_import_crate(MANIFEST_PATH "${CMAKE_SOURCE_DIR}/rust/libobs-rust/Cargo.toml")',
        ))
        self.assertEqual(mentioned_crates(text, ["obs-c-oracle", "libobs-rust"]), {"libobs-rust"})

    def test_native_oracle_legacy_stays_a_tier2_input(self):
        self.assertFalse(excluded("rust/obs-c-oracle/legacy/obs-encoder-audio-buffers.c"))
        self.assertTrue(excluded("rust/obs-c-oracle/src/lib.rs"))

    def test_roots_are_found(self):
        self.assertTrue({"libobs-rust", "obs-nvenc-plugin", "obs-preflight-ffi"} <= tier2_roots(), tier2_roots())

    def test_no_built_crate_source_is_dropped(self):
        for c in sorted(tier2_closure()):
            for rel in ("src/lib.rs", "build.rs", "src/ffi/mod.rs"):
                self.assertFalse(excluded(f"rust/{c}/{rel}"), f"rust/{c}/{rel} is built by Tier 2")

    def test_manifests_stay_inputs(self):
        for c in crates():
            self.assertFalse(excluded(f"rust/{c}/Cargo.toml"), c)

    def test_tests_benches_examples_are_dropped(self):
        for path in ("rust/obs-util/tests/profiler/profiler.rs", "rust/obs-codec/benches/nal.rs",
                     "rust/obs-gpu-status/examples/probe.rs"):
            self.assertTrue(excluded(path), path)

    def test_crates_tier2_never_builds_are_dropped(self):
        unbuilt = [c for c in crates() if c not in tier2_closure()]
        self.assertIn("obs-c-oracle", unbuilt)
        for c in unbuilt:
            self.assertTrue(excluded(f"rust/{c}/src/lib.rs"), f"{c} is never built by Tier 2")


if __name__ == "__main__":
    unittest.main()
