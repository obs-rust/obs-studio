#!/usr/bin/env -S uv run --script
"""Deterministic generated-conflict recovery, without a model or GitHub."""
import contextlib
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parent.parent
REGISTRATION = "rust/obs-build/tests/native/registration/"
GENERATED = [REGISTRATION + name + "_expected.json" for name in
             ("headless_characterizations", "portal_output")]


def load(name):
    spec = importlib.util.spec_from_file_location(name, ROOT / "ci" / (name + ".py"))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


@contextlib.contextmanager
def fixture():
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        graph = json.loads((ROOT / "test/cmocka/build/test-graph.json").read_text())
        path = root / "test/cmocka/build/test-graph.json"
        path.parent.mkdir(parents=True)
        path.write_text(json.dumps(graph))
        for name in GENERATED:
            output = root / name
            output.parent.mkdir(parents=True, exist_ok=True)
            output.write_text("<<<<<<< HEAD\ninvalid JSON\n=======\n>>>>>>> PR\n")
        previous = Path.cwd()
        os.chdir(root)
        try:
            yield root, graph
        finally:
            os.chdir(previous)


class GeneratedConflicts(unittest.TestCase):
    def test_fixer_reconstructs_both_corrupt_snapshots_without_changing_graph(self):
        fixer = load("native_expectations")
        with fixture() as (root, graph):
            original = (root / "test/cmocka/build/test-graph.json").read_bytes()
            fixer.regenerate(root, GENERATED)
            for name in GENERATED:
                self.assertEqual(json.loads((root / name).read_text()),
                                 json.loads((ROOT / name).read_text()))
            self.assertEqual((root / "test/cmocka/build/test-graph.json").read_bytes(), original)
            # New link support is propagated; nested lists retain their order.
            graph["targets"]["test_service_state"]["sources"].append("new-support.c")
            (root / "test/cmocka/build/test-graph.json").write_text(json.dumps(graph))
            fixer.regenerate(root, GENERATED)
            actual = json.loads((root / GENERATED[0]).read_text())
            self.assertEqual(actual["targets"]["test_service_state"]["sources"][-1], "new-support.c")

    def test_missing_canonical_entry_refuses_before_writing(self):
        fixer = load("native_expectations")
        with fixture() as (root, graph):
            del graph["targets"]["test_service_state"]
            (root / "test/cmocka/build/test-graph.json").write_text(json.dumps(graph))
            before = [(root / name).read_bytes() for name in GENERATED]
            with self.assertRaises((KeyError, ValueError)):
                fixer.regenerate(root, GENERATED)
            self.assertEqual([(root / name).read_bytes() for name in GENERATED], before)

    def test_generated_only_conflicts_never_call_ai(self):
        bot = load("ai_rebase")
        with fixture() as (root, _), \
                patch.object(bot, "stage", return_value=""), \
                patch.object(bot, "clud_cmd", side_effect=AssertionError("no AI")), \
                patch.object(bot, "run", return_value="") as run:
            self.assertIsNone(bot.resolve_step(GENERATED, "add independent link support"))
            for name in GENERATED:
                self.assertEqual(json.loads((root / name).read_text()),
                                 json.loads((ROOT / name).read_text()))
            self.assertTrue(any(call.args[:3] == ("git", "add", "--") for call in run.call_args_list))

    def test_canonical_conflict_is_resolved_before_snapshots(self):
        bot = load("ai_rebase")
        with fixture() as (root, graph):
            path = root / "test/cmocka/build/test-graph.json"
            path.write_text("<<<<<<< unresolved graph\n")

            def resolve(name, _subject):
                self.assertEqual(name, "test/cmocka/build/test-graph.json")
                graph["targets"]["test_service_state"]["sources"].append("both-ports.c")
                path.write_text(json.dumps(graph))

            with patch.object(bot, "stage", return_value=""), \
                    patch.object(bot, "clud_cmd", return_value=None), \
                    patch.object(bot, "resolve", side_effect=resolve), \
                    patch.object(bot, "run", return_value=""):
                self.assertIsNone(bot.resolve_step([*GENERATED, str(path.relative_to(root))], "port"))
            self.assertEqual(json.loads((root / GENERATED[0]).read_text())
                             ["targets"]["test_service_state"]["sources"][-1], "both-ports.c")

    def test_invalid_graph_leaves_generated_conflicts_unstaged(self):
        bot = load("ai_rebase")
        with fixture() as (root, _), \
                patch.object(bot, "stage", return_value=""), \
                patch.object(bot, "clud_cmd", return_value=None), \
                patch.object(bot, "run", return_value="") as run:
            (root / "test/cmocka/build/test-graph.json").write_text("<<<<<<< invalid\n")
            self.assertIn("regenerat", bot.resolve_step(GENERATED, "port").lower())
            self.assertFalse(any(call.args[:2] == ("git", "add") for call in run.call_args_list))


class OracleFragments(unittest.TestCase):
    def test_oracle_has_independent_extraction_entries(self):
        directory = ROOT / "rust/obs-c-oracle/oracle.d"
        fragments = sorted(directory.glob("*.list"))
        self.assertGreater(len(fragments), 40, "migrate per-port build.rs extraction registrations")
        outputs = []
        for path in fragments:
            entries = [line for line in path.read_text().splitlines() if line and not line.startswith("#")]
            self.assertEqual(len(entries), 1, str(path))
            output, source, _pch = entries[0].split("|")
            self.assertTrue((ROOT / source).is_file(), source)
            outputs.append(output)
        self.assertEqual(len(outputs), len(set(outputs)))
        build = (ROOT / "rust/obs-c-oracle/build.rs").read_text()
        self.assertIn("oracle.d", build)
        self.assertNotIn('extract_dir.join("graphics-stencil.inc")', build)


if __name__ == "__main__":
    unittest.main()
