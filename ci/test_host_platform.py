#!/usr/bin/env -S uv run --script
"""Host checks in ci/ go through ci/host_platform.py (ci-lint LAYOUT-001).

    uv run python -m unittest ci/test_host_platform.py

`[allow].platform-code` exempts exactly the facade, so a `sys.platform`
comparison anywhere else in `ci/` is the boundary violation the linter
reports. This test is the offline copy of that rule.
"""

import re
import tempfile
import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CI = ROOT / "ci"
FACADE = CI / "host_platform.py"
EXEMPT = {FACADE, Path(__file__)}
SYS_PLATFORM = re.compile(r"\bsys\.platform\b")


def find_sys_platform(root: Path, exempt: set[Path]) -> list[str]:
    """`sys.platform` comparisons in `root/*.py` outside `exempt`."""
    hits = []
    for path in sorted(root.glob("*.py")):
        if path in exempt:
            continue
        for n, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            if SYS_PLATFORM.search(line):
                hits.append(f"{path.relative_to(root.parent).as_posix()}:{n}: {line.strip()}")
    return hits


class HostPlatformTest(unittest.TestCase):
    def test_detector(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp) / "ci"
            root.mkdir()
            (root / "detector.py").write_text("if sys.platform != 'linux':\n    pass\n")
            facade = root / "host_platform.py"
            facade.write_text("import sys\nsys.platform\n")
            hits = find_sys_platform(root, {facade})
            self.assertEqual(hits, ["ci/detector.py:1: if sys.platform != 'linux':"])

    def test_only_the_facade_reads_sys_platform(self):
        hits = find_sys_platform(CI, EXEMPT)
        self.assertEqual(hits, [], "read the host through ci/host_platform.py:\n" + "\n".join(hits))

    def test_facade_is_the_declared_boundary(self):
        allow = tomllib.loads((ROOT / "ci.toml").read_text())["allow"]
        self.assertIn("ci/host_platform.py", allow["platform-code"])

    def test_at_most_one_predicate_names_this_host(self):
        import host_platform as host

        self.assertLessEqual(sum([host.is_macos(), host.is_windows(), host.is_linux()]), 1)


if __name__ == "__main__":
    unittest.main()
