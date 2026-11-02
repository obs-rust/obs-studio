"""Tests for ci/knowledge_lint.py (#3438)."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

import knowledge_lint

GOOD = "---\ntype: concept\ntitle: A\ndescription: about a\n---\n\n# A\n"
INTRO = "---\ntype: index\ndescription: the bundle\n---\n\nIntroduction.\n\n"
START = "<!-- concepts: generated, `./lint --fix` writes this list -->\n"
END = "<!-- /concepts -->\n"
INDEX = f"{INTRO}{START}- [A](a.md) — about a\n{END}"
EMPTY_INDEX = f"{INTRO}{START}{END}"


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
        self.assertIn("./lint --fix", found[0])

    def test_edited_description_fails(self):
        changed = GOOD.replace("about a", "about something else")
        found = knowledge_lint.problems(self.bundle({"index.md": INDEX, "a.md": changed}))
        self.assertEqual(len(found), 1)
        self.assertIn("./lint --fix", found[0])

    def test_missing_markers_fail(self):
        plain = f"{INTRO}- [A](a.md) — about a\n"
        found = knowledge_lint.problems(self.bundle({"index.md": plain, "a.md": GOOD}))
        self.assertEqual(len(found), 1)
        self.assertIn("marker", found[0])

    def test_missing_frontmatter_fails(self):
        found = knowledge_lint.problems(self.bundle({"index.md": INDEX, "a.md": "# A\n"}))
        self.assertIn("no YAML frontmatter", found[0])

    def test_empty_type_or_description_fails(self):
        found = knowledge_lint.problems(self.bundle({"index.md": INDEX, "a.md": "---\ntype:\ntitle: A\n---\n"}))
        self.assertEqual(sorted(f.split("`")[1] for f in found), ["description", "type"])

    def test_missing_title_fails(self):
        found = knowledge_lint.problems(self.bundle({"index.md": INDEX, "a.md": "---\ntype: c\ndescription: d\n---\n"}))
        self.assertEqual([f.split("`")[1] for f in found], ["title"])

    def test_quoted_value_keeps_its_hash(self):
        """A plain YAML value ends at ` #`, so a description that names an issue is quoted."""
        quoted = '---\ntype: c\ntitle: A\ndescription: "fails since #3290."\n---\n'
        root = self.bundle({"index.md": EMPTY_INDEX, "a.md": quoted})
        self.assertEqual(knowledge_lint.main(["knowledge_lint", str(root), "--fix"]), 0)
        self.assertIn("- [A](a.md) — fails since #3290.\n", (root / "index.md").read_text(encoding="utf-8"))

    def test_unquoted_hash_fails(self):
        plain = "---\ntype: c\ntitle: A\ndescription: fails since #3290.\n---\n"
        found = knowledge_lint.problems(self.bundle({"index.md": EMPTY_INDEX, "a.md": plain}))
        self.assertTrue(any("quote" in f for f in found), found)

    def test_fix_writes_the_list_sorted_by_file_name(self):
        second = GOOD.replace("title: A", "title: Z").replace("about a", "about z")
        root = self.bundle({"index.md": EMPTY_INDEX, "z.md": second, "a.md": GOOD})
        self.assertEqual(knowledge_lint.main(["knowledge_lint", str(root), "--fix"]), 0)
        text = (root / "index.md").read_text(encoding="utf-8")
        self.assertIn(f"{START}- [A](a.md) — about a\n- [Z](z.md) — about z\n{END}", text)
        self.assertTrue(text.startswith(INTRO), "the hand-written part stays")
        self.assertEqual(knowledge_lint.problems(root), [])

    def test_fix_does_not_hide_other_problems(self):
        root = self.bundle({"index.md": EMPTY_INDEX, "a.md": "# A\n"})
        self.assertEqual(knowledge_lint.main(["knowledge_lint", str(root), "--fix"]), 1)


if __name__ == "__main__":
    unittest.main()
