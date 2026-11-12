#!/usr/bin/env -S uv run --script
"""Merge queue runner availability and fail-open hosted selection (no network)."""

import io
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


def runner(status="online", *, busy=False, labels=None):
    return {
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
                result = selector.select_labels(
                    event, labels, token, "obs-rust/obs-studio"
                )
        return result, http, output.getvalue()

    def test_online_all_labels(self):
        result, http, _ = self.select([runner(labels=[*json.loads(LABELS), "extra"])])
        self.assertEqual(json.loads(result), json.loads(LABELS))
        request = http.call_args_list[0].args[0]
        self.assertEqual(
            request.full_url,
            "https://api.github.com/repos/obs-rust/obs-studio/actions/runners?per_page=100&page=1",
        )
        self.assertEqual(request.get_header("Authorization"), "Bearer fake-token")
        self.assertLessEqual(http.call_args.kwargs["timeout"], 5)

    def test_online_busy_runner_still_matches(self):
        self.assertEqual(
            json.loads(self.select([runner(busy=True)])[0]), json.loads(LABELS)
        )

    def test_label_matching_is_case_insensitive(self):
        self.assertEqual(
            json.loads(
                self.select(
                    [runner(labels=[label.upper() for label in json.loads(LABELS)])]
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
        result, http, output = self.select(org_runners=[runner()])
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
            for repo_runners in ([], [runner()]):
                with self.subTest(status=status, repo_match=bool(repo_runners)):
                    error = HTTPError(
                        "https://api.github.com/orgs/obs-rust/actions/runners",
                        status,
                        "fake-token must not be logged",
                        None,
                        None,
                    )
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
            error=URLError("fake-token must not be logged"), org_runners=[runner()]
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
                    {"total_count": 101, "runners": [runner("offline")] * 100}
                ).encode()
            ),
            io.BytesIO(
                json.dumps({"total_count": 101, "runners": [runner()]}).encode()
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
                    {"total_count": 101, "runners": [runner("offline")] * 100}
                ).encode()
            ),
            io.BytesIO(
                json.dumps({"total_count": 101, "runners": [runner()]}).encode()
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
            payload = json.dumps({"total_count": 1, "runners": [runner()]}).encode()
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
                    selector, "urlopen", return_value=io.BytesIO(payload)
                ):
                    self.assertEqual(selector.main(), 0)
            self.assertEqual(output.read_text(), f"labels={LABELS}\n")

    def test_main_output_error_does_not_fail(self):
        with patch.dict(os.environ, {}, clear=True):
            with redirect_stdout(io.StringIO()) as output:
                self.assertEqual(selector.main(), 0)
        self.assertIn("::warning::", output.getvalue())


if __name__ == "__main__":
    unittest.main()
