#!/usr/bin/env -S uv run --script
"""leaderboard.yml's three multi-line steps are one Python call each (#962).

    uv run python -m unittest ci/test_leaderboard_python.py

GEN-005 gives a workflow `run:` step a one-line shell budget. Three steps of
`.github/workflows/leaderboard.yml` (`jobs.update.steps[3]` `Generate`,
`steps[5]` `Render JPEG`, `steps[6]` `Commit to leaderboard branch`) were over
it, and RUN-001 flagged the job's `runs-on: ubuntu-latest`. All four are now
`ci/leaderboard.py` behind one line each, on the fleet's `ubuntu-24.04`.

RED first: committed while the steps were still shell blocks, the runner was
still `ubuntu-latest` and the module did not exist; every case here failed then.

The `uv` stub records the argv of every child, so the assertions are about the
exact command lines the shell steps used to run -- the point of the port is
that they did not change.
"""

from __future__ import annotations

import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib
import unittest

ROOT = Path(__file__).resolve().parent.parent
MODULE = ROOT / "ci" / "leaderboard.py"
WORKFLOW = ROOT / ".github" / "workflows" / "leaderboard.yml"
CI_TOML = ROOT / "ci.toml"
WORKFLOW_PATH = ".github/workflows/leaderboard.yml"
SITE = ROOT / "tools" / "leaderboard" / "site"

PLAYWRIGHT = "playwright==1.49.1"
RENDER = "tools/leaderboard/render.py"
GENERATE = "tools/leaderboard/generate.py"

# The uv stub: append one JSON argv line per call, then exit FAIL_CODE when this
# is the FAIL_ON_CALL-th call, so a test can make the Nth child fail and see
# whether the module stops there. Its shebang is the running interpreter: a
# `#!/usr/bin/env python3` stub is a bare-python invocation (TOOL-003), and a
# `uv run` shebang would make `env` resolve `uv` back to this stub.
UV_STUB = """#!{interpreter}
import json, os, sys

with open(os.environ["UV_LOG"], "a", encoding="utf-8") as log:
    log.write(json.dumps(sys.argv[1:]) + chr(10))
with open(os.environ["UV_LOG"], encoding="utf-8") as log:
    call = sum(1 for _ in log)
fail_on = os.environ.get("FAIL_ON_CALL", "")
if fail_on and int(fail_on) == call:
    sys.exit(int(os.environ.get("FAIL_CODE", "3")))
sys.exit(0)
"""


