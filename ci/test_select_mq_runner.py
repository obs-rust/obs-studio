#!/usr/bin/env -S uv run --script
"""Merge queue runner availability and fail-open hosted selection (no network)."""

import io
import itertools
import json
import os
import tempfile
import unittest
from contextlib import ExitStack
from urllib.parse import parse_qs, urlparse
from contextlib import redirect_stdout
from pathlib import Path
from unittest.mock import patch
from urllib.error import HTTPError, URLError

import select_mq_runner as selector

LABELS = '["self-hosted","linux","x64","obs-mq"]'
RUNNER_IDS = itertools.count(1)


def runner(status="online", *, busy=False, labels=None):
    return {
        "id": next(RUNNER_IDS),
        "status": status,
        "busy": busy,
        "labels": [
            {"name": name}
            for name in (json.loads(LABELS) if labels is None else labels)
        ],
    }


class SelectMqRunnerTest(unittest.TestCase):
    def setUp(self):
        # These cases isolate the runner census; admission is exercised below.
        admission = patch.object(selector, "inflight_commitments", return_value=(0, 0))
        admission.start()
        self.addCleanup(admission.stop)
        self.enterContext(patch.object(selector.time, "sleep"))

    def select(
        self,
        runners=(),
        *,
        event="merge_group",
        labels=LABELS,
        token="fake-token",
        error=None,
        org_runners=(),
        org_error=None,
        min_idle=None,
        demand=None,
        mode=None,
    ):
        def respond(request, **kwargs):
            is_org = "/orgs/" in request.full_url
            failure = org_error if is_org else error
            if failure is not None:
                raise failure
            entries = org_runners if is_org else runners
            return io.BytesIO(
                json.dumps(
                    {"total_count": len(entries), "runners": list(entries)}
                ).encode()
            )

        with patch.object(selector, "urlopen", side_effect=respond) as http:
            with redirect_stdout(io.StringIO()) as output:
                options = {}
                if min_idle is not None:
                    options["min_idle"] = min_idle
                if demand is not None:
                    options["demand"] = demand
                if mode is not None:
                    options["mode"] = mode
                result = selector.select_labels(
                    event, labels, token, "obs-rust/obs-studio", **options
                )
        return result, http, output.getvalue()

    def test_online_all_labels(self):
        result, http, _ = self.select(
            [runner(labels=[*json.loads(LABELS), "extra"]), runner()]
        )
        self.assertEqual(json.loads(result), json.loads(LABELS))
        request = http.call_args_list[0].args[0]
        self.assertEqual(
            request.full_url,
            "https://api.github.com/repos/obs-rust/obs-studio/actions/runners?per_page=100&page=1",
        )
        self.assertEqual(request.get_header("Authorization"), "Bearer fake-token")
        self.assertEqual(http.call_args.kwargs["timeout"], 10)

    def test_all_six_online_runners_busy_with_no_commitments_use_dell(self):
        result, _, output = self.select([runner(busy=True) for _ in range(6)])
        self.assertEqual(result, LABELS)
        self.assertIn("idle=0", output)
        self.assertIn("busy=6", output)

    def test_prefer_busy_runners_queue_below_run_cap(self):
        entries = [runner(busy=True) for _ in range(6)]
        for scope in ("repository", "organization"):
            with self.subTest(scope=scope):
                options = {
                    "runners" if scope == "repository" else "org_runners": entries
                }
                self.assertEqual(self.select(mode="prefer", **options)[0], LABELS)

    def test_prefer_all_offline_uses_hosted(self):
        result, _, output = self.select([runner("offline", busy=True)], mode="prefer")
        self.assertEqual(result, "")
        self.assertIn("online=0", output)

    def test_prefer_probe_failure_uses_hosted(self):
        error = URLError("fake-token")
        result, _, output = self.select(mode="prefer", error=error, org_error=error)
        self.assertEqual(result, "")
        self.assertIn("availability probe failed", output)
        self.assertNotIn("fake-token", output)

    def test_prefer_preserves_available_scope_on_probe_failure(self):
        self.assertEqual(
            self.select([runner()], mode="prefer", org_error=URLError("down"))[0],
            LABELS,
        )

    def test_prefer_missing_token_uses_hosted_without_api(self):
        result, http, _ = self.select([runner(busy=True)], mode="prefer", token="")
        self.assertEqual(result, "")
        http.assert_not_called()

    def test_empty_mode_allows_busy_but_requires_online_capacity(self):
        self.assertEqual(self.select([runner(busy=True)], mode="")[0], LABELS)
        self.assertEqual(self.select([runner()], mode="")[0], LABELS)
        self.assertEqual(self.select([runner()], mode="", min_idle="2")[0], "")

    def test_prefer_respects_minimum_and_per_run_demand(self):
        for mode in ("", "prefer"):
            for idle in range(7):
                with self.subTest(mode=mode, idle=idle):
                    entries = [runner() for _ in range(idle)]
                    entries += [runner(busy=True) for _ in range(6 - idle)]
                    self.assertEqual(
                        self.select(entries, mode=mode, demand="6")[0],
                        LABELS,
                    )
            self.assertEqual(self.select([runner()], mode=mode, min_idle="2")[0], "")
            for option in ("min_idle", "demand"):
                result, http, _ = self.select([runner()], mode=mode, **{option: "bad"})
                self.assertEqual(result, "")
                http.assert_not_called()

    def test_unknown_mode_uses_hosted_without_api(self):
        result, http, _ = self.select([runner()], mode="typo")
        self.assertEqual(result, "")
        http.assert_not_called()

    def test_default_requires_one_online_runner(self):
        self.assertEqual(self.select([runner()])[0], LABELS)
        self.assertEqual(self.select([runner(), runner()])[0], LABELS)

    def test_five_busy_runners_do_not_strand_the_last_idle_runner(self):
        entries = [runner(busy=True) for _ in range(5)]
        self.assertEqual(self.select([*entries, runner()])[0], LABELS)
        self.assertEqual(self.select([*entries, runner("offline")])[0], LABELS)
        self.assertEqual(self.select([*entries, runner()], demand="2")[0], LABELS)

    def test_empty_actions_thresholds_use_one_online_runner(self):
        self.assertEqual(self.select([runner()], min_idle="", demand="")[0], LABELS)

    def test_only_matching_online_runners_supply_capacity(self):
        entries = [
            runner(),
            runner(busy=True),
            runner("offline"),
            runner(labels=["x64"]),
        ]
        self.assertEqual(self.select(entries, min_idle="2")[0], LABELS)
        self.assertEqual(self.select([*entries, runner()], min_idle="2")[0], LABELS)

    def test_configurable_min_idle_and_demand(self):
        entries = [runner(), runner(), runner()]
        self.assertEqual(self.select(entries, min_idle="4")[0], "")
        self.assertEqual(self.select(entries, min_idle="3")[0], LABELS)
        self.assertEqual(self.select(entries, min_idle="1", demand="4")[0], "")
        self.assertEqual(self.select(entries, min_idle="1", demand="3")[0], LABELS)
        self.assertEqual(self.select(entries[:1], min_idle="1")[0], LABELS)
        self.assertEqual(self.select(entries[:1], min_idle="2", demand="1")[0], "")

    def test_invalid_capacity_configuration_uses_hosted_without_api(self):
        for option in ("min_idle", "demand"):
            for value in ("0", "-1", "oops", "1.5"):
                with self.subTest(option=option, value=value):
                    result, http, output = self.select(
                        [runner(), runner()], **{option: value}
                    )
                    self.assertEqual(result, "")
                    http.assert_not_called()
                    self.assertIn("::warning::", output)
        self.assertEqual(
            self.select([runner(), runner()], min_idle="", demand="")[0], LABELS
        )

    def test_same_runner_in_both_scopes_counts_once(self):
        same = runner()
        self.assertEqual(self.select([same], org_runners=[same], min_idle="2")[0], "")
        self.assertEqual(
            self.select([same], org_runners=[same, runner()], min_idle="2")[0], LABELS
        )

    def test_disagreeing_busy_state_is_conservative(self):
        same = runner()
        selected, _, output = self.select(
            [same, runner()], org_runners=[{**same, "busy": True}], min_idle="2"
        )
        self.assertEqual(selected, LABELS)
        self.assertIn("online=2, busy=1, idle=1", output)
        self.assertIn("admission=busy queue", output)

    def test_unknown_busy_state_is_not_idle(self):
        for busy in (None, "false", 0):
            with self.subTest(busy=busy):
                unknown = {**runner(), "busy": busy}
                self.assertEqual(self.select([unknown, runner()])[0], "")
        unknown = runner()
        del unknown["busy"]
        self.assertEqual(self.select([unknown, runner()])[0], "")

    def test_label_matching_is_case_insensitive(self):
        self.assertEqual(
            json.loads(
                self.select(
                    [
                        runner(labels=[label.upper() for label in json.loads(LABELS)]),
                        runner(),
                    ]
                )[0]
            ),
            json.loads(LABELS),
        )

    def test_offline(self):
        result, _, output = self.select([runner("offline")])
        self.assertEqual(result, "")
        self.assertIn("::warning::", output)

    def test_missing_token(self):
        result, http, output = self.select([runner()], token="")
        self.assertEqual(result, "")
        http.assert_not_called()
        self.assertIn("::warning::", output)

    def test_partial_label_match(self):
        self.assertEqual(
            self.select([runner(labels=["self-hosted", "linux", "x64"])])[0], ""
        )

    def test_labels_cannot_be_combined_across_runners(self):
        self.assertEqual(
            self.select(
                [
                    runner(labels=["self-hosted", "linux"]),
                    runner(labels=["x64", "obs-mq"]),
                ]
            )[0],
            "",
        )

    def test_api_error(self):
        result, _, output = self.select(error=URLError("fake-token must not be logged"))
        self.assertEqual(result, "")
        self.assertIn("::warning::", output)
        self.assertNotIn("fake-token", output)

    def test_empty_response(self):
        self.assertEqual(self.select()[0], "")

    def test_org_only_online_runner(self):
        result, http, output = self.select(org_runners=[runner(), runner()])
        self.assertEqual(result, LABELS)
        self.assertEqual(http.call_count, 2)
        request = http.call_args.args[0]
        self.assertEqual(
            request.full_url,
            "https://api.github.com/orgs/obs-rust/actions/runners?per_page=100&page=1",
        )
        self.assertEqual(request.get_header("Authorization"), "Bearer fake-token")
        self.assertEqual(http.call_args.kwargs["timeout"], 10)
        self.assertIn("admission=idle", output)

    def test_org_forbidden_or_missing_preserves_repo_result(self):
        for status in (403, 404):
            for repo_runners in ([], [runner(), runner()]):
                with self.subTest(status=status, repo_match=bool(repo_runners)):
                    error = HTTPError(
                        "https://api.github.com/orgs/obs-rust/actions/runners",
                        status,
                        "fake-token must not be logged",
                        None,
                        None,
                    )
                    with error:
                        result, http, output = self.select(
                            repo_runners, org_error=error
                        )
                    self.assertEqual(result, LABELS if repo_runners else "")
                    self.assertEqual(http.call_count, 2)
                    self.assertIn("::warning::", output)
                    self.assertIn("org", output.lower())
                    self.assertIn(str(status), output)
                    self.assertNotIn("fake-token", output)

    def test_both_lists_empty_use_hosted(self):
        result, http, output = self.select()
        self.assertEqual(result, "")
        self.assertEqual(http.call_count, 2)
        self.assertIn("::warning::", output)

    def test_repo_error_does_not_hide_org_match(self):
        result, _, output = self.select(
            error=URLError("fake-token must not be logged"),
            org_runners=[runner(), runner()],
        )
        self.assertEqual(result, LABELS)
        self.assertIn("::warning::", output)
        self.assertNotIn("fake-token", output)

    def test_no_api_outside_opt_in_merge_group(self):
        for event in (
            "pull_request",
            "push",
            "schedule",
            "workflow_dispatch",
            "release",
        ):
            with self.subTest(event=event):
                result, http, output = self.select(event=event)
                self.assertEqual(result, "")
                http.assert_not_called()
                self.assertEqual(output, "")
        result, http, _ = self.select(labels="")
        self.assertEqual(result, "")
        http.assert_not_called()

    def test_invalid_label_config(self):
        for labels in ("bad json", "null", "{}", "[]", '[""]', "[3]", '"linux"'):
            with self.subTest(labels=labels):
                result, http, output = self.select(labels=labels)
                self.assertEqual(result, "")
                http.assert_not_called()
                self.assertIn("::warning::", output)

    def test_malformed_api_response(self):
        for payload in (b"not json", b"{}", b'{"runners":null}', b'{"runners":[{}]}'):
            with self.subTest(payload=payload):
                with patch.object(
                    selector, "urlopen", return_value=io.BytesIO(payload)
                ):
                    with redirect_stdout(io.StringIO()) as output:
                        self.assertEqual(
                            selector.select_labels(
                                "merge_group", LABELS, "token", "owner/repo"
                            ),
                            "",
                        )
                self.assertIn("::warning::", output.getvalue())

    def test_pagination(self):
        pages = [
            io.BytesIO(
                json.dumps(
                    {"total_count": 102, "runners": [runner("offline")] * 100}
                ).encode()
            ),
            io.BytesIO(
                json.dumps(
                    {"total_count": 102, "runners": [runner(), runner()]}
                ).encode()
            ),
            io.BytesIO(b'{"total_count":0,"runners":[]}'),
        ]
        with patch.object(selector, "urlopen", side_effect=pages) as http:
            self.assertEqual(
                json.loads(
                    selector.select_labels("merge_group", LABELS, "token", "owner/repo")
                ),
                json.loads(LABELS),
            )
        self.assertEqual(http.call_count, 3)
        self.assertTrue(http.call_args_list[1].args[0].full_url.endswith("page=2"))

    def test_org_pagination(self):
        pages = [
            io.BytesIO(b'{"total_count":0,"runners":[]}'),
            io.BytesIO(
                json.dumps(
                    {"total_count": 102, "runners": [runner("offline")] * 100}
                ).encode()
            ),
            io.BytesIO(
                json.dumps(
                    {"total_count": 102, "runners": [runner(), runner()]}
                ).encode()
            ),
        ]
        with patch.object(selector, "urlopen", side_effect=pages) as http:
            self.assertEqual(
                selector.select_labels("merge_group", LABELS, "token", "owner/repo"),
                LABELS,
            )
        self.assertEqual(http.call_count, 3)
        self.assertEqual(
            http.call_args.args[0].full_url,
            "https://api.github.com/orgs/owner/actions/runners?per_page=100&page=2",
        )

    def test_org_pagination_is_bounded(self):
        def respond(request, **kwargs):
            entries = [runner("offline")] * 100 if "/orgs/" in request.full_url else []
            return io.BytesIO(
                json.dumps({"total_count": 601, "runners": entries}).encode()
            )

        with patch.object(selector, "urlopen", side_effect=respond) as http:
            with redirect_stdout(io.StringIO()) as output:
                self.assertEqual(
                    selector.select_labels(
                        "merge_group", LABELS, "token", "owner/repo"
                    ),
                    "",
                )
        self.assertEqual(http.call_count, 6)
        self.assertTrue(http.call_args.args[0].full_url.endswith("page=5"))
        self.assertIn("::warning::", output.getvalue())

    def test_main_writes_empty_output_for_fallback(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "output"
            with patch.dict(
                os.environ,
                {
                    "GITHUB_EVENT_NAME": "merge_group",
                    "MQ_RUNNER_LABELS": LABELS,
                    "MQ_RUNNER_STATUS_TOKEN": "",
                    "GITHUB_OUTPUT": str(output),
                },
                clear=True,
            ):
                with redirect_stdout(io.StringIO()):
                    self.assertEqual(selector.main(), 0)
            self.assertEqual(output.read_text(), "heavy-labels=\n")

    def test_main_writes_single_line_label_json(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "output"
            payload = json.dumps(
                {"total_count": 2, "runners": [runner(), runner()]}
            ).encode()
            with patch.dict(
                os.environ,
                {
                    "GITHUB_EVENT_NAME": "merge_group",
                    "MQ_RUNNER_LABELS": LABELS,
                    "MQ_RUNNER_STATUS_TOKEN": "fake-token",
                    "GITHUB_REPOSITORY": "owner/repo",
                    "GITHUB_OUTPUT": str(output),
                },
                clear=True,
            ):
                with patch.object(
                    selector,
                    "urlopen",
                    side_effect=lambda *a, **kw: io.BytesIO(payload),
                ):
                    self.assertEqual(selector.main(), 0)
            self.assertEqual(output.read_text(), f"heavy-labels={LABELS}\n")

    def test_capacity_accumulates_across_scopes(self):
        self.assertEqual(self.select([runner()], org_runners=[runner()])[0], LABELS)

    def test_idle_capacity_on_later_page_is_counted(self):
        pages = [
            io.BytesIO(
                json.dumps(
                    {
                        "total_count": 101,
                        "runners": [runner(), *[runner(busy=True) for _ in range(99)]],
                    }
                ).encode()
            ),
            io.BytesIO(
                json.dumps({"total_count": 101, "runners": [runner()]}).encode()
            ),
            io.BytesIO(b'{"total_count":0,"runners":[]}'),
        ]
        with patch.object(selector, "urlopen", side_effect=pages) as http:
            self.assertEqual(
                selector.select_labels("merge_group", LABELS, "token", "owner/repo"),
                LABELS,
            )
        self.assertEqual(http.call_count, 3)

    def test_main_summary_and_environment_thresholds(self):
        cases = [
            (
                [runner(busy=True) for _ in range(6)],
                {"MQ_RUNNER_MODE": "prefer"},
                LABELS,
                "online=6, busy=6, idle=0",
                None,
            ),
            ([runner(busy=True)], {"MQ_RUNNER_MODE": ""}, LABELS, "idle=0", None),
            (
                [runner(busy=True) for _ in range(6)],
                {},
                LABELS,
                "busy=6, idle=0, required=1",
                None,
            ),
            ([runner(), runner()], {}, LABELS, "busy=0, idle=2, required=1", None),
            ([runner()], {}, LABELS, "busy=0, idle=1, required=1", None),
            ([runner("offline"), runner("offline")], {}, "", "online=0", None),
            ([runner(), runner()], {"MQ_RUNNER_MIN_IDLE": "3"}, "", "required=3", None),
            ([runner(), runner()], {"MQ_RUNNER_DEMAND": "11"}, "", "required=11", None),
            ([runner()], {"MQ_RUNNER_MIN_IDLE": "1"}, LABELS, "required=1", None),
            (
                [],
                {},
                "",
                "unavailable scopes: repository, organization",
                URLError("fake-token"),
            ),
            ([], {"MQ_RUNNER_STATUS_TOKEN": ""}, "", "token is missing", None),
            ([], {"MQ_RUNNER_MIN_IDLE": "bad"}, "", "ValueError", None),
            ([], {"MQ_RUNNER_LABELS": ""}, "", "disabled", None),
        ]
        for entries, config, expected, reason, error in cases:
            with (
                self.subTest(config=config, reason=reason),
                tempfile.TemporaryDirectory() as directory,
            ):
                output = Path(directory) / "output"
                summary = Path(directory) / "summary"
                environment = {
                    "GITHUB_EVENT_NAME": "merge_group",
                    "MQ_RUNNER_LABELS": LABELS,
                    "MQ_RUNNER_STATUS_TOKEN": "fake-token",
                    "GITHUB_REPOSITORY": "owner/repo",
                    "GITHUB_OUTPUT": str(output),
                    "GITHUB_STEP_SUMMARY": str(summary),
                    **config,
                }
                payload = json.dumps(
                    {"total_count": len(entries), "runners": entries}
                ).encode()
                with (
                    patch.dict(os.environ, environment, clear=True),
                    patch.object(
                        selector,
                        "urlopen",
                        side_effect=error or (lambda *a, **kw: io.BytesIO(payload)),
                    ),
                    redirect_stdout(io.StringIO()),
                ):
                    self.assertEqual(selector.main(), 0)
                self.assertEqual(output.read_text(), f"heavy-labels={expected}\n")
                report = summary.read_text()
                self.assertIn("**self-hosted**" if expected else "**hosted**", report)
                self.assertIn(reason, report)
                self.assertNotIn("fake-token", report)

    def test_summary_error_does_not_change_selection(self):
        with (
            tempfile.TemporaryDirectory() as directory,
            patch.dict(os.environ, {"GITHUB_STEP_SUMMARY": directory}),
        ):
            result, _, output = self.select([runner(), runner()])
        self.assertEqual(result, LABELS)
        self.assertIn("could not publish summary", output)

    def test_main_output_error_does_not_fail(self):
        with patch.dict(os.environ, {}, clear=True):
            with redirect_stdout(io.StringIO()) as output:
                self.assertEqual(selector.main(), 0)
        self.assertIn("::warning::", output.getvalue())


class RetryDeadlineTest(unittest.TestCase):
    def setUp(self):
        self.elapsed = 0.0
        self.sleeps = []
        self.calls = []

    def sleep(self, seconds):
        self.sleeps.append(seconds)
        self.elapsed += seconds

    def select(self, failures=(), *, duration=0, delay=None, org_success=False):
        failures = iter(failures)

        def respond(request, *, timeout):
            self.calls.append((request.full_url, timeout))
            self.elapsed += min(duration, timeout)
            failure = next(failures, None)
            if failure:
                raise failure
            entries = [runner()] if org_success or "/repos/" in request.full_url else []
            return io.BytesIO(
                json.dumps({"total_count": len(entries), "runners": entries}).encode()
            )

        # All elapsed time and backoff are virtual; these cases never sleep.
        with ExitStack() as stack:
            stack.enter_context(
                patch.object(
                    selector.time, "monotonic", side_effect=lambda: self.elapsed
                )
            )
            stack.enter_context(
                patch.object(selector.time, "sleep", side_effect=self.sleep)
            )
            stack.enter_context(patch.object(selector, "urlopen", side_effect=respond))
            stack.enter_context(
                patch.object(selector, "inflight_commitments", return_value=(0, 0))
            )
            stack.enter_context(redirect_stdout(io.StringIO()))
            if delay is not None:
                # Exhaust the deadline while reading the first successful body.
                load = selector.json.load

                def delayed_load(response):
                    self.elapsed += delay
                    return load(response)

                stack.enter_context(
                    patch.object(selector.json, "load", side_effect=delayed_load)
                )
            return selector.select_labels(
                "merge_group", LABELS, "token", "obs-rust/obs-studio"
            )

    def test_timeout_then_success_selects_dell(self):
        self.assertEqual(self.select([TimeoutError()]), LABELS)
        self.assertEqual(len(self.calls), 3)
        self.assertEqual(len(self.sleeps), 1)
        self.assertEqual(self.calls[0][1], 10)

    def test_connection_error_then_success(self):
        self.assertEqual(self.select([URLError(ConnectionResetError())]), LABELS)
        self.assertEqual(len(self.calls), 3)

    def test_persistent_timeouts_are_bounded(self):
        self.assertEqual(self.select([TimeoutError()] * 10, duration=10), "")
        self.assertLessEqual(self.elapsed, 45)
        self.assertEqual(len(self.calls), 5)
        self.assertLess(self.calls[-1][1], 10)

    def test_server_errors_and_rate_limits_retry(self):
        for status in (500, 502, 503, 504, 429):
            with self.subTest(status=status):
                self.setUp()
                error = HTTPError(
                    "https://api.github.com", status, "transient", {}, None
                )
                self.assertEqual(self.select([error]), LABELS)
                self.assertEqual(len(self.calls), 3)
                self.assertEqual(len(self.sleeps), 1)

    def test_retry_after_is_capped(self):
        error = HTTPError(
            "https://api.github.com", 429, "slow", {"Retry-After": "120"}, None
        )
        self.assertEqual(self.select([error]), LABELS)
        self.assertEqual(self.sleeps, [5])

    def test_other_client_errors_never_retry(self):
        for status in (400, 401, 403, 404, 422):
            with self.subTest(status=status):
                self.setUp()
                error = HTTPError(
                    "https://api.github.com", status, "permanent", {}, None
                )
                self.assertEqual(self.select([error, error]), "")
                self.assertEqual(len(self.calls), 2)
                self.assertEqual(self.sleeps, [])

    def test_deadline_discards_successful_response(self):
        self.assertEqual(self.select(delay=45), "")
        self.assertEqual(len(self.calls), 1)

    def test_deadline_discards_earlier_scope_match(self):
        self.assertEqual(self.select(delay=23), "")
        self.assertEqual(len(self.calls), 2)

    def test_backoff_stops_at_deadline(self):
        probe = selector.Probe(
            "token",
            "https://api.github.com",
            clock=lambda: self.elapsed,
            sleep=self.sleep,
        )
        self.elapsed = 44
        error = HTTPError(
            "https://api.github.com", 429, "slow", {"Retry-After": "5"}, None
        )
        with patch.object(selector, "urlopen", side_effect=error) as http:
            with self.assertRaises(selector.SelectionDeadline):
                probe.get("runners")
        self.assertEqual(http.call_count, 1)
        self.assertEqual(self.elapsed, 45)
        self.assertEqual(self.sleeps, [1])

    def test_retries_consume_shared_request_budget(self):
        self.enterContext(patch.object(selector, "MAX_REQUESTS", 10))
        probe = selector.Probe(
            "token",
            "https://api.github.com",
            clock=lambda: self.elapsed,
            sleep=self.sleep,
        )
        with patch.object(selector, "urlopen", side_effect=TimeoutError()) as http:
            for _ in range(3):
                with self.assertRaises(TimeoutError):
                    probe.get("runners")
            with self.assertRaisesRegex(
                selector.RequestBudgetExhausted, "request budget"
            ):
                probe.get("runners")
            with self.assertRaisesRegex(ValueError, "request budget"):
                probe.get("runners")
        self.assertEqual(http.call_count, 10)

    def test_retry_limit_is_per_request(self):
        self.assertEqual(self.select([TimeoutError()] * 3, org_success=True), LABELS)
        self.assertEqual(len(self.calls), 4)
        self.assertEqual(len(self.sleeps), 2)

    @unittest.skipUnless(hasattr(selector.signal, "setitimer"), "Linux selector timer")
    def test_swallowed_alarm_still_publishes_hosted(self):
        with (
            tempfile.TemporaryDirectory() as temporary,
            patch.dict(
                os.environ,
                {"GITHUB_OUTPUT": str(Path(temporary) / "output")},
                clear=True,
            ),
            patch.object(selector.signal, "signal") as install,
            patch.object(selector.signal, "getitimer", return_value=(0, 0)),
            patch.object(selector.signal, "setitimer"),
            patch.object(selector, "select_labels") as select,
            redirect_stdout(io.StringIO()),
        ):

            def swallow(*args, **kwargs):
                try:
                    install.call_args_list[0].args[1](selector.signal.SIGALRM, None)
                except Exception:
                    return LABELS

            select.side_effect = swallow
            self.assertEqual(selector.main(), 0)
            self.assertEqual(
                Path(os.environ["GITHUB_OUTPUT"]).read_text(), "heavy-labels=\n"
            )

    @unittest.skipUnless(hasattr(selector.signal, "setitimer"), "Linux selector timer")
    def test_wall_timer_interrupts_body_and_publishes_hosted(self):
        previous_handler = object()
        with (
            tempfile.TemporaryDirectory() as temporary,
            patch.dict(
                os.environ,
                {
                    "GITHUB_EVENT_NAME": "merge_group",
                    "MQ_RUNNER_LABELS": LABELS,
                    "MQ_RUNNER_STATUS_TOKEN": "token",
                    "GITHUB_REPOSITORY": "obs-rust/obs-studio",
                    "GITHUB_OUTPUT": str(Path(temporary) / "output"),
                },
                clear=True,
            ),
            patch.object(selector.signal, "getsignal", return_value=previous_handler),
            patch.object(selector.signal, "getitimer", return_value=(60, 2)),
            patch.object(selector.signal, "signal") as install,
            patch.object(selector.signal, "setitimer") as timer,
            patch.object(selector.time, "monotonic", side_effect=lambda: self.elapsed),
            patch.object(selector, "urlopen", return_value=io.BytesIO()) as http,
            patch.object(selector.json, "load") as body,
            redirect_stdout(io.StringIO()),
        ):

            def interrupt(response):
                # Fire the installed alarm from inside a body read. Keeping the
                # injected clock below 45 also proves the exception cannot be
                # swallowed by a scope's generic error fallback.
                self.elapsed = 2
                install.call_args_list[0].args[1](selector.signal.SIGALRM, None)

            body.side_effect = interrupt
            self.assertEqual(selector.main(), 0)
            self.assertEqual(
                Path(os.environ["GITHUB_OUTPUT"]).read_text(), "heavy-labels=\n"
            )
            self.assertEqual(http.call_count, 1)
            self.assertEqual(
                [call.args for call in timer.call_args_list],
                [
                    (selector.signal.ITIMER_REAL, 45),
                    (selector.signal.ITIMER_REAL, 0),
                    (selector.signal.ITIMER_REAL, 58, 2),
                ],
            )
            self.assertEqual(
                install.call_args.args, (selector.signal.SIGALRM, previous_handler)
            )


class CollectionRaceTest(unittest.TestCase):
    def setUp(self):
        self.fixture = json.loads(
            (Path(__file__).parent / "fixtures/mq-probe-races.json").read_text()
        )

    def collect(self, pages):
        responses = [io.BytesIO(json.dumps(page).encode()) for page in pages]
        probe = selector.Probe(
            "runner-token", "https://api.github.com", sleep=lambda _: None
        )
        with (
            patch.object(selector, "urlopen", side_effect=responses) as http,
            redirect_stdout(io.StringIO()) as output,
        ):
            result = probe.entries(
                "repos/obs-rust/obs-studio/actions/workflows/ci.yml/runs?status=queued",
                "workflow_runs",
                "actions-token",
            )
        self.http = http
        self.output = output.getvalue()
        return result

    def test_recorded_short_page_retries_then_accepts_matching_ids(self):
        for name in ("short_runs_page", "empty_runs_page"):
            with self.subTest(page=name):
                page = self.fixture[name]
                self.assertEqual(self.collect([page, page]), page["workflow_runs"])
                self.assertEqual(self.http.call_count, 2)
                self.assertIn("retry", self.output)

    def test_count_mismatch_retries_from_first_page(self):
        page = self.fixture["short_runs_page"]
        settled = dict(page, total_count=len(page["workflow_runs"]))
        self.assertEqual(self.collect([page, settled]), page["workflow_runs"])
        self.assertEqual(self.http.call_count, 2)
        self.assertTrue(
            all("page=1" in call.args[0].full_url for call in self.http.call_args_list)
        )

    def test_full_page_does_not_stop_at_unstable_total(self):
        template = self.fixture["short_runs_page"]["workflow_runs"][0]
        runs = [dict(template, id=index + 1) for index in range(101)]
        pages = [
            {"total_count": 100, "workflow_runs": runs[:100]},
            {"total_count": 101, "workflow_runs": runs[100:]},
            {"total_count": 101, "workflow_runs": runs[:100]},
            {"total_count": 101, "workflow_runs": runs[100:]},
        ]
        self.assertEqual(self.collect(pages), runs)
        self.assertEqual(self.http.call_count, 4)

    def test_page_shift_compares_unique_ids_and_retries(self):
        template = self.fixture["short_runs_page"]["workflow_runs"][0]
        runs = [dict(template, id=index + 1) for index in range(101)]
        pages = [
            {"total_count": 101, "workflow_runs": runs[:100]},
            {"total_count": 101, "workflow_runs": runs[99:]},
            {"total_count": 102, "workflow_runs": runs[:100]},
            {"total_count": 102, "workflow_runs": runs[100:]},
        ]
        self.assertEqual(self.collect(pages), runs)
        self.assertEqual(self.http.call_count, 4)

    def test_changing_short_pages_are_bounded(self):
        page = self.fixture["short_runs_page"]
        pages = [
            dict(page, workflow_runs=[dict(page["workflow_runs"][0], id=i + 1)])
            for i in range(selector.MAX_RETRIES + 1)
        ]
        with self.assertRaisesRegex(ValueError, "did not stabilize"):
            self.collect(pages)

    def test_full_pages_exceed_limit_even_with_small_total(self):
        template = self.fixture["short_runs_page"]["workflow_runs"][0]
        pages = [
            {
                "total_count": 1,
                "workflow_runs": [
                    dict(template, id=i + page * 100 + 1) for i in range(100)
                ],
            }
            for page in range(selector.MAX_PAGES)
        ]
        with self.assertRaisesRegex(ValueError, "exceeds probe limit"):
            self.collect(pages)


class InflightAdmissionTest(unittest.TestCase):
    def setUp(self):
        self.enterContext(patch.object(selector.time, "sleep"))
        self.fixture = json.loads(
            (Path(__file__).parent / "fixtures/mq-admission.json").read_text()
        )
        self.runs = self.fixture["runs"]
        self.jobs = {run["id"]: [] for run in self.runs}
        self.labels = json.dumps(
            self.fixture["queued_jobs"][0]["labels"], separators=(",", ":")
        )

    def select(
        self,
        run_id,
        *,
        api_error=None,
        busy=0,
        capacity=6,
        demand="3",
        transform=None,
        environment=None,
    ):
        def respond(request, **kwargs):
            url = urlparse(request.full_url)
            query = parse_qs(url.query)
            if url.path.endswith("/runners"):
                entries = (
                    [
                        runner(busy=index < busy, labels=json.loads(self.labels))
                        for index in range(capacity)
                    ]
                    if "/orgs/" in url.path
                    else []
                )
                payload = {"total_count": len(entries), "runners": entries}
            else:
                if api_error:
                    raise api_error
                self.assertEqual(
                    request.get_header("Authorization"), "Bearer actions-token"
                )
                if url.path.endswith("/runs"):
                    entries = [
                        run for run in self.runs if run["status"] == query["status"][0]
                    ]
                    payload = {"total_count": len(entries), "workflow_runs": entries}
                else:
                    other = int(url.path.split("/runs/")[1].split("/")[0])
                    entries = self.jobs[other]
                    payload = {"total_count": len(entries), "jobs": entries}
            if transform:
                payload = transform(url.path, payload)
            return io.BytesIO(json.dumps(payload).encode())

        with (
            patch.dict(
                os.environ,
                {
                    "GITHUB_RUN_ID": str(run_id),
                    "GITHUB_TOKEN": "actions-token",
                    # Preserve the recorded snapshot's original run bound for
                    # legacy reservation cases; default-cap tests unset it.
                    "MQ_RUNNER_MAX_RUNS": str(max(1, capacity // int(demand))),
                    **(environment or {}),
                },
                clear=True,
            ),
            patch.object(selector, "urlopen", side_effect=respond) as http,
            redirect_stdout(io.StringIO()) as output,
        ):
            selected = selector.select_labels(
                "merge_group",
                self.labels,
                "runner-token",
                "obs-rust/obs-studio",
                min_idle=demand,
                demand=demand,
            )
        self.output = output.getvalue()
        self.http = http
        self.assertLessEqual(http.call_count, selector.MAX_REQUESTS)
        return selected

    def test_busy_dell_under_default_max_runs_queues_on_dell(self):
        self.runs = self.runs[:2]
        self.jobs[self.runs[0]["id"]] = self.fixture["queued_jobs"]
        self.assertTrue(self.select(self.runs[1]["id"], busy=6, environment={"MQ_RUNNER_MAX_RUNS": ""}))
        self.assertIn("committed=1, run_limit=3", self.output)

    def test_at_default_max_runs_uses_hosted_even_when_idle(self):
        for run in self.runs[:3]:
            self.jobs[run["id"]] = self.fixture["queued_jobs"][:1]
        self.assertEqual(self.select(self.runs[3]["id"], environment={"MQ_RUNNER_MAX_RUNS": ""}), "")
        self.assertIn("committed=3, run_limit=3", self.output)

    def test_configured_max_runs_caps_busy_admission(self):
        self.runs = self.runs[:3]
        for run in self.runs[:2]:
            self.jobs[run["id"]] = self.fixture["queued_jobs"][:1]
        self.assertEqual(self.select(
            self.runs[2]["id"], busy=6, environment={"MQ_RUNNER_MAX_RUNS": "2"}
        ), "")
        self.assertIn("committed=2, run_limit=2", self.output)

    def test_invalid_max_runs_uses_hosted(self):
        for value in ("0", "-1", "oops"):
            with self.subTest(value=value):
                self.assertEqual(self.select(
                    self.runs[0]["id"], environment={"MQ_RUNNER_MAX_RUNS": value}
                ), "")
                self.assertIn("ValueError", self.output)

    def test_busy_default_cap_admits_same_oldest_three_on_identical_snapshots(self):
        actual = [bool(self.select(
            run["id"], busy=6, environment={"MQ_RUNNER_MAX_RUNS": ""}
        )) for run in reversed(self.runs)]
        self.assertEqual(actual, [False, True, True, True])

    def test_same_second_selectors_reserve_unknown_routes_until_heavy_jobs_appear(self):
        # Report 25: #5291/#5297/#5298 all saw idle_run_slots=1. Replay
        # identical pre-heavy snapshots, including the successful-selector gap.
        candidates = [
            dict(self.runs[0], id=run_id, created_at="2026-11-19T23:48:30Z")
            for run_id in (44640669849, 44640670722, 44640672752)
        ]
        for status in ("queued", "in_progress"):
            for phase in ("absent", "in_progress", "success", "unknown"):
                for committed in range(4):
                    for limit in (1, 2, 3):
                        with self.subTest(status=status, phase=phase, committed=committed, limit=limit):
                            peers = [
                                dict(candidates[-1], id=44640672753 + index)
                                for index in range(committed)
                            ]
                            self.runs = [dict(run, status=status) for run in candidates] + peers
                            self.jobs = {run["id"]: [] for run in self.runs}
                            for run in peers:
                                self.jobs[run["id"]] = self.fixture["queued_jobs"][:1]
                            if phase != "absent":
                                for run in candidates:
                                    self.jobs[run["id"]] = [{
                                        "name": "Select Merge Queue Runner",
                                        "status": "in_progress" if phase == "in_progress" else "completed",
                                        "conclusion": "success" if phase == "success" else None,
                                        "labels": ["ubuntu-latest"],
                                    }]
                            # Deliberately query newest first; winners cannot depend
                            # on selector execution order or jobs becoming visible.
                            winners = [run["id"] for run in reversed(candidates) if self.select(
                                run["id"], busy=1, environment={"MQ_RUNNER_MAX_RUNS": str(limit)}
                            )]
                            free = min(len(candidates), max(0, limit - committed))
                            self.assertEqual(winners, [run["id"] for run in reversed(candidates[:free])])

    def test_vanished_peer_jobs_use_hosted_instead_of_releasing_reservation(self):
        self.runs = self.runs[:2]
        peer, current = self.runs

        def vanish(path, payload):
            if path.endswith(f"/runs/{peer['id']}/jobs"):
                raise HTTPError(path, 404, "Not Found", {}, None)
            return payload

        self.assertEqual(self.select(current["id"], transform=vanish), "")
        self.assertIn("HTTPError 404", self.output)

    def test_cancelled_same_second_run_releases_provisional_reservation(self):
        self.runs = self.runs[:3]
        for run in self.runs:
            run["created_at"] = self.runs[0]["created_at"]
        self.runs[0]["conclusion"] = "cancelled"
        active = self.runs[1:]
        actual = [bool(self.select(run["id"], environment={"MQ_RUNNER_MAX_RUNS": "1"})) for run in active]
        self.assertEqual(actual, [True, False])

    def test_recorded_retry_without_current_run_reserves_all_peers(self):
        # 44586086466 / 135779273252: entries=2, total_count=3, then an
        # unwrapped KeyError. Reconstruct a stable missing-current snapshot;
        # the historical log did not retain its API response or missing key.
        current = self.runs[2]["id"]
        self.runs = self.runs[:2]

        def stale_count(path, payload):
            if path.endswith("/runs") and payload["workflow_runs"]:
                return dict(payload, total_count=3)
            return payload

        self.assertTrue(self.select(current, transform=stale_count, environment={"MQ_RUNNER_MAX_RUNS": ""}))
        self.assertIn("collection changed (entries=2, total_count=3)", self.output)
        self.assertIn(str(current), self.output)
        self.assertIn("committed=2, run_limit=3", self.output)
        self.assertNotIn("probe failed", self.output)

    def test_missing_run_fields_reserve_without_probe_failure(self):
        self.runs = self.runs[:2]
        peer, current = self.runs
        for field in ("id", "created_at", "event", "status"):
            with self.subTest(field=field):
                def missing_field(path, payload):
                    if path.endswith("/runs"):
                        return dict(payload, workflow_runs=[
                            {key: value for key, value in run.items() if key != field}
                            if run["id"] == peer["id"] else run
                            for run in payload["workflow_runs"]
                        ])
                    return payload

                self.assertTrue(self.select(current["id"], transform=missing_field))
                self.assertIn(field, self.output)
                self.assertIn("committed=1", self.output)
                self.assertNotIn("probe failed", self.output)

    def test_missing_current_timestamp_reserves_every_undecided_peer(self):
        current = self.runs[0]["id"]
        for run in self.runs:
            run.pop("created_at")
        self.assertEqual(self.select(current, environment={"MQ_RUNNER_MAX_RUNS": ""}), "")
        self.assertIn("committed=3", self.output)
        self.assertNotIn("probe failed", self.output)

    def test_missing_job_labels_reserve_peer_without_probe_failure(self):
        self.runs = self.runs[:2]
        self.jobs[self.runs[0]["id"]] = [{"name": "Build", "status": "queued"}]
        self.assertTrue(self.select(self.runs[1]["id"]))
        self.assertIn("labels", self.output)
        self.assertIn("committed=1", self.output)
        self.assertNotIn("probe failed", self.output)

    def test_four_simultaneous_recorded_runs_admit_oldest_two(self):
        # Each probe sees the same idle census and no heavy jobs yet. Reverse
        # probe order too: priority follows creation, not which probe ran first.
        for runs in (self.runs, list(reversed(self.runs))):
            with self.subTest(order=[run["id"] for run in runs]):
                actual = {run["id"]: bool(self.select(run["id"])) for run in runs}
                self.assertEqual(
                    actual,
                    {run["id"]: index < 2 for index, run in enumerate(self.runs)},
                )

    def test_queued_dell_jobs_reduce_idle_capacity_but_allow_queueing(self):
        self.jobs[self.runs[0]["id"]] = self.fixture["queued_jobs"][:4]
        self.assertTrue(self.select(self.runs[1]["id"]))
        self.assertIn("queued=4, committed=1", self.output)
        self.assertIn("idle_run_slots=0; admission=busy queue", self.output)

    def test_actions_api_error_uses_hosted(self):
        for error in (
            URLError("private server text"),
            HTTPError("url", 403, "denied", {}, None),
        ):
            try:
                with self.subTest(error=type(error).__name__):
                    self.assertEqual(
                        self.select(self.runs[0]["id"], api_error=error), ""
                    )
                    self.assertIn("runs probe (queued)", self.output)
                    self.assertIn(type(error).__name__, self.output)
                    self.assertNotIn("private server text", self.output)
                    if isinstance(error, HTTPError):
                        self.assertIn("403", self.output)
            finally:
                if isinstance(error, HTTPError):
                    error.close()

    def test_job_api_error_uses_hosted(self):
        def fail_jobs(path, payload):
            if path.endswith("/jobs"):
                raise URLError("private server text")
            return payload

        self.assertEqual(self.select(self.runs[0]["id"], transform=fail_jobs), "")
        self.assertIn("jobs probe (run", self.output)
        self.assertIn("URLError", self.output)
        self.assertNotIn("private server text", self.output)

    def test_missing_identity_or_actions_token_uses_hosted(self):
        for config in (
            {"GITHUB_RUN_ID": ""},
            {"GITHUB_RUN_ID": "0"},
            {"GITHUB_TOKEN": ""},
        ):
            with self.subTest(config=config):
                self.assertEqual(
                    self.select(self.runs[0]["id"], environment=config), ""
                )

    def test_missing_current_run_uses_hosted(self):
        self.assertEqual(self.select(1), "")

    def test_known_newer_dell_runs_reserve_slots(self):
        for run in self.runs[2:]:
            self.jobs[run["id"]] = [
                dict(self.fixture["queued_jobs"][0], status="in_progress")
            ]
        self.assertEqual(self.select(self.runs[0]["id"]), "")

    def test_hosted_heavy_job_releases_older_reservation(self):
        self.jobs[self.runs[0]["id"]] = [
            dict(self.fixture["queued_jobs"][0], labels=["ubuntu-26.04"])
        ]
        self.assertTrue(self.select(self.runs[2]["id"]))

    def test_recorded_hosted_backlog_releases_queued_run_reservations(self):
        fixture = json.loads(
            (Path(__file__).parent / "fixtures/mq-hosted-backlog.json").read_text()
        )
        current = dict(self.runs[-1], created_at="2026-11-18T18:15:00Z")
        self.runs = [record["run"] for record in fixture["records"]] + [current]
        for record in fixture["records"]:
            self.jobs[record["run"]["id"]] = [record["selector_job"]] + [
                dict(job, status="queued", conclusion=None, runner_name="")
                for job in record["heavy_jobs"]
            ]
        self.assertTrue(self.select(current["id"]))

    def test_recorded_completed_hosted_jobs_release_older_reservations(self):
        fixture = json.loads(
            (Path(__file__).parent / "fixtures/mq-hosted-backlog.json").read_text()
        )
        current = dict(self.runs[-1], created_at="2026-11-18T18:15:00Z")
        self.runs = [
            dict(record["run"], status="in_progress") for record in fixture["records"]
        ] + [current]
        for record in fixture["records"]:
            self.jobs[record["run"]["id"]] = [record["selector_job"]] + record[
                "heavy_jobs"
            ]
        self.assertTrue(self.select(current["id"]))

    def test_hosted_artifact_producer_alone_establishes_route(self):
        for run in self.runs[:-1]:
            self.jobs[run["id"]] = [
                dict(
                    self.fixture["queued_jobs"][0],
                    name="Build 🧱 / macOS Rust Artifacts 🐧",
                    labels=["ubuntu-26.04"],
                    status="completed",
                    conclusion="success",
                )
            ]
        self.assertTrue(self.select(self.runs[-1]["id"]))

    def test_unsuccessful_selector_without_live_dell_jobs_releases_slot(self):
        for conclusion in ("failure", "cancelled", "skipped", "timed_out"):
            with self.subTest(conclusion=conclusion):
                self.jobs[self.runs[0]["id"]] = [
                    {
                        "name": "Select Merge Queue Runner",
                        "status": "completed",
                        "conclusion": conclusion,
                        "labels": ["ubuntu-latest"],
                    }
                ]
                self.assertTrue(self.select(self.runs[1]["id"], capacity=3))
                self.assertIn("queued=0, committed=0", self.output)

    def test_recorded_peer_before_heavy_jobs_does_not_fail_probe(self):
        fixture = json.loads(
            (Path(__file__).parent / "fixtures/mq-probe-races.json").read_text()
        )
        self.runs = fixture["short_runs_page"]["workflow_runs"]
        self.jobs = {
            run["id"]: fixture["peer_jobs_before_heavy"]["jobs"] for run in self.runs
        }
        for run in self.runs:
            with self.subTest(current=run["id"]):
                older = int(run != self.runs[0])
                self.assertEqual(bool(self.select(run["id"], capacity=3)), not older)
                self.assertIn(f"queued=0, committed={older}", self.output)
                self.assertNotIn("probe failed", self.output)

    def test_retried_duplicate_terminal_run_cannot_regain_reservation(self):
        self.runs = self.runs[:2]
        peer, current = self.runs

        def duplicate_snapshots(path, payload):
            if path.endswith("/runs") and payload["workflow_runs"]:
                return {
                    "total_count": 3,
                    "workflow_runs": [
                        dict(peer, status="completed", conclusion="cancelled"),
                        peer,
                        current,
                    ],
                }
            return payload

        self.assertTrue(
            self.select(current["id"], capacity=3, transform=duplicate_snapshots)
        )
        self.assertIn("queued=0, committed=0", self.output)
        self.assertIn("retry", self.output)

    def test_completed_selector_still_counts_live_dell_jobs(self):
        self.runs = self.runs[:2]
        for status in ("queued", "waiting", "pending", "requested", "in_progress"):
            with self.subTest(status=status):
                self.jobs[self.runs[0]["id"]] = [
                    {
                        "name": "Select Merge Queue Runner",
                        "status": "completed",
                        "conclusion": "success",
                        "labels": ["ubuntu-latest"],
                    },
                    dict(self.fixture["queued_jobs"][0], status=status),
                ]
                self.assertEqual(self.select(self.runs[1]["id"], capacity=3), "")
                self.assertIn("committed=1", self.output)

    def test_skipped_selectors_release_reservations(self):
        for run in self.runs[:-1]:
            self.jobs[run["id"]] = [
                {
                    "name": "Select Merge Queue Runner",
                    "status": "completed",
                    "conclusion": "skipped",
                    "labels": [],
                }
            ]
        self.assertTrue(self.select(self.runs[-1]["id"]))

    def test_queued_workflow_with_newer_dell_route_reserves_slot(self):
        for run in self.runs[1:3]:
            run["status"] = "queued"
            self.jobs[run["id"]] = [self.fixture["queued_jobs"][0]]
        self.assertEqual(self.select(self.runs[0]["id"]), "")
        self.assertIn("committed=2", self.output)

    def test_capacity_slots_are_computed_from_census_and_requirement(self):
        for capacity, demand, admitted in ((6, "3", 2), (9, "3", 3), (6, "6", 1)):
            with self.subTest(capacity=capacity, demand=demand):
                actual = [
                    bool(self.select(run["id"], capacity=capacity, demand=demand))
                    for run in self.runs
                ]
                self.assertEqual(sum(actual), admitted)
                self.assertIn(f"run_limit={admitted}", self.output)

    def test_running_jobs_are_not_subtracted_twice(self):
        self.runs = self.runs[:2]
        self.jobs[self.runs[0]["id"]] = [
            dict(job, status="in_progress", runner_id=index + 1)
            for index, job in enumerate(self.fixture["queued_jobs"][:3])
        ]
        self.assertTrue(self.select(self.runs[1]["id"], busy=3))
        self.assertIn("idle=3, required=3", self.output)
        self.assertIn("run_limit=2, idle_run_slots=1", self.output)

    def test_equal_timestamps_use_run_id(self):
        for run in self.runs:
            run["created_at"] = self.runs[0]["created_at"]
        self.test_four_simultaneous_recorded_runs_admit_oldest_two()

    def test_queued_workflow_reserves_older_slot(self):
        self.runs[0]["status"] = "queued"
        self.assertEqual(self.select(self.runs[2]["id"]), "")

    def test_unrelated_queued_labels_do_not_use_dell_capacity(self):
        self.runs = self.runs[:2]
        self.jobs[self.runs[0]["id"]] = [
            dict(job, labels=["ubuntu-latest"], name="Format")
            for job in self.fixture["queued_jobs"]
        ]
        self.assertTrue(self.select(self.runs[1]["id"]))

    def test_queued_label_matching_is_case_insensitive(self):
        self.jobs[self.runs[0]["id"]] = [
            dict(job, labels=[label.upper() for label in job["labels"]])
            for job in self.fixture["queued_jobs"][:4]
        ]
        self.assertTrue(self.select(self.runs[1]["id"]))
        self.assertIn("queued=4, committed=1", self.output)

    def test_incomplete_jobs_use_hosted(self):
        for endpoint in ("/jobs",):

            def incomplete(path, payload):
                return (
                    dict(payload, total_count=999)
                    if path.endswith(endpoint)
                    else payload
                )

            with self.subTest(endpoint=endpoint):
                self.assertEqual(
                    self.select(self.runs[0]["id"], transform=incomplete), ""
                )

    def test_nine_active_runs_fit_request_budget(self):
        # Five queue entries plus four still-visible runs from a regroup.
        self.runs.extend(
            dict(self.runs[0], id=50000000000 + index) for index in range(5)
        )
        self.jobs.update({run["id"]: [] for run in self.runs})
        self.assertTrue(self.select(self.runs[0]["id"]))
        self.assertEqual(self.http.call_count, 12)
        self.assertNotIn("probe failed", self.output)

    def test_budget_exhaustion_reserves_unprobed_runs(self):
        with patch.object(selector, "MAX_REQUESTS", 5):
            self.assertEqual(self.select(self.runs[0]["id"]), "")
        self.assertIn("unprobed=2", self.output)
        self.assertIn("queued=6", self.output)
        self.assertIn("committed=2", self.output)
        self.assertIn("request budget exhausted", self.output)
        self.assertNotIn("probe failed", self.output)

    def test_queued_undecided_runs_reserve_capacity_after_jobs_probe(self):
        self.runs[0]["status"] = "queued"
        self.assertEqual(self.select(self.runs[2]["id"]), "")
        urls = [call.args[0].full_url for call in self.http.call_args_list]
        self.assertTrue(any(f"/runs/{self.runs[0]['id']}/jobs" in url for url in urls))

    def test_retry_budget_exhaustion_reserves_all_unprobed_runs(self):
        def timeout_jobs(path, payload):
            if path.endswith("/jobs"):
                raise TimeoutError()
            return payload

        with patch.object(selector, "MAX_REQUESTS", 6):
            self.assertEqual(
                self.select(self.runs[0]["id"], transform=timeout_jobs), ""
            )
        self.assertIn("unprobed=3", self.output)
        self.assertIn("queued=9", self.output)
        self.assertNotIn("probe failed", self.output)

    def test_pagination_budget_exhaustion_reserves_whole_run(self):
        self.runs = self.runs[:2]
        self.jobs[self.runs[1]["id"]] = self.fixture["queued_jobs"][:1] * 100

        def more_jobs(path, payload):
            return dict(payload, total_count=101) if path.endswith("/jobs") else payload

        with patch.object(selector, "MAX_REQUESTS", 5):
            self.assertTrue(self.select(self.runs[0]["id"], transform=more_jobs))
        self.assertIn("unprobed=1", self.output)
        self.assertIn("queued=100", self.output)
        self.assertIn("committed=1, run_limit=2", self.output)
        self.assertIn("reserving at least 3 runners", self.output)
        self.assertNotIn("probe failed", self.output)

    def test_terminal_runs_in_active_listing_are_ignored(self):
        current = self.runs[-1]["id"]
        for terminal in (
            {"status": "completed"},
            {"status": "cancelled"},
            {"conclusion": "cancelled"},
            {"conclusion": "success"},
        ):

            def finish_runs(path, payload):
                if path.endswith("/runs"):
                    payload["workflow_runs"] = [
                        run if run["id"] == current else dict(run, **terminal)
                        for run in payload["workflow_runs"]
                    ]
                return payload

            with self.subTest(terminal=terminal):
                self.assertTrue(self.select(current, transform=finish_runs))
                self.assertEqual(self.http.call_count, 4)

    def test_terminal_dell_jobs_release_capacity_without_undecided_reservation(self):
        for terminal in (
            {"status": "completed"},
            {"status": "queued", "conclusion": "cancelled"},
            {"status": "in_progress", "conclusion": "success"},
        ):
            for run in self.runs[:-1]:
                self.jobs[run["id"]] = [
                    dict(job, **terminal) for job in self.fixture["queued_jobs"]
                ]
            with self.subTest(terminal=terminal):
                self.assertTrue(self.select(self.runs[-1]["id"]))
                self.assertIn("queued=0, committed=0", self.output)

    def test_completed_dell_jobs_waiting_only_for_hosted_macos_release_slot(self):
        self.runs = self.runs[:2]
        self.jobs[self.runs[0]["id"]] = [
            {
                "name": "Select Merge Queue Runner",
                "status": "completed",
                "conclusion": "success",
                "labels": ["ubuntu-latest"],
            },
            *[
                dict(job, status="completed", conclusion="success")
                for job in self.fixture["queued_jobs"]
            ],
            {
                "name": "Build 🧱 / Rust Tests 🦀 (macos-26)",
                "status": "queued",
                "conclusion": None,
                "labels": ["macos-26"],
            },
        ]
        self.assertTrue(self.select(self.runs[1]["id"], capacity=3))
        self.assertIn("queued=0, committed=0, run_limit=1", self.output)

    def test_live_dell_jobs_retain_slot_after_other_dell_jobs_finish(self):
        self.runs = self.runs[:2]
        for status in ("queued", "waiting", "pending", "requested", "in_progress"):
            with self.subTest(status=status):
                self.jobs[self.runs[0]["id"]] = [
                    dict(job, status="completed", conclusion="success")
                    for job in self.fixture["queued_jobs"]
                ] + [dict(self.fixture["queued_jobs"][0], status=status)]
                self.assertEqual(self.select(self.runs[1]["id"], capacity=3), "")
                self.assertIn("committed=1, run_limit=1", self.output)

    def test_warning_names_probe_step_and_value_error(self):
        def incomplete_jobs(path, payload):
            return dict(payload, total_count=999) if path.endswith("/jobs") else payload

        self.assertEqual(self.select(self.runs[0]["id"], transform=incomplete_jobs), "")
        self.assertIn("jobs probe", self.output)
        self.assertIn("incomplete collection", self.output)

    def test_warning_names_runs_probe_and_value_error(self):
        def incomplete_runs(path, payload):
            return (
                dict(payload, total_count="invalid")
                if path.endswith("/runs")
                else payload
            )

        self.assertEqual(self.select(self.runs[0]["id"], transform=incomplete_runs), "")
        self.assertIn("runs probe (queued)", self.output)
        self.assertIn("invalid collection", self.output)

    def test_malformed_job_labels_use_hosted(self):
        self.jobs[self.runs[0]["id"]] = [
            dict(self.fixture["queued_jobs"][0], labels="not an array")
        ]
        self.assertEqual(self.select(self.runs[1]["id"]), "")


if __name__ == "__main__":
    unittest.main()
