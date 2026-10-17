#!/usr/bin/env -S uv run --script
"""A workflow step that builds with `ccache` as the compiler launcher runs
after ccache is installed.

#957 moved the main Linux builds to zccache and dropped ccache from
rust/tools/linux-validate/apt-packages.txt; the Fast Gate's "Build Touched
Linux Targets" step still configured -DCMAKE_C_COMPILER_LAUNCHER=ccache and
failed every plugin PR with "ccache: not found".

The workflow is split into jobs and steps with regular expressions, as in
test_soldr_toolchain_cache.py: ci/ tests use only the standard library
(test_ci_tests_stdlib_only.py, #1196).
"""

from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = ROOT / ".github/workflows/build-project.yaml"
APT_LIST = ROOT / "rust/tools/linux-validate/apt-packages.txt"
# Scripts a step runs that configure a ccache launcher themselves.
CCACHE_SCRIPTS = ("ci/linux_qt_frontend.sh",)


def jobs(text: str) -> dict[str, str]:
    """Split the workflow's `jobs:` mapping into {job id: job text}."""
    body = text.split("\njobs:\n", 1)[1]
    parts = re.split(r"^  ([A-Za-z0-9_-]+):\s*$", body, flags=re.M)
    return dict(zip(parts[1::2], parts[2::2]))


def steps(job: str) -> list[tuple[str, str]]:
    """(name, text) of each step of a job, in order."""
    out = []
    for step in re.split(r"^      - ", job, flags=re.M)[1:]:
        name = re.search(r"^(?:\s*)name:\s*(.+)$", step, re.M)
        out.append((name.group(1).strip() if name else "", step))
    return out


def uses_ccache(run: str) -> bool:
    return bool(re.search(r"COMPILER_LAUNCHER=ccache\b", run)) or any(s in run for s in CCACHE_SCRIPTS)


def installs_ccache(run: str) -> bool:
    apt_list = [
        line.strip() for line in APT_LIST.read_text().splitlines() if line.strip() and not line.startswith("#")
    ]
    # An apt-get install command (with its continuation lines) naming ccache.
    joined = run.replace("\\\n", " ")
    for line in joined.splitlines():
        if "apt-get install" in line and re.search(r"(?<![\w-])ccache(?![\w-])", line):
            return True
    return "apt-packages.txt" in run and "ccache" in apt_list


class CcacheLauncherTest(unittest.TestCase):
    def test_ccache_is_installed_before_it_launches_a_build(self):
        checked = 0
        for job_name, job in jobs(WORKFLOW.read_text()).items():
            installed = False
            for step_name, step in steps(job):
                installed = installed or installs_ccache(step)
                if uses_ccache(step):
                    checked += 1
                    with self.subTest(job=job_name, step=step_name):
                        self.assertTrue(installed, "install ccache before configuring it as the launcher")
        self.assertGreater(checked, 0)


if __name__ == "__main__":
    unittest.main()
