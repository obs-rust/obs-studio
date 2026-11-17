#!/usr/bin/env -S uv run --script
"""Run-only Qt bundle regressions; no SDK download in self-tests."""
import json
import tempfile
import unittest
from pathlib import Path

import macos_qt_artifact as qt


class QtArtifactTest(unittest.TestCase):
    def test_ctest_records_preserve_runtime_contract(self):
        inventory = {"tests": [{"name": "answer", "command": ["/work/answer", "42"],
                                "properties": [{"name": "WORKING_DIRECTORY", "value": "/work"},
                                               {"name": "ENVIRONMENT", "value": ["QT_QPA_PLATFORM=minimal"]},
                                               {"name": "TIMEOUT", "value": 60.0}]}]}
        entries = qt.ctest_entries(inventory)
        self.assertEqual(entries, [{"name": "answer", "binary": "/work/answer", "args": ["42"],
                                    "cwd": "/work", "env": {"QT_QPA_PLATFORM": "minimal"}, "timeout": 60.0}])
        inventory["tests"][0]["properties"].append({"name": "WILL_FAIL", "value": True})
        with self.assertRaisesRegex(ValueError, "unsupported execution property"):
            qt.ctest_entries(inventory)

    def test_incomplete_contract_inventory_fails_closed(self):
        with tempfile.TemporaryDirectory() as temporary:
            work = Path(temporary)
            (work / "records.jsonl").write_text(json.dumps({"contract": qt.CONTRACTS[0], "binary": "/missing"}) + "\n")
            with self.assertRaisesRegex(ValueError, "incomplete Qt contracts"):
                qt.read_records(work)

    def test_runtime_path_translation_rejects_unmapped_producer_paths(self):
        mappings = {"/work": "{root}/target/macos-qt", "/checkout": "{root}"}
        self.assertEqual(qt.portable("/work/answer", mappings), "{root}/target/macos-qt/answer")
        self.assertEqual(qt.portable("/checkout/frontend", mappings), "{root}/frontend")
        with self.assertRaisesRegex(ValueError, "unmapped producer path"):
            qt.portable("/tmp/other/answer", mappings)


if __name__ == "__main__":
    unittest.main()
