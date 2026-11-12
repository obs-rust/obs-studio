#!/usr/bin/env -S uv run --no-project --script
"""Exercise the real client orchestration and shared validators in committed repos."""
from __future__ import annotations

import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


class ClientChecksTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        for name in ("ci/local_gate.py", "ci/host_platform.py", "ci/nextest_tool.py",
                     "ci/client_checks.py", "ci/flatpak_manifest_validate.py", "ci/qt_xml_validate.py",
                     "ci/compatibility_validate.py", "ci/services_validate.py", "ci/run_swift_format.py",
                     "ci/swift_changes.py", "build-aux/format-manifest.py",
                     ".github/scripts/utils.py/check-jsonschema.py", "ci-attestations.yml", "local-gate.toml"):
            target = self.root / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / name, target)
        for directory, data in (("plugins/win-capture/data", "compatibility"),
                                ("plugins/rtmp-services/data", "services")):
            self.write(f"{directory}/schema.json", json.dumps({"type": "object", "properties": {"value": {"type": "integer"}}}))
            for filename in (data, "package"):
                self.write(f"{directory}/{filename}.json", json.dumps({"$schema": "schema.json", "value": 1}))
        self.write("build-aux/io.github.obs_rust.OBSStudioRust.json", '{}\n')
        self.write("frontend/forms/XML-Schema-Qt5.15.xsd", '<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"><xs:element name="ui"/></xs:schema>')
        self.write("frontend/forms/a space.ui", '<ui/>')
        self.write("ci/test_fixture.py", 'import unittest\nclass Test(unittest.TestCase):\n def test_ok(self): self.assertTrue(True)\n')
        self.git("init", "-q")
        self.git("config", "user.name", "Client checks fixture")
        self.git("config", "user.email", "fixture@example.invalid")
        self.git("config", "commit.gpgsign", "false")
        self.commit()
        self.base = self.git("rev-parse", "HEAD").stdout.strip()

    def write(self, name, text):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)

    def git(self, *args):
        return subprocess.run(["git", *args], cwd=self.root, text=True, capture_output=True, check=True)

    def commit(self):
        self.git("add", ".")
        self.git("commit", "--allow-empty", "-qm", "Fixture")

    def checks(self, *, base=None, swift=False, env=None):
        args = ["uv", "run", "--no-project", "ci/client_checks.py", "--base", base or self.base]
        if swift:
            args.append("--swift")
        return subprocess.run(args, cwd=self.root, env={**os.environ, **(env or {})}, text=True, capture_output=True)

    def test_real_validators_reject_bad_inputs_before_stamping(self):
        for name, content in (("plugins/win-capture/data/compatibility.json", '{"$schema":"schema.json","value":"bad"}'),
                              ("plugins/rtmp-services/data/services.json", '{"$schema":"schema.json","value":"bad"}'),
                              ("frontend/forms/a space.ui", '<ui>'),
                              ("build-aux/io.github.obs_rust.OBSStudioRust.json", '{"bad":true}')):
            with self.subTest(name=name):
                self.git("reset", "--hard", self.base)
                self.write(name, content)
                self.commit()
                result = self.checks()
                self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertNotIn("Ci-Attestation:", self.git("log", "-1", "--format=%B").stdout)

    def test_harmless_diff_and_no_diff_validate_full_files(self):
        for changed in (False, True):
            if changed:
                self.write("harmless space.txt", "unchanged checks")
                self.commit()
            result = self.checks()
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertIn("full-file", result.stdout)

    def test_deleted_and_escaped_qt_paths(self):
        self.git("rm", "frontend/forms/a space.ui")
        self.write('frontend/forms/a "quote", space.ui', '<ui/>')
        self.commit()
        result = self.checks()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_missing_base_is_failure_not_an_empty_diff(self):
        result = self.checks(base="f" * 40)
        self.assertNotEqual(result.returncode, 0)

    def test_dirty_inputs_are_not_provable(self):
        self.write("frontend/forms/a space.ui", '<ui>')
        self.assertNotEqual(self.checks().returncode, 0)

    def test_self_test_failure_cannot_be_hidden_by_skip_environment(self):
        self.write("ci/test_fixture.py", 'import unittest\nclass Test(unittest.TestCase):\n def test_bad(self): self.fail("client failure")\n')
        self.commit()
        result = self.checks(env={"OBS_GATE_SKIP_SELF_TEST": "1"})
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("client failure", result.stdout + result.stderr)

    def test_unavailable_swift_emits_no_proof(self):
        self.write("frontend/a.swift", "let value = 1\n")
        self.commit()
        result = self.checks(swift=True, env={"PATH": os.path.dirname(shutil.which("uv")) + ":/usr/bin:/bin"})
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("Ci-Attestation:", self.git("log", "-1", "--format=%B").stdout)


if __name__ == "__main__":
    unittest.main()
