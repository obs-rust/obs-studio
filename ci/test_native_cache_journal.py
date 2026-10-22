"""Native miss diagnostics retain useful attribution without publishing arguments."""

import importlib.util
import json
import re
import subprocess
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("native_cache_journal", ROOT / "ci/native_cache_journal.py")
journal = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(journal)


class NativeCacheJournalTest(unittest.TestCase):
    def test_workflow_starts_after_restore_and_keeps_diagnostics_nonblocking(self):
        workflow = (ROOT / ".github/workflows/build-project.yaml").read_text()
        block = workflow.split("  ubuntu-build:\n", 1)[1].split("\n  flatpak-build:", 1)[0]
        selector = re.search(r"(?m)^    if: (.+)$", block).group(1)
        self.assertIn("fromJSON(needs.check-event.outputs.full)", selector)
        self.assertLess(block.index("id: ubuntu-compiler-cache"), block.index("id: native-cache-journal"))
        self.assertLess(block.index("- name: Set Up uv"), block.index("id: native-cache-journal"))
        self.assertLess(block.index("id: native-cache-journal"), block.index("- name: Build OBS Studio"))
        steps = re.split(r"(?m)^      - ", block)
        start = next(s for s in steps if "id: native-cache-journal" in s)
        summary = next(s for s in steps if "id: native-cache-summary" in s)
        build = next(s for s in steps if s.startswith("name: Build OBS Studio"))
        self.assertIn("continue-on-error: true", start)
        self.assertIn("continue-on-error: true", summary)
        self.assertIn("always()", summary)
        self.assertNotIn("continue-on-error", build, "Compiler integrity failures must remain fatal")

    def test_workflow_publishes_only_sanitized_json_not_raw_journals(self):
        workflow = (ROOT / ".github/workflows/build-project.yaml").read_text()
        block = workflow.split("  ubuntu-build:\n", 1)[1].split("\n  flatpak-build:", 1)[0]
        step = next(s for s in re.split(r"(?m)^      - ", block)
                    if s.startswith("name: Upload Native Cache Diagnostics"))
        self.assertIn("steps.native-cache-summary.outcome == 'success'", step)
        self.assertIn("continue-on-error: true", step)
        self.assertIn("retention-days: 2", step)
        self.assertIn("path: ${{ runner.temp }}/native-cache-${{ matrix.rust }}.json", step)
        self.assertNotIn(".jsonl", step)

    def test_source_and_reason_survive_without_arguments_or_diff_values(self):
        record = {"outcome": "miss", "compiler": "/private/sdk/bin/gcc", "latency_ns": 12,
                  "args": ["-include", "/repo/build/pch.h", "-DKEY=SECRET_TOKEN.c",
                           "-c", "/repo/libobs/build/core.c", "-o", "/repo/build/core.o"],
                  "miss_reason": "input_fingerprint_mismatch",
                  "miss_diff": {"env": {"KEY": "SECRET_TOKEN"}, "SECRET_TOKEN": "value"},
                  "env": {"KEY": "SECRET_TOKEN"}, "stderr": "SECRET_TOKEN"}
        result = journal.summarize(json.dumps(record), Path("/repo"))
        self.assertEqual(result["counts"], {"miss": 1})
        row = result["records"][0]
        self.assertEqual(row["source_candidates"], ["libobs/build/core.c"])
        self.assertEqual(row["miss_reason"], "input_fingerprint_mismatch")
        self.assertEqual(row["miss_diff_fields"], ["env"])
        self.assertEqual(row["latency_ns"], 12)
        self.assertNotIn("SECRET_TOKEN", json.dumps(result))
        self.assertNotIn("/private/sdk", json.dumps(result))
        self.assertNotIn("args", row)

    def test_malformed_or_unknown_records_are_visible_without_raw_values(self):
        text = '\n'.join(["SECRET_TOKEN not JSON", json.dumps({"outcome": "SECRET_TOKEN"}),
                          json.dumps({"outcome": "hit", "args": ["/elsewhere/private.c"]})])
        result = journal.summarize(text, Path("/repo"))
        self.assertEqual(result["counts"], {"unclassified": 1, "hit": 1})
        self.assertEqual(result["malformed_lines"], 1)
        self.assertEqual(result["status"], "partial")
        self.assertEqual(result["records"][1]["source_candidates"], [])
        self.assertNotIn("SECRET_TOKEN", json.dumps(result))
        self.assertNotIn("private.c", json.dumps(result))

    def test_empty_input_has_no_invented_compiler_counts(self):
        result = journal.summarize("", Path("/repo"))
        self.assertEqual(result["status"], "no_compile_records")
        self.assertEqual(result["counts"], {})

    def test_record_limit_marks_partial_evidence(self):
        result = journal.summarize('\n'.join([json.dumps({"outcome": "hit"})] * 3),
                                   Path("/repo"), max_records=2)
        self.assertEqual(len(result["records"]), 2)
        self.assertTrue(result["record_limit_reached"])
        self.assertEqual(result["status"], "partial")

    def test_start_requires_job_workspace_cache_and_exports_only_valid_session(self):
        session = "d8328209-b959-47e7-a750-e16a5b65f1c1"
        completed = subprocess.CompletedProcess([], 0, json.dumps({"session_id": session}), "")
        writer = mock.mock_open()
        with mock.patch.object(journal, "subprocess", create=True) as process, \
                mock.patch.object(Path, "open", writer):
            process.run.return_value = completed
            journal.start(Path("/tool/zccache"), Path("/repo"), Path("/repo/.zccache"),
                          Path("/runner/journal.jsonl"), Path("/runner/env"))
            args = process.run.call_args.args[0]
        self.assertIn("--stats", args)
        self.assertIn("--profile", args)
        self.assertEqual(args[args.index("--cache-dir") + 1], "/repo/.zccache")
        self.assertEqual(args[args.index("--endpoint") + 1], "/repo/.zccache/native-journal.sock")
        writer().write.assert_called_once_with(
            f"ZCCACHE_CACHE_DIR=/repo/.zccache\nZCCACHE_ENDPOINT=/repo/.zccache/native-journal.sock\n"
            f"ZCCACHE_SESSION_ID={session}\n")
        with self.assertRaises(ValueError):
            journal.start(Path("/tool/zccache"), Path("/repo"), Path("/shared/cache"),
                          Path("/runner/journal.jsonl"), Path("/runner/env"))

    def test_invalid_session_cannot_write_runner_environment(self):
        completed = subprocess.CompletedProcess([], 0, json.dumps({"session_id": "bad\nINJECT=value"}), "")
        writer = mock.mock_open()
        with mock.patch.object(journal.subprocess, "run", return_value=completed), \
                mock.patch.object(Path, "open", writer), self.assertRaises(ValueError):
            journal.start(Path("/tool/zccache"), Path("/repo"), Path("/repo/.zccache"),
                          Path("/runner/journal.jsonl"), Path("/runner/env"))
        writer.assert_not_called()


if __name__ == "__main__":
    unittest.main()
