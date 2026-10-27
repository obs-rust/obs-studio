"""The opt-in cache experiment must measure real rebuilds without mutating its seed."""
import json
import importlib.util
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent
SPEC = importlib.util.spec_from_file_location("native_cache_probe", ROOT / "ci/native_cache_probe.py")
PROBE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PROBE)
run_probe = PROBE.run_probe


class NativeCacheProbeTest(unittest.TestCase):
    def test_isolated_counterbalanced_rebuilds_preserve_the_original_cache(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            seed = root / "seed"
            seed.mkdir()
            (seed / "object").write_bytes(b"original compiler object")
            calls = []

            def execute(command, **kwargs):
                env = kwargs["env"]
                cache = Path(env["CCACHE_DIR"])
                self.assertNotEqual(cache, seed)
                if command[0] == "cmake":
                    self.assertEqual((cache / "object").read_bytes(), b"original compiler object")
                    (cache / "object").write_bytes(b"rebuild changed private copy")
                calls.append((command, env["CCACHE_MAXSIZE"], str(cache)))
                if command == ["ccache", "--get-config", "cache_dir"]:
                    stdout = str(cache)
                elif command == ["ccache", "--print-stats", "--format=json"]:
                    stdout = '{"direct_cache_hit": 12, "cache_miss": 3}'
                else:
                    stdout = "cache statistics\n"
                return subprocess.CompletedProcess(command, 0, stdout=stdout)

            report = run_probe(seed, root / "probe", "arm64", "RelWithDebInfo", execute=execute)
            builds = [(command, limit, cache) for command, limit, cache in calls if command[0] == "cmake"]
            self.assertEqual([limit for _, limit, _ in builds], ["2G", "350M", "350M", "2G"])
            self.assertEqual(len({cache for _, _, cache in builds}), 4)
            for command, _, _ in builds:
                self.assertEqual(command, ["cmake", "--build", "--preset", "windows-arm64",
                                          "--config", "RelWithDebInfo", "--clean-first", "--parallel",
                                          "--", "/consoleLoggerParameters:Summary", "/noLogo"])
            for _, limit, cache in builds:
                commands = [command for command, _, path in calls if path == cache]
                clean = commands.index(["ccache", "--cleanup"])
                build = next(i for i, command in enumerate(commands) if command[0] == "cmake")
                self.assertLess(commands.index(["ccache", f"--max-size={limit}"]), clean)
                self.assertLess(clean, build)
            self.assertEqual((seed / "object").read_bytes(), b"original compiler object")
            self.assertEqual(report["status"], "success")
            self.assertEqual(len(report["observations"]), 4)

    def test_failed_rebuild_is_reported_and_not_treated_as_a_timing_sample(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            seed = root / "seed"
            seed.mkdir()

            def fail(command, **kwargs):
                if command[0] == "cmake":
                    raise subprocess.CalledProcessError(7, command)
                return subprocess.CompletedProcess(command, 0, stdout=kwargs["env"]["CCACHE_DIR"])

            with self.assertRaises(subprocess.CalledProcessError):
                run_probe(seed, root / "probe", "x64", "RelWithDebInfo", execute=fail)
            report = json.loads((root / "probe" / "report.json").read_text())
            self.assertEqual(report["status"], "failure")
            self.assertEqual(report["observations"], [])

    def test_no_compilation_is_rejected_instead_of_reported_as_a_fast_build(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            seed = root / "seed"
            seed.mkdir()

            def no_compiles(command, **kwargs):
                stdout = "{}" if "--print-stats" in command else kwargs["env"]["CCACHE_DIR"]
                return subprocess.CompletedProcess(command, 0, stdout=stdout)

            with self.assertRaisesRegex(RuntimeError, "no cacheable"):
                run_probe(seed, root / "probe", "x64", "RelWithDebInfo", execute=no_compiles)
            report = json.loads((root / "probe" / "report.json").read_text())
            self.assertEqual(report["status"], "failure")
            self.assertEqual(report["observations"], [])

    def test_probe_refuses_recursive_copy_into_the_seed(self):
        with tempfile.TemporaryDirectory() as directory:
            seed = Path(directory)
            with self.assertRaisesRegex(ValueError, "separate directory trees"):
                run_probe(seed, seed / "probe", "x64", "RelWithDebInfo")
            self.assertFalse((seed / "probe").exists())

    def test_workflow_probe_is_explicit_manual_only_and_defaults_off(self):
        workflow = (ROOT / ".github/workflows/ci.yml").read_text()
        build = (ROOT / ".github/workflows/build-project.yaml").read_text()
        self.assertIn("native-cache-probe:", workflow)
        self.assertIn("github.event_name == 'workflow_dispatch' && inputs.native-cache-probe", workflow)
        header = build.split("\nenv:", 1)[0]
        probe = header.split("      native-cache-probe:", 1)[1]
        self.assertIn("default: false", probe)
        step = build.split("- name: Measure Windows Cache Budgets", 1)[1].split("\n      - ", 1)[0]
        self.assertIn("github.event_name == 'workflow_dispatch' && inputs.native-cache-probe", step)
        self.assertIn("uv run --no-project ci/native_cache_probe.py", step)
        self.assertNotIn("continue-on-error", step)
        # Ordinary compilation happens first and is not conditional on the probe.
        windows = build.split("  windows-build:", 1)[1].split("  rust-tests-linux:", 1)[0]
        self.assertLess(windows.index("- name: Build OBS Studio"), windows.index("- name: Measure Windows"))


if __name__ == "__main__":
    unittest.main()
