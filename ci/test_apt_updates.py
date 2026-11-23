#!/usr/bin/env -S uv run --script
"""The package lock must be generated, deterministic and human-diffable.

These tests cover the pure half: parsing apt's simulation output, rendering and
reading the lock, and the drift report a reviewer actually reads. Resolving
against a live archive needs a runner and is exercised by the scheduled job.
"""
from __future__ import annotations

import contextlib
import importlib.util
import io
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parent.parent
HELPER = ROOT / 'ci' / 'apt_updates.py'
WORKFLOW = ROOT / '.github' / 'workflows' / 'apt-updates.yaml'
LOCK = ROOT / 'rust' / 'tools' / 'linux-validate' / 'apt-versions.txt'

SIMULATION = """\
Reading package lists...
Building dependency tree...
The following NEW packages will be installed:
  cmake libcurl4
0 upgraded, 2 newly installed, 0 to remove and 0 not upgraded.
Inst cmake (3.31.6-1ubuntu2 Ubuntu:26.04/resolute [amd64])
Inst libcurl4 (8.12.1-1ubuntu2.6 Ubuntu:26.04/resolute-updates [amd64])
Conf cmake (3.31.6-1ubuntu2 Ubuntu:26.04/resolute [amd64])
Inst git (1:2.51.0-1 Ubuntu:26.04/resolute [amd64])
"""


def load():
    spec = importlib.util.spec_from_file_location('apt_updates', HELPER)
    assert spec is not None and spec.loader is not None, HELPER
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class ResolveTest(unittest.TestCase):
    def setUp(self):
        self.updates = load()

    def test_simulation_lines_become_name_version_pairs(self):
        self.assertEqual(self.updates.parse_installs(SIMULATION), {
            'cmake': '3.31.6-1ubuntu2',
            'libcurl4': '8.12.1-1ubuntu2.6',
            'git': '1:2.51.0-1',
        })

    def test_conf_lines_and_chatter_are_ignored(self):
        versions = self.updates.parse_installs(SIMULATION)
        self.assertNotIn('Building', versions)
        self.assertEqual(len(versions), 3)

    def test_package_list_skips_comments_and_blanks(self):
        names = self.updates.selected_packages()
        self.assertTrue(names)
        self.assertNotIn('', names)
        self.assertFalse([name for name in names if name.startswith('#')])
        self.assertIn('cmake', names)


class ResolveCommandTest(unittest.TestCase):
    def test_the_resolve_command_prints_a_sorted_lock(self):
        # Exercises simulate() + render_lock() + stdout without an apt host: a
        # fake apt-get on PATH answers with the recorded simulation.
        updates = load()
        with tempfile.TemporaryDirectory() as tmp:
            fake = Path(tmp) / 'apt-get'
            fake.write_text('#!/bin/sh\ncat <<EOF\n' + SIMULATION + 'EOF\n')
            fake.chmod(0o755)
            with patch.dict(os.environ, {'PATH': tmp + os.pathsep + os.environ['PATH']}), \
                    patch.object(updates.os, 'geteuid', return_value=0), \
                    patch.object(updates.sys, 'argv', ['apt_updates.py', 'resolve']), \
                    contextlib.redirect_stdout(io.StringIO()) as out:
                updates.main()
        text = out.getvalue()
        self.assertIn('cmake=3.31.6-1ubuntu2', text)
        self.assertLess(text.index('cmake='), text.index('git='))
        self.assertEqual(len(updates.parse_lock(text)), 3)


class LockTest(unittest.TestCase):
    def setUp(self):
        self.updates = load()

    def test_render_sorts_and_round_trips(self):
        text = self.updates.render_lock({'zlib1g': '1:1.3', 'cmake': '3.31.6'})
        self.assertLess(text.index('cmake='), text.index('zlib1g='))
        self.assertEqual(self.updates.parse_lock(text),
                         {'cmake': '3.31.6', 'zlib1g': '1:1.3'})

    def test_a_line_without_a_version_is_refused(self):
        with self.assertRaises(ValueError):
            self.updates.parse_lock('cmake\n')

    def test_drift_is_reported_by_package(self):
        old = {'cmake': '3.30', 'git': '2.50', 'zlib1g': '1:1.3'}
        new = {'cmake': '3.31', 'zlib1g': '1:1.3', 'libcurl4': '8.12'}
        self.assertEqual(self.updates.changed(old, new), [
            'changed cmake: 3.30 -> 3.31',
            'removed git=2.50',
            'added libcurl4=8.12',
        ])

    def test_an_identical_lock_reports_nothing(self):
        versions = {'cmake': '3.31'}
        self.assertEqual(self.updates.changed(versions, dict(versions)), [])


class WorkflowTest(unittest.TestCase):
    def test_it_is_scheduled_and_guarded_off_forks(self):
        text = WORKFLOW.read_text()
        self.assertIn('schedule:', text)
        self.assertIn("github.event_name != 'schedule' || github.repository == 'obs-rust/obs-studio'", text)
        self.assertIn('timeout-minutes:', text)

    def test_it_writes_the_lock_the_workflow_records(self):
        text = WORKFLOW.read_text()
        self.assertIn('ci/apt_updates.py resolve', text)
        self.assertIn(str(LOCK.relative_to(ROOT)), text)

    def test_actions_are_pinned(self):
        import re
        for action in re.findall(r'uses: ([^\s]+)', WORKFLOW.read_text()):
            if action.startswith('./'):
                continue
            self.assertRegex(action, r'@[0-9a-f]{40}$')


if __name__ == '__main__':
    unittest.main()
