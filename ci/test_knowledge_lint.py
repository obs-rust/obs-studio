"""Tests for ci/knowledge_lint.py (#3438)."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

import knowledge_lint

GOOD = "---\ntype: concept\ntitle: A\ndescription: about a\n---\n\n# A\n"
INDEX = "---\ntype: index\ndescription: the bundle\n---\n\n- [A](a.md) — about a\n"


class KnowledgeLintTest(unittest.TestCase):
    def bundle(self, files: dict[str, str]) -> Path:
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        root = Path(tmp.name)
        for name, text in files.items():
            (root / name).write_text(text, encoding="utf-8")
        return root

    def test_repo_bundle_is_clean(self):
        self.assertEqual(knowledge_lint.problems(knowledge_lint.ROOT / "docs" / "knowledge"), [])

    def test_good_bundle(self):
        self.assertEqual(knowledge_lint.problems(self.bundle({"index.md": INDEX, "a.md": GOOD})), [])

    def test_unlisted_file_fails(self):
        found = knowledge_lint.problems(self.bundle({"index.md": INDEX, "a.md": GOOD, "b.md": GOOD}))
        self.assertEqual(len(found), 1)
        self.assertIn("b.md: not listed", found[0])

    def test_missing_frontmatter_fails(self):
        found = knowledge_lint.problems(self.bundle({"index.md": INDEX, "a.md": "# A\n"}))
        self.assertIn("no YAML frontmatter", found[0])

    def test_empty_type_or_description_fails(self):
        found = knowledge_lint.problems(self.bundle({"index.md": INDEX, "a.md": "---\ntype:\ntitle: A\n---\n"}))
        self.assertEqual(sorted(f.split("`")[1] for f in found), ["description", "type"])


if __name__ == "__main__":
    unittest.main()
