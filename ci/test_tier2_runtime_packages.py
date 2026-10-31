"""Runtime manifests must fail closed and retain dynamically loaded services (#1694)."""
import importlib.util
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parent.parent
SCRIPT = ROOT / 'rust/tools/linux-validate/runtime-packages.py'


def module():
    spec = importlib.util.spec_from_file_location('tier2_runtime_packages', SCRIPT)
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


class RuntimePackagesTest(unittest.TestCase):
    def test_loader_paths_include_transitive_dependencies_and_loader(self):
        m = module()
        text = '''linux-vdso.so.1 (0xabc)
libobs.so.0 => /build/tree/libobs/libobs.so.0 (0x123)
libc.so.6 => /usr/lib/x86_64-linux-gnu/libc.so.6 (0x234)
/lib64/ld-linux-x86-64.so.2 (0x456)
'''
        self.assertEqual(m.library_paths(text), {
            '/build/tree/libobs/libobs.so.0',
            '/usr/lib/x86_64-linux-gnu/libc.so.6',
            '/lib64/ld-linux-x86-64.so.2',
        })

    def test_missing_library_is_an_error_instead_of_silently_dropping_coverage(self):
        with self.assertRaisesRegex(ValueError, 'not found'):
            module().library_paths('libvlc.so.5 => not found\n')

    def test_multiarch_ownership_and_exact_version_are_preserved(self):
        m = module()
        self.assertEqual(m.package_names('libc6:amd64: /usr/lib/libc.so.6\n'), {'libc6:amd64'})
        self.assertEqual(m.pinned_package('libc6:amd64', '2.42-0ubuntu3'), 'libc6:amd64=2.42-0ubuntu3')

    def test_loader_diversions_are_not_package_ownership_records(self):
        m = module()
        diversion = 'diversion by libc6 from: /lib64/ld-linux-x86-64.so.2\n'
        self.assertEqual(m.package_names(diversion), set())
        self.assertEqual(m.package_names(diversion + 'libc6:amd64: /usr/lib64/ld-linux-x86-64.so.2\n'), {'libc6:amd64'})

    def test_malformed_package_metadata_cannot_become_apt_options(self):
        m = module()
        for name, version in [('--allow-unauthenticated', '1'), ('libc6', '1\n--option'), ('libc6', '')]:
            with self.subTest(name=name, version=version), self.assertRaises(ValueError):
                m.pinned_package(name, version)
        with self.assertRaises(ValueError):
            m.package_names('unrecognized ownership output')

    def test_non_elf_and_build_only_objects_are_not_runtime_inputs(self):
        import tempfile
        m = module()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            for name, data in [('test', b'\x7fELFpayload'), ('obj.o', b'\x7fELFobject'), ('data.txt', b'data')]:
                (root / name).write_bytes(data)
            (root / 'cargo').mkdir()
            (root / 'cargo' / 'build-script').write_bytes(b'\x7fELFbuildscript')
            self.assertEqual(list(m.elf_files(root)), [root / 'test'])

    def test_batch_ownership_retains_per_path_fallback_for_loader_diversion(self):
        import subprocess
        from unittest.mock import patch
        m = module()
        response = subprocess.CompletedProcess([], 1,
            'libc6:amd64: /usr/lib/libc.so.6\n'
            'diversion by libc6 from: /lib64/ld-linux-x86-64.so.2\n', '')
        with patch.object(m, 'query', return_value=response) as query, \
             patch.object(m, 'owners', return_value={'libc6:amd64'}) as fallback:
            self.assertEqual(m.ownership_packages(['/usr/lib/libc.so.6', '/lib64/ld-linux-x86-64.so.2']), {'libc6:amd64'})
            self.assertEqual(query.call_count, 1)
            fallback.assert_called_once_with('/lib64/ld-linux-x86-64.so.2')

    def test_batch_ownership_cannot_hide_an_unowned_library(self):
        import subprocess
        from unittest.mock import patch
        m = module()
        response = subprocess.CompletedProcess([], 1, 'libc6:amd64: /usr/lib/libc.so.6\n', 'no match')
        with patch.object(m, 'query', return_value=response), \
             patch.object(m, 'owners', side_effect=ValueError('No installed package')):
            with self.assertRaisesRegex(ValueError, 'No installed package'):
                m.ownership_packages(['/usr/lib/libc.so.6', '/missing.so'])

    def test_runtime_only_inputs_cover_non_linked_drivers_plugins_and_tools(self):
        m = module()
        for name in ('libgl1-mesa-dri', 'libegl-mesa0', 'vlc-plugin-base', 'pipewire',
                     'dbus-daemon', 'xvfb', 'xauth', 'cmake', m.PYTHON_RUNTIME_PACKAGE, 'ninja-build',
                     'gcc', 'libc6-dev', 'libvlc-dev',
                     # test_v4l2_vmic_server runs a private pipewire-pulse (#3186).
                     'pipewire-pulse', 'wireplumber', 'pulseaudio-utils'):
            self.assertIn(name, m.RUNTIME_TOOLS)
        self.assertNotIn('build-essential', m.RUNTIME_TOOLS)


class RuntimeWorkflowTest(unittest.TestCase):
    def test_manifest_is_produced_and_transferred_before_runtime_install(self):
        text = (ROOT / '.github/workflows/build-project.yaml').read_text()
        build = text.split('  rust-linux-tier2-build:', 1)[1].split('  rust-linux-tier2:', 1)[0]
        tests = text.split('  rust-linux-tier2:', 1)[1].split('  rust-linux-tier2-done:', 1)[0]
        self.assertIn('runtime-packages.py', build)
        self.assertLess(build.index('runtime-packages.py'), build.index('Upload Tier 2 Tree'))
        self.assertIn('path: ${{ runner.temp }}/tier2-pack/', build)
        self.assertLess(tests.index('actions/download-artifact@'), tests.index('ci-runtime.py'))
        self.assertIn('--packages "$RUNNER_TEMP/tier2-pack/runtime-packages.txt"', tests)
        helper = (ROOT / 'rust/tools/linux-validate/ci-runtime.py').read_text()
        self.assertLess(helper.index("selection.records(requirements, '=')"), helper.index('prepared_image(descriptor, requirements, args.state)'))
        self.assertIn('*requirements.splitlines()', helper)
        self.assertNotIn("grep -v '^#'", helper)


if __name__ == '__main__':
    unittest.main()
