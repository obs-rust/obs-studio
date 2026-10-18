#!/usr/bin/env -S uv run --script
"""Perf guards keep the code shapes a measured fix depends on (#1303).

    uv run python -m unittest ci/test_perf_guard_lint.py
"""

from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import perf_guard_lint as g  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent

GUARD = {
    "issue": 1303,
    "file": "src/av1.rs",
    "function": "parse_obu_header",
    "attributes": ["#[inline(always)]"],
    "require": ["obu_after(buf, 1)", "obu_after(buf, 2)"],
    "forbid": ["1 + usize::from("],
    "reason": "the computed start makes LLVM emit a select",
    "bench": "cargo bench -p obs-c-oracle --bench port_comparison -- av1/keyframe",
}

GOOD = """\
/// Doc comment.
// PERF GUARD #1303: ci/perf-guards.toml.
#[inline(always)]
fn parse_obu_header(buf: &[u8]) -> Obu {
    if get_bits(buf[0], 5, 1) != 0 {
        obu_after(buf, 2)
    } else {
        obu_after(buf, 1)
    }
}

fn other() {}
"""


class CheckSourceTest(unittest.TestCase):
    def problems(self, text: str, **changes) -> list[str]:
        return g.check_source(text, {**GUARD, **changes})

    def test_guarded_shape_passes(self):
        self.assertEqual(self.problems(GOOD), [])

    def test_formatting_does_not_matter(self):
        text = GOOD.replace("obu_after(buf, 2)", "obu_after(\n            buf,\n            2,\n        )")
        text = text.replace("2,\n        )", "2\n        )")
        self.assertEqual(self.problems(text), [])

    def test_a_missing_required_shape_fails_with_reason_and_bench(self):
        text = GOOD.replace("obu_after(buf, 2)", "obu_after(buf, 1 + usize::from(ext))")
        found = self.problems(text)
        self.assertEqual(len(found), 2, found)
        joined = "\n".join(found)
        self.assertIn("obu_after(buf, 2)", joined)
        self.assertIn("1 + usize::from(", joined)
        self.assertIn("#1303", joined)
        self.assertIn(GUARD["reason"], joined)
        self.assertIn(GUARD["bench"], joined)
        self.assertIn("ci/perf-guards.toml", joined)

    def test_a_required_shape_only_in_a_comment_does_not_count(self):
        text = GOOD.replace("        obu_after(buf, 2)\n", "        // obu_after(buf, 2)\n        todo!()\n")
        self.assertEqual(len(self.problems(text)), 1)
        text = GOOD.replace("        obu_after(buf, 2)\n", "        /* obu_after(buf, 2) */ todo!()\n")
        self.assertEqual(len(self.problems(text)), 1)

    def test_a_forbidden_shape_fails(self):
        text = GOOD.replace("    if get_bits", "    let start = 1 + usize::from(true);\n    if get_bits")
        found = self.problems(text)
        self.assertEqual(len(found), 1, found)
        self.assertIn("1 + usize::from(", found[0])

    def test_attributes_must_match_exactly(self):
        self.assertEqual(len(self.problems(GOOD.replace("#[inline(always)]\n", ""))), 1)
        self.assertEqual(len(self.problems(GOOD.replace("#[inline(always)]", "#[inline]"))), 1)
        self.assertEqual(self.problems(GOOD.replace("#[inline(always)]", "#[inline]"), attributes=["#[inline]"]), [])

    def test_an_attribute_of_another_function_does_not_count(self):
        text = GOOD.replace("#[inline(always)]\nfn parse_obu_header", "fn parse_obu_header")
        text = text.replace("fn other() {}", "#[inline(always)]\nfn other() {}")
        self.assertEqual(len(self.problems(text)), 1)

    def test_the_marker_comment_is_required(self):
        found = self.problems(GOOD.replace("// PERF GUARD #1303: ci/perf-guards.toml.\n", ""))
        self.assertEqual(len(found), 1, found)
        self.assertIn("PERF GUARD #1303", found[0])

    def test_a_missing_function_fails(self):
        found = self.problems(GOOD.replace("fn parse_obu_header", "fn parse_header"))
        self.assertEqual(len(found), 1, found)
        self.assertIn("not found", found[0])

    def test_braces_in_strings_and_comments_do_not_end_the_body(self):
        text = GOOD.replace("    if get_bits", '    let _ = "}"; // }\n    if get_bits')
        self.assertEqual(self.problems(text), [])

    def test_pub_and_generic_functions_are_found(self):
        text = GOOD.replace("fn parse_obu_header(", "pub(crate) fn parse_obu_header<'a>(")
        self.assertEqual(self.problems(text), [])


class RegistryTest(unittest.TestCase):
    def load(self, text: str) -> list[dict]:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "perf-guards.toml"
            path.write_text(text)
            return g.load_guards(path)

    def test_valid_entry_loads(self):
        guards = self.load(
            '[[guard]]\nissue = 1\nfile = "a.rs"\nfunction = "f"\nrequire = ["x"]\n'
            'reason = "r"\nbench = "b"\n'
        )
        self.assertEqual(guards[0]["function"], "f")
        self.assertEqual(guards[0]["forbid"], [])
        self.assertEqual(guards[0]["attributes"], [])

    def test_unknown_or_missing_keys_are_errors(self):
        with self.assertRaises(ValueError):
            self.load('[[guard]]\nissue = 1\nfile = "a.rs"\nfunction = "f"\nreason = "r"\nbench = "b"\nrequires = ["x"]\n')
        with self.assertRaises(ValueError):
            self.load('[[guard]]\nissue = 1\nfile = "a.rs"\nfunction = "f"\nrequire = ["x"]\nbench = "b"\n')

    def test_a_guard_must_check_something(self):
        with self.assertRaises(ValueError):
            self.load('[[guard]]\nissue = 1\nfile = "a.rs"\nfunction = "f"\nreason = "r"\nbench = "b"\n')


class RepoTest(unittest.TestCase):
    def test_the_repo_registry_holds(self):
        guards = g.load_guards(ROOT / "ci" / "perf-guards.toml")
        self.assertTrue(any(guard["issue"] == 1303 for guard in guards))
        self.assertEqual(g.check_repo(ROOT), [])


if __name__ == "__main__":
    unittest.main()
