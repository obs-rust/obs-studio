#!/usr/bin/env -S uv run --script
"""Keep port records in the table instead of creating an EOF conflict hot spot.

    uv run python -m unittest ci/test_rust_readme.py
"""

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PUBLISHED_BINARIES = (
    "Published binaries go on the app's `obs-studio-rust-v*` releases; the\n"
    "`obs-stream-rust-v0.1.0` tag is retired."
)


class RustReadmeTest(unittest.TestCase):
    def test_no_port_records_after_published_binaries(self):
        readme = (ROOT / "rust" / "README.md").read_text(encoding="utf-8")
        self.assertEqual(readme.count(PUBLISHED_BINARIES), 1, "keep the published-binaries paragraph as the README's final paragraph")
        trailing = readme.split(PUBLISHED_BINARIES, 1)[1]
        self.assertFalse(
            trailing.strip(),
            "Add or extend the port table row for the C file, in sorted position, never at the end of rust/README.md",
        )


if __name__ == "__main__":
    unittest.main()
