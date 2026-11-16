#!/usr/bin/env -S uv run --script
"""Merge queue runner availability and fail-open hosted selection (no network)."""

import io
import itertools
import json
import os
import tempfile
import unittest
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
        self.assertLessEqual(http.call_args.kwargs["timeout"], 5)

    def test_all_six_online_runners_busy_use_hosted(self):
        result, _, output = self.select([runner(busy=True) for _ in range(6)])
        self.assertEqual(result, "")
        self.assertIn("idle=0", output)
        self.assertIn("busy=6", output)

    def test_default_requires_two_idle_runners(self):
        self.assertEqual(self.select([runner()])[0], "")
        self.assertEqual(self.select([runner(), runner()])[0], LABELS)

    def test_busy_offline_and_nonmatching_do_not_supply_idle_capacity(self):
        entries = [
            runner(),
            runner(busy=True),
            runner("offline"),
            runner(labels=["x64"]),
        ]
        self.assertEqual(self.select(entries)[0], "")
        self.assertEqual(self.select([*entries, runner()])[0], LABELS)

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
        self.assertEqual(self.select([same], org_runners=[same])[0], "")
        self.assertEqual(self.select([same], org_runners=[same, runner()])[0], LABELS)

    def test_disagreeing_busy_state_is_conservative(self):
        same = runner()
        self.assertEqual(
            self.select([same, runner()], org_runners=[{**same, "busy": True}])[0], ""
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
        self.assertLessEqual(http.call_args.kwargs["timeout"], 5)
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
            self.assertEqual(output.read_text(), "labels=\n")

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
            self.assertEqual(output.read_text(), f"labels={LABELS}\n")

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
                {},
                "",
                "busy=6, idle=0, required=2",
                None,
            ),
            ([runner(), runner()], {}, LABELS, "busy=0, idle=2, required=2", None),
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
                self.assertEqual(output.read_text(), f"labels={expected}\n")
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


if __name__ == "__main__":
    unittest.main()
