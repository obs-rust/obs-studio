#!/usr/bin/env -S uv run --script
"""The docs version stamp is one Python call, not a shell block (#962, GEN-005).

    uv run python -m unittest ci/test_docs_version_stamp.py

`.github/actions/generate-docs/action.yaml`'s `setup` step ran a 30-line shell
block that read the libobs API version out of `libobs/obs-config.h` and rewrote
`docs/sphinx/conf.py` with sed -- a GEN-005 finding whose fix is "one call to a
Python script". The step is now a single
`uv run --no-project --script ci/docs_version_stamp.py` line, the module below
does the work, and the action's GEN-005 `[[exceptions]]` block is gone.

The action's three callers (`ci.yml`, `push.yaml`, `dispatch.yaml`) run the
`update-documentation` job on a bare `ubuntu-26.04` runner with no uv on PATH, so the
action installs it itself, exactly as `compatibility-validator` does.

RED first: committed while the step was still shell and the module did not
exist; every test here failed then (no module to import, the run step was a
`run: |` block, and the ci.toml exception was still present).
"""

from __future__ import annotations

from datetime import date
import importlib.util
import io
import os
from pathlib import Path
import tempfile
import tomllib
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parent.parent
MODULE = ROOT / 'ci' / 'docs_version_stamp.py'
ACTION = ROOT / '.github' / 'actions' / 'generate-docs' / 'action.yaml'
CI_TOML = ROOT / 'ci.toml'
ACTION_PATH = '.github/actions/generate-docs/action.yaml'
CALL = 'uv run --no-project --script ci/docs_version_stamp.py'

OBS_CONFIG = """\
#define LIBOBS_API_MAJOR_VER 33
#define LIBOBS_API_MINOR_VER 0
#define LIBOBS_API_PATCH_VER 0
"""

CONF_PY = """\
project = 'OBS Studio'
copyright = '2017-2022, Lain Bailey'
version = '27.2.0'
release = '27.2.0'
html_link_suffix = None
"""


def load_module():
    spec = importlib.util.spec_from_file_location('docs_version_stamp', MODULE)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class DocsVersionStampTest(unittest.TestCase):
    def checkout(self, directory: Path) -> Path:
        (directory / 'libobs').mkdir(parents=True)
        (directory / 'libobs' / 'obs-config.h').write_text(OBS_CONFIG, encoding='utf-8')
        (directory / 'docs' / 'sphinx').mkdir(parents=True)
        (directory / 'docs' / 'sphinx' / 'conf.py').write_text(CONF_PY, encoding='utf-8')
        return directory

    def run_stamp(self, disable_link_extensions: bool) -> tuple[dict[str, str], str]:
        module = load_module()
        with tempfile.TemporaryDirectory() as raw:
            checkout = self.checkout(Path(raw))
            outputs = module.stamp(
                checkout,
                disable_link_extensions=disable_link_extensions,
                env={'GITHUB_SHA': '637558458deadbeef'},
            )
            conf = (checkout / 'docs' / 'sphinx' / 'conf.py').read_text(encoding='utf-8')
        return outputs, conf

    def test_stamps_version_release_and_copyright(self):
        outputs, conf = self.run_stamp(disable_link_extensions=False)
        self.assertIn("version = '33.0.0'", conf)
        self.assertIn("release = '33.0.0'", conf)
        self.assertIn(f"copyright = '2017-{date.today().year}, Lain Bailey'", conf)
        self.assertEqual(outputs['artifactName'], 'OBS Studio Docs')
        self.assertEqual(outputs['commitHash'], '637558458')

    def test_disable_link_extensions(self):
        outputs, conf = self.run_stamp(disable_link_extensions=True)
        self.assertIn("html_link_suffix = ''", conf)
        self.assertEqual(outputs['artifactName'], 'OBS Studio Docs (No Extensions)')

    def test_link_extensions_kept_by_default(self):
        _, conf = self.run_stamp(disable_link_extensions=False)
        self.assertIn('html_link_suffix = None', conf)

    def test_outputs_are_written_to_the_step_file(self):
        module = load_module()
        with tempfile.TemporaryDirectory() as raw:
            checkout = self.checkout(Path(raw))
            output_file = Path(raw) / 'github_output'
            with mock.patch.dict(
                os.environ,
                {'GITHUB_OUTPUT': str(output_file), 'GITHUB_SHA': 'abcdef123456'},
            ):
                module.main(['--source-directory', str(checkout)])
            self.assertEqual(
                output_file.read_text(encoding='utf-8'),
                'artifactName=OBS Studio Docs\ncommitHash=abcdef123\n',
            )

    def test_missing_version_definition_is_an_error(self):
        module = load_module()
        with tempfile.TemporaryDirectory() as raw:
            checkout = self.checkout(Path(raw))
            (checkout / 'libobs' / 'obs-config.h').write_text(
                '#define LIBOBS_API_MAJOR_VER 33\n', encoding='utf-8'
            )
            with self.assertRaises(SystemExit):
                module.read_version(checkout)


class GenerateDocsActionWiringTest(unittest.TestCase):
    def test_action_step_is_one_python_call(self):
        text = ACTION.read_text(encoding='utf-8')
        self.assertTrue(CALL in text, 'the setup step must call the module through uv')
        setup = text.split('id: setup', 1)[1].split('- name:', 1)[0]
        run_lines = [line for line in setup.splitlines() if 'run:' in line]
        self.assertEqual(
            len(run_lines),
            1,
            'the setup step keeps exactly one run: line (GEN-005 shell budget)',
        )
        self.assertNotIn('run: |', setup, 'the setup step must not be a shell block')

    def test_action_installs_uv_before_the_call(self):
        text = ACTION.read_text(encoding='utf-8')
        self.assertIn('astral-sh/setup-uv@', text)
        self.assertIn('cache-suffix:', text)
        self.assertLess(text.index('astral-sh/setup-uv@'), text.index(CALL))

    def test_no_gen_005_exception_left(self):
        data = tomllib.loads(CI_TOML.read_text(encoding='utf-8'))
        stale = [
            entry
            for entry in data.get('exceptions', [])
            if entry.get('rule') == 'GEN-005'
            and entry.get('path') == ACTION_PATH
        ]
        self.assertEqual(
            stale,
            [],
            'the GEN-005 exception for generate-docs must be deleted with the port',
        )


class HelperTest(unittest.TestCase):
    def test_write_outputs_ignores_a_missing_path(self):
        module = load_module()
        with mock.patch('sys.stdout', new=io.StringIO()):
            module.write_outputs({'a': 'b'}, None)


if __name__ == '__main__':
    unittest.main()
