#!/usr/bin/env -S uv run --script
"""tools/user_quotes.py: the README's user quotes come only from comments a
reviewer approved, word for word, and a rejected comment stays rejected (#3239).

    uv run --no-project ci/test_user_quotes.py
"""

from __future__ import annotations

import importlib.util
import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SPEC = importlib.util.spec_from_file_location("user_quotes", ROOT / "tools" / "user_quotes.py")
uq = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(uq)

URL = "https://github.com/obs-rust/obs-studio/issues/{}#issuecomment-{}"


def comment(cid, body, login="someone", kind="User", assoc="NONE", issue=7, at="2026-10-12T10:00:00Z"):
    return {"html_url": URL.format(issue, cid), "body": body, "created_at": at,
            "author_association": assoc, "user": {"login": login, "type": kind},
            "issue_url": f"https://api.github.com/repos/obs-rust/obs-studio/issues/{issue}"}


class CleanTest(unittest.TestCase):
    def test_removes_code_logs_html_comments_and_quotes(self):
        body = ("> did it work?\nIt works now.\n```\nerror: log line\n```\n"
                "<details><summary>log</summary>\n12:00 info\n</details>\n<!-- template -->Thanks!")
        self.assertEqual(uq.clean(body), "It works now. Thanks!")

    def test_unclosed_block_is_dropped_to_the_end(self):
        self.assertEqual(uq.clean("Works.\n```\nlog without an end"), "Works.")

    def test_keeps_the_words_as_written(self):
        self.assertEqual(uq.clean("It  works!\n\nIn both ways, *really*."), "It works! In both ways, *really*.")


class CandidateTest(unittest.TestCase):
    def test_only_users_who_are_not_maintainers_or_contributors(self):
        rows = [comment(1, "a", kind="Bot", login="ci-bot"),
                comment(2, "b", login="coderabbitai[bot]"),
                comment(3, "c", login="claude-agent"),
                comment(4, "d", assoc="MEMBER"),
                comment(5, "e", assoc="OWNER"),
                comment(6, "f", assoc="COLLABORATOR"),
                comment(7, "g", login="Leader"),
                comment(8, "h", assoc="CONTRIBUTOR", login="upstream-dev"),
                comment(9, "i", login="user")]
        got = uq.candidates(rows, {"leader"}, set())
        self.assertEqual([c["login"] for c in got], ["upstream-dev", "user"])

    def test_skips_judged_empty_and_long_comments(self):
        rows = [comment(1, "judged"), comment(2, "```\nonly a log\n```"),
                comment(3, "x" * 50), comment(4, "short and kind")]
        got = uq.candidates(rows, set(), {URL.format(7, 1)}, max_chars=40)
        self.assertEqual([c["text"] for c in got], ["short and kind"])

    def test_candidate_fields(self):
        got = uq.candidates([comment(9, "It works!", issue=251)], set(), set())
        self.assertEqual(got, [{"url": URL.format(251, 9), "login": "someone", "date": "2026-10-12",
                                "issue": 251, "text": "It works!"}])


