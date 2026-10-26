#!/usr/bin/env -S uv run --script
"""Daily beta signing, notarization, and Gatekeeper recovery (#630).

    uv run python -m unittest ci/test_release_notarization.py
"""

from __future__ import annotations

import ast
import re
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def step(text: str, marker: str) -> str:
    for block in text.split("\n      - "):
        if marker in block:
            return block
    return ""


def input_value(block: str, name: str) -> str:
    match = re.search(rf"^          {name}: (.+)$", block, re.M)
    return match.group(1) if match else "false"


def enabled(expression: str, os_: str, signed: bool, notarized: bool) -> bool:
    """Evaluate the workflow's boolean input for representative action outputs."""
    if expression.strip("'\"") in ("true", "false"):
        return expression.strip("'\"") == "true"
    if not expression.startswith("${{") or not expression.endswith("}}"):
        raise ValueError(f"expected a GitHub boolean expression: {expression}")
    source = expression[3:-2].strip()
    for reference, name in (
        ("matrix.os", "os"),
        ("steps.codesign.outputs.haveCodesignIdent", "signed"),
        ("steps.codesign.outputs.haveNotarizationUser", "notarized"),
    ):
        source = source.replace(reference, name)
    source = source.replace("&&", "and").replace("||", "or")
    context = {"os": os_, "signed": str(signed).lower(), "notarized": str(notarized).lower()}

    def evaluate(node):
        if isinstance(node, ast.Constant):
            return node.value
        if isinstance(node, ast.Name):
            return context[node.id]
        if isinstance(node, ast.BoolOp):
            values = [bool(evaluate(value)) for value in node.values]
            return all(values) if isinstance(node.op, ast.And) else any(values)
        if isinstance(node, ast.Compare) and len(node.ops) == 1 and isinstance(node.ops[0], ast.Eq):
            return evaluate(node.left) == evaluate(node.comparators[0])
        raise ValueError(f"unsupported workflow expression: {ast.dump(node)}")

    return bool(evaluate(ast.parse(source, mode="eval").body))


class ReleaseNotarizationTest(unittest.TestCase):
    def test_release_launches_both_macos_dmg_architectures_with_saved_profiles(self):
        self.assertIn("macos_smoke:", self.workflow, "release never launches its DMGs with saved capture sources")
        self.assertIn("runner: macos-26-intel", self.workflow, "Intel DMG has no native runtime check")
        self.assertIn("name: obs-studio-rust-macos-${{ matrix.target }}", self.workflow)
        self.assertIn("--platform macos", self.workflow)
        self.assertIn("codesign --verify --deep --strict", self.workflow)
        self.assertIn("needs.macos_smoke.result == 'success'", self.workflow)

    def test_beta_bundle_versions_remain_numeric_and_keep_the_full_identity(self):
        with tempfile.TemporaryDirectory() as root:
            driver = Path(root) / "probe.cmake"
            helper = ROOT / "cmake/macos/bundle-version.cmake"
            driver.write_text(
                f'include("{helper.as_posix()}")\n'
                'set(OBS_VERSION_CANONICAL "33.1.0")\n'
                'set(OBS_BUILD_NUMBER "12")\n'
                'set(OBS_RUST_RELEASE_VERSION "0.1.0-beta.20261013")\n'
                'obs_bundle_versions(short build)\n'
                'if(NOT short STREQUAL "0.1.0" OR NOT build STREQUAL "2026.10.13")\n'
                '  message(FATAL_ERROR "Invalid beta bundle versions: ${short}, ${build}")\n'
                'endif()\n'
                'set(OBS_RUST_RELEASE_VERSION "")\n'
                'obs_bundle_versions(short build)\n'
                'if(NOT short STREQUAL "33.1.0" OR NOT build STREQUAL "12")\n'
                '  message(FATAL_ERROR "Upstream bundle versions changed")\n'
                'endif()\n'
            )
            result = subprocess.run(["cmake", "-P", str(driver)], capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        plist = (ROOT / "frontend/cmake/macos/Info.plist.in").read_text()
        self.assertIn("<key>OBSStudioRustVersion</key>", plist)
        self.assertIn("<string>${OBS_RUST_RELEASE_VERSION}</string>", plist)
        self.assertIn("obs_bundle_versions(_obs_bundle_marketing _obs_bundle_build)",
                      (ROOT / "cmake/macos/helpers.cmake").read_text())
        self.assertIn("Print :OBSStudioRustVersion", self.workflow)

    def setUp(self):
        self.workflow = (ROOT / ".github/workflows/release-rust.yaml").read_text()
        self.setup = step(self.workflow, "uses: ./.github/actions/setup-macos-codesigning")
        self.build = step(self.workflow, "name: Build OBS-Studio-Rust")
        self.package = step(self.workflow, "name: Package OBS-Studio-Rust")

    def test_release_imports_the_signing_credentials_before_build(self):
        self.assertTrue(self.setup, "the daily build never imports its Developer ID certificate")
        self.assertIn("if: matrix.os == 'macos'", self.setup)
        self.assertIn("id: codesign", self.setup)
        self.assertLess(self.workflow.index(self.setup), self.workflow.index(self.build))
        for secret in (
            "MACOS_SIGNING_IDENTITY", "MACOS_SIGNING_CERT", "MACOS_SIGNING_CERT_PASSWORD",
            "MACOS_NOTARIZATION_USERNAME", "MACOS_NOTARIZATION_PASSWORD",
        ):
            with self.subTest(secret=secret):
                self.assertIn(f"secrets.{secret}", self.setup)

    def test_flags_follow_available_credentials_on_macos_only(self):
        for os_ in ("macos", "windows", "linux"):
            for signed in (False, True):
                for notarized in (False, True):
                    with self.subTest(os=os_, signed=signed, notarized=notarized):
                        want_signing = os_ == "macos" and signed
                        for block in (self.build, self.package):
                            self.assertEqual(
                                enabled(input_value(block, "codesign"), os_, signed, notarized), want_signing
                            )
                        self.assertEqual(
                            enabled(input_value(self.package, "notarize"), os_, signed, notarized),
                            want_signing and notarized,
                        )

    def test_identity_and_notarization_credentials_reach_the_actions(self):
        for block in (self.build, self.package):
            self.assertIn("steps.codesign.outputs.codesignIdent", input_value(block, "codesignIdent"))
        self.assertIn("steps.codesign.outputs.codesignTeam", input_value(self.build, "codesignTeam"))
        self.assertIn("secrets.MACOS_NOTARIZATION_USERNAME", input_value(self.package, "codesignUser"))
        self.assertIn("secrets.MACOS_NOTARIZATION_PASSWORD", input_value(self.package, "codesignPass"))

    def test_release_notes_carry_no_gatekeeper_workaround(self):
        # The DMGs are signed and notarized (#2430), so the notes need no
        # quarantine instructions and the workflow no step testing them (#2493).
        for instruction in ("Privacy & Security", "Open Anyway", "com.apple.quarantine"):
            self.assertFalse(instruction in self.workflow, f"release workflow still carries {instruction}")
        self.assertFalse("right-click → Open" in self.workflow, "release notes still rely on the old shortcut")
        self.assertFalse("**Pre-release, unsigned.**" in self.workflow, "release notes mislabel signed builds")


if __name__ == "__main__":
    unittest.main()
