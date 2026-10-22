"""Opt-in compiler observations must not expose commands or change defaults."""

import copy
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from flatpak_compiler_diagnostics import configure, summarize


class FlatpakCompilerDiagnosticsTest(unittest.TestCase):
    def test_only_obs_logging_environment_changes(self):
        manifest = {"modules": [{"name": "cef", "build-options": {"env": {"X": "1"}}},
                                {"name": "obs", "build-options": {"env": {"CCACHE_DEPEND": "true"}}}]}
        original = copy.deepcopy(manifest)
        result = configure(manifest)
        self.assertEqual(manifest, original)
        self.assertEqual(result["modules"][0], original["modules"][0])
        env = result["modules"][1]["build-options"]["env"]
        self.assertEqual(env.pop("CCACHE_LOGFILE"), "/run/build/obs/flatpak-compiler.log")
        self.assertEqual(env, original["modules"][1]["build-options"]["env"])

    def test_missing_or_ambiguous_obs_rejected(self):
        for modules in [[], [{"name": "obs"}, {"name": "obs"}]]:
            with self.assertRaises(ValueError):
                configure({"modules": modules})

    def test_interleaved_calls_and_sensitive_values(self):
        log = "\n".join([
            "[2026-10-22T10:00:00.000000 10 ] === CCACHE 4.14.1 STARTED ===",
            "[2026-10-22T10:00:00.100000 11 ] === CCACHE 4.14.1 STARTED ===",
            "[2026-10-22T10:00:00.200000 10 ] Source file: /run/build/obs/plugins/obs-browser/browser-app.cpp",
            "[2026-10-22T10:00:00.300000 11 ] Source file: /private/secret.cpp",
            "[2026-10-22T10:00:00.400000 11 ] Command line: secret-token=do-not-publish",
            "[2026-10-22T10:00:00.500000 11 ] Unsupported compiler option: --private-option=do-not-publish",
            "[2026-10-22T10:00:00.600000 11 ] Result: unsupported_compiler_option",
            "[2026-10-22T10:00:02.000000 10 ] Result: direct_cache_hit",
        ])
        result = summarize(log)
        self.assertEqual(result["calls"], 2)
        self.assertEqual(result["results"]["direct_cache_hit"], 1)
        self.assertEqual(result["unsupported_options"], {"--private-option": 1})
        browser = next(x for x in result["slowest_calls"] if x["source"].startswith("plugins/"))
        self.assertEqual(browser["log_span_seconds"], 2)
        self.assertNotIn("do-not-publish", str(result))
        self.assertNotIn("/private/", str(result))

    def test_reused_pid_is_a_new_call(self):
        log = "\n".join(f"[2026-10-22T10:00:0{i}.000000 10 ] === CCACHE 4.14.1 STARTED ===" for i in [0, 2])
        self.assertEqual(summarize(log)["calls"], 2)

    def test_actual_ccache_unsupported_message_before_source_detection(self):
        log = "\n".join([
            "[2026-10-22T10:00:00.000000 10 ] === CCACHE 4.14.1 STARTED ===",
            "[2026-10-22T10:00:00.010000 10 ] Command line: ccache c++ '-DPRIVATE=do-not-publish' -c /run/build/obs/plugins/obs-browser/browser-app.cpp",
            "[2026-10-22T10:00:00.020000 10 ] Compiler option -Wp,-P is unsupported",
            "[2026-10-22T10:00:00.030000 10 ] Result: unsupported_compiler_option",
        ])
        result = summarize(log)
        self.assertEqual(result["unsupported_options"], {"-Wp,-P": 1})
        self.assertEqual(result["uncacheable_calls"][0]["source"], "plugins/obs-browser/browser-app.cpp")
        self.assertNotIn("do-not-publish", str(result))

    def test_oversized_log_rejected(self):
        with self.assertRaises(ValueError):
            summarize("x" * (16 * 1024 * 1024 + 1))

    def test_default_off_manual_workflow_wiring(self):
        root = Path(__file__).resolve().parents[1]
        caller = (root / ".github/workflows/ci.yml").read_text()
        builder = (root / ".github/workflows/build-project.yaml").read_text()
        self.assertIn("flatpak-compiler-diagnostics:", caller)
        self.assertIn("github.event_name == 'workflow_dispatch' && inputs.flatpak-compiler-diagnostics", caller)
        self.assertIn("name: Configure Flatpak Compiler Diagnostics", builder)
        self.assertIn("name: Summarize Flatpak Compiler Diagnostics", builder)
        self.assertIn("name: flatpak-compiler-diagnostics-${{ github.run_id }}", builder)
        self.assertIn("keep-build-dirs: ${{ inputs.flatpak-compiler-diagnostics }}", builder)


if __name__ == "__main__":
    unittest.main()
