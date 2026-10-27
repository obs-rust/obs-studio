#!/usr/bin/env -S uv run --script
"""The AppImage build saves its compiler cache (#1832).

    uv run python -m unittest ci/test_appimage_zccache_save.py

The build steps run as root in the glibc 2.28 container
(build-aux/appimage/manylinux/run.py), so zccache creates .zccache owned by
root with a private mode. actions/cache/save runs as the runner user: it
warned "EACCES: permission denied, scandir '.zccache'", saved nothing and
still passed, so every AppImage build compiled cold (master release run
40097160881: 322 total, 0 cached). The cache goes back to the runner user
before it is saved.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = ROOT / ".github" / "workflows" / "appimage.yaml"


def steps() -> list[str]:
    text = WORKFLOW.read_text(encoding="utf-8")
    return re.split(r"\n(?=      - (?:name|uses): )", text)


class AppImageZccacheSave(unittest.TestCase):
    def test_cache_is_returned_to_the_runner_user_before_save(self) -> None:
        all_steps = steps()
        save = next(i for i, s in enumerate(all_steps) if "name: Save zccache" in s)
        build = next(i for i, s in enumerate(all_steps) if "name: Build 🧱" in s)
        between = all_steps[build + 1 : save]
        owner = [s for s in between if re.search(r"sudo chown -R \"\$\(id -u\):\$\(id -g\)\" \.zccache", s)]
        self.assertEqual(len(owner), 1, "no step returns .zccache to the runner user before Save zccache")
        # It must run whenever the save runs, or a failed build saves nothing.
        cond = re.search(r"if: (.*)", all_steps[save]).group(1)
        self.assertIn(f"if: {cond}", owner[0])


if __name__ == "__main__":
    unittest.main()