class RecordTest(unittest.TestCase):
    def setUp(self):
        self.found = {"scanned_through": "2026-10-31T00:00:00Z",
                      "candidates": uq.candidates([comment(1, "It works!"), comment(2, "Works, but slow.")],
                                                  set(), set())}
        self.empty = {"scanned_through": "", "quotes": []}

    def test_approved_text_comes_from_the_candidate(self):
        verdicts = {URL.format(7, 1): ["approved", ""], URL.format(7, 2): ["rejected", "works, but"]}
        data = uq.record(self.empty, self.found, verdicts, {URL.format(7, 1): "It works!"})
        self.assertEqual(data["scanned_through"], "2026-10-31T00:00:00Z")
        approved, rejected = data["quotes"]
        self.assertEqual(approved["text"], "It works!")
        self.assertEqual(approved["login"], "someone")
        self.assertEqual(rejected, {"url": URL.format(7, 2), "date": "2026-10-12", "issue": 7,
                                    "status": "rejected", "reason": "works, but"})

    def test_every_candidate_needs_a_verdict(self):
        with self.assertRaisesRegex(ValueError, "no verdict"):
            uq.record(self.empty, self.found, {URL.format(7, 1): ["approved", ""]}, {})

    def test_comment_edited_since_collect_is_refused(self):
        verdicts = {URL.format(7, 1): ["approved", ""], URL.format(7, 2): ["rejected", "mixed"]}
        with self.assertRaisesRegex(ValueError, "changed"):
            uq.record(self.empty, self.found, verdicts, {URL.format(7, 1): "It works! Edited."})

    def test_rejection_needs_a_reason(self):
        verdicts = {URL.format(7, 1): ["approved", ""], URL.format(7, 2): ["rejected", ""]}
        with self.assertRaisesRegex(ValueError, "reason"):
            uq.record(self.empty, self.found, verdicts, {URL.format(7, 1): "It works!"})

    def test_a_rejected_comment_stays_rejected(self):
        before = {"scanned_through": "2026-10-20T00:00:00Z",
                  "quotes": [{"url": URL.format(7, 1), "date": "2026-10-12", "issue": 7,
                              "status": "rejected", "reason": "struck by a maintainer"}]}
        verdicts = {URL.format(7, 1): ["approved", ""], URL.format(7, 2): ["rejected", "mixed"]}
        data = uq.record(before, self.found, verdicts, {URL.format(7, 1): "It works!"})
        self.assertEqual([q["status"] for q in data["quotes"]], ["rejected", "rejected"])
        self.assertNotIn("text", data["quotes"][0])


class RenderTest(unittest.TestCase):
    def data(self):
        q = lambda n, status, date: {"url": URL.format(n, n), "date": date, "issue": n, "status": status,
                                     **({"login": f"u{n}", "text": f"quote {n}"} if status == "approved"
                                        else {"reason": "no"})}
        return {"scanned_through": "", "quotes": [q(1, "approved", "2026-10-01"), q(2, "rejected", "2026-10-02"),
                                                  q(3, "approved", "2026-10-03"), q(4, "approved", "2026-10-04")]}

    def test_fills_only_between_the_markers_newest_first(self):
        readme = f"# Title\n\n{uq.START}\nold\n{uq.END}\n\n# Next\n"
        out = uq.render(readme, self.data(), shown=2)
        self.assertTrue(out.startswith("# Title\n\n" + uq.START))
        self.assertTrue(out.endswith(uq.END + "\n\n# Next\n"))
        self.assertNotIn("old", out)
        self.assertLess(out.index("quote 4"), out.index("quote 3"))
        self.assertNotIn("quote 1", out)
        self.assertNotIn(URL.format(2, 2), out)
        self.assertIn(f"> — [u4](https://github.com/u4), [#4]({URL.format(4, 4)}), 2026-10-04", out)

    def test_render_is_stable(self):
        readme = f"{uq.START}\n{uq.END}\n"
        once = uq.render(readme, self.data())
        self.assertEqual(uq.render(once, self.data()), once)

    def test_missing_markers_fail(self):
        with self.assertRaises(ValueError):
            uq.render("# no markers\n", self.data())


class CommittedDataTest(unittest.TestCase):
    def setUp(self):
        self.text = uq.DATA.read_text()
        self.data = json.loads(self.text)

    def test_readme_matches_the_data_file(self):
        readme = uq.README.read_text()
        self.assertEqual(uq.render(readme, self.data), readme,
                         "README.md is stale: run uv run tools/user_quotes.py render")

    def test_data_file_is_canonical(self):
        self.assertEqual(uq.dump(self.data), self.text, "rewrite it with tools/user_quotes.py")

    def test_entries(self):
        urls = [q["url"] for q in self.data["quotes"]]
        self.assertEqual(len(urls), len(set(urls)), "one verdict per comment")
        for q in self.data["quotes"]:
            self.assertRegex(q["url"], r"^https://github\.com/obs-rust/obs-studio/(issues|pull)/\d+#issuecomment-\d+$")
            self.assertIn(q["status"], ("approved", "rejected"))
            if q["status"] == "approved":
                self.assertTrue(q["text"].strip(), q["url"])
                self.assertNotIn("reason", q)
            else:
                self.assertTrue(q["reason"].strip(), q["url"])
                self.assertNotIn("text", q, "rejected comments keep no text")
                self.assertNotIn("login", q, "rejected comments keep no login")


if __name__ == "__main__":
    unittest.main()
