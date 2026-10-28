"""Tier 2 can run its RUN_SERIAL part beside the suite, on its own CPUs (#2773).

Usage:
    uv run python -m unittest ci/test_tier2_serial_cpus.py

Inside one Tier 2 container the five test_deinterlace_2x_field_order tests are
RUN_SERIAL (#1343, #1643), so ctest runs them one at a time while every other
core idles; on a large host that barrier is most of the lane (147 s of the
242 s of one mode, measured in #2773). CI has two runners per mode (#2071,
#2072), a local host does not, so the isolation has to come from CPU time
instead: OBS_TIER2_SERIAL_CPUS gives the serial part its own cores and runs
the suite part beside it. Unset, nothing changes.
"""

import importlib.util
import json
import os
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TOOLS = ROOT / "rust/tools/linux-validate"
RUNNER = TOOLS / "serial-runner.py"
VALIDATE = TOOLS / "validate.sh"
RUN_PY = TOOLS / "run.py"


def runner(*args: str) -> subprocess.CompletedProcess:
    return subprocess.run(["uv", "run", "--no-project", str(RUNNER), *args], capture_output=True, text=True)


def cpus(text: str) -> set[int]:
    """Parse a Linux CPU list ("0-3,8,10-11") into a set."""
    result: set[int] = set()
    for part in text.strip().split(","):
        if not part:
            continue
        first, _, last = part.partition("-")
        result.update(range(int(first), int(last or first) + 1))
    return result


def registry(commands: list[list[str]], serial: list[bool] | None = None) -> dict:
    serial = serial or [True] * len(commands)
    return {
        "tests": [
            {
                "name": f"test_probe_{i}",
                "command": command,
                "properties": [{"name": "RUN_SERIAL", "value": True}] if serial[i] else [],
            }
            for i, command in enumerate(commands)
        ]
    }


class AffinityTest(unittest.TestCase):
    def test_suite_cpus_are_the_affinity_minus_the_serial_set(self):
        affinity = sorted(os.sched_getaffinity(0))
        if len(affinity) < 3:
            self.skipTest("needs at least three CPUs")
        serial = affinity[:2]
        result = runner("suite-cpus", "--serial", ",".join(str(cpu) for cpu in serial))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(cpus(result.stdout), set(affinity) - set(serial))

    def test_a_serial_set_that_covers_the_host_is_refused(self):
        # Every core given to the serial part leaves the suite part no CPU.
        result = runner("suite-cpus", "--serial", "0-" + str(max(os.sched_getaffinity(0)) + 8))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("serial", result.stderr.lower())

    def test_a_malformed_cpu_list_is_refused(self):
        result = runner("suite-cpus", "--serial", "1,,x")
        self.assertNotEqual(result.returncode, 0)


