"""Opt-in compiler observations must not expose commands or change defaults."""

import copy
import json
import unittest
from pathlib import Path

from flatpak_compiler_diagnostics import configure, summarize


class FlatpakCompilerDiagnosticsTest(unittest.TestCase):
    def test_only_obs_logging_environment_changes(self):
        manifest = {"modules": [{"name": "cef", "build-options": {"env": {"X": "1"}}},
                                {"name": "obs", "build-options": {"env": {"ZCCACHE_DISABLE": "1"}}}]}
        original = copy.deepcopy(manifest)
        result = configure(manifest)
        self.assertEqual(manifest, original)
        self.assertEqual(result["modules"][0], original["modules"][0])
        env = result["modules"][1]["build-options"]["env"]
        self.assertEqual(env.pop("OBS_FLATPAK_ZCCACHE_JOURNAL"), "1")
        self.assertEqual(env, original["modules"][1]["build-options"]["env"])

    def test_missing_or_ambiguous_obs_rejected(self):
        for modules in [[], [{"name": "obs"}, {"name": "obs"}]]:
            with self.assertRaises(ValueError):
                configure({"modules": modules})

    def test_interleaved_calls_and_sensitive_values(self):
        log = '\n'.join(json.dumps(row) for row in [
            {"outcome": "hit", "args": ["-c", "/run/build/obs/plugins/obs-browser/browser-app.cpp"],
             "latency_ns": 2_000_000_000},
            {"outcome": "passthrough", "args": ["-DPRIVATE=do-not-publish", "-c", "/private/secret.cpp"],
             "passthrough_reason": "unmodeled compiler side output requested by --private-option=do-not-publish"},
        ])
        result = summarize(log)
        self.assertEqual(result["calls"], 2)
        self.assertEqual(result["results"], {"hit": 1, "passthrough": 1})
        self.assertEqual(result["unsupported_options"], {"--private-option": 1})
        browser = next(x for x in result["slowest_calls"] if x["source"].startswith("plugins/"))
        self.assertEqual(browser["log_span_seconds"], 2)
        self.assertNotIn("do-not-publish", str(result))
        self.assertNotIn("/private/", str(result))

    def test_repeated_compiler_call_is_counted_each_time(self):
        row = json.dumps({"outcome": "miss", "args": ["-c", "source.cpp"]})
        self.assertEqual(summarize(row + "\n" + row)["calls"], 2)

    def test_actual_zccache_passthrough_journal_shape(self):
        log = json.dumps({
            "outcome": "passthrough", "compiler": "/usr/bin/cc",
            "args": ["-DPRIVATE=do-not-publish", "-c", "/run/build/obs/plugins/obs-browser/browser-app.cpp"],
            "passthrough_reason": "system include discovery returned zero paths",
        })
        result = summarize(log)
        self.assertEqual(result["results"], {"passthrough": 1})
        self.assertEqual(result["unsupported_options"], {})
        self.assertEqual(result["uncacheable_calls"][0]["source"], "plugins/obs-browser/browser-app.cpp")
        self.assertNotIn("do-not-publish", str(result))
        self.assertNotIn("system include discovery", str(result))

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
