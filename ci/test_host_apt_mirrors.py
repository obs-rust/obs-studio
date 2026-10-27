#!/usr/bin/env -S uv run --script
"""Exercise mirror preference on isolated files, never the host apt configuration.

The helper is ci/host_apt_mirrors.py (GEN-005, #962); it runs as root in CI,
so every case here runs it by path on a fixture file instead.
"""

from pathlib import Path
import os
import re
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent
SCRIPT = ROOT / 'ci/host_apt_mirrors.py'
# The workflows' invocation: `sudo -E "$(command -v uv)" run --no-project ...`
INVOCATION = 'sudo -E "$(command -v uv)" run --no-project ci/host_apt_mirrors.py'
ORIGINAL = ('# runner mirrors\nhttp://azure.archive.ubuntu.com/ubuntu/\tpriority:1\n'
            'https://archive.ubuntu.com/ubuntu/\tpriority:2\n'
            'https://security.ubuntu.com/ubuntu/\tpriority:3\n')
EXPECTED = ORIGINAL.replace('/ubuntu/\tpriority:1', '/ubuntu/\tpriority:2', 1).replace(
    'https://archive.ubuntu.com/ubuntu/\tpriority:2', 'https://archive.ubuntu.com/ubuntu/\tpriority:1')


class HostAptMirrorsTest(unittest.TestCase):
    def execute(self, text, repeat=1):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'apt-mirrors.txt'
            path.write_text(text)
            path.chmod(0o640)
            for _ in range(repeat):
                result = subprocess.run([sys.executable, str(SCRIPT), str(path)], check=False,
                                        capture_output=True, text=True, timeout=5)
                self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(path.stat().st_mode & 0o777, 0o640)
            self.assertEqual([p.name for p in Path(directory).iterdir()], ['apt-mirrors.txt'])
            return path.read_text()

    def test_known_list_prefers_existing_https_archive_and_keeps_fallbacks(self):
        self.assertEqual(self.execute(ORIGINAL), EXPECTED)

    def test_idempotent(self):
        self.assertEqual(self.execute(ORIGINAL, repeat=2), EXPECTED)
        self.assertEqual(self.execute(EXPECTED), EXPECTED)

    def test_unknown_partial_and_duplicate_layouts_are_unchanged(self):
        for text in ('https://ports.ubuntu.com/ubuntu-ports/\tpriority:1\n',
                     ORIGINAL.replace('priority:1', 'priority:1 arch:amd64'),
                     ORIGINAL.replace('priority:2', 'priority:7'),
                     ORIGINAL + 'http://azure.archive.ubuntu.com/ubuntu/\tpriority:4\n'):
            with self.subTest(text=text):
                self.assertEqual(self.execute(text), text)

    def test_missing_and_linked_lists_are_unchanged(self):
        with tempfile.TemporaryDirectory() as directory:
            missing = Path(directory) / 'absent'
            subprocess.run([sys.executable, str(SCRIPT), str(missing)], check=True,
                           capture_output=True, timeout=5)
            self.assertFalse(missing.exists())
            target = Path(directory) / 'target'
            target.write_text(ORIGINAL)
            link = Path(directory) / 'linked'
            link.symlink_to(target)
            subprocess.run([sys.executable, str(SCRIPT), str(link)], check=True,
                           capture_output=True, timeout=5)
            self.assertTrue(link.is_symlink())
            self.assertEqual(target.read_text(), ORIGINAL)

    def test_rewrite_failure_is_not_reported_as_success(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'apt-mirrors.txt'
            path.write_text(ORIGINAL)
            tools = Path(directory) / 'tools'
            tools.mkdir()
            sed = tools / 'sed'
            sed.write_text('#!/usr/bin/env bash\nexit 42\n')
            sed.chmod(0o755)
            result = subprocess.run([sys.executable, str(SCRIPT), str(path)],
                                    env={**os.environ, 'PATH': str(tools) + os.pathsep + os.environ['PATH']},
                                    capture_output=True, timeout=5, check=False)
            self.assertEqual(result.returncode, 42)
            self.assertEqual(path.read_text(), ORIGINAL)

    def test_existing_host_jobs_prepare_mirrors_before_package_work(self):
        workflow = (ROOT / '.github/workflows/build-project.yaml').read_text()
        for name, boundary in (('rust-tests-linux', 'Install libuuid and GIO'),
                               ('linux-qt-frontend', 'Install Build Dependencies'),
                               ('ubuntu-build', 'Build OBS Studio')):
            body = re.search(rf'^  {name}:\n(.*?)(?=^  [a-z0-9-]+:|\Z)', workflow, re.M | re.S).group(1)
            with self.subTest(job=name):
                self.assertIn(INVOCATION, body)
                if name == 'ubuntu-build':
                    self.assertLess(body.index(INVOCATION), body.index(boundary))
                else:
                    script = body.split(boundary, 1)[1]
                    self.assertLess(script.index(INVOCATION), script.index('sudo apt-get'))
                # The helper is Python: the step has to have uv before it, either
                # in this job or in a setup-uv step of its own.
                self.assertIn('setup-uv', body.split(INVOCATION, 1)[0])


if __name__ == '__main__':
    unittest.main()
