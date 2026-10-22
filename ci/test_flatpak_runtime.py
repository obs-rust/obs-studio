"""Flatpak packaged runtime evidence must fail closed (#1760)."""
import importlib.util
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


class FlatpakRuntimeTest(unittest.TestCase):
    def module(self):
        spec = importlib.util.spec_from_file_location('flatpak_runtime', ROOT / 'ci/flatpak_runtime.py')
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module

    def test_browser_pixels_require_real_green_frames(self):
        module = self.module()
        module.check_browser_pixels(bytes([0, 255, 0]) * 150)
        for pixels in (b'', bytes([0, 255, 0]) * 119, bytes([0, 0, 0]) * 150,
                       bytes([255, 0, 0]) * 150, bytes([0, 255, 0]) * 149 + bytes([0, 0, 0])):
            with self.subTest(pixels=len(pixels)), self.assertRaises(ValueError):
                module.check_browser_pixels(pixels)

    def test_capture_log_requires_every_source_and_plugin(self):
        module = self.module()
        lines = ['[pipewire] Screencast session created', 'state: "streaming"', 'state: "streaming"',
                 '[camera-portal] Found device obs-test-camera', "Started recording from 'obs-test-mic'",
                 'Recording Start', 'Loaded Modules:', '    obs-browser.so', '    obs-websocket.so',
                 '    obs-ffmpeg.so', '----------------']
        modules = ['obs-browser.so', 'obs-websocket.so', 'obs-ffmpeg.so']
        module.check_capture_log('\n'.join(lines), modules)
        for index in (0, 1, 3, 4, 5, 7, 8, 9):
            with self.subTest(index=index), self.assertRaises(ValueError):
                module.check_capture_log('\n'.join(lines[:index] + lines[index + 1:]), modules)

    def test_qualification_is_manual_and_preserves_existing_validators(self):
        for name in ('ci.yml', 'build-project.yaml'):
            text = (ROOT / '.github/workflows' / name).read_text()
            self.assertIn('flatpak-runtime-test:', text)
            block = text.split('      flatpak-runtime-test:\n', 1)[1].split('\n      ', 1)[0]
            self.assertIn('default: false', block)
        build = (ROOT / '.github/workflows/build-project.yaml').read_text()
        self.assertIn('uv run ci/flatpak_runtime.py', build)
        self.assertIn('name: Validate repository', build)
        self.assertIn('name: Validate build directory', build)
        self.assertIn('name: Validate AppStream', build)
        job = build.split('  flatpak-runtime-test:\n', 1)[1].split('\n  windows-build:', 1)[0]
        self.assertIn('if: inputs.flatpak-runtime-test', job)
        self.assertNotIn('continue-on-error', job)
