#!/usr/bin/env -S uv run --script
"""Tier 2 Linux runs natively on the CI runner, not in a per-job image (#966).

    uv run python -m unittest ci/test_tier2_native.py

Every Tier 2 Linux job built rust/tools/linux-validate/Dockerfile from
scratch (about 60 apt packages and a rustup toolchain), most of its 2.4 min
setup. CI now installs the shared package list natively and runs
validate.sh on the runner, like the macOS and Windows Tier 2 jobs; locally,
`bosn ci run` runs the same job through act2. The Docker harness stays for
./unittest and reads the same package list.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
HERE = ROOT / "rust" / "tools" / "linux-validate"
PACKAGES = HERE / "apt-packages.txt"
WORKFLOW = ROOT / ".github" / "workflows" / "build-project.yaml"


def tier2_job() -> str:
    """The Tier 2 Linux build job and test jobs (#937), together."""
    text = WORKFLOW.read_text(encoding="utf-8")
    out = []
    for name in ("rust-linux-tier2-build", "rust-linux-tier2"):
        m = re.search(rf"^  {name}:\n(.*?)(?=^  [a-z0-9-]+:\n)", text, re.S | re.M)
        assert m, f"no {name} job"
        out.append(m.group(1))
    return "\n".join(out)


class PackageListTest(unittest.TestCase):
    def test_list_has_the_test_runtime(self):
        names = PACKAGES.read_text(encoding="utf-8").split()
        for pkg in ("libcmocka-dev", "xvfb", "xauth", "pipewire", "dbus-daemon", "ninja-build", "zstd"):
            self.assertIn(pkg, names)

    def test_dockerfile_installs_the_shared_list(self):
        text = (HERE / "Dockerfile").read_text(encoding="utf-8")
        self.assertIn("apt-packages.txt", text)
        self.assertNotIn("libcmocka-dev", text, "the package list lives in apt-packages.txt only")


class ValidatePathsTest(unittest.TestCase):
    def setUp(self):
        self.text = (HERE / "validate.sh").read_text(encoding="utf-8")

    def test_roots_are_configurable(self):
        # /ro: run.py mounts the checkout read-only there and builds out of source.
        self.assertIn('SRC="${OBS_TIER2_SRC:-/ro}"', self.text)
        self.assertIn('BUILD="${OBS_TIER2_BUILD:-/build}"', self.text)

    def test_submodules_are_initialized_only_when_building(self):
        block = self.text.split('if [ "$stage" != test ]; then', 1)[1].split('\nfi', 1)[0]
        self.assertIn('submodule update', block)

    def test_no_hardcoded_container_paths_remain(self):
        body = "\n".join(l for l in self.text.splitlines() if not l.lstrip().startswith("#"))
        body = body.replace('"${OBS_TIER2_SRC:-/ro}"', "").replace('"${OBS_TIER2_BUILD:-/build}"', "")
        self.assertIsNone(re.search(r"(?<![\w$}])/(src|build)\b", body), "use $SRC / $BUILD")


class WorkflowTest(unittest.TestCase):
    def setUp(self):
        self.job = tier2_job()

    def test_no_docker_harness_in_ci(self):
        code = "\n".join(l for l in self.job.splitlines() if not l.lstrip().startswith("#"))
        self.assertNotIn("--lane tier2-linux", code)
        self.assertNotIn("run.sh", code)
        self.assertNotRegex(code, r"docker (build|run)")

    def test_installs_packages_natively(self):
        self.assertIn("apt-packages.txt", self.job)
        self.assertIn(".github/scripts/retry", self.job)

    def test_toolchain_from_setup_soldr_with_shims(self):
        self.assertRegex(self.job, r"zackees/setup-soldr@[0-9a-f]{40}")
        self.assertIn("shims: true", self.job)

    def test_test_runner_does_not_fetch_build_only_submodules_or_history(self):
        text = WORKFLOW.read_text()
        test = re.search(r"^  rust-linux-tier2:\n(.*?)(?=^  [a-z0-9-]+:\n)", text, re.S | re.M)[1]
        checkout = test.split("uses: actions/checkout@", 1)[1].split("- name:", 1)[0]
        self.assertNotIn("submodules: recursive", checkout)
        self.assertNotIn("fetch-depth: 0", checkout)

    def test_build_fetches_commits_and_tags_but_no_old_trees_or_submodules(self):
        """#3022: git describe/log/rev-parse read commits only, and validate.sh
        initializes obs-browser and obs-websocket at --depth 1 itself; the
        full recursive fetch cost 10 s."""
        text = WORKFLOW.read_text()
        build = re.search(r"^  rust-linux-tier2-build:\n(.*?)(?=^  [a-z0-9-]+:\n)", text, re.S | re.M)[1]
        checkout = build.split("uses: actions/checkout@", 1)[1].split("- name:", 1)[0]
        self.assertIn("fetch-depth: 0", checkout)
        self.assertIn("filter: tree:0", checkout)
        self.assertNotIn("submodules: recursive", checkout)
        self.assertIn("submodule update --init --depth 1", (ROOT / "rust/tools/linux-validate/validate.sh").read_text())

    def test_runs_validate_on_the_runner(self):
        self.assertIn("bash rust/tools/linux-validate/validate.sh", self.job)
        self.assertIn("OBS_TIER2_SRC: ${{ github.workspace }}", self.job)


class ContainerTest(unittest.TestCase):
    def test_runs_in_a_digest_pinned_stock_image(self):
        """No Dockerfile or image build, but the container environment the
        tests pass in (#982)."""
        self.assertRegex(tier2_job(), r"container:\n\s+image: ubuntu:26\.04@sha256:[0-9a-f]{64}")


class GitOwnershipTest(unittest.TestCase):
    def test_source_is_a_safe_git_directory_in_every_harness(self):
        """In CI's job container git runs as root on a checkout owned by the
        runner user; CMake then failed to read the version tag ("dubious
        ownership") and Tier 2 failed on master (#1019)."""
        text = (HERE / "validate.sh").read_text(encoding="utf-8")
        line = 'git config --global --add safe.directory "$SRC"'
        self.assertIn(line, text)
        # Unconditional (not inside an if), and before the first git command.
        self.assertTrue(text.splitlines()[text[: text.index(line)].count("\n")].startswith("git config"))
        self.assertLess(text.index(line), text.index('git -C "$SRC" submodule update'))

    def test_no_temporary_diagnostic_left(self):
        self.assertNotIn("Diagnose Group Tests", WORKFLOW.read_text(encoding="utf-8"))


class TimeoutTest(unittest.TestCase):
    def test_tier2_jobs_time_out(self):
        """A hung Tier 2 test held a scarce ubuntu-26.04 runner for 4.5 h
        until cancelled (#983); GitHub's default is 6 h."""
        m = re.search(r"timeout-minutes:\s*(\d+)", tier2_job())
        self.assertIsNotNone(m)
        self.assertLessEqual(int(m.group(1)), 45)


class DocsTest(unittest.TestCase):
    def test_ci_tooling_names_the_local_bosn_run(self):
        text = (ROOT / "docs" / "ci-tooling.md").read_text(encoding="utf-8")
        self.assertIn("--job rust-linux-tier2", text)


if __name__ == "__main__":
    unittest.main()
