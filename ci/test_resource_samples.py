#!/usr/bin/env -S uv run --script
"""The gate samples memory and disk while a lane runs (#2593, part A4).

    uv run python -m unittest discover -s ci -p test_resource_samples.py

A runner that dies mid-job (exit 143, "lost communication") leaves no
footprint beyond the log it had already streamed, so a memory or disk
exhaustion cannot be read from it afterwards. The gate therefore prints one
memory and disk sample into the live step log while a lane's child runs, which
is what the lost Fast Gate job in #2593 needed to be diagnosable.
"""

from __future__ import annotations

import contextlib
import importlib.util
import io
import os
import threading
import unittest
from pathlib import Path
from unittest import mock

CI = Path(__file__).resolve().parent
_SPEC = importlib.util.spec_from_file_location("local_gate_resources_target", CI / "local_gate.py")
if _SPEC is None or _SPEC.loader is None:  # pragma: no cover - the file is in the tree
    raise ImportError("ci/local_gate.py is not importable")
local_gate = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(local_gate)


class ResourceSampleTest(unittest.TestCase):
    def test_run_waits_for_an_in_progress_sample(self):
        started = threading.Event()
        release = threading.Event()
        cleanup = threading.Event()
        returned = threading.Event()
        threads = []
        errors = []
        real_thread = threading.Thread

        def sample():
            started.set()
            if not release.wait(5):
                raise TimeoutError("sample was not released")
            return "resources: delayed sample"

        def make_thread(*args, **kwargs):
            thread = real_thread(*args, **kwargs)
            join = thread.join

            def finish(*args, **kwargs):
                cleanup.set()
                return join(*args, **kwargs)

            thread.join = finish
            threads.append(thread)
            return thread

        def output():
            if not started.wait(5):
                raise TimeoutError("sampler did not start")
            return iter(())

        def run():
            try:
                local_gate.run(["child"])
            except BaseException as error:
                errors.append(error)
            finally:
                returned.set()
                cleanup.set()

        runner = real_thread(target=run)
        out = io.StringIO()
        with (
            mock.patch.object(local_gate, "RESOURCE_SAMPLE_SECONDS", 0),
            mock.patch.dict(os.environ, {"OBS_GATE_RESOURCE_SAMPLES": "1"}),
            mock.patch.object(local_gate, "resource_sample", side_effect=sample),
            mock.patch.object(local_gate.threading, "Thread", side_effect=make_thread),
            mock.patch.object(local_gate.subprocess, "Popen") as popen,
            contextlib.redirect_stdout(out),
        ):
            proc = popen.return_value.__enter__.return_value
            proc.stdout.__iter__.side_effect = output
            proc.returncode = 0
            runner.start()
            try:
                self.assertTrue(cleanup.wait(5), "run never reached sampler cleanup")
                self.assertFalse(returned.is_set(), "run returned while a sample was still in progress")
            finally:
                release.set()
                runner.join(5)
                for thread in threads:
                    thread.join(5)
            self.assertFalse(runner.is_alive())
            self.assertEqual(errors, [])
        self.assertIn("resources: delayed sample", out.getvalue())

    def test_sample_names_memory_and_disk(self):
        sample = local_gate.resource_sample()
        self.assertIn("resources:", sample)
        self.assertIn("mem", sample.lower())
        self.assertIn("disk", sample.lower())
        self.assertNotIn("\n", sample)

    def test_sample_survives_a_host_without_proc_meminfo(self):
        with mock.patch.object(local_gate, "MEMINFO", Path("/nonexistent/proc/meminfo")):
            sample = local_gate.resource_sample()
        self.assertIn("disk", sample.lower())
        self.assertNotIn("\n", sample)

    def test_run_prints_a_sample_while_the_child_runs(self):
        out = io.StringIO()
        with (
            mock.patch.object(local_gate, "RESOURCE_SAMPLE_SECONDS", 0.02),
            mock.patch.dict(os.environ, {"OBS_GATE_RESOURCE_SAMPLES": "1"}),
            contextlib.redirect_stdout(out),
        ):
            local_gate.run(["sh", "-c", "sleep 0.2"])
        self.assertIn("resources:", out.getvalue())

    def test_samples_are_off_on_request(self):
        out = io.StringIO()
        with (
            mock.patch.object(local_gate, "RESOURCE_SAMPLE_SECONDS", 0.02),
            mock.patch.dict(os.environ, {"OBS_GATE_RESOURCE_SAMPLES": "0"}),
            contextlib.redirect_stdout(out),
        ):
            local_gate.run(["sh", "-c", "sleep 0.2"])
        self.assertNotIn("resources:", out.getvalue())


if __name__ == "__main__":
    unittest.main()
