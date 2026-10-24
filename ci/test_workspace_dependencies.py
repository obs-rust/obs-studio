#!/usr/bin/env -S uv run --script
"""Third-party dependencies are declared once, in [workspace.dependencies] (#2056).

    uv run python -m unittest ci/test_workspace_dependencies.py

The workspace table holds the version, pinned with `=` to the version in
Cargo.lock, and `default-features = false`. Each crate inherits it with
`workspace = true` and says which features it needs (`default-features =
true` for a crate's defaults, `features = [...]` for the others). A
dependency's feature set then shows in the manifests of the crates that
use it, not only in the result of Cargo's feature unification.
"""

from __future__ import annotations

import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
KINDS = ("dependencies", "dev-dependencies", "build-dependencies")


def load(path: Path) -> dict:
    return tomllib.loads(path.read_text(encoding="utf-8"))


def members() -> dict[Path, dict]:
    workspace = load(ROOT / "Cargo.toml")["workspace"]
    excluded = {(ROOT / p).resolve() for p in workspace.get("exclude", [])}
    found = {}
    for pattern in workspace["members"]:
        for path in sorted(ROOT.glob(f"{pattern}/Cargo.toml")):
            if path.parent.resolve() not in excluded:
                found[path] = load(path)
    return found


def third_party(manifest: dict):
    """(table, kind, name, spec) for every non-path dependency, every target."""
    tables = [("", manifest), *((f"target.{t}", v) for t, v in manifest.get("target", {}).items())]
    for table_name, table in tables:
        for kind in KINDS:
            for name, spec in table.get(kind, {}).items():
                if isinstance(spec, dict) and "path" in spec:
                    continue
                yield f"{table_name} {kind}".strip(), name, spec


class WorkspaceDependenciesTest(unittest.TestCase):
    def setUp(self):
        self.workspace = load(ROOT / "Cargo.toml")["workspace"].get("dependencies", {})
        self.members = members()
        self.locked = {}
        for package in load(ROOT / "Cargo.lock")["package"]:
            self.locked.setdefault(package["name"], []).append(package["version"])

    def test_members_inherit_every_third_party_dependency(self):
        errors = [
            f"{path.relative_to(ROOT)} [{kind}] {name}: use `{name} = {{ workspace = true }}`"
            for path, manifest in self.members.items()
            for kind, name, spec in third_party(manifest)
            if not (isinstance(spec, dict) and spec.get("workspace") is True)
        ]
        self.assertEqual(errors, [], "\n".join(errors))

    def test_workspace_entries_pin_the_locked_version_without_features(self):
        errors = []
        for name, spec in sorted(self.workspace.items()):
            if not isinstance(spec, dict):
                errors.append(f"{name}: write it as a table with version and default-features = false")
                continue
            version = spec.get("version", "")
            if not version.startswith("="):
                errors.append(f"{name}: pin the version with `=` (have {version!r})")
            elif self.locked.get(name) != [version[1:]]:
                errors.append(f"{name}: {version} is not the one version in Cargo.lock ({self.locked.get(name)})")
            if spec.get("default-features") is not False:
                errors.append(f"{name}: set default-features = false; crates enable what they need")
            if "features" in spec:
                errors.append(f"{name}: features belong in the crates that need them")
        self.assertEqual(errors, [], "\n".join(errors))

    def test_every_workspace_entry_is_used(self):
        used = {name for manifest in self.members.values() for _, name, _ in third_party(manifest)}
        unused = sorted(set(self.workspace) - used)
        self.assertEqual(unused, [], f"unused [workspace.dependencies]: {unused}")


if __name__ == "__main__":
    unittest.main()
