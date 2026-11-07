#!/usr/bin/env -S uv run --script
"""Keep per-port Rust-static expectations out of shared registration lists."""

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REGISTRATION = ROOT / "rust/obs-build/tests/native/registration"


def shared_list(source):
    return bool(re.search(r"const\s+CASES\s*:", source))


class RegistrationEntries(unittest.TestCase):
    def test_shared_append_fixture_is_rejected(self):
        self.assertTrue(shared_list('const CASES: &[(&str, bool)] = &[("test_port", false)];'))
        self.assertFalse(shared_list('static CASES: LazyLock<Vec<Case>> = LazyLock::new(discover);'))

    def test_repository_uses_independent_entries(self):
        for path in sorted(REGISTRATION.glob("canonical_rust_static_*.rs")):
            self.assertFalse(shared_list(path.read_text()), f"{path}: add a per-port entry instead")
        entries = REGISTRATION / "rust-static/cases"
        self.assertTrue(list(entries.glob("*/*.json")), "missing independent registration entries")


if __name__ == "__main__":
    unittest.main()
