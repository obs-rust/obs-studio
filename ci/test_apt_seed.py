#!/usr/bin/env -S uv run --script
"""The scheduled seed must fill the archive the builds already restore.

The value is in the *parity*: a seed job that writes a different path or key
warms nothing, and the build jobs would keep re-downloading. These tests pin
that parity against build-project.yaml rather than against a copy of the key.
"""
from __future__ import annotations

import importlib.util
from pathlib import Path
import re
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent
HELPER = ROOT / 'ci' / 'seed_apt_cache.py'
BUILD = ROOT / '.github' / 'workflows' / 'build-project.yaml'
SEED = ROOT / '.github' / 'workflows' / 'apt-seed.yaml'


def load():
    spec = importlib.util.spec_from_file_location('seed_apt_cache', HELPER)
    assert spec is not None and spec.loader is not None, HELPER
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def key_of(block: str) -> str:
    match = re.search(r'(?m)^\s+key: (.+)$', block)
    assert match, 'no key in block'
    return match.group(1).strip()


def restore_of(workflow: str) -> str:
    return workflow.split('id: apt-cache', 1)[1].split('\n      - ', 1)[0]


def step(workflow: str, name: str) -> str:
    start = workflow.index(f'- name: {name}')
    return workflow[start:].split('\n      - ', 1)[0]


class PackageListTest(unittest.TestCase):
    def setUp(self):
        self.seed = load()

    def test_comments_and_blanks_are_not_packages(self):
        names = self.seed.selected_packages()
        self.assertTrue(names)
        self.assertNotIn('', names)
        self.assertFalse([name for name in names if name.startswith('#')])

    def test_count_ignores_partials_and_other_files(self):
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            (base / 'a.deb').write_bytes(b'x')
            (base / 'b.deb').write_bytes(b'x')
            (base / 'c.deb.partial').write_bytes(b'x')
            (base / 'lock').write_bytes(b'')
            self.assertEqual(self.seed.archive_count(base), 2)

    def test_a_spent_budget_does_not_run_a_command(self):
        self.assertEqual(self.seed.bounded(['true'], 0), 124)


class ParityTest(unittest.TestCase):
    def setUp(self):
        self.build = BUILD.read_text()
        self.seed = SEED.read_text()

    def test_seed_restores_the_key_shape_the_builds_publish(self):
        producer = self.build.split('- name: Save apt Downloads', 1)[1].split('\n      - ', 1)[0]
        suffix = '-${{ steps.apt-count.outputs.count }}'
        published = key_of(producer)
        self.assertTrue(published.endswith(suffix), published)
        base = published.removesuffix(suffix)
        self.assertIn(base + '-', restore_of(self.seed), 'seed must restore the same prefix')
        self.assertEqual(key_of(step(self.seed, 'Save apt Downloads 💾')),
                         base + '-${{ steps.apt-count.outputs.count }}')

    def test_seed_writes_the_path_the_builds_read(self):
        self.assertIn('path: .tier2-apt-archives/*.deb', restore_of(self.seed))
        self.assertIn('path: .tier2-apt-archives/*.deb',
                      step(self.seed, 'Save apt Downloads 💾'))
        self.assertIn('path: .tier2-apt-archives/*.deb', self.build)

    def test_seed_uses_the_sanctioned_cache_action(self):
        self.assertNotIn('actions/cache@', self.seed)
        self.assertIn('uses: ./.github/actions/cache', self.seed)

    def test_seed_is_scheduled_and_cannot_run_on_a_fork(self):
        self.assertIn('schedule:', self.seed)
        self.assertIn("github.repository == 'obs-rust/obs-studio'", self.seed)
        self.assertIn('timeout-minutes:', self.seed)

    def test_pinned_actions(self):
        for action in re.findall(r'uses: ([^\s]+)', self.seed):
            if action.startswith('./'):
                continue
            self.assertRegex(action, r'@[0-9a-f]{40}$')


if __name__ == '__main__':
    unittest.main()
