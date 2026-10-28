#!/usr/bin/env -S uv run --script
"""plugins/obs-websocket must point at a commit that has the project fork's
bounded websocket parser (#99).

    uv run python -m unittest ci/test_websocket_submodule.py

#492 moved the submodule back to upstream 073a294cd from a stale checkout
and silently undid the #99 fix. The pointer must be 7d89109c9 or a
descendant of it.
"""

from __future__ import annotations

import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SUBMODULE = "plugins/obs-websocket"
BOUNDED_PARSER = "7d89109c90a60ed119cf38fa4e35eae774959d7f"
# obs-rust/obs-websocket#22: Stop() reads the drain predicate under the session
# mutex and bounds the wait (obs-studio#2713).
BOUNDED_SESSION_DRAIN = "3253206ff1ba62213936f4e9458e0ebf5eb421ea"
# Upstream commits without the fix that stale checkouts have committed (#492, #496).
KNOWN_BAD = {"073a294cd0646fa1744b0ebf3293bfa63a869b50"}


def git(*args: str, cwd: Path = ROOT) -> subprocess.CompletedProcess:
    return subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True, check=False)


class WebsocketSubmoduleTest(unittest.TestCase):
    def _assert_pointer_at_least(self, commit: str, what: str) -> None:
        tree = git("ls-tree", "HEAD", SUBMODULE).stdout.split()
        self.assertEqual(tree[:2], ["160000", "commit"], f"{SUBMODULE} is not a submodule")
        pointer = tree[2]
        self.assertNotIn(pointer, KNOWN_BAD, f"{SUBMODULE} points at upstream {pointer[:9]}, without {what}")
        if pointer == commit:
            return
        sub = ROOT / SUBMODULE
        if not (sub / ".git").exists():
            self.skipTest(f"{SUBMODULE} is not checked out; cannot check ancestry of {pointer}")
        has = git("merge-base", "--is-ancestor", commit, pointer, cwd=sub)
        if has.returncode != 0:
            known = git("cat-file", "-e", f"{commit}^{{commit}}", cwd=sub).returncode == 0
            shallow = git("rev-parse", "--is-shallow-repository", cwd=sub).stdout.strip() == "true"
            if not known and shallow:
                # A shallow checkout may stop above the fix, so a miss here proves nothing.
                self.skipTest(f"{SUBMODULE} history is shallow; cannot check ancestry of {pointer[:9]}")
        self.assertEqual(
            has.returncode,
            0,
            f"{SUBMODULE} points at {pointer[:9]}, which lacks {what} {commit[:9]}",
        )

    def test_pointer_keeps_the_bounded_parser(self):
        self._assert_pointer_at_least(BOUNDED_PARSER, "the bounded parser (#99)")

    def test_pointer_keeps_the_bounded_session_drain(self):
        self._assert_pointer_at_least(BOUNDED_SESSION_DRAIN, "the bounded session drain (#2713)")


class SessionDrainShapeTest(unittest.TestCase):
    """Stop() must read the drain predicate under the session mutex and bound the wait."""

    def setUp(self) -> None:
        self.source = ROOT / SUBMODULE / "src/websocketserver/WebSocketServer.cpp"
        if not self.source.is_file():
            self.skipTest(f"{self.source} is not checked out")
        self.text = self.source.read_text(encoding="utf-8")

    def test_stop_waits_on_the_predicate_under_the_mutex(self) -> None:
        stop = self.text[self.text.index("void WebSocketServer::Stop()") :]
        stop = stop[: stop.index("_serverThread.join()")]
        self.assertIn("_sessionsDrained.wait_for(", stop)
        self.assertIn("_sessions.empty()", stop)
        self.assertIn("lock.lock()", stop)
        self.assertIn("_sessionMutex", self.text, "Stop() must wait on the same mutex onClose() erases under")
        self.assertNotIn("while (_sessions.size() > 0)", self.text, "the unbounded unlocked poll must be gone; it races onClose()'s erase")

    def test_the_wait_is_bounded_and_reports_what_did_not_close(self) -> None:
        self.assertRegex(self.text, r"kSessionDrainTimeout = std::chrono::seconds\(\d+\)")
        self.assertIn("still open after", self.text, "an abandoned drain must name the sessions it gave up on")
        self.assertIn("DescribeSessions(_sessions)", self.text)

    def test_onclose_wakes_the_drain_after_erasing(self) -> None:
        close = self.text[self.text.index("void WebSocketServer::onClose(") :]
        close = close[: close.index("\n}", close.index("_sessionsDrained"))]
        self.assertLess(close.index("_sessions.erase(hdl)"), close.index("_sessionsDrained.notify_all()"))


if __name__ == "__main__":
    unittest.main()
