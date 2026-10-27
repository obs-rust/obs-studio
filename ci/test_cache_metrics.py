#!/usr/bin/env -S uv run --script
"""Evidence coverage, actual-diff cohorts and CI duration arithmetic."""
import importlib.util
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parent.parent
SPEC = importlib.util.spec_from_file_location("cache_metrics", ROOT / "ci/cache_metrics.py")
metrics = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(metrics)


class CacheMetricsTest(unittest.TestCase):
    def test_rust_test_phases_handle_soldr_prefixes_and_ansi(self):
        log = "\n".join([
            "Gate\tTests\t2026-01-01T00:00:00Z + soldr cargo test --workspace --locked",
            "Gate\tTests\t2026-01-01T00:00:35Z 35.0 \x1b[92mFinished\x1b[0m `test` profile [unoptimized] target(s) in 34.50s",
            "Gate\tTests\t2026-01-01T00:00:36Z 36.0 ^[[92mRunning^[[0m tests/core.rs (target/debug/deps/core)",
            "Gate\tTests\t2026-01-01T00:01:00Z test result: ok. 4 passed; 0 failed; finished in 24s",
            "Gate\tTests\t2026-01-01T00:01:36Z test result: ok. 9 passed; 0 failed; finished in 36s",
        ])
        report = metrics.parse_rust_test_logs(log)["Gate"][0]
        self.assertEqual(report["cargo_build_and_link_seconds"], 34.5)
        self.assertEqual(report["command_to_first_test_seconds"], 36)
        self.assertEqual(report["observed_test_output_seconds"], 60)
        self.assertEqual(report["result_summaries"], 2)

    def test_no_run_and_missing_rust_phase_markers_are_not_execution_samples(self):
        log = "\n".join([
            "Gate\tTests\t2026-01-01T00:00:00Z + soldr cargo test --workspace --no-run",
            "Gate\tTests\t2026-01-01T00:00:30Z Finished `test` profile [unoptimized] target(s) in 30s",
        ])
        self.assertEqual(metrics.parse_rust_test_logs(log), {})
        report = metrics.parse_rust_test_logs(
            "Gate\tTests\t2026-01-01T00:00:00Z + soldr cargo test --workspace"
        )["Gate"][0]
        self.assertIsNone(report["cargo_build_and_link_seconds"])
        self.assertIsNone(report["observed_test_output_seconds"])
        self.assertEqual(report["result_summaries"], 0)

    def test_nextest_summary_is_separate_from_compilation_and_cargo_doctests(self):
        log = "\n".join([
            "Gate\tTests\t2026-01-01T00:00:00Z + soldr cargo nextest run --workspace --locked",
            "Gate\tTests\t2026-01-01T00:00:31Z 31.0 ^[[92mFinished^[[0m `test` profile [unoptimized] target(s) in 30.50s",
            "Gate\tTests\t2026-01-01T00:00:32Z 32.0 ^[[32mStarting^[[0m 2020 tests across 8 binaries (1 test skipped)",
            "Gate\tTests\t2026-01-01T00:01:44Z 104.0 ^[[32mSummary^[[0m [ 72.242s] 2020 tests run: 2020 passed, 1 skipped",
            "Gate\tTests\t2026-01-01T00:01:45Z + soldr cargo test --workspace --locked --doc",
            "Gate\tTests\t2026-01-01T00:01:46Z Doc-tests obs_audio_io",
            "Gate\tTests\t2026-01-01T00:01:47Z test result: ok. 2 passed; 0 failed; 0 ignored; finished in 0.03s",
        ])
        execution, docs = metrics.parse_rust_test_logs(log)['Gate']
        self.assertEqual(execution['runner'], 'nextest')
        self.assertEqual(execution['cargo_build_and_link_seconds'], 30.5)
        self.assertEqual(execution['command_to_first_test_seconds'], 32)
        self.assertEqual(execution['observed_test_output_seconds'], 72)
        self.assertEqual(execution['nextest_reported_execution_seconds'], 72.242)
        self.assertEqual(execution['nextest_summaries'][0]['tests_run'], 2020)
        self.assertEqual(execution['nextest_summaries'][0]['counts'], {'passed': 2020, 'skipped': 1})
        self.assertEqual(docs['runner'], 'cargo-doctests')
        self.assertEqual(docs['cargo_results'][0]['passed'], 2)
        self.assertEqual(docs['observed_test_output_seconds'], 1)

    def test_interrupted_nextest_and_other_commands_do_not_invent_success(self):
        log = "\n".join([
            "Gate\tTests\t2026-01-01T00:00:00Z + soldr cargo nextest run --workspace",
            "Gate\tTests\t2026-01-01T00:00:01Z Starting 2000 tests across 8 binaries",
            "Gate\tTests\t2026-01-01T00:00:02Z + soldr cargo check --workspace",
            "Gate\tTests\t2026-01-01T00:00:03Z Summary [2.0s] 2000 tests run: 2000 passed",
        ])
        report = metrics.parse_rust_test_logs(log)['Gate'][0]
        self.assertIsNone(report['nextest_reported_execution_seconds'])
        self.assertEqual(report['nextest_summaries'], [])
        self.assertIsNone(report['observed_test_output_seconds'])

    def test_nextest_failed_partial_execution_retains_explicit_counts_only(self):
        log = "\n".join([
            "Gate\tTests\t2026-01-01T00:00:00Z + soldr cargo nextest run --workspace",
            "Gate\tTests\t2026-01-01T00:00:03Z Summary [1.25s] 4/10 tests run: 3 passed, 1 failed, 6 not run, 1 skipped",
        ])
        report = metrics.parse_rust_test_logs(log)['Gate'][0]
        self.assertEqual(report['nextest_summaries'][0]['tests_run'], 4)
        self.assertEqual(report['nextest_summaries'][0]['counts'], {'passed': 3, 'failed': 1, 'not run': 6, 'skipped': 1})
        self.assertNotIn('failed', metrics.parse_rust_test_logs(
            "Gate\tTests\t2026-01-01T00:00:00Z + soldr cargo nextest run --workspace\n"
            "Gate\tTests\t2026-01-01T00:00:03Z Summary [1.25s] 4 tests run: 4 passed"
        )['Gate'][0]['nextest_summaries'][0]['counts'])

    def test_dispatch_time_uses_job_creation_not_run_creation(self):
        job = {"created_at": "2026-10-21T00:26:13Z", "started_at": "2026-10-21T00:28:29Z"}
        self.assertEqual(metrics.creation_to_start_seconds(job), 136)
        self.assertIsNone(metrics.creation_to_start_seconds({"started_at": job["started_at"]}))
        self.assertIsNone(metrics.creation_to_start_seconds({**job, "conclusion": "skipped"}))
        self.assertIsNone(metrics.creation_to_start_seconds({**job, "started_at": "2026-10-21T00:26:12Z"}))

    def test_lane_tail_and_creation_to_start_samples_keep_unknowns_out(self):
        jobs = [
            {"name": "Gate", "conclusion": "success", "created_at": "2026-01-01T00:00:00Z",
             "started_at": "2026-01-01T00:00:10Z", "completed_at": "2026-01-01T00:00:30Z"},
            {"name": "Gate", "conclusion": "success", "started_at": "2026-01-01T00:01:00Z",
             "completed_at": "2026-01-01T00:02:00Z"},
        ]
        record = {"event": "merge_group", "cohort": "code", "conclusion": "success", "jobs": jobs}
        lane = metrics.lane_summaries([record])["merge_group/code/Gate"]
        self.assertEqual(lane["median_job_seconds"], 40)
        self.assertEqual(lane["p95_job_seconds"], 60)
        self.assertEqual(lane["creation_to_start_samples"], 1)
        self.assertEqual(lane["median_creation_to_start_seconds"], 10)
        self.assertEqual(lane["p95_creation_to_start_seconds"], 10)
        self.assertIsNone(metrics.p95([]))

    def test_archive_restore_is_distinct_from_compiler_hits(self):
        log = "\n".join([
            "Tier2\tRestore cache\t2026-10-17T18:39:59Z Cache Size: ~266 MB (278493433 B)",
            "Tier2\tBuild\t2026-10-17T18:46:22Z Compilations: 772 total (0 cached, 772 cold, 0 non-cacheable)",
            "Gate\tPost setup\t2026-10-17T18:47:01Z final zccache session stats: hits=0 misses=73 compilations=75 non_cacheable=0 errors=2",
            "Qt\tReport cache\t2026-10-17T18:47:02Z Hits: 304 / 304 (100.0%)",
            "Tests ON\tTest\t2026-10-17T18:47:03Z 100% tests passed, 0 tests failed out of 283",
            "Tier2\tSave\t2026-10-17T18:47:04Z Cache saved with key: zccache-tier2-linux-both-abc",
        ])
        parsed = metrics.parse_cache_logs(log)
        self.assertEqual(parsed["Tier2"]["archive_bytes"], [278493433])
        self.assertEqual(parsed["Tier2"]["compiler_reports"][0]["cached"], 0)
        self.assertEqual(parsed["Tier2"]["compiler_reports"][0]["cold"], 772)
        self.assertEqual(parsed["Gate"]["compiler_reports"][0]["errors"], 2)
        self.assertEqual(parsed["Qt"]["compiler_reports"][0]["hits"], 304)
        self.assertEqual(parsed["Tests ON"]["ctest_reports"], [{"total": 283, "failed": 0}])
        self.assertEqual(parsed["Tier2"]["saved_keys"], ["zccache-tier2-linux-both-abc"])

    def test_missing_log_counters_are_not_invented(self):
        self.assertEqual(metrics.parse_cache_logs("Build\tstep\t2026-10-17T18:00:00Z Cache restored successfully"), {})

    def test_restored_flatpak_counters_are_not_job_deltas(self):
        name = "Build 🧱 / Flatpak 📦"
        log = "\n".join([
            f"{name}\tCcache Stats\t2026-10-18T02:38:56Z   Hits: 602 / 889 (67.72%)",
            "Qt\tCcache Stats\t2026-10-18T02:38:56Z   Hits: 343 / 345 (99.42%)",
        ])
        parsed = metrics.parse_cache_logs(log)
        flatpak = parsed[name]["compiler_reports"][0]
        self.assertEqual(flatpak["hits"], 602, "Keep the observed cumulative count")
        self.assertEqual(flatpak["cacheable"], 889)
        self.assertEqual(flatpak.get("counter_scope"), "restored-cumulative")
        self.assertIs(flatpak.get("job_delta_available"), False,
                      "No baseline exists after the action's final restore")
        self.assertEqual(parsed["Qt"]["compiler_reports"][0]["hits"], 343)
        self.assertNotEqual(parsed["Qt"]["compiler_reports"][0].get("counter_scope"),
                            "restored-cumulative", "Do not generalize Flatpak's counter behavior")

    def test_current_ctest_success_preserves_failures_and_unknown_counts(self):
        log = "\n".join([
            "Qt\tBuild frontend\t2026-10-18T01:29:45Z 100% tests passed out of 15",
            "OFF\tTest\t2026-10-18T01:19:08Z 99% tests passed, 1 tests failed out of 295",
            "Unknown\tTest\t2026-10-18T01:29:45Z Test project /build",
        ])
        parsed = metrics.parse_cache_logs(log)
        self.assertIn("Qt", parsed, "Capture the observed shorter success summary")
        self.assertEqual(parsed["Qt"]["ctest_reports"], [{"total": 15, "failed": 0}])
        self.assertEqual(parsed["OFF"]["ctest_reports"], [{"total": 295, "failed": 1}])
        self.assertNotIn("Unknown", parsed)

    def test_source_cohort_requires_files(self):
        self.assertEqual(metrics.cohort([]), "unknown")
        self.assertEqual(metrics.cohort(["docs/a.md", "AGENTS.md"]), "docs")
        self.assertEqual(metrics.cohort(["ci/test_a.py", ".github/workflows/ci.yml"]), "ci-config")
        self.assertEqual(metrics.cohort(["docs/a.md", "libobs/obs.c"]), "code")

    def test_structured_report_evidence_is_not_product_code(self):
        evidence = "docs/ci-evidence/2026-10-17-results.json"
        self.assertEqual(metrics.cohort(["docs/report.md", evidence]), "docs")
        self.assertEqual(metrics.cohort(["ci/cache_metrics.py", evidence]), "ci-config")
        self.assertEqual(metrics.cohort(["libobs/obs.c", evidence]), "code")
        self.assertEqual(metrics.cohort(["plugins/service/data/config.json"]), "code")

    def test_report_figures_and_generators_stay_in_documentation_cohort(self):
        evidence = ["docs/ci-evidence/libobs-2026-10-17/build-trials.svg",
                    "docs/ci-evidence/libobs-2026-10-17/plot_trials.py"]
        self.assertEqual(metrics.cohort(evidence), "docs")
        self.assertEqual(metrics.cohort(evidence + ["ci/cache_metrics.py"]), "ci-config")
        self.assertEqual(metrics.cohort(evidence + ["libobs/obs.c.h"]), "code")
        self.assertEqual(metrics.cohort(["scripts/plot_trials.py"]), "code")

    def collect(self, missing_compare=False):
        run = {"databaseId": 1, "status": "completed", "conclusion": "success", "event": "pull_request",
               "headSha": "b" * 40, "headBranch": "docs/one", "createdAt": "2026-01-01T08:00:00Z",
               "updatedAt": "2026-01-01T08:02:00Z", "url": "https://example.invalid/run/1"}

        def fake_gh(*args):
            if args[:2] == ("run", "list"):
                return [run] if args[args.index("--workflow") + 1] == "ci.yml" else []
            if args[:2] == ("pr", "list"):
                return [{"headRefName": "docs/one", "headRefOid": "b" * 40, "baseRefOid": "a" * 40,
                         "createdAt": "2026-01-01T07:00:00Z", "closedAt": "2026-01-01T09:00:00Z"}]
            endpoint = args[1]
            if endpoint.endswith("/actions/runs/1"):
                return {"run_attempt": 1, "pull_requests": []}
            if "/jobs?" in endpoint:
                return [{"jobs": [{"name": "CI Gate ✅", "conclusion": "success", "steps": [],
                                   "started_at": "2026-01-01T08:01:00Z", "completed_at": "2026-01-01T08:02:00Z"}]}]
            if "/compare/" in endpoint:
                if missing_compare:
                    raise subprocess.CalledProcessError(1, ["gh", "api", endpoint])
                return {"files": [{"filename": "docs/a.md"}]}
            if "/actions/caches?per_page=100" in endpoint:
                self.assertIn("--paginate", args)
                self.assertIn("--slurp", args)
                return [{"actions_caches": []}]
            raise AssertionError(args)

        scratch = ROOT / "target"
        scratch.mkdir(exist_ok=True)
        with tempfile.TemporaryDirectory(dir=scratch) as directory, mock.patch.object(metrics, "gh", side_effect=fake_gh):
            return metrics.collect("test/repo", "2026-01-01T00:00:00+00:00", "2026-01-02T00:00:00Z", Path(directory), 12)

    def test_cached_run_preserves_its_historical_comparison_base(self):
        import json
        run = {'databaseId': 1, 'status': 'completed', 'conclusion': 'success',
               'event': 'pull_request', 'headSha': 'b' * 40, 'headBranch': 'fix/one',
               'createdAt': '2026-01-01T08:00:00Z', 'updatedAt': '2026-01-01T08:02:00Z'}
        cached = {**run, 'jobs': [], 'run_attempt': 1, 'comparison_base': 'a' * 40,
                  'comparison_error': None, 'changed_files': ['libobs/obs.c']}

        def fake_gh(*args):
            if args[:2] == ('run', 'list'):
                return [run] if args[args.index('--workflow') + 1] == 'ci.yml' else []
            if args[:2] == ('pr', 'list'):
                return [{'headRefName': 'fix/one', 'headRefOid': 'b' * 40, 'baseRefOid': 'c' * 40,
                         'createdAt': '2026-01-01T07:00:00Z', 'closedAt': None}]
            if '/actions/caches?per_page=100' in args[1]:
                self.assertIn('--paginate', args)
                self.assertIn('--slurp', args)
                return [{'actions_caches': []}]
            raise AssertionError('An unchanged cached run must not be reclassified against a later base: ' + str(args))

        with tempfile.TemporaryDirectory() as directory, mock.patch.object(metrics, 'gh', side_effect=fake_gh):
            output = Path(directory)
            (output / 'run-1.json').write_text(json.dumps(cached))
            report = metrics.collect('test/repo', '2026-01-01T00:00:00Z', '2026-01-02T00:00:00Z', output, 12)
            self.assertIn('pull_request/code', report['cohorts'])
            self.assertEqual(json.loads((output / 'run-1.json').read_text())['comparison_base'], 'a' * 40)

    def test_resolves_old_pr_diff_and_deduplicates_docs_candidates(self):
        report = self.collect()
        self.assertEqual(report["listed_runs"], 1)
        self.assertEqual(report["sampled_runs"], 1)
        self.assertEqual(report["cohorts"]["pull_request/docs"]["median_successful_gate_seconds"], 120)
        self.assertEqual(report["lanes"]["pull_request/CI Gate ✅"]["median_job_seconds"], 60)
        self.assertTrue(report["listing_coverage"]["ci.yml"]["window_covered"])

    def test_missing_compare_is_unknown_even_on_a_docs_branch(self):
        report = self.collect(missing_compare=True)
        self.assertIn("pull_request/unknown", report["cohorts"])
        self.assertNotIn("pull_request/docs", report["cohorts"])


if __name__ == "__main__":
    unittest.main()
