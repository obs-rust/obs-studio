#!/usr/bin/env -S uv run --script
"""Tests for ci/flatpak_builder_lint.py, the Python port of the flatpak-builder-lint
composite action (GEN-005, obs-rust/obs-studio#2700).

The old action had two inline `run:` blocks -- an artifact-type guard and the
`flatpak-builder-lint` invocation with its two-stage `jq` annotation pipeline. Both
must be gone, because the GEN-005 exception is path-scoped: one left behind keeps the
whole path in the baseline.

    uv run python -m unittest ci/test_flatpak_builder_lint.py
"""

from __future__ import annotations

import contextlib
import importlib.util
import io
import json
import os
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MODULE = ROOT / "ci" / "flatpak_builder_lint.py"
ACTION = ROOT / ".github" / "actions" / "flatpak-builder-lint" / "action.yaml"

ARTIFACTS = ("builddir", "repo", "manifest", "appstream")


def load_module():
    if not MODULE.is_file():
        raise AssertionError(f"{MODULE.relative_to(ROOT)} is missing: the action still runs shell")
    spec = importlib.util.spec_from_file_location("flatpak_builder_lint", MODULE)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def run(module, argv):
    """Call the module's main, returning (exit code, stdout, stderr)."""
    out, err = io.StringIO(), io.StringIO()
    with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
        code = module.main(argv)
    return code, out.getvalue(), err.getvalue()


class StubTool:
    """A `flatpak-builder-lint` stand-in that prints a fixed body and exits with a code."""

    def __init__(self, body: str, code: int = 0):
        self.dir = tempfile.TemporaryDirectory()
        self.path = Path(self.dir.name) / "flatpak-builder-lint"
        self.path.write_text(
            f"#!{sys.executable}\n"
            "import sys\n"
            f"sys.stdout.write({body!r})\n"
            f"sys.exit({code})\n"
        )
        self.path.chmod(0o755)
        self.exceptions = Path(self.dir.name) / "exceptions.json"
        self.exceptions.write_text("{}")

    def argv(self, artifact: str, path: str = "flatpak_app") -> list[str]:
        return [
            "lint",
            "--artifact",
            artifact,
            "--path",
            path,
            "--exceptions",
            str(self.exceptions),
            "--tool",
            str(self.path),
        ]

    def close(self) -> None:
        self.dir.cleanup()


class ActionHasNoShell(unittest.TestCase):
    """GEN-005: the action must call the module instead of carrying inline shell."""

    def test_action_has_no_inline_run_blocks(self):
        text = ACTION.read_text()
        inline = [
            line.strip()
            for line in text.splitlines()
            if line.lstrip().startswith("run:") and line.rstrip().endswith(("|", ">"))
        ]
        self.assertEqual(inline, [], "port the remaining run: block to ci/flatpak_builder_lint.py")

    def test_action_calls_the_module(self):
        text = ACTION.read_text()
        self.assertEqual(text.count("ci/flatpak_builder_lint.py"), 2, text)
        self.assertEqual(text.count("uv run --no-project"), 2, text)


class Guard(unittest.TestCase):
    """The artifact-type guard: four accepted kinds, everything else exits 2."""

    def test_accepted_artifacts(self):
        module = load_module()
        for artifact in ARTIFACTS:
            code, out, _ = run(module, ["guard", "--artifact", artifact])
            self.assertEqual((code, out), (0, ""), artifact)

    def test_rejected_artifact_annotates_and_exits_2(self):
        module = load_module()
        code, out, _ = run(module, ["guard", "--artifact", "metainfo"])
        self.assertEqual(code, 2)
        self.assertEqual(out, "::error::Given artifact type is incorrect\n")


class Lint(unittest.TestCase):
    def setUp(self):
        self.module = load_module()

    def test_warnings_and_errors_become_annotations(self):
        body = json.dumps(
            {"warnings": ["unused-key", "old-runtime"], "errors": ["missing-icon"]}
        )
        stub = StubTool(body, code=1)
        self.addCleanup(stub.close)
        code, out, _ = run(self.module, stub.argv("builddir"))
        self.assertEqual(code, 1)
        self.assertEqual(
            out.splitlines(),
            [
                "::warning:: unused-key found in the Flatpak builddir",
                "::warning:: old-runtime found in the Flatpak builddir",
                "::error:: missing-icon found in the Flatpak builddir",
            ],
        )

    def test_clean_body_annotates_nothing(self):
        stub = StubTool(json.dumps({"warnings": [], "errors": []}), code=0)
        self.addCleanup(stub.close)
        code, out, _ = run(self.module, stub.argv("repo"))
        self.assertEqual((code, out), (0, ""))

    def test_missing_keys_are_treated_as_empty(self):
        stub = StubTool(json.dumps({"errors": []}), code=0)
        self.addCleanup(stub.close)
        code, out, _ = run(self.module, stub.argv("manifest"))
        self.assertEqual((code, out), (0, ""))

    def test_appstream_prints_the_body_and_returns_the_tool_exit_code(self):
        body = "appstream: ok"
        stub = StubTool(body, code=0)
        self.addCleanup(stub.close)
        code, out, _ = run(self.module, stub.argv("appstream"))
        self.assertEqual((code, out), (0, body + "\n"))

        failing = StubTool(body, code=1)
        self.addCleanup(failing.close)
        code, out, _ = run(self.module, failing.argv("appstream"))
        self.assertEqual(code, 1)
        self.assertEqual(
            out.splitlines(),
            [body, "::error::Flatpak appstream info is not valid"],
        )

    def test_empty_body_with_failure_is_an_error(self):
        stub = StubTool("", code=3)
        self.addCleanup(stub.close)
        code, out, _ = run(self.module, stub.argv("builddir"))
        self.assertEqual(code, 2)
        self.assertEqual(out, "::error::Error while running flatpak-builder-lint\n")


class ToolInvocation(unittest.TestCase):
    """The tool is called with the same arguments the shell produced."""

    def test_arguments(self):
        module = load_module()
        stub = StubTool(json.dumps({}), code=0)
        self.addCleanup(stub.close)
        recorded = []
        def fake(argv, cwd=None):
            recorded.append(argv)
            return 0, ""

        setattr(module, "run_linter", fake)
        module.main(stub.argv("builddir", "flatpak_app"))
        self.assertEqual(
            recorded,
            [
                [
                    str(stub.path),
                    "--exceptions",
                    "--user-exceptions",
                    str(stub.exceptions),
                    "builddir",
                    "flatpak_app",
                ]
            ],
        )


if __name__ == "__main__":
    unittest.main()