class SerialRunTest(unittest.TestCase):
    def probe(self, tmp: Path) -> list[str]:
        """A command that records its CPUs, its start and its end, then sleeps 1 s."""
        script = tmp / "probe.py"
        script.write_text(
            "import os, pathlib, sys, time\n"
            "record = pathlib.Path(sys.argv[1])\n"
            "record.write_text(f'{sorted(os.sched_getaffinity(0))} {time.time()}\\n')\n"
            "time.sleep(1)\n"
            "with record.open('a') as handle:\n"
            "    handle.write(f'{sorted(os.sched_getaffinity(0))} {time.time()}\\n')\n",
            encoding="utf-8",
        )
        return [sys.executable, str(script)]

    def pins(self, tmp: Path, count: int) -> list[list[int]]:
        affinity = sorted(os.sched_getaffinity(0))
        if len(affinity) < 2 * count:
            self.skipTest(f"needs {2 * count} CPUs")
        return [affinity[2 * i : 2 * i + 2] for i in range(count)]

    def test_serial_tests_run_at_the_same_time_on_their_own_cpus(self):
        tmp = Path(self.enterContext(tempfile.TemporaryDirectory()))
        pairs = self.pins(tmp, 2)
        commands = [self.probe(tmp) + [str(tmp / f"{i}.record")] for i in range(2)]
        (tmp / "registry.json").write_text(json.dumps(registry(commands)), encoding="utf-8")
        started = time.monotonic()
        result = runner(
            "run",
            str(tmp / "registry.json"),
            "--cpus",
            ",".join("-".join(str(cpu) for cpu in pair) for pair in pairs),
            "--log-dir",
            str(tmp / "logs"),
        )
        elapsed = time.monotonic() - started
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertLess(elapsed, 1.8, "the two tests did not overlap")
        for index, pair in enumerate(pairs):
            lines = (tmp / f"{index}.record").read_text(encoding="utf-8").splitlines()
            self.assertEqual(len(lines), 2)
            for line in lines:
                self.assertEqual(cpus(line.rsplit(" ", 1)[0].strip("[]")), set(pair))
            self.assertLess(abs(float(lines[0].rsplit(" ", 1)[1]) - float(lines[1].rsplit(" ", 1)[1])), 10)

    def test_only_the_run_serial_tests_are_run(self):
        tmp = Path(self.enterContext(tempfile.TemporaryDirectory()))
        (tmp / "registry.json").write_text(
            json.dumps(registry([[sys.executable, "-c", "print('serial')"],
                                 [sys.executable, "-c", "print('suite')"]], [True, False])),
            encoding="utf-8",
        )
        result = runner("run", str(tmp / "registry.json"), "--cpus", ",".join(str(cpu) for cpu in self.pins(tmp, 1)[0]))
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("serial", result.stdout)
        self.assertNotIn("suite", result.stdout)

    def test_a_failing_test_fails_the_run_and_keeps_its_output(self):
        tmp = Path(self.enterContext(tempfile.TemporaryDirectory()))
        (tmp / "registry.json").write_text(
            json.dumps(registry([[sys.executable, "-c", "import sys; print('boom-marker'); sys.exit(3)"]])),
            encoding="utf-8",
        )
        result = runner(
            "run",
            str(tmp / "registry.json"),
            "--cpus",
            ",".join(str(cpu) for cpu in self.pins(tmp, 1)[0]),
            "--log-dir",
            str(tmp / "logs"),
        )
        logs = "".join(path.read_text(encoding="utf-8") for path in (tmp / "logs").glob("*.log"))
        self.assertEqual(result.returncode, 1)
        self.assertIn("boom-marker", logs)
        self.assertIn("FAIL", result.stdout)

    def test_more_tests_than_cpu_pairs_is_refused(self):
        tmp = Path(self.enterContext(tempfile.TemporaryDirectory()))
        (tmp / "registry.json").write_text(
            json.dumps(registry([[sys.executable, "-c", "pass"], [sys.executable, "-c", "pass"]])),
            encoding="utf-8",
        )
        result = runner("run", str(tmp / "registry.json"), "--cpus", str(min(os.sched_getaffinity(0))))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("cpu", result.stderr.lower())


class ValidateScriptTest(unittest.TestCase):
    def test_the_concurrent_part_is_opt_in(self):
        text = VALIDATE.read_text(encoding="utf-8")
        self.assertIn("OBS_TIER2_SERIAL_CPUS", text)
        self.assertIn("serial-runner.py", text)
        # The one ctest invocation per mode stays for the default path.
        self.assertRegex(text, r"ctest --test-dir \"\$build\" --output-on-failure -j")
        # Combining it with the CI part/shard knobs is a usage error.
        self.assertRegex(text, r"OBS_TIER2_SERIAL_CPUS cannot be combined")

    def test_the_suite_part_excludes_the_serial_tests(self):
        text = VALIDATE.read_text(encoding="utf-8")
        self.assertRegex(text, r"-E \"\$serial_args?\"|--exclude|serial_regex")
        self.assertIn("check_skips.py", text)


class RunPyTest(unittest.TestCase):
    def test_the_variable_reaches_the_container(self):
        spec = importlib.util.spec_from_file_location("tier2_run", RUN_PY)
        self.assertIsNotNone(spec)
        self.assertIsNotNone(spec.loader)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        arguments = module.task_arguments({"OBS_TIER2_SERIAL_CPUS": "0-3"}, "")
        self.assertIn("--env", arguments)
        self.assertIn("OBS_TIER2_SERIAL_CPUS=0-3", arguments)


if __name__ == "__main__":
    unittest.main()
