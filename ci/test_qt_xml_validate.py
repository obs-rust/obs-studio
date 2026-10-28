#!/usr/bin/env -S uv run --script
"""The Qt UI XML validator action is one Python call per step (#2724, GEN-005).

`.github/actions/qt-xml-validator/action.yaml` carried three multi-line `run:`
blocks -- the Windows runner guard, the `libxml2-utils` install and the
`xmllint` schema check over the changed `frontend/forms/**/*.ui` files -- so
the action produced one `GEN-005` finding per block. The port keeps the same
behaviour in `ci/qt_xml_validate.py`, called once per step.

This pin fails on master (the module is absent, the action still carries the
shell blocks and `ci.toml` still has the path-scoped `GEN-005` exception) and
passes with the port in place. It drives the module directly, with a stub
`xmllint` on `PATH`, so it needs no `libxml2-utils` and no network.

Usage:
    uv run python -m unittest ci/test_qt_xml_validate.py
"""

import importlib.util
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MODULE = ROOT / "ci" / "qt_xml_validate.py"
ACTION = ROOT / ".github" / "actions" / "qt-xml-validator" / "action.yaml"
CI_TOML = ROOT / "ci.toml"


def load_module():
    """Import `ci/qt_xml_validate.py` by path (it is not a package member)."""

    spec = importlib.util.spec_from_file_location("qt_xml_validate", MODULE)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class PortedActionTest(unittest.TestCase):
    def test_module_exists(self):
        self.assertTrue(MODULE.is_file(), "ci/qt_xml_validate.py is missing")

    def test_action_has_no_multiline_shell_block(self):
        text = ACTION.read_text(encoding="utf-8")
        self.assertNotIn(
            "run: |",
            text,
            "the action still carries a multi-line shell `run:` block",
        )

    def test_action_calls_the_module_for_both_steps(self):
        text = ACTION.read_text(encoding="utf-8")
        self.assertIn("ci/qt_xml_validate.py guard", text)
        self.assertIn("ci/qt_xml_validate.py validate", text)
        for line in text.splitlines():
            if "run:" in line:
                self.assertIn(
                    "uv run",
                    line,
                    f"a run step must be a single `uv run` call: {line.strip()}",
                )

    def test_gen_005_exception_is_gone(self):
        self.assertNotIn(
            'path = ".github/actions/qt-xml-validator/action.yaml"',
            CI_TOML.read_text(encoding="utf-8"),
            "the GEN-005 exception for this action must go with the port",
        )


class ChangedFilesTest(unittest.TestCase):
    def test_parses_the_check_changes_json_list(self):
        module = load_module()
        self.assertEqual(
            module.parse_changed_files('["frontend/forms/a.ui","frontend/forms/b.ui "]'),
            ["frontend/forms/a.ui", "frontend/forms/b.ui"],
        )

    def test_parses_an_empty_list(self):
        module = load_module()
        self.assertEqual(module.parse_changed_files("[]"), [])
        self.assertEqual(module.parse_changed_files(""), [])


class ValidateStepTest(unittest.TestCase):
    def setUp(self):
        self.scratch = Path(tempfile.mkdtemp(prefix="qt-xml-"))
        self.addCleanup(self._cleanup)
        self.calls = self.scratch / "xmllint-calls.txt"
        self.apt_calls = self.scratch / "apt-calls.txt"
        self._stub("xmllint", f'printf "%s\\n" "$@" >> "{self.calls}"\nexit 0\n')
        self._stub("sudo", f'printf "%s\\n" "$@" >> "{self.apt_calls}"\n')
        self._stub("apt-get", f'printf "%s\\n" "$@" >> "{self.apt_calls}"\n')

    def _cleanup(self):
        for path in self.scratch.iterdir():
            path.unlink()
        self.scratch.rmdir()

    def _stub(self, name, body):
        script = self.scratch / name
        script.write_text(f"#!/bin/sh\n{body}", encoding="utf-8")
        script.chmod(0o755)

    def run_module(self, *argv):
        env = dict(os.environ)
        env["PATH"] = os.pathsep.join([str(self.scratch), env.get("PATH", "/usr/bin:/bin")])
        return subprocess.run(
            [sys.executable, str(MODULE), *argv],
            capture_output=True,
            text=True,
            env=env,
            check=False,
        )

    def validate(self, changed, fail_condition="never"):
        return self.run_module(
            "validate",
            "--changed-files-json",
            changed,
            "--schema",
            "frontend/forms/XML-Schema-Qt5.15.xsd",
            "--fail-condition",
            fail_condition,
        )

    def test_passes_every_changed_file_to_xmllint(self):
        completed = self.validate('["frontend/forms/a.ui","frontend/forms/b.ui"]')
        self.assertEqual(completed.returncode, 0, completed.stderr)
        argv = self.calls.read_text(encoding="utf-8").split()
        self.assertEqual(
            argv,
            ["--schema", "frontend/forms/XML-Schema-Qt5.15.xsd", "--noout", "frontend/forms/a.ui", "frontend/forms/b.ui"],
        )

    def test_installs_nothing_when_xmllint_is_present(self):
        self.validate('["frontend/forms/a.ui"]')
        self.assertFalse(self.apt_calls.exists(), "apt-get ran although xmllint is on PATH")

    def test_empty_list_runs_no_check(self):
        completed = self.validate("[]")
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertFalse(self.calls.exists(), "xmllint ran with no changed file")


class FailConditionTest(unittest.TestCase):
    def setUp(self):
        self.scratch = Path(tempfile.mkdtemp(prefix="qt-xml-fail-"))
        self.addCleanup(self._cleanup)
        script = self.scratch / "xmllint"
        script.write_text("#!/bin/sh\nexit 3\n", encoding="utf-8")
        script.chmod(0o755)

    def _cleanup(self):
        (self.scratch / "xmllint").unlink()
        self.scratch.rmdir()

    def run_module(self, fail_condition):
        env = dict(os.environ)
        env["PATH"] = os.pathsep.join([str(self.scratch), env.get("PATH", "/usr/bin:/bin")])
        return subprocess.run(
            [
                sys.executable,
                str(MODULE),
                "validate",
                "--changed-files-json",
                '["frontend/forms/a.ui"]',
                "--schema",
                "frontend/forms/XML-Schema-Qt5.15.xsd",
                "--fail-condition",
                fail_condition,
            ],
            capture_output=True,
            text=True,
            env=env,
            check=False,
        )

    def test_never_is_advisory(self):
        completed = self.run_module("never")
        self.assertEqual(completed.returncode, 0, completed.stderr)

    def test_error_returns_the_check_status(self):
        completed = self.run_module("error")
        self.assertEqual(completed.returncode, 3, completed.stderr)


class GuardStepTest(unittest.TestCase):
    def test_prints_the_notice_and_exits_2(self):
        completed = subprocess.run(
            [sys.executable, str(MODULE), "guard", "--notice", "requires Linux or macOS"],
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertEqual(completed.returncode, 2)
        self.assertIn("::notice::requires Linux or macOS", completed.stdout)


class SchemaMatchesTheFormsTest(unittest.TestCase):
    def test_the_pinned_schema_exists(self):
        self.assertTrue((ROOT / "frontend" / "forms" / "XML-Schema-Qt5.15.xsd").is_file())

    def test_the_action_passes_the_pinned_schema(self):
        text = ACTION.read_text(encoding="utf-8")
        self.assertIn("XML-Schema-Qt5.15.xsd", text)


if __name__ == "__main__":
    unittest.main()
