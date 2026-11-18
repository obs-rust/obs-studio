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

    def test_all_six_online_runners_busy_use_hosted(self):
        result, _, output = self.select([runner(busy=True) for _ in range(6)])
        self.assertEqual(result, "")
        self.assertIn("idle=0", output)
        self.assertIn("busy=6", output)

    def test_prefer_busy_runners_do_not_override_capacity(self):
        entries = [runner(busy=True) for _ in range(6)]
        for scope in ("repository", "organization"):
            with self.subTest(scope=scope):
                options = {
                    "runners" if scope == "repository" else "org_runners": entries
                }
                self.assertEqual(self.select(mode="prefer", **options)[0], "")

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

    def test_empty_mode_keeps_idle_threshold_behavior(self):
        self.assertEqual(self.select([runner(busy=True)], mode="")[0], "")
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
                        LABELS if idle == 6 else "",
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

    def test_default_requires_one_idle_runner(self):
        self.assertEqual(self.select([runner()])[0], LABELS)
        self.assertEqual(self.select([runner(), runner()])[0], LABELS)

    def test_five_busy_runners_do_not_strand_the_last_idle_runner(self):
        entries = [runner(busy=True) for _ in range(5)]
        self.assertEqual(self.select([*entries, runner()])[0], LABELS)
        self.assertEqual(self.select([*entries, runner("offline")])[0], "")
        self.assertEqual(self.select([*entries, runner()], demand="2")[0], "")

    def test_empty_actions_thresholds_use_one_idle_runner(self):
        self.assertEqual(self.select([runner()], min_idle="", demand="")[0], LABELS)

    def test_busy_offline_and_nonmatching_do_not_supply_idle_capacity(self):
        entries = [
            runner(),
            runner(busy=True),
            runner("offline"),
            runner(labels=["x64"]),
        ]
        self.assertEqual(self.select(entries, min_idle="2")[0], "")
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
        self.assertEqual(
            self.select(
                [same, runner()], org_runners=[{**same, "busy": True}], min_idle="2"
            )[0],
            "",
        )

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
        self.assertEqual(output, "")

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
                "",
                "online=6, busy=6, idle=0",
                None,
            ),
            ([runner(busy=True)], {"MQ_RUNNER_MODE": ""}, "", "idle=0", None),
            (
                [runner(busy=True) for _ in range(6)],
                {},
                "",
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
        self, run_id, *, api_error=None, busy=0, transform=None, environment=None
    ):
        def respond(request, **kwargs):
            url = urlparse(request.full_url)
            query = parse_qs(url.query)
            if url.path.endswith("/runners"):
                entries = (
                    [
                        runner(busy=index < busy, labels=json.loads(self.labels))
                        for index in range(6)
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
                min_idle="3",
                demand="3",
            )
        self.output = output.getvalue()
        self.http = http
        self.assertLessEqual(http.call_count, selector.MAX_REQUESTS)
        return selected

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

    def test_queued_dell_jobs_reduce_idle_capacity(self):
        self.jobs[self.runs[0]["id"]] = self.fixture["queued_jobs"][:4]
        self.assertEqual(self.select(self.runs[1]["id"]), "")

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
            finally:
                if isinstance(error, HTTPError):
                    error.close()

    def test_job_api_error_uses_hosted(self):
        def fail_jobs(path, payload):
            if path.endswith("/jobs"):
                raise URLError("private server text")
            return payload

        self.assertEqual(self.select(self.runs[0]["id"], transform=fail_jobs), "")

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

    def test_running_jobs_are_not_subtracted_twice(self):
        self.runs = self.runs[:2]
        self.jobs[self.runs[0]["id"]] = [
            dict(job, status="in_progress", runner_id=index + 1)
            for index, job in enumerate(self.fixture["queued_jobs"][:3])
        ]
        self.assertTrue(self.select(self.runs[1]["id"], busy=3))

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
        self.assertEqual(self.select(self.runs[1]["id"]), "")

    def test_incomplete_jobs_or_runs_use_hosted(self):
        for endpoint in ("/jobs", "/runs"):

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

    def test_queued_runs_reserve_capacity_without_jobs_calls(self):
        self.runs[0]["status"] = "queued"
        self.assertEqual(self.select(self.runs[2]["id"]), "")
        urls = [call.args[0].full_url for call in self.http.call_args_list]
        self.assertFalse(any(f"/runs/{self.runs[0]['id']}/jobs" in url for url in urls))

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
            self.assertEqual(self.select(self.runs[0]["id"], transform=more_jobs), "")
        self.assertIn("unprobed=1", self.output)
        self.assertIn("queued=100", self.output)
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

    def test_terminal_jobs_do_not_commit_capacity(self):
        for terminal in (
            {"status": "completed"},
            {"status": "queued", "conclusion": "cancelled"},
            {"status": "in_progress", "conclusion": "success"},
        ):
            for run in self.runs[1:]:
                self.jobs[run["id"]] = [
                    dict(job, **terminal) for job in self.fixture["queued_jobs"]
                ]
            with self.subTest(terminal=terminal):
                self.assertTrue(self.select(self.runs[0]["id"]))

    def test_warning_names_probe_step_and_value_error(self):
        def incomplete_jobs(path, payload):
            return dict(payload, total_count=999) if path.endswith("/jobs") else payload

        self.assertEqual(self.select(self.runs[0]["id"], transform=incomplete_jobs), "")
        self.assertIn("jobs probe", self.output)
        self.assertIn("incomplete collection", self.output)

    def test_warning_names_runs_probe_and_value_error(self):
        def incomplete_runs(path, payload):
            return dict(payload, total_count=999) if path.endswith("/runs") else payload

        self.assertEqual(self.select(self.runs[0]["id"], transform=incomplete_runs), "")
        self.assertIn("runs probe (queued)", self.output)
        self.assertIn("incomplete collection", self.output)

    def test_malformed_job_labels_use_hosted(self):
        self.jobs[self.runs[0]["id"]] = [
            dict(self.fixture["queued_jobs"][0], labels="not an array")
        ]
        self.assertEqual(self.select(self.runs[1]["id"]), "")


if __name__ == "__main__":
    unittest.main()
