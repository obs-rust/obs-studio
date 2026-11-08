#!/usr/bin/env -S uv run --script
"""Cache ledger and baseline rollup. No network.

    uv run --no-project python -m unittest discover -s ci -p test_cache_ledger.py
"""

from __future__ import annotations

import contextlib
import io
import json
import re
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import cache_baseline
import cache_ledger

ROOT = Path(__file__).resolve().parent.parent

FIXTURE = ROOT / "ci" / "fixtures" / "soldr_cache_report_0.9.45.json"
WORKFLOW = ROOT / ".github" / "workflows" / "build-project.yaml"
ACTION = ROOT / ".github" / "actions" / "job-cache-ledger" / "action.yaml"

# Unedited lines from job logs of run 42339865660.
STATUS_LOG = """2026-11-07T04:52:49.1913645Z == zccache
2026-11-07T04:52:49.1969863Z   Compilations:  1833 total (1828 cached, 5 cold, 0 non-cacheable)
2026-11-07T04:52:49.1970348Z   Hit rate:      99.7%
"""
LAYER_LOG = """ soldr-mini-cache      HIT       soldr-mini-v2-linux-x64-glibc-v0.9.45           11.5MB           -         -     1.4s
 setup-cache           MISS      (no match)                                           -           -         -     0.1s
 cargo-registry-legacy-v1  HIT       …x64-7d46f1b3ea231b6d-a999cea964b13f7f          17.4MB      70.9MB      5047     9.1s
 3/4 exact-hit  3/4 any-hit  total restore: 15.4s
"""


def row(outcome="miss", ts="2026-11-07T00:00:00.000Z", key="k", latency=1, cwd="/w", session=None, **extra):
    return {"outcome": outcome, "ts": ts, "context_key": key, "latency_ns": latency, "cwd": cwd,
            "session_id": session, **extra}


def write_journal(cache: Path, name: str, lines: list) -> None:
    directory = cache / "history" / name
    directory.mkdir(parents=True)
    text = "\n".join(line if isinstance(line, str) else json.dumps(line) for line in lines)
    (directory / "compile_journal.jsonl").write_text(text + "\n", encoding="utf-8")


def record(**kwargs):
    base = {"meta": {"lane": "l"}, "wrapper": None, "counters": None, "source": "none", "versions": {},
            "layers": [], "changed_known": False}
    return cache_ledger.build_record(**(base | kwargs))


class DedupeTest(unittest.TestCase):
    def test_same_compile_in_two_history_dirs_counts_once(self):
        with tempfile.TemporaryDirectory() as tmp:
            cache = Path(tmp)
            write_journal(cache, "a", [row(miss_reason="context_not_found"), row("hit", key="other")])
            write_journal(cache, "b", [row(miss_reason="context_not_found")])
            rows, raw, malformed = cache_ledger.read_journals(cache)
        self.assertEqual((len(rows), raw, malformed), (2, 3, 0))

    def test_each_key_field_separates_rows(self):
        base = row()
        for change in ({"ts": "x"}, {"context_key": "x"}, {"latency_ns": 2}, {"cwd": "x"}, {"session_id": "x"}):
            self.assertNotEqual(cache_ledger.dedupe_key(base), cache_ledger.dedupe_key(base | change), change)

    def test_a_field_outside_the_key_does_not_separate_rows(self):
        self.assertEqual(cache_ledger.dedupe_key(row()), cache_ledger.dedupe_key(row(args=["x"])))

    def test_since_drops_restored_history_and_undated_rows(self):
        with tempfile.TemporaryDirectory() as tmp:
            cache = Path(tmp)
            write_journal(cache, "a", [row("hit", ts="2026-01-01T00:00:00Z"), row("hit", ts="2026-06-01T00:00:00Z", key="n"),
                                       {"outcome": "hit", "context_key": "undated"}])
            rows, _, _ = cache_ledger.read_journals(cache, cache_ledger.parse_ts("2026-03-01T00:00:00Z"))
        self.assertEqual([r["context_key"] for r in rows], ["n"])

    def test_miss_reasons_are_counted_after_dedupe(self):
        with tempfile.TemporaryDirectory() as tmp:
            cache = Path(tmp)
            for name in "ab":
                write_journal(cache, name, [row(miss_reason="context_not_found"), row(key="2", miss_reason="uncacheable_input")])
            rows, _, _ = cache_ledger.read_journals(cache)
            counts = cache_ledger.count_rows(rows, None, cache)
        self.assertEqual(counts["miss_reasons"], {"context_not_found": 1, "uncacheable_input": 1})
        self.assertEqual(counts["misses"], 2)


