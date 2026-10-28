#!/usr/bin/env -S uv run --script
"""The `./lint` and `./unittest` entry points are Python, not shell (GEN-005).

    uv run python -m unittest ci/test_entry_points.py

Both files were two-line `#!/bin/sh` shims that `exec uv run ci/local_gate.py`:
the logic is already Python, so GEN-005 (`ci-lint precheck`) only ever saw the
shell shebang, and the same two exceptions sat in `ci.toml`. AGENTS.md documents
both commands, so they stay as entry points -- as `uv run --script` launchers
with the gate's lane arguments, so a shell interpreter is no longer in the path.
"""

from __future__ import annotations

import os
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
GATE = ROOT / "ci" / "local_gate.py"
SHEBANG = "#!/usr/bin/env -S uv run --script"

# entry point -> the local_gate.py arguments it must pass through
ENTRY_POINTS = {
    "lint": '"--lane", "lint"',
    "unittest": '"--lane", "rust-test", "--lane", "tier2-linux"',
}


class EntryPointTest(unittest.TestCase):
    def test_entry_points_are_uv_python_scripts(self):
        for name in ENTRY_POINTS:
            with self.subTest(entry_point=name):
                path = ROOT / name
                self.assertTrue(path.is_file(), f"{name} is missing")
                self.assertEqual(path.read_text(encoding="utf-8").splitlines()[0], SHEBANG)
                self.assertTrue(os.access(path, os.X_OK), f"{name} is not executable")

    def test_entry_points_run_the_local_gate(self):
        for name, lanes in ENTRY_POINTS.items():
            with self.subTest(entry_point=name):
                lines = [
                    line
                    for line in (ROOT / name).read_text(encoding="utf-8").splitlines()
                    if line.strip() and not line.lstrip().startswith("#")
                ]
                body = "\n".join(lines)
                self.assertIn("ci/local_gate.py", body)
                self.assertIn(lanes, body)
                self.assertNotIn("/bin/sh", (ROOT / name).read_text(encoding="utf-8"))

    def test_no_shell_shebang(self):
        for name in ENTRY_POINTS:
            with self.subTest(entry_point=name):
                first = (ROOT / name).read_text(encoding="utf-8").splitlines()[0]
                self.assertFalse(first.startswith(("#!/bin/sh", "#!/usr/bin/env bash", "#!/bin/bash")))


if __name__ == "__main__":
    unittest.main()