def load_module():
    """ci/leaderboard.py by path: `uv run python -m unittest` does not put the
    repo root on sys.path, and no file here edits sys.path."""
    spec = importlib.util.spec_from_file_location("leaderboard", MODULE)
    assert spec is not None and spec.loader is not None, f"cannot load {MODULE}"
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class StubCase(unittest.TestCase):
    """A temporary `uv` on PATH that records the argv of every child."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.bin = self.root / "bin"
        self.bin.mkdir()
        stub = self.bin / "uv"
        stub.write_text(UV_STUB.format(interpreter=sys.executable), encoding="utf-8")
        stub.chmod(0o755)
        self.log = self.root / "uv.log"
        self.log.touch()
        self.data = self.root / "data"
        self.data.mkdir()

    def env(self, **extra: str) -> dict[str, str]:
        env = dict(os.environ)
        env["PATH"] = f"{self.bin}{os.pathsep}{env.get('PATH', '')}"
        env["UV_LOG"] = str(self.log)
        env.pop("FAIL_ON_CALL", None)
        env.update(extra)
        return env

    def calls(self) -> list[list[str]]:
        return [json.loads(line) for line in self.log.read_text().splitlines() if line]

    def run_module(self, *args: str, **extra: str) -> subprocess.CompletedProcess:
        return subprocess.run(
            [sys.executable, str(MODULE), *args],
            capture_output=True,
            text=True,
            env=self.env(**extra),
            check=False,
        )


class GenerateTest(StubCase):
    """`Generate`: the site overlay, `.nojekyll`, and one generate.py run."""

    def test_copies_the_site_and_runs_generate_once(self):
        done = self.run_module(
            "generate", "--repo", "obs-rust/obs-studio", "--data", str(self.data), "--site", str(SITE)
        )
        self.assertEqual(done.returncode, 0, done.stderr)
        for name in ("index.html", "conversion.html", "conversion-minimal.html", "logo.png"):
            self.assertTrue((self.data / name).is_file(), name)
        self.assertTrue((self.data / ".nojekyll").is_file())
        self.assertEqual(
            self.calls(),
            [
                [
                    "run",
                    "--no-project",
                    "--python",
                    "3.12",
                    GENERATE,
                    "--repo",
                    "obs-rust/obs-studio",
                    "--out",
                    str(self.data / "leaderboard.json"),
                ]
            ],
        )

    def test_generate_failure_is_the_step_status(self):
        done = self.run_module(
            "generate",
            "--repo",
            "obs-rust/obs-studio",
            "--data",
            str(self.data),
            "--site",
            str(SITE),
            FAIL_ON_CALL="1",
        )
        self.assertEqual(done.returncode, 3)


class RenderTest(StubCase):
    """`Render JPEG`: playwright install, then the three renders in order."""

    def expected(self) -> list[list[str]]:
        with_playwright = ["run", "--no-project", "--python", "3.12", "--with", PLAYWRIGHT, RENDER]
        return [
            ["run", "--python", "3.12", "--with", PLAYWRIGHT, "playwright", "install", "--with-deps", "chromium"],
            [*with_playwright, str(self.data)],
            [*with_playwright, str(self.data), "conversion.html", "conversion.jpg"],
            [*with_playwright, str(self.data), "conversion-minimal.html", "conversion-minimal.jpg", "640x100"],
        ]

    def test_installs_then_renders_three_pages(self):
        done = self.run_module("render", "--data", str(self.data))
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertEqual(self.calls(), self.expected())

    def test_a_failing_render_stops_the_rest(self):
        # The step ran under `bash -e`, so the second render's failure was the
        # step's status and the third render never ran.
        done = self.run_module("render", "--data", str(self.data), FAIL_ON_CALL="3", FAIL_CODE="7")
        self.assertEqual(done.returncode, 7)
        self.assertEqual(len(self.calls()), 3)


class CommitTest(StubCase):
    """`Commit to leaderboard branch`: config, add, nothing-to-do, commit, push."""

    def setUp(self):
        super().setUp()
        self.remote = self.root / "remote.git"
        self.git("init", "--bare", "-b", "leaderboard", str(self.remote), cwd=self.root)
        self.git("clone", str(self.remote), str(self.data), cwd=self.root)
        self.git("config", "user.name", "test", cwd=self.data)
        self.git("config", "user.email", "test@example.com", cwd=self.data)

    def git(self, *args: str, cwd: Path) -> subprocess.CompletedProcess:
        return subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True, check=True)

    def test_no_changes_is_success_and_says_so(self):
        done = self.run_module("commit", "--data", str(self.data))
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertIn("no changes", done.stdout)
        self.assertEqual(self.git("ls-remote", "--heads", "origin", cwd=self.data).stdout, "")

    def test_a_change_is_committed_with_the_utc_message_and_pushed(self):
        (self.data / "leaderboard.json").write_text("{}\n", encoding="utf-8")
        done = self.run_module("commit", "--data", str(self.data))
        self.assertEqual(done.returncode, 0, done.stderr)
        subject = self.git("log", "-1", "--format=%s", cwd=self.data).stdout.strip()
        self.assertRegex(subject, r"^leaderboard: update \d{4}-\d{2}-\d{2}T\d{2}:\d{2}Z$")
        author = self.git("log", "-1", "--format=%an <%ae>", cwd=self.data).stdout.strip()
        self.assertEqual(author, "github-actions[bot] <41898282+github-actions[bot]@users.noreply.github.com>")
        pushed = self.git("ls-remote", "--heads", "origin", cwd=self.data).stdout
        self.assertIn("refs/heads/leaderboard", pushed)
        self.assertEqual(self.calls(), [])

    def test_a_failing_push_fails_the_step(self):
        (self.data / "leaderboard.json").write_text("{}\n", encoding="utf-8")
        self.git("remote", "set-url", "origin", str(self.root / "missing.git"), cwd=self.data)
        done = self.run_module("commit", "--data", str(self.data))
        self.assertNotEqual(done.returncode, 0)


class WorkflowTest(unittest.TestCase):
    """Each step is one line, the runner is a fleet label, no exceptions left."""

    def setUp(self):
        self.text = WORKFLOW.read_text(encoding="utf-8")

    def step_run(self, marker: str) -> str:
        block = self.text.split(marker, 1)
        self.assertEqual(len(block), 2, f"no {marker!r} step")
        rest = block[1].lstrip("\n")
        runs = []
        for line in rest.splitlines():
            if line == "" or line.startswith("      - "):
                break
            if line.strip().startswith("run:"):
                runs.append(line.strip())
        self.assertTrue(runs, f"{marker!r} has no run: line")
        return runs[0]

    def test_generate_is_one_call(self):
        run = self.step_run("- name: Generate")
        self.assertIn("uv run --no-project ci/leaderboard.py generate", run)
        self.assertNotRegex(run, r"[;|&]")

    def test_render_is_one_call(self):
        run = self.step_run("- name: Render JPEG")
        self.assertIn("uv run --no-project ci/leaderboard.py render", run)
        self.assertNotRegex(run, r"[;|&]")

    def test_commit_is_one_call(self):
        run = self.step_run("- name: Commit to leaderboard branch")
        self.assertIn("uv run --no-project ci/leaderboard.py commit", run)
        self.assertNotRegex(run, r"[;|&]")

    def test_runner_is_a_fleet_label(self):
        self.assertIn("runs-on: ubuntu-24.04", self.text)
        self.assertNotIn("ubuntu-latest", self.text)

    def test_the_schedule_and_guards_are_unchanged(self):
        crons = re.findall(r'cron:\s*"([^"]+)"', self.text)
        self.assertEqual(crons, ["17 */4 * * *"])
        self.assertIn("workflow_dispatch:", self.text)
        self.assertIn("github.repository == 'obs-rust/obs-studio'", self.text)
        self.assertIn("group: leaderboard", self.text)
        self.assertIn("cancel-in-progress: false", self.text)

    def test_no_exception_left_for_either_rule(self):
        exceptions = tomllib.loads(CI_TOML.read_text(encoding="utf-8"))["exceptions"]
        left = [
            e
            for e in exceptions
            if e.get("path") == WORKFLOW_PATH and e.get("rule") in {"GEN-005", "RUN-001"}
        ]
        self.assertEqual(left, [])


class PortShapeTest(unittest.TestCase):
    """The module is a port, not a shell wrapper."""

    def test_module_has_a_main_and_no_shell(self):
        module = load_module()
        self.assertTrue(callable(module.main))
        source = MODULE.read_text(encoding="utf-8")
        self.assertNotIn("shell=True", source)
        self.assertNotIn("subprocess.getoutput", source)


if __name__ == "__main__":
    unittest.main()
