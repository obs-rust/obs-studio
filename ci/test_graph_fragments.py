#!/usr/bin/env -S uv run --script
"""Contracts for independently registered native test graph fragments (#3973)."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

from native_test_graph import load_graph

ROOT = Path(__file__).resolve().parents[1]


def fragment(name):
    return {"schema": 1, "targets": {name: {"sources": [name + ".c"]}},
            "tests": {name: {"target": name}}}


class GraphFragments(unittest.TestCase):
    def test_disjoint_additions_compose_in_name_order(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory = Path(tmp)
            # Independent patches add distinct paths; neither edits an append list.
            for name in ("z", "a"):
                (directory / (name + ".json")).write_text(json.dumps(fragment(name)))
            graph = load_graph(directory)
            self.assertEqual(list(graph["targets"]), ["a", "z"])
            self.assertEqual(list(graph["tests"]), ["a", "z"])
            self.assertEqual(graph["targets"], {**fragment("a")["targets"], **fragment("z")["targets"]})
            self.assertEqual(graph, load_graph(directory))

    def test_invalid_fragment_ownership_schema_and_fields_fail(self):
        for change in ({"schema": 2}, {"extra": True},
                       {"targets": {"other": {"sources": ["a.c"]}}},
                       {"tests": {"a": {"target": "other"}}}):
            with self.subTest(change=change), tempfile.TemporaryDirectory() as tmp:
                path = Path(tmp)
                (path / "a.json").write_text(json.dumps(fragment("a") | change))
                with self.assertRaises(ValueError):
                    load_graph(path)

    def test_cmake_and_python_assemble_identical_documents(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory = Path(tmp)
            fragments = directory / "fragments"
            fragments.mkdir()
            for name in ("z", "a"):
                value = fragment(name)
                value["targets"][name]["definitions"] = [r'QUOTED="a;b\c"']
                (fragments / (name + ".json")).write_text(json.dumps(value))
            script = directory / "read.cmake"
            output = directory / "assembled.json"
            script.write_text(f'''include("{ROOT}/test/cmocka/native-test-graph-fragments.cmake")
obs_read_test_graph(graph "{fragments}")
file(WRITE "{output}" "${{graph}}")
''')
            result = subprocess.run(["cmake", "-P", str(script)], capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(json.loads(output.read_text()), load_graph(fragments))


if __name__ == "__main__":
    unittest.main()
