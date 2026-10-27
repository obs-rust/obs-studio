#!/usr/bin/env -S uv run --script
"""The Flatpak lint action addresses its module inside the job's container (#2705).

The `Flatpak 📦` job in `build-project.yaml` runs in a container whose working
directory is `/__w/obs-studio/obs-studio`, but a `${{ github.workspace }}`
interpolation in a composite action's `run:` resolves to the *runner host* path
(`/home/runner/work/obs-studio/obs-studio`). Passing that path as an argument
is not translated the way `working-directory:` is, so the step died before the
build with

    error: Failed to spawn: /home/runner/work/obs-studio/obs-studio/ci/flatpak_builder_lint.py

Both steps already run with `working-directory: ${{ inputs.workingDirectory }}`
(default: the workspace), so the module and the action's own `exceptions.json`
are addressed relative to that directory -- the same shape as the neighbouring
`uv run --no-project ci/flatpak_compiler_diagnostics.py` step.

    uv run python -m unittest discover -s ci -p test_flatpak_lint_paths.py
"""

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ACTION = ROOT / ".github" / "actions" / "flatpak-builder-lint" / "action.yaml"
MODULE = ROOT / "ci" / "flatpak_builder_lint.py"
EXCEPTIONS = ".github/actions/flatpak-builder-lint/exceptions.json"

# Context interpolations that expand to a runner-host path, not a container one.
HOST_PATHS = ("github.workspace", "github.action_path")

RUN_LINE = re.compile(r"^(\s*)(?:-\s+)?run:\s*(.+)$")


def run_lines() -> list[tuple[int, str]]:
    """(line number, command) of every `run:` line in the action."""
    lines = ACTION.read_text(encoding="utf-8").splitlines()
    found = []
    for number, line in enumerate(lines, start=1):
        match = RUN_LINE.match(line)
        if match:
            found.append((number, match.group(2).strip()))
    return found


class FlatpakLintActionPaths(unittest.TestCase):
    def test_the_module_and_the_action_exist(self) -> None:
        self.assertTrue(MODULE.is_file(), f"{MODULE} is missing")
        self.assertTrue((ACTION.parent / "exceptions.json").is_file())

    def test_every_invocation_is_relative(self) -> None:
        invocations = [(n, c) for n, c in run_lines() if "flatpak_builder_lint.py" in c]
        self.assertEqual(len(invocations), 2, f"expected guard and lint: {invocations}")
        for number, command in invocations:
            for host_path in HOST_PATHS:
                self.assertNotIn(
                    host_path,
                    command,
                    f"{ACTION}:{number} passes {host_path}, a runner-host path the "
                    "container job cannot resolve",
                )
            self.assertIn(
                "ci/flatpak_builder_lint.py",
                command,
                f"{ACTION}:{number} should address the module relatively",
            )

    def test_the_lint_step_reads_the_action_own_exceptions(self) -> None:
        lint = [(n, c) for n, c in run_lines() if " lint " in c]
        self.assertEqual(len(lint), 1, f"expected one lint step: {lint}")
        number, command = lint[0]
        self.assertIn(
            f"--exceptions '{EXCEPTIONS}'",
            command.replace('"', "'"),
            f"{ACTION}:{number} should read {EXCEPTIONS} relative to the workspace",
        )


if __name__ == "__main__":
    unittest.main()
