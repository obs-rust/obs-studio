#!/usr/bin/env -S uv run --script
"""Build scripts compile their C in parallel (#3253, #3249).

    uv run python -m unittest ci/test_cc_parallel.py

The `cc` crate compiles a build's C files one at a time unless its
`parallel` feature is on, and its default features are empty. obs-c-oracle
alone compiles more than 225 translation units, and any change under libobs/
reruns it, so a serial cold rebuild cost tens of minutes. Workspace
dependencies carry no features (ci/test_workspace_dependencies.py), so every
member that depends on `cc` enables `parallel` itself.
"""

from __future__ import annotations

import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


class CcParallelTest(unittest.TestCase):
    def test_every_cc_dependency_compiles_in_parallel(self):
        errors = []
        found = 0
        for path in sorted((ROOT / "rust").glob("*/Cargo.toml")):
            manifest = tomllib.loads(path.read_text(encoding="utf-8"))
            for table in ("dependencies", "build-dependencies", "dev-dependencies"):
                cc = manifest.get(table, {}).get("cc")
                if cc is None:
                    continue
                found += 1
                features = cc.get("features", []) if isinstance(cc, dict) else []
                if "parallel" not in features:
                    errors.append(f"{path.relative_to(ROOT)} [{table}] cc lacks the parallel feature")
        self.assertGreater(found, 0, "no crate depends on cc; update this test")
        self.assertEqual(errors, [], "\n".join(errors))


if __name__ == "__main__":
    unittest.main()
