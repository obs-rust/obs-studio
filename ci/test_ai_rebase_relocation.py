#!/usr/bin/env -S uv run --script
"""Directory-discovery relocation contracts for the rebase guard; no model calls.

#3973 moves per-target native test registrations out of the monolithic
`rust/obs-build/tests/native.rs` list and into `tests/native/registration/`,
where `rust/obs-studio-rust/build.rs` discovers them by scanning the directory.
A resolution of that migration therefore *drops* `#[path]`/`mod` pairs that
master added. The guard must read that as a move when — and only when — the
generated include is present and the fragment is still on disk.
"""
import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("ai_rebase", HERE / "ai_rebase.py")
bot = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bot)

NATIVE_RS = "rust/obs-build/tests/native.rs"
DISCOVERY = 'include!(concat!(env!("OUT_DIR"), "/native-registration.rs"));\n'

BASE = """//! Build-system integration contracts.
#[path = "native/archive_freshness.rs"]
mod archive_freshness;

use std::fs;
"""

# Master added three ordinary modules and three registration pairs.
OURS = """//! Build-system integration contracts.
#[path = "native/archive_freshness.rs"]
mod archive_freshness;
#[path = "native/core_module_manifest.rs"]
mod core_module_manifest;
#[path = "native/prebuilt.rs"]
mod prebuilt;
#[path = "native/registration/canonical_canvas_release_is_registered.rs"]
mod canonical_canvas_release_is_registered;
#[path = "native/registration/canonical_encoder_defaults_swap_is_registered.rs"]
mod canonical_encoder_defaults_swap_is_registered;
#[path = "native/registration/canonical_core_private_data_is_registered.rs"]
mod canonical_core_private_data_is_registered;

use std::fs;
"""

# The PR replaces the list with discovery.
THEIRS = """//! Build-system integration contracts.
include!(concat!(env!("OUT_DIR"), "/native-registration.rs"));
#[path = "native/archive_freshness.rs"]
mod archive_freshness;

use std::fs;
"""

MODULES = """//! Build-system integration contracts.
#[path = "native/archive_freshness.rs"]
mod archive_freshness;
#[path = "native/core_module_manifest.rs"]
mod core_module_manifest;
#[path = "native/prebuilt.rs"]
mod prebuilt;
""" + DISCOVERY + """
use std::fs;
"""


class DiscoveryRelocation(unittest.TestCase):
    """A registration pair is a move when the fragment is discovered by scan."""

    def setUp(self):
        self._cwd = Path.cwd()
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)
        (self.root / bot.REGISTRATION_DIR).mkdir(parents=True)
        for name in ("canonical_canvas_release_is_registered",
                     "canonical_encoder_defaults_swap_is_registered",
                     "canonical_core_private_data_is_registered"):
            (self.root / bot.REGISTRATION_DIR / f"{name}.rs").write_text("#[test]\nfn it() {}\n")
        (self.root / NATIVE_RS).parent.mkdir(parents=True, exist_ok=True)
        import os
        os.chdir(self.root)

    def tearDown(self):
        import os
        os.chdir(self._cwd)
        self._tmp.cleanup()

    def judge(self, resolution):
        Path(NATIVE_RS).write_text(resolution)
        return bot.check_resolution(NATIVE_RS, BASE, OURS, THEIRS)

    def test_registration_pairs_are_a_move_not_a_loss(self):
        self.assertIsNone(self.judge(MODULES))

    def test_ordinary_modules_still_load_bearing(self):
        # Dropping core_module_manifest and prebuilt leaves two losses; the
        # tolerance is two, so drop the third ordinary line to trip the guard.
        resolution = MODULES.replace("#[path = \"native/prebuilt.rs\"]\nmod prebuilt;\n", "")
        self.assertIsNone(self.judge(resolution), "two losses remain inside tolerance")
        resolution = (MODULES
                      .replace("#[path = \"native/prebuilt.rs\"]\nmod prebuilt;\n", "")
                      .replace("#[path = \"native/core_module_manifest.rs\"]\n"
                               "mod core_module_manifest;\n", "")
                      .replace('use std::fs;\n', ''))
        why = self.judge(resolution)
        self.assertIsNotNone(why)
        self.assertIn("added lines dropped", why)

    def test_relocation_needs_the_generated_include(self):
        without = MODULES.replace(DISCOVERY, "")
        why = self.judge(without)
        self.assertIsNotNone(why)
        self.assertIn("added lines dropped", why)

    def test_missing_fragment_is_still_refused(self):
        (self.root / bot.REGISTRATION_DIR / "canonical_prebuilt_probe.rs").write_text("")
        ours = OURS.replace(
            "mod core_module_manifest;",
            "mod core_module_manifest;\n#[path = \"native/registration/canonical_absent_x.rs\"]\n"
            "mod canonical_absent_x;\n#[path = \"native/registration/canonical_absent_y.rs\"]\n"
            "mod canonical_absent_y;\n#[path = \"native/registration/canonical_absent_z.rs\"]\n"
            "mod canonical_absent_z;")
        Path(NATIVE_RS).write_text(MODULES)
        why = bot.check_resolution(NATIVE_RS, BASE, ours, THEIRS)
        self.assertIsNotNone(why)
        self.assertIn("canonical_absent", why)


if __name__ == "__main__":
    unittest.main(verbosity=2)
