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


def git(*args: str, cwd: Path = ROOT) -> subprocess.CompletedProcess:
    return subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True, check=False)


class BrowserSubmoduleTest(unittest.TestCase):
    def test_pointer_keeps_the_warning_renames(self) -> None:
        tree = git("ls-tree", "HEAD", SUBMODULE).stdout.split()
        self.assertEqual(tree[:2], ["160000", "commit"], f"{SUBMODULE} is not a submodule")
        pointer = tree[2]
        if pointer == WARNING_SHADOWING_FIX:
            return
        sub = ROOT / SUBMODULE
        if not (sub / ".git").exists():
            self.skipTest(f"{SUBMODULE} is not checked out; cannot check ancestry of {pointer}")
        has = git("merge-base", "--is-ancestor", WARNING_SHADOWING_FIX, pointer, cwd=sub)
        if has.returncode != 0:
            known = git("cat-file", "-e", f"{WARNING_SHADOWING_FIX}^{{commit}}", cwd=sub).returncode == 0
            shallow = git("rev-parse", "--is-shallow-repository", cwd=sub).stdout.strip() == "true"
            if not known and shallow:
                # A shallow checkout may stop above the fix, so a miss here proves nothing.
                self.skipTest(f"{SUBMODULE} history is shallow; cannot check ancestry of {pointer[:9]}")
        self.assertEqual(
            has.returncode,
            0,
            f"{SUBMODULE} points at {pointer[:9]}, which lacks the /W4 shadowing renames "
            f"({WARNING_SHADOWING_FIX[:9]})",
        )


if __name__ == "__main__":
    unittest.main()