class EligibleRateTest(unittest.TestCase):
    def test_plain_rate(self):
        self.assertEqual(cache_ledger.eligible_hit_rate(3, 1), 0.75)

    def test_changed_input_misses_leave_the_denominator(self):
        self.assertEqual(cache_ledger.eligible_hit_rate(8, 4, 2), 0.8)

    def test_every_miss_changed_is_100_percent(self):
        self.assertEqual(cache_ledger.eligible_hit_rate(1, 2, 2), 1.0)

    def test_excess_changed_count_cannot_inflate_past_100(self):
        self.assertEqual(cache_ledger.eligible_hit_rate(1, 1, 9), 1.0)

    def test_nothing_eligible_is_none_not_zero(self):
        self.assertIsNone(cache_ledger.eligible_hit_rate(0, 0))
        self.assertIsNone(cache_ledger.eligible_hit_rate(0, 2, 2))

    def test_real_misses_with_no_hits_are_zero(self):
        self.assertEqual(cache_ledger.eligible_hit_rate(0, 2), 0.0)

    def test_malformed_counts_are_none(self):
        self.assertIsNone(cache_ledger.eligible_hit_rate("3", 1))
        self.assertIsNone(cache_ledger.eligible_hit_rate(True, 1))
        self.assertIsNone(cache_ledger.eligible_hit_rate(-1, 1))

    def test_miss_on_a_changed_source_is_excluded(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve()
            (root / "build").mkdir()
            rows = [
                row(key="a", cwd=str(root / "build"), args=["-c", "../libobs/a.c", "-o", "a.o"]),
                row(key="b", cwd=str(root / "build"), args=["-c", str(root / "libobs/abs.c")]),
                row(key="c", cwd=str(root), args=["-c", "libobs/same.c"]),
                row(key="d", cwd=str(root), miss_diff={"changed_inputs": ["libobs/hdr.h"]}),
                row("hit", key="e", cwd=str(root), args=["-c", "libobs/a.c"]),
            ]
            changed = {"libobs/a.c", "libobs/abs.c", "libobs/hdr.h"}
            counts = cache_ledger.count_rows(rows, changed, root)
        self.assertEqual((counts["hits"], counts["misses"], counts["misses_from_changed_inputs"]), (1, 4, 3))
        self.assertEqual(cache_ledger.eligible_hit_rate(counts["hits"], counts["misses"], 3), 0.5)

    def test_unknown_changed_paths_exclude_nothing(self):
        with tempfile.TemporaryDirectory() as tmp:
            counts = cache_ledger.count_rows([row(args=["a.c"], cwd=tmp)], None, Path(tmp))
        self.assertEqual(counts["misses_from_changed_inputs"], 0)


class BypassedTest(unittest.TestCase):
    def test_bypassed_with_no_compilations_is_na_never_zero(self):
        out = record(wrapper="none", counters=cache_ledger.count_rows([], None, Path(".")))
        self.assertEqual(out["wrapper_state"], "bypassed")
        self.assertIsNone(out["eligible_hit_rate"])
        self.assertEqual(out["eligible_hit_rate_display"], "n/a (bypassed)")
        self.assertNotIn("0", out["eligible_hit_rate_display"])

    def test_bypassed_with_no_data_is_na(self):
        self.assertEqual(record(wrapper="none")["eligible_hit_rate_display"], "n/a (bypassed)")

    def test_bypassed_c_lane_rate_is_scoped_to_non_rust(self):
        out = record(wrapper="none", counters=cache_ledger.counters_from_log(STATUS_LOG), source="job-log")
        self.assertEqual(out["rate_scope"], "non-rust")
        self.assertEqual(out["eligible_hit_rate_display"], "99.7% (non-Rust)")

    def test_active_without_data_is_not_called_bypassed(self):
        out = record(wrapper="zccache")
        self.assertEqual((out["wrapper_state"], out["eligible_hit_rate_display"]), ("active", "n/a (no data)"))

    def test_rollup_marks_bypassed_lane_na(self):
        rows = cache_baseline.rollup([record(wrapper="none", source="soldr-report", counters=cache_ledger.count_rows([], None, Path(".")))
                                      | {"event": "merge_group", "run_id": 1}])
        self.assertEqual(cache_baseline.rate_cell(rows[0]), "n/a (bypassed)")
        self.assertIn("bypassed", cache_baseline.table(rows))


class FailOpenTest(unittest.TestCase):
    def test_malformed_lines_are_counted_and_skipped(self):
        with tempfile.TemporaryDirectory() as tmp:
            write_journal(Path(tmp), "a", ["{not json", "[1]", row("hit")])
            rows, raw, malformed = cache_ledger.read_journals(Path(tmp))
        self.assertEqual((len(rows), raw, malformed), (1, 1, 2))

    def test_missing_cache_dir(self):
        self.assertEqual(cache_ledger.read_journals(Path("/nonexistent/x")), ([], 0, 0))
        self.assertEqual(cache_ledger.read_journals(None), ([], 0, 0))

    def test_report_shapes(self):
        for raw in ("null", "", "not json", "[]", '{"last_session": 3}', None):
            self.assertIsNone(cache_ledger.counters_from_report(raw)[0], raw)

    def test_report_with_wrong_typed_counters_reads_zero(self):
        counters, _ = cache_ledger.counters_from_report('{"last_session": {"hits": "many", "misses": 1}}')
        self.assertEqual((counters["hits"], counters["misses"]), (0, 1))

    def test_real_soldr_report(self):
        counters, versions = cache_ledger.counters_from_report(FIXTURE.read_text(encoding="utf-8"))
        self.assertEqual((counters["hits"], counters["compilations"]), (1, 1))
        self.assertEqual(versions, {"soldr": "0.9.45", "zccache": "1.15.5"})

    def test_phase_summary_shapes(self):
        for raw in (None, "", "not json", "[1]", "x" * 20000, '{"caches": 3}', '{"caches": [1, {"name": 2}]}'):
            self.assertEqual(cache_ledger.layers_from_phase_summary(raw)[0], [], raw)

    def test_unreadable_changed_paths_are_unknown(self):
        self.assertIsNone(cache_ledger.read_changed_paths(Path("/nonexistent/x")))

    def test_changed_paths_never_call_the_network_without_inputs(self):
        with mock.patch("urllib.request.urlopen", side_effect=AssertionError("network")):
            self.assertIsNone(cache_ledger.fetch_changed_paths("", "a", "b", None))
            self.assertIsNone(cache_ledger.fetch_changed_paths("o/r", "0" * 40, "b", None))

    def test_changed_paths_fetch_failure_and_truncation_are_unknown(self):
        with mock.patch("urllib.request.urlopen", side_effect=OSError("down")):
            self.assertIsNone(cache_ledger.fetch_changed_paths("o/r", "a", "b", None))
        full = io.BytesIO(json.dumps({"files": [{"filename": str(i)} for i in range(300)]}).encode())
        with mock.patch("urllib.request.urlopen", return_value=contextlib.nullcontext(full)):
            self.assertIsNone(cache_ledger.fetch_changed_paths("o/r", "a", "b", None))

    def run_main(self, tmp: Path, env: dict, *args: str) -> dict:
        output = tmp / "out" / "ledger.json"
        with mock.patch.dict("os.environ", env, clear=True), mock.patch.object(cache_ledger, "read_report", return_value="null"):
            self.assertEqual(cache_ledger.main(["--output", str(output), "--lane", "l", "--workspace", str(tmp), *args]), 0)
        return json.loads(output.read_text(encoding="utf-8"))

    def test_main_with_nothing_available_writes_a_record(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = self.run_main(Path(tmp), {"SOLDR_RUSTC_WRAPPER": "none"})
        self.assertEqual((out["counter_source"], out["eligible_hit_rate_display"]), ("none", "n/a (bypassed)"))

    def test_main_uses_the_journal_and_changed_paths(self):
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp).resolve()
            write_journal(base / "cache", "a", [row("hit", key="h", cwd=str(base)), row(key="m", cwd=str(base), args=["-c", "x.c"])])
            (base / "changed").write_text("x.c\n", encoding="utf-8")
            out = self.run_main(base, {"GITHUB_SHA": "abc", "GITHUB_EVENT_NAME": "merge_group"},
                                "--cache-dir", str(base / "cache"), "--changed-paths", str(base / "changed"))
        self.assertEqual((out["counter_source"], out["sha"], out["event"]), ("journal", "abc", "merge_group"))
        self.assertEqual((out["eligible_hit_rate"], out["changed_inputs_known"]), (1.0, True))

    def test_main_survives_an_internal_error(self):
        with tempfile.TemporaryDirectory() as tmp, mock.patch.object(cache_ledger, "collect", side_effect=RuntimeError):
            out = self.run_main(Path(tmp), {})
        self.assertEqual(out["eligible_hit_rate_display"], "n/a (ledger error)")

    def test_main_survives_an_unwritable_output(self):
        with tempfile.TemporaryDirectory() as tmp, contextlib.redirect_stderr(io.StringIO()):
            blocker = Path(tmp) / "file"
            blocker.write_text("", encoding="utf-8")
            with mock.patch.object(cache_ledger, "read_report", return_value="null"):
                self.assertEqual(cache_ledger.main(["--output", str(blocker / "x" / "y.json"), "--lane", "l"]), 0)

    def test_baseline_skips_bad_files(self):
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            (base / "bad.json").write_text("{nope", encoding="utf-8")
            (base / "list.json").write_text("[]", encoding="utf-8")
            (base / "ok.json").write_text(json.dumps({"lane": "l", "hits": 1, "misses": 0}), encoding="utf-8")
            records, skipped = cache_baseline.load_records(base)
        self.assertEqual((len(records), skipped), (1, 2))

    def test_rollup_tolerates_wrong_types(self):
        rows = cache_baseline.rollup([{"lane": "l", "hits": "x", "misses": None, "wall_seconds": "y"}])
        self.assertEqual(cache_baseline.rate_cell(rows[0]), "not measured")
        self.assertIn("| ? |", cache_baseline.table(rows))


class LogBackfillTest(unittest.TestCase):
    def test_status_lines(self):
        counters = cache_ledger.counters_from_log(STATUS_LOG)
        self.assertEqual((counters["compilations"], counters["hits"], counters["misses"]), (1833, 1828, 5))
        self.assertIsNone(cache_ledger.counters_from_log("nothing"))

    def test_layer_table(self):
        layers = cache_ledger.layers_from_log(LAYER_LOG)
        self.assertEqual([(l["layer"], l["status"], l["seconds"]) for l in layers], [
            ("soldr-mini-cache", "HIT", 1.4), ("setup-cache", "MISS", 0.1), ("cargo-registry-legacy-v1", "HIT", 9.1)])
        self.assertEqual(layers[1]["bytes"], None)
        self.assertEqual(layers[0]["bytes"], int(11.5 * 1024**2))

    def test_job_without_a_log_is_timing_only_and_not_measured(self):
        job = {"name": "Build 🧱 / X", "startedAt": "2026-01-01T00:00:00Z", "completedAt": "2026-01-01T00:01:00Z"}
        out = cache_baseline.record_from_log(job, {"event": "push", "databaseId": 1}, None)
        self.assertEqual((out["lane"], out["wall_seconds"], out["counter_source"]), ("X", 60.0, "timing-only"))
        self.assertEqual(cache_baseline.rate_cell(cache_baseline.rollup([out])[0]), "not measured")


class RollupTest(unittest.TestCase):
    def test_pooled_rate_and_n_counts_runs_not_jobs(self):
        members = [{"lane": "l", "event": "push", "run_id": r, "hits": h, "misses": m, "misses_from_changed_inputs": c,
                    "wall_seconds": w, "counter_source": "journal", "changed_inputs_known": True}
                   for r, h, m, c, w in [(1, 90, 10, 0, 60), (1, 0, 0, 0, 100), (2, 0, 10, 10, 140)]]
        row = cache_baseline.rollup(members)[0]
        self.assertEqual((row["n"], row["jobs"], row["rate"], row["wall"]), (2, 3, 0.9, 100))
        self.assertEqual(cache_baseline.rate_cell(row), "90.0%")

    def test_unknown_changed_paths_mark_a_lower_bound(self):
        row = cache_baseline.rollup([{"lane": "l", "hits": 1, "misses": 1, "counter_source": "job-log"}])[0]
        self.assertEqual(cache_baseline.rate_cell(row), "50.0% (≥)")

    def test_main_reports_the_true_sample(self):
        with tempfile.TemporaryDirectory() as tmp, contextlib.redirect_stdout(io.StringIO()) as out:
            (Path(tmp) / "a.json").write_text(json.dumps({"lane": "l", "run_id": 7, "hits": 1, "misses": 0}), encoding="utf-8")
            self.assertEqual(cache_baseline.main(["--ledger-dir", tmp]), 0)
        self.assertRegex(out.getvalue(), r"\| l \| \? \| 1 \|")
        self.assertIn("1 job records", out.getvalue())


class WorkflowTest(unittest.TestCase):
    def test_every_ledger_step_cannot_fail_its_job(self):
        text = WORKFLOW.read_text(encoding="utf-8")
        steps = re.findall(r"      - name: Cache ledger 📒\n((?:        .*\n)+)", text)
        self.assertEqual(len(steps), 4)
        for step in steps:
            self.assertIn("if: always()", step)
            # The gate rejects continue-on-error in its jobs: the action's own steps carry it.
            self.assertNotIn("continue-on-error", step)
            self.assertIn("uses: ./.github/actions/job-cache-ledger", step)

    def test_action_is_fail_open_and_runs_through_uv(self):
        text = ACTION.read_text(encoding="utf-8")
        self.assertEqual(text.count("continue-on-error: true"), 2)
        self.assertIn("uv run --no-project ci/cache_ledger.py", text)
        self.assertIn("cache ledger unavailable", text)

    def test_caching_stays_bypassed(self):
        text = WORKFLOW.read_text(encoding="utf-8")
        self.assertIn("SOLDR_RUSTC_WRAPPER: none", text)


if __name__ == "__main__":
    unittest.main()
