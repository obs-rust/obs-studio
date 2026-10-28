#!/usr/bin/env -S uv run --script
"""plugins/obs-browser must point at a commit that carries the /W4 shadowing fixes.

    uv run python -m unittest ci/test_browser_submodule.py

The MSVC ``/W4`` ratchet (#2626) reports ``browser-app.cpp``'s shadowing locals in
the app build, which no PR path compiles. Those two fixes live in the submodule
(obs-rust/obs-browser#15), so the product build only becomes clean once this
checkout's pointer is at that commit or a descendant of it. A pointer that moves
back to an older submodule commit silently reintroduces the C4456/C4457 findings.
"""

from __future__ import annotations

import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SUBMODULE = "plugins/obs-browser"
# obs-rust/obs-browser#15: rename the browser-app locals that hide a parameter
# and a local (C4457 `frame`, C4456 `args`) for obs-studio#2626.
WARNING_SHADOWING_FIX = "b9c5db137711717bc02cdd265801ad26f270bd66"
# obs-rust/obs-browser#16: rename browser-client.cpp's shadowing `name` (C4456) and
# `channels` (C4458) locals, found in the same app-lane build.
WARNING_SHADOWING_FIX_CLIENT = "96e724b2c948c8500a263320520f971f3f6565c2"
# obs-rust/obs-browser#17: rename the panel locals that hide a class member
# (C4458 `script` in browser-panel-client.cpp, `url` in browser-panel.cpp).
WARNING_SHADOWING_FIX_PANEL = "463e16b468ba607dfea2cc8bfcc1a0204b441ed3"


def git(*args: str, cwd: Path = ROOT) -> subprocess.CompletedProcess:
    return subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True, check=False)


class BrowserSubmoduleTest(unittest.TestCase):
    def _assert_pointer_at_least(self, commit: str, what: str) -> None:
        tree = git("ls-tree", "HEAD", SUBMODULE).stdout.split()
        self.assertEqual(tree[:2], ["160000", "commit"], f"{SUBMODULE} is not a submodule")
        pointer = tree[2]
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
            f"{SUBMODULE} points at {pointer[:9]}, which lacks {what} ({commit[:9]})",
        )

    def test_pointer_keeps_the_browser_app_renames(self) -> None:
        self._assert_pointer_at_least(WARNING_SHADOWING_FIX, "the browser-app shadowing renames (#15)")

    def test_pointer_keeps_the_browser_client_renames(self) -> None:
        self._assert_pointer_at_least(WARNING_SHADOWING_FIX_CLIENT, "the browser-client shadowing renames (#16)")

    def test_pointer_keeps_the_panel_renames(self) -> None:
        self._assert_pointer_at_least(WARNING_SHADOWING_FIX_PANEL, "the panel shadowing renames (#17)")


if __name__ == "__main__":
    unittest.main()
