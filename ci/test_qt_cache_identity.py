"""Exercise Qt snapshot identity without treating it as build/test attestation."""

import os
from pathlib import Path
import re
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent


class QtCacheIdentityTest(unittest.TestCase):
    def setUp(self):
        text = (ROOT / ".github/workflows/build-project.yaml").read_text()
        self.job = text.split("  linux-qt-frontend:\n", 1)[1].split("\n  rust-tier2-macos:", 1)[0]
        scratch = ROOT / "build_ci_audit"
        scratch.mkdir(exist_ok=True)
        temporary = tempfile.TemporaryDirectory(prefix="qt-identity-test-", dir=scratch)
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        subprocess.run(["git", "init", "-q", str(self.root)], check=True)
        self.stage("frontend/widget.cpp", "int widget() { return 1; }\n")
        self.stage("cmake/options.cmake", "set(OPTION ON)\n")
        self.stage("docs/audit.json", "{}\n")
        self.stage("README.md", "Documentation\n")
        self.stage(".github/workflows/build-project.yaml", "Qt build workflow\n")
        self.stage("ci/linux_qt_frontend.py", "Qt build driver\n")
        self.stage("ci/test_ci_toml.py", "cache policy checks\n")
        self.stage("ci.toml", "cache policy\n")
        self.stage(".github/ISSUE_TEMPLATE/bug.yml", "issue template\n")
        tools = self.root / "tools"
        tools.mkdir()
        self.env = dict(os.environ, PATH=f"{tools}{os.pathsep}{os.environ['PATH']}",
                        RUNNER_TEMP=str(self.root), GITHUB_OUTPUT=str(self.root / "outputs"),
                        QT_SHA256="qt-a", FFMPEG_SHA256="ff-a", ImageVersion="image-a")
        for command, variable in (("cc", "TEST_CC"), ("c++", "TEST_CXX"), ("cmake", "TEST_CMAKE"),
                                  ("ccache", "TEST_CCACHE"), ("ninja", "TEST_NINJA")):
            tool = tools / command
            tool.write_text(f'#!/bin/sh\nprintf "%s\\n" "${{{variable}}}"\n')
            tool.chmod(0o755)
            self.env[variable] = f"{command}-version-a"

    def stage(self, name, content):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content)
        subprocess.run(["git", "add", "--", name], cwd=self.root, check=True)

    def key(self, **overrides):
        identity = re.search(r"id: qt-cache-identity\n[\s\S]*?        run: \|\n((?:          [^\n]*\n|\n)*)",
                             self.job)
        self.assertIsNotNone(identity, "Qt cache publication needs an input identity")
        script = "\n".join(line[10:] for line in identity[1].splitlines())
        output = self.root / "outputs"
        output.write_text("")
        subprocess.run(["bash", "-euo", "pipefail", "-c", script], cwd=self.root,
                       env={**self.env, **overrides}, capture_output=True, text=True, check=True)
        result = output.read_text().strip()
        self.assertRegex(result, r"^digest=[0-9a-f]{64}$")
        return result

    def test_documentation_changes_keep_the_same_snapshot(self):
        initial = self.key()
        self.assertEqual(self.key(), initial)
        self.stage("docs/audit.json", '{"new_measurement": true}\n')
        self.stage("README.md", "Updated documentation\n")
        self.assertEqual(self.key(), initial)

    def test_code_and_build_configuration_change_identity(self):
        initial = self.key()
        for name in ("frontend/widget.cpp", "cmake/options.cmake",
                     ".github/workflows/build-project.yaml", "ci/linux_qt_frontend.py"):
            with self.subTest(path=name):
                original = (self.root / name).read_text()
                self.stage(name, original + "// changed input\n")
                self.assertNotEqual(self.key(), initial)
                self.stage(name, original)
                self.assertEqual(self.key(), initial)

    def test_ci_metadata_changes_keep_the_same_snapshot(self):
        initial = self.key()
        for name in ("ci/test_ci_toml.py", "ci.toml", ".github/ISSUE_TEMPLATE/bug.yml"):
            with self.subTest(path=name):
                original = (self.root / name).read_text()
                try:
                    self.stage(name, "updated CI metadata\n")
                    self.assertEqual(self.key(), initial)
                finally:
                    self.stage(name, original)

    def test_build_and_tests_are_not_conditional_on_cache_hit(self):
        build = self.job.split("- name: Build Frontend and Run Qt Tests", 1)[1].split("- name:", 1)[0]
        self.assertNotIn("if:", build)
        self.assertIn("run: uv run --no-project ci/linux_qt_frontend.py", build)

    def test_compiler_tools_image_and_library_pins_change_identity(self):
        initial = self.key()
        for variable in ("TEST_CC", "TEST_CXX", "TEST_CMAKE", "TEST_CCACHE", "TEST_NINJA",
                         "ImageVersion", "QT_SHA256", "FFMPEG_SHA256"):
            with self.subTest(variable=variable):
                self.assertNotEqual(self.key(**{variable: "changed"}), initial)

    def test_restore_and_save_share_identity_and_exact_hits_skip_upload(self):
        reference = "key: qt-ccache-linux-inputs-v2-${{ steps.qt-cache-identity.outputs.digest }}"
        self.assertEqual(self.job.count(reference), 2)
        self.assertIn("id: qt-compiler-cache", self.job)
        save = self.job.split("- name: Save Qt ccache", 1)[1]
        self.assertIn("steps.qt-compiler-cache.outputs.cache-hit != 'true'", save)
        self.assertIn("github.ref == 'refs/heads/master'", save)
        self.assertIn("restore-keys: qt-ccache-linux-", self.job)
        self.assertLess(self.job.index("- name: Install Build Dependencies"),
                        self.job.index("id: qt-cache-identity"))
        self.assertLess(self.job.index("id: qt-cache-identity"),
                        self.job.index("id: qt-compiler-cache"))


if __name__ == "__main__":
    unittest.main()
